//! Loopback-only `tiny_http` transport for the MCP server (ADR 0078 decision 2/4).
//!
//! Binds `127.0.0.1:<port>` ONLY (hardcoded loopback — G-4: never configurable).
//! Single route `POST /mcp`; `GET /mcp` → 405; anything else → 404. Defenses:
//! `Host` header must be a localhost form (DNS-rebinding), bearer token checked
//! by constant-time compare of SHA-256 digests (the plaintext token is hashed
//! at construction and never stored or logged — G-3), request body capped at
//! 1 MiB. JSON-RPC notifications → 202 empty. Runs on a dedicated
//! `std::thread`; synchronous handlers call the blocking `AppState` directly —
//! no tokio involvement. Graceful stop via `tiny_http`'s `unblock()`.
//!
//! Where the token comes from (keychain) and when the server runs (Settings
//! toggle, app-open-only lifetime) are owned by M1/M3 — this struct takes the
//! token as a constructor argument.

use std::io::Read;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::app_state::AppState;
use crate::mcp::protocol;
use crate::mcp::registry::McpScope;

/// Request bodies larger than this are rejected with `413` (ADR 0078 decision 2).
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// How long shutdown waits for the processor to finish its in-flight request
/// before parking it in [`STALLED_PROCESSORS`].
const PROCESSOR_JOIN_TIMEOUT: Duration = Duration::from_millis(250);

/// Stalled-processor cap (ADR 0099 dec. 2 shutdown posture): at this many
/// parked processors the lifecycle refuses to start a NEW server until one
/// exits. Threat model: reaching the cap requires a LOCAL process holding a
/// valid bearer that deliberately withholds request bodies across repeated
/// owner-triggered rotations — the listener itself is always released, so
/// token revocation stays effective; only thread leakage is being bounded.
pub const STALLED_PROCESSOR_CAP: usize = 4;

/// Processors that outlived their server's shutdown (blocked in a body read
/// tiny_http cannot cancel). Parked, not dropped, so the count stays exact:
/// a parked processor holds only its accepted socket — never the listener.
static STALLED_PROCESSORS: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

/// Test seam for the lifecycle latch: park an arbitrary thread as if it were
/// a stalled processor (building four REAL stalled servers would be slow and
/// flaky; the latch only reads the count).
#[cfg(test)]
pub(crate) fn park_stalled_for_tests(handle: JoinHandle<()>) {
    STALLED_PROCESSORS
        .lock()
        .expect("stalled-processor registry poisoned")
        .push(handle);
}

/// Live count of parked processors (finished ones are pruned on read).
pub fn stalled_processor_count() -> usize {
    let mut parked = STALLED_PROCESSORS
        .lock()
        .expect("stalled-processor registry poisoned");
    parked.retain(|handle| !handle.is_finished());
    parked.len()
}

/// The bearer digests one server instance authenticates against (ADR 0099
/// dec. 2). `kpi` is `None` when the acquisition credential is absent OR its
/// digest collides with the primary (fail-closed misconfiguration guard);
/// `dummy` is random per start so the absent-credential compare costs the
/// same as a real one without a known preimage.
struct ExpectedDigests {
    full: [u8; 32],
    kpi: Option<[u8; 32]>,
    dummy: [u8; 32],
}

/// A running MCP HTTP listener. The ACCEPTOR thread is the sole owner of the
/// `tiny_http::Server` and only ever sits in interruptible `recv()`; requests
/// travel over a channel to the PROCESSOR thread. This split is what makes
/// shutdown correct: `Server::drop` (the only thing that closes the accept
/// loop) never waits on a request body tiny_http cannot cancel.
pub struct McpServerHandle {
    server: Option<Arc<tiny_http::Server>>,
    acceptor: Option<JoinHandle<()>>,
    processor: Option<JoinHandle<()>>,
    processor_done: mpsc::Receiver<()>,
    stop: Arc<AtomicBool>,
    kpi_acquisition_active: bool,
    addr: SocketAddr,
}

impl McpServerHandle {
    /// Bind `127.0.0.1:<port>` (port `0` picks an ephemeral port) and start
    /// serving. `token` (and the optional acquisition token, ADR 0099 dec. 2)
    /// are the shared secrets the caller resolved (M1/M3 own their storage);
    /// only SHA-256 digests are kept — plaintext is never stored or logged
    /// (ADR 0078 G-3).
    pub fn start(
        state: AppState,
        token: &str,
        kpi_token: Option<&str>,
        port: u16,
    ) -> Result<Self, String> {
        let full = token_digest(token);
        let kpi = kpi_token.map(token_digest).filter(|digest| {
            // Equal secrets would silently grant the acquisition credential
            // the Full surface — fail closed instead (ADR 0099 dec. 2); the
            // UI renders the configured-but-unavailable state.
            !digests_equal(digest, &full)
        });
        let kpi_acquisition_active = kpi.is_some();
        let mut dummy = [0u8; 32];
        getrandom::fill(&mut dummy)
            .map_err(|error| format!("failed to derive a dummy digest: {error}"))?;
        let expected = ExpectedDigests { full, kpi, dummy };
        // Loopback is hardcoded on purpose (ADR 0078 G-4): the bind address is
        // not configurable and never widens beyond 127.0.0.1.
        //
        // SO_REUSEADDR (standard server practice): a connection the server side
        // closed leaves a TIME_WAIT entry on the port, and a plain bind then
        // refuses for up to a minute — which breaks the real quick
        // disable→enable toggle on the same port (and reddened the closure
        // coverage run's rebind test, 2026-07-12). Safe for listeners; it does
        // NOT allow hijacking an actively-listening port.
        let listener = {
            use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
            let socket = socket2::Socket::new(
                socket2::Domain::IPV4,
                socket2::Type::STREAM,
                Some(socket2::Protocol::TCP),
            )
            .map_err(|error| format!("failed to create MCP listener socket: {error}"))?;
            socket
                .set_reuse_address(true)
                .map_err(|error| format!("failed to set SO_REUSEADDR: {error}"))?;
            socket
                .bind(&SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into())
                .map_err(|error| format!("failed to bind 127.0.0.1:{port}: {error}"))?;
            socket
                .listen(128)
                .map_err(|error| format!("failed to listen on 127.0.0.1:{port}: {error}"))?;
            TcpListener::from(socket)
        };
        let server = tiny_http::Server::from_listener(listener, None)
            .map_err(|error| format!("failed to start MCP server on 127.0.0.1:{port}: {error}"))?;
        let addr = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| "listener has no IP address".to_owned())?;
        let server = Arc::new(server);
        let stop = Arc::new(AtomicBool::new(false));
        let (request_tx, request_rx) = mpsc::channel::<tiny_http::Request>();
        let (done_tx, processor_done) = mpsc::channel::<()>();

        let acceptor = {
            let owner = Arc::clone(&server);
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("brawler-mcp-accept".to_owned())
                .spawn(move || {
                    // `recv()` blocks until a request arrives or `unblock()`
                    // makes it error. The unblock marker is FIFO — queued
                    // requests would still come out first — so the stop flag
                    // is re-checked after EVERY recv and late requests are
                    // dropped (connection reset) instead of forwarded.
                    while let Ok(request) = owner.recv() {
                        if stop.load(Ordering::SeqCst) {
                            drop(request);
                            break;
                        }
                        if request_tx.send(request).is_err() {
                            break;
                        }
                    }
                })
                .map_err(|error| format!("failed to spawn MCP accept thread: {error}"))?
        };

        let processor = {
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("brawler-mcp".to_owned())
                .spawn(move || {
                    // Keep the done sender alive for the whole loop; its drop
                    // at thread end is the shutdown's join signal.
                    let _done_tx = done_tx;
                    while let Ok(request) = request_rx.recv() {
                        // A request queued before shutdown must not run on
                        // revoked digests: dropping the receiver (thread
                        // exit) is what discards the buffered queue.
                        if stop.load(Ordering::SeqCst) {
                            break;
                        }
                        handle_request(&state, &expected, request);
                    }
                })
                .map_err(|error| format!("failed to spawn MCP server thread: {error}"))?
        };

        Ok(Self {
            server: Some(server),
            acceptor: Some(acceptor),
            processor: Some(processor),
            processor_done,
            stop,
            kpi_acquisition_active,
            addr,
        })
    }

    /// The bound address (always loopback).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Whether this instance authenticates the acquisition credential (false
    /// when the token is absent or its digest collides with the primary).
    pub fn kpi_acquisition_active(&self) -> bool {
        self.kpi_acquisition_active
    }

    /// Gracefully stop: release the listener, then bounded-join the processor.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        // Order matters (ADR 0099 dec. 2 shutdown posture): flag BEFORE
        // unblock so neither thread acts on late traffic; join the acceptor
        // (fast — recv is interruptible) and drop the control Arc so the
        // final `Server::drop` closes the listener BEFORE any restart tries
        // to rebind; only then bounded-join the processor, which may sit in a
        // body read tiny_http cannot cancel.
        self.stop.store(true, Ordering::SeqCst);
        if let Some(server) = &self.server {
            server.unblock();
        }
        if let Some(acceptor) = self.acceptor.take() {
            let _ = acceptor.join();
        }
        self.server = None;
        if let Some(processor) = self.processor.take() {
            match self.processor_done.recv_timeout(PROCESSOR_JOIN_TIMEOUT) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = processor.join();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Stalled in a body read: park it (it holds only its
                    // accepted socket, never the listener) so the count stays
                    // exact for the lifecycle's start refusal at the cap.
                    STALLED_PROCESSORS
                        .lock()
                        .expect("stalled-processor registry poisoned")
                        .push(processor);
                }
            }
        }
    }
}

impl Drop for McpServerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// One request, start to finish. Defense order: route (404/405) → Host (403,
/// DNS-rebinding) → bearer scope resolution (401) → body cap (413) →
/// dispatch (200/202).
fn handle_request(state: &AppState, expected: &ExpectedDigests, mut request: tiny_http::Request) {
    if request.url() != "/mcp" {
        return respond_empty(request, 404);
    }
    if *request.method() != tiny_http::Method::Post {
        // No SSE stream in the MVP: GET /mcp (and everything else) is 405.
        return respond_empty(request, 405);
    }
    let host_ok = header_value(&request, "Host")
        .map(|host| is_localhost_host(&host))
        .unwrap_or(false);
    if !host_ok {
        return respond_empty(request, 403);
    }
    let Some(scope) = resolve_scope(state, expected, header_value(&request, "Authorization"))
    else {
        return respond_empty(request, 401);
    };
    // Declared-length check first (refuses without reading), then a hard cap
    // on the actual read for clients that lie or stream.
    if request
        .body_length()
        .map(|length| length > MAX_BODY_BYTES)
        .unwrap_or(false)
    {
        return respond_empty(request, 413);
    }
    let mut body = String::new();
    {
        let mut reader = request.as_reader().take(MAX_BODY_BYTES as u64 + 1);
        if reader.read_to_string(&mut body).is_err() {
            return respond_empty(request, 400);
        }
    }
    if body.len() > MAX_BODY_BYTES {
        return respond_empty(request, 413);
    }
    match protocol::dispatch(state, scope, &body) {
        Some(response_json) => {
            let response = tiny_http::Response::from_string(response_json)
                .with_status_code(200)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .expect("static header is valid"),
                );
            let _ = request.respond(response);
        }
        // A JSON-RPC notification: accepted, never answered (202, empty body).
        None => respond_empty(request, 202),
    }
}

/// Resolve the caller's scope from the `Authorization` header — or `None`
/// (401). Oracle-free by construction (ADR 0099 dec. 2): every request does
/// the same work before classification — hash the candidate (empty when the
/// header is absent), run BOTH constant-time compares (a random dummy stands
/// in for an absent acquisition digest), read the acquisition gate ALWAYS
/// (a dedicated single-row read, never the full settings model) — so an
/// unknown token, a valid-but-disabled acquisition token, and a gate-read
/// failure are indistinguishable on the wire. Classification order: a Full
/// match wins REGARDLESS of the gate-read outcome (fail-closed applies only
/// to the acquisition scope); acquisition requires presence AND digest match
/// AND the gate — never a dummy match alone.
fn resolve_scope(
    state: &AppState,
    expected: &ExpectedDigests,
    authorization: Option<String>,
) -> Option<McpScope> {
    let bearer = authorization.and_then(|value| value.strip_prefix("Bearer ").map(str::to_owned));
    let candidate = token_digest(bearer.as_deref().unwrap_or(""));
    let full_match = digests_equal(&candidate, &expected.full);
    let kpi_match = digests_equal(&candidate, expected.kpi.as_ref().unwrap_or(&expected.dummy));
    let gate = state.settings().kpi_acquisition_gate().unwrap_or(false);
    let bearer_present = bearer.is_some();
    if bearer_present && full_match {
        return Some(McpScope::Full);
    }
    if bearer_present && expected.kpi.is_some() && kpi_match && gate {
        return Some(McpScope::KpiAcquisition);
    }
    None
}

fn respond_empty(request: tiny_http::Request, status: u16) {
    let _ = request.respond(tiny_http::Response::empty(status));
}

fn header_value(request: &tiny_http::Request, name: &'static str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str().to_owned())
}

/// `Host` must be a localhost form — `127.0.0.1[:port]` or `localhost[:port]`
/// — so a malicious page resolving its own name to 127.0.0.1 (DNS rebinding)
/// cannot reach the server through a browser.
fn is_localhost_host(host: &str) -> bool {
    let trimmed = host.trim();
    let without_port = match trimmed.rsplit_once(':') {
        Some((name, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => name,
        _ => trimmed,
    };
    without_port.eq_ignore_ascii_case("127.0.0.1") || without_port.eq_ignore_ascii_case("localhost")
}

/// Constant-time equality over two SHA-256 digests: fold XOR over every byte
/// so timing does not reveal the first differing position.
fn digests_equal(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// SHA-256 digest of a bearer token.
fn token_digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{open_in_memory_database, AppState};
    use serde_json::Value;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    const TOKEN: &str = "test-token-123";
    const PING: &str = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;

    const KPI_TOKEN: &str = "test-kpi-token-456";

    fn start_server() -> McpServerHandle {
        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        McpServerHandle::start(state, TOKEN, None, 0).expect("bind ephemeral loopback port")
    }

    /// A server with BOTH credentials and a controllable gate setting.
    fn start_scoped_server(gate_enabled: bool, kpi_token: &str) -> (McpServerHandle, AppState) {
        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        state
            .update_settings(crate::storage::SettingsUpdate {
                kpi_acquisition_enabled: Some(gate_enabled),
                ..Default::default()
            })
            .expect("persist gate setting");
        let handle = McpServerHandle::start(state.clone(), TOKEN, Some(kpi_token), 0)
            .expect("bind ephemeral loopback port");
        (handle, state)
    }

    fn tools_of(body: &str) -> Vec<String> {
        let v: Value = serde_json::from_str(body).expect("json body");
        v["result"]["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .map(|tool| tool["name"].as_str().expect("name").to_owned())
            .collect()
    }

    const TOOLS_LIST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;

    /// Raw HTTP/1.1 request over `TcpStream` — full control over Host,
    /// Authorization, and the declared Content-Length (reqwest would fight us
    /// on Host and would actually transmit an oversized body).
    fn raw_request(
        addr: SocketAddr,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        declared_len: Option<usize>,
        body: &[u8],
    ) -> (u16, String) {
        let mut stream = TcpStream::connect(addr).expect("connect");
        let mut request = format!("{method} {path} HTTP/1.1\r\n");
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        let length = declared_len.unwrap_or(body.len());
        request.push_str(&format!(
            "Content-Length: {length}\r\nConnection: close\r\n\r\n"
        ));
        stream.write_all(request.as_bytes()).expect("write head");
        let _ = stream.write_all(body); // may be cut short by an early error response
        let mut raw = Vec::new();
        let _ = stream.read_to_end(&mut raw);
        let text = String::from_utf8_lossy(&raw).into_owned();
        let status: u16 = text
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or_else(|| panic!("no status line in response: {text:?}"));
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or(("", ""));
        // A large tools/list body trips tiny_http's chunked-transfer threshold;
        // de-chunk so the helper stays valid as the tool surface grows.
        let response_body = if head
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            dechunk(body)
        } else {
            body.to_owned()
        };
        (status, response_body)
    }

    /// Decode an HTTP/1.1 chunked body (`<hex-size>\r\n<data>\r\n … 0\r\n\r\n`).
    fn dechunk(body: &str) -> String {
        let mut rest = body;
        let mut out = String::new();
        while let Some((size_line, tail)) = rest.split_once("\r\n") {
            let size = usize::from_str_radix(size_line.trim(), 16).unwrap_or(0);
            if size == 0 {
                break;
            }
            out.push_str(&tail[..size]);
            rest = &tail[size..];
            rest = rest.strip_prefix("\r\n").unwrap_or(rest);
        }
        out
    }

    fn host_of(addr: SocketAddr) -> String {
        format!("127.0.0.1:{}", addr.port())
    }

    fn bearer(token: &str) -> String {
        format!("Bearer {token}")
    }

    #[test]
    fn missing_token_401() {
        let server = start_server();
        let addr = server.local_addr();
        let (status, _) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr))],
            None,
            PING.as_bytes(),
        );
        assert_eq!(status, 401);
    }

    #[test]
    fn wrong_token_401() {
        let server = start_server();
        let addr = server.local_addr();
        let auth = bearer("wrong-token");
        let (status, _) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            None,
            PING.as_bytes(),
        );
        assert_eq!(status, 401);
    }

    #[test]
    fn get_is_405() {
        let server = start_server();
        let addr = server.local_addr();
        let auth = bearer(TOKEN);
        let (status, _) = raw_request(
            addr,
            "GET",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            None,
            b"",
        );
        assert_eq!(status, 405, "no SSE stream in the MVP: GET /mcp is 405");
    }

    #[test]
    fn unknown_route_is_404() {
        let server = start_server();
        let addr = server.local_addr();
        let auth = bearer(TOKEN);
        let (status, _) = raw_request(
            addr,
            "POST",
            "/other",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            None,
            PING.as_bytes(),
        );
        assert_eq!(status, 404);
    }

    #[test]
    fn bad_host_header_403() {
        let server = start_server();
        let addr = server.local_addr();
        let auth = bearer(TOKEN);
        let (status, _) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", "evil.example.com"), ("Authorization", &auth)],
            None,
            PING.as_bytes(),
        );
        assert_eq!(status, 403, "DNS-rebinding defense: non-localhost Host");
    }

    #[test]
    fn oversized_body_413() {
        let server = start_server();
        let addr = server.local_addr();
        let auth = bearer(TOKEN);
        // Declare > 1 MiB; the server must refuse without reading it.
        let (status, _) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            Some(MAX_BODY_BYTES + 1),
            b"",
        );
        assert_eq!(status, 413);
    }

    #[test]
    fn valid_token_tools_list_round_trip() {
        let server = start_server();
        let addr = server.local_addr();
        let auth = bearer(TOKEN);
        let (status, body) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[
                ("Host", &host_of(addr)),
                ("Authorization", &auth),
                ("Content-Type", "application/json"),
            ],
            None,
            br#"{"jsonrpc":"2.0","id":7,"method":"tools/list"}"#,
        );
        assert_eq!(status, 200);
        let parsed: Value = serde_json::from_str(&body).expect("JSON body");
        assert_eq!(parsed["id"], 7);
        let names: Vec<&str> = parsed["result"]["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        // Transport-level check: the four MVP tools lead the list (frozen order)
        // and the full exposed surface is served. The exact schema contract is
        // frozen by the `tools/list` insta snapshot in `protocol::tests`.
        assert_eq!(
            &names[..4],
            &[
                "get_company_dossier",
                "search_research",
                "list_claims_due",
                "get_quality_assessment"
            ]
        );
        assert_eq!(
            names.len(),
            crate::mcp::registry::FROZEN_EXPOSED_TOOL_COUNT,
            "the frozen exposed-tool count (single source: registry::FROZEN_EXPOSED_TOOL_COUNT)"
        );
    }

    #[test]
    fn notification_returns_202_empty() {
        let server = start_server();
        let addr = server.local_addr();
        let auth = bearer(TOKEN);
        let (status, body) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            None,
            br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        );
        assert_eq!(status, 202);
        assert!(body.is_empty(), "202 must carry an empty body: {body:?}");
    }

    #[test]
    fn binds_loopback_only() {
        let server = start_server();
        let addr = server.local_addr();
        assert!(
            addr.ip().is_loopback(),
            "listener must bind loopback, got {addr}"
        );
        // And the loopback route actually serves.
        let auth = bearer(TOKEN);
        let (status, _) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            None,
            PING.as_bytes(),
        );
        assert_eq!(status, 200);
    }

    #[test]
    fn digest_compare_is_exact() {
        assert!(digests_equal(&token_digest("a"), &token_digest("a")));
        assert!(!digests_equal(&token_digest("a"), &token_digest("b")));
        assert!(!digests_equal(b"short", &token_digest("a")));
    }

    #[test]
    fn acquisition_token_with_gate_on_lists_exactly_the_scoped_tools() {
        // ADR 0099 dec. 3: the scoped tools/list is EXACTLY the allowlist —
        // empty until the workflow tools land (#384–#386); the frozen scoped
        // snapshot arrives with them. This is the mechanism fixture.
        let (server, _state) = start_scoped_server(true, KPI_TOKEN);
        let addr = server.local_addr();
        let auth = bearer(KPI_TOKEN);
        let (status, body) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            None,
            TOOLS_LIST.as_bytes(),
        );
        assert_eq!(status, 200);
        let names = tools_of(&body);
        assert_eq!(
            names,
            crate::mcp::registry::KPI_ACQUISITION_TOOLS
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>(),
            "the acquisition surface is exactly the allowlist"
        );
        // The full token on the SAME server still sees the whole surface.
        let full_auth = bearer(TOKEN);
        let (status, body) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &full_auth)],
            None,
            TOOLS_LIST.as_bytes(),
        );
        assert_eq!(status, 200);
        assert_eq!(
            tools_of(&body).len(),
            crate::mcp::registry::FROZEN_EXPOSED_TOOL_COUNT
        );
    }

    #[test]
    fn acquisition_token_with_gate_off_is_401_like_unknown() {
        // The gate kills the WHOLE scope at auth (ADR 0099 dec. 2): a valid
        // acquisition token with the setting off is indistinguishable from an
        // unknown token.
        let (server, _state) = start_scoped_server(false, KPI_TOKEN);
        let addr = server.local_addr();
        let auth = bearer(KPI_TOKEN);
        let (status, _) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            None,
            PING.as_bytes(),
        );
        assert_eq!(status, 401);
    }

    #[test]
    fn digest_collision_disables_the_acquisition_scope() {
        // Equal secrets would silently grant the acquisition credential the
        // Full surface — fail closed instead (ADR 0099 dec. 2).
        let (server, _state) = start_scoped_server(true, TOKEN);
        assert!(!server.kpi_acquisition_active());
        // The colliding token still authenticates — as FULL (it IS the
        // primary secret); nothing is granted through the kpi slot.
        let addr = server.local_addr();
        let auth = bearer(TOKEN);
        let (status, body) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            None,
            TOOLS_LIST.as_bytes(),
        );
        assert_eq!(status, 200);
        assert_eq!(
            tools_of(&body).len(),
            crate::mcp::registry::FROZEN_EXPOSED_TOOL_COUNT
        );
    }

    #[test]
    fn no_kpi_token_plus_gate_on_never_authenticates_a_third_token() {
        // Presence mask (ADR 0099): with no acquisition credential the dummy
        // digest must never authenticate anything, gate on or off.
        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        state
            .update_settings(crate::storage::SettingsUpdate {
                kpi_acquisition_enabled: Some(true),
                ..Default::default()
            })
            .expect("persist gate setting");
        let server =
            McpServerHandle::start(state, TOKEN, None, 0).expect("bind ephemeral loopback port");
        let addr = server.local_addr();
        let auth = bearer("some-other-token");
        let (status, _) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &auth)],
            None,
            PING.as_bytes(),
        );
        assert_eq!(status, 401);
    }

    #[test]
    fn a_broken_gate_read_rejects_acquisition_but_not_full() {
        // Failure seam (ADR 0099 dec. 2): a gate-read failure fails closed
        // ONLY for the acquisition scope; a valid Full token keeps working.
        let (server, state) = start_scoped_server(true, KPI_TOKEN);
        state
            .checkout_for_tests()
            .expect("connection")
            .execute("DROP TABLE settings", [])
            .expect("break the settings table");
        let addr = server.local_addr();
        let kpi_auth = bearer(KPI_TOKEN);
        let (status, _) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &kpi_auth)],
            None,
            PING.as_bytes(),
        );
        assert_eq!(status, 401, "acquisition fails closed on a gate-read error");
        let full_auth = bearer(TOKEN);
        let (status, _) = raw_request(
            addr,
            "POST",
            "/mcp",
            &[("Host", &host_of(addr)), ("Authorization", &full_auth)],
            None,
            PING.as_bytes(),
        );
        assert_eq!(status, 200, "a valid Full token is unaffected");
    }

    /// Open a connection that authenticates, declares a body, and then
    /// withholds it — the stall tiny_http cannot cancel. The declared length
    /// MUST exceed 1024: tiny_http pre-buffers smaller bodies on its own
    /// per-connection task (the request never reaches our processor), while
    /// larger ones hand our processor the blocking reader — the real stall.
    fn stalled_connection(addr: SocketAddr) -> TcpStream {
        let mut stream = TcpStream::connect(addr).expect("connect");
        let head = format!(
            "POST /mcp HTTP/1.1\r\nHost: {}\r\nAuthorization: {}\r\nContent-Length: 2000\r\n\r\n",
            host_of(addr),
            bearer(TOKEN),
        );
        stream.write_all(head.as_bytes()).expect("write head");
        stream.write_all(b"{").expect("write a partial body");
        stream.flush().expect("flush");
        // Give the acceptor time to hand the request to the processor.
        std::thread::sleep(Duration::from_millis(150));
        stream
    }

    #[test]
    fn stop_completes_despite_a_stalled_body_and_the_port_rebinds() {
        // ADR 0099 dec. 2 shutdown posture: a client withholding its body
        // must not block rotation — stop() bounds the processor join, parks
        // the stalled worker, and ALWAYS releases the listener.
        let server = start_server();
        let addr = server.local_addr();
        let stalled = stalled_connection(addr);

        let started = std::time::Instant::now();
        server.stop();
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "stop must be bounded, took {:?}",
            started.elapsed()
        );

        // The listener is truly released: a fresh server binds the SAME port
        // immediately (the lifecycle retry ladder is not needed here).
        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        let rebound = McpServerHandle::start(state, TOKEN, None, addr.port())
            .expect("rebind the same port immediately after a stalled stop");
        drop(rebound);

        // Cleanup: closing the stalled socket lets the parked processor exit.
        drop(stalled);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while stalled_processor_count() > 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "parked processor should exit once its connection closes"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    #[test]
    fn a_request_queued_behind_a_stalled_one_never_executes_after_stop() {
        // Cancellation covers BOTH channel sides (ADR 0099): a request queued
        // while the processor was stalled must not run on revoked digests
        // after stop — its connection is dropped without a response.
        let server = start_server();
        let addr = server.local_addr();
        let stalled = stalled_connection(addr);

        // A fully valid request that queues behind the stalled one.
        let mut queued = TcpStream::connect(addr).expect("connect queued");
        let head = format!(
            "POST /mcp HTTP/1.1\r\nHost: {}\r\nAuthorization: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            host_of(addr),
            bearer(TOKEN),
            PING.len(),
            PING,
        );
        queued.write_all(head.as_bytes()).expect("write queued");
        queued.flush().expect("flush queued");
        std::thread::sleep(Duration::from_millis(150));

        server.stop();
        drop(stalled);

        // The queued request must never DISPATCH: tiny_http auto-answers a
        // dropped, unanswered request with an empty 500 — acceptable; a 200
        // with a JSON-RPC result would mean it executed on revoked digests.
        queued
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("read timeout");
        let mut response = Vec::new();
        let _ = queued.read_to_end(&mut response);
        let text = String::from_utf8_lossy(&response);
        assert!(
            text.is_empty() || text.starts_with("HTTP/1.1 500"),
            "a queued request must be dropped on stop (empty or tiny_http's \
             auto-500), got: {text:?}"
        );
        assert!(
            !text.contains("jsonrpc"),
            "a queued request must never dispatch after stop: {text:?}"
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while stalled_processor_count() > 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "parked processor should exit once its connection closes"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
