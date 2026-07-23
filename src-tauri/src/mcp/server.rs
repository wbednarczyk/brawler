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
use std::sync::Arc;
use std::thread::JoinHandle;

use sha2::{Digest, Sha256};

use crate::app_state::AppState;
use crate::mcp::protocol;

/// Request bodies larger than this are rejected with `413` (ADR 0078 decision 2).
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// A running MCP HTTP listener. Dropping (or [`Self::stop`]) unblocks the
/// accept loop and joins the worker thread.
pub struct McpServerHandle {
    server: Arc<tiny_http::Server>,
    thread: Option<JoinHandle<()>>,
    addr: SocketAddr,
}

impl McpServerHandle {
    /// Bind `127.0.0.1:<port>` (port `0` picks an ephemeral port) and start
    /// serving on a dedicated thread. `token` is the shared-secret the caller
    /// resolved (M1/M3 own its storage); only its SHA-256 digest is kept —
    /// the plaintext is never stored or logged (ADR 0078 G-3).
    pub fn start(state: AppState, token: &str, port: u16) -> Result<Self, String> {
        let expected_digest = token_digest(token);
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
        let worker = Arc::clone(&server);
        let thread = std::thread::Builder::new()
            .name("brawler-mcp".to_owned())
            .spawn(move || {
                // `recv()` blocks until a request arrives or `unblock()` is
                // called, which makes it return an error — the exit signal.
                while let Ok(request) = worker.recv() {
                    handle_request(&state, &expected_digest, request);
                }
            })
            .map_err(|error| format!("failed to spawn MCP server thread: {error}"))?;
        Ok(Self {
            server,
            thread: Some(thread),
            addr,
        })
    }

    /// The bound address (always loopback).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Gracefully stop: unblock the accept loop and join the thread.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.server.unblock();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for McpServerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// One request, start to finish. Defense order: route (404/405) → Host (403,
/// DNS-rebinding) → bearer token (401) → body cap (413) → dispatch (200/202).
fn handle_request(state: &AppState, expected_digest: &[u8; 32], mut request: tiny_http::Request) {
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
    let authorized = header_value(&request, "Authorization")
        .and_then(|value| value.strip_prefix("Bearer ").map(str::to_owned))
        .map(|token| digests_equal(&token_digest(&token), expected_digest))
        .unwrap_or(false);
    if !authorized {
        return respond_empty(request, 401);
    }
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
    match protocol::dispatch(state, &body) {
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

    fn start_server() -> McpServerHandle {
        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        McpServerHandle::start(state, TOKEN, 0).expect("bind ephemeral loopback port")
    }

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
            96,
            "41 read (4 MVP + 34 read-wave + list_alert_rules + list_flagged_extraction_outcomes \
             + list_unclassified_filings) + 55 act tools incl. classify_filing (ADR 0088 dec. 2/3/4)"
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
}
