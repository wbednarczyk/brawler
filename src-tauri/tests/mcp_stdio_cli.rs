//! Subprocess coverage for the `brawler-mcp-stdio` binary's config resolution
//! and exit codes (ADR 0078 decision 6). Runs the real compiled binary via
//! `CARGO_BIN_EXE_brawler-mcp-stdio` so `Config::from_env_and_args`'s env
//! fallbacks are exercised against a real, `.env_clear()`-ed process
//! environment (see [`bin`]) rather than mutating the test process's own env
//! — the bin's `#[cfg(test)]` unit tests stay hermetic (no
//! `std::env::set_var`).
//!
//! These tests join the serialized `loopback-sockets` nextest group
//! (`src-tauri/.config/nextest.toml`, `binary(=mcp_stdio_cli)`).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// A `Command` for the compiled binary, pre-cleared of the process env so
/// each test controls exactly what config the child sees. `LLVM_PROFILE_FILE`
/// is forwarded when set (`cargo llvm-cov nextest`, `Makefile:338`) so the
/// subprocess still writes its `.profraw` — its `%p` pattern is substituted
/// with the child's own pid at write time, so re-using the same value across
/// tests/children is safe. Without this, `.env_clear()` would silently drop
/// `main`/`run` out of the coverage report despite being exercised here.
fn bin() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brawler-mcp-stdio"));
    command.env_clear();
    if let Ok(profile_file) = std::env::var("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile_file);
    }
    command
}

/// A free loopback port that is then dropped, so a connection to it is
/// refused — used to force the adapter's connection-error path without a
/// real MCP server.
fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("probe a free port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

/// What the one-shot stub observed on the single connection it accepted.
struct StubResult {
    path: String,
    authorization: Option<String>,
}

/// A minimal, ONE-SHOT loopback HTTP/1.1 stub for the subprocess-facing CLI
/// tests: binds an ephemeral port (`127.0.0.1:0`, never a probe-then-drop),
/// accepts exactly one connection bounded by a 5s deadline (so a binary that
/// never posts — e.g. because an env/flag fallback silently broke — fails
/// this test fast instead of hanging it), reads the request head + body,
/// replies 200 with `body`, and returns what it observed. Deliberately
/// separate from the bin's own internal `spawn_stub_server`
/// (src/bin/brawler-mcp-stdio.rs) — that one lives in a different test
/// binary and this one only ever needs a single request/response.
fn spawn_one_shot_stub(body: &'static str) -> (u16, JoinHandle<StubResult>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub listener");
    listener
        .set_nonblocking(true)
        .expect("stub listener nonblocking");
    let port = listener.local_addr().expect("stub local addr").port();

    let handle = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        panic!(
                            "stub timed out after 5s waiting for the binary's POST — a \
                             production regression likely broke the port/token it used"
                        );
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("stub accept failed: {e}"),
            }
        };
        stream.set_nonblocking(false).expect("stub stream blocking");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("stub read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("stub write timeout");

        let mut reader = BufReader::new(stream.try_clone().expect("clone stub stream"));
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .expect("read request line");
        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_owned();

        let mut content_length = 0usize;
        let mut authorization = None;
        loop {
            let mut header_line = String::new();
            reader
                .read_line(&mut header_line)
                .expect("read header line");
            let trimmed = header_line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                let value = value.trim().to_owned();
                match name.trim().to_ascii_lowercase().as_str() {
                    "content-length" => content_length = value.parse().unwrap_or(0),
                    "authorization" => authorization = Some(value),
                    _ => {}
                }
            }
        }
        let mut body_bytes = vec![0u8; content_length];
        reader
            .read_exact(&mut body_bytes)
            .expect("read request body");

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write stub response");

        StubResult {
            path,
            authorization,
        }
    });

    (port, handle)
}

fn send_one_request_line(child: &mut std::process::Child) {
    {
        let stdin = child.stdin.as_mut().expect("piped stdin");
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list"}}"#)
            .expect("write request line");
    }
    drop(child.stdin.take());
}

#[test]
fn missing_token_is_fatal_with_exit_code_2() {
    let output = bin()
        .arg("--port")
        .arg("9000")
        .stdin(Stdio::null())
        .output()
        .expect("spawn brawler-mcp-stdio");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("brawler-mcp-stdio:"),
        "stderr names the binary: {stderr}"
    );
    assert!(
        stderr.contains("token"),
        "stderr explains the missing token: {stderr}"
    );
}

#[test]
fn invalid_port_flag_is_fatal_with_exit_code_2() {
    let output = bin()
        .args(["--port", "abc", "--token", "sometoken"])
        .stdin(Stdio::null())
        .output()
        .expect("spawn brawler-mcp-stdio");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("brawler-mcp-stdio:"),
        "stderr names the binary: {stderr}"
    );
}

#[test]
fn env_config_relays_a_real_request_through_the_stub() {
    // A real round trip, not just "empty stdin exits 0" (which survives
    // deleting the env-port fallback entirely, since no connection is ever
    // attempted): if BRAWLER_MCP_PORT's fallback were broken, the binary
    // would fall back to the DEFAULT_PORT (8317) instead, almost certainly
    // miss the stub, and this test would see a connection-error envelope
    // instead of the stub's body.
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
    let (port, stub) = spawn_one_shot_stub(body);

    let mut child = bin()
        .env("BRAWLER_MCP_TOKEN", "x")
        .env("BRAWLER_MCP_PORT", port.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn brawler-mcp-stdio");
    send_one_request_line(&mut child);

    let output = child.wait_with_output().expect("wait for exit");
    let result = stub.join().expect("stub thread joins");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(result.path, "/mcp");
    assert_eq!(result.authorization.as_deref(), Some("Bearer x"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        format!("{body}\n"),
        "the binary must print the stub's body verbatim"
    );
}

#[test]
fn flags_win_over_env_and_the_stub_observes_the_flag_token() {
    // Env points at a CLOSED port with the WRONG token; only the flags point
    // at the real stub with the right token. This proves both halves flags
    // must win on: if the PORT fallback leaked through, the request would hit
    // the closed port (never the stub) and `stub.join()` would time out; if
    // the TOKEN fallback leaked through, the stub would observe "Bearer
    // wrongtoken" instead of the flag token.
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
    let (stub_port, stub) = spawn_one_shot_stub(body);
    let closed = closed_port();

    let mut child = bin()
        .env("BRAWLER_MCP_TOKEN", "wrongtoken")
        .env("BRAWLER_MCP_PORT", closed.to_string())
        .args(["--port", &stub_port.to_string(), "--token", "flagtoken"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn brawler-mcp-stdio");
    send_one_request_line(&mut child);

    let output = child.wait_with_output().expect("wait for exit");
    let result = stub.join().expect("stub thread joins");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        result.authorization.as_deref(),
        Some("Bearer flagtoken"),
        "the stub observed the FLAG token, proving flags win over env"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, format!("{body}\n"));
}

#[test]
fn ambient_proxy_configuration_is_ignored_for_the_loopback_post() {
    // Ambient proxy env must never redirect the loopback POST (#494). Reddens
    // on a plain `Client::new()` (env proxy discovery sends the POST to the
    // bogus closed-port proxy; the stub's 5s accept deadline fires). `bin()`
    // is `env_clear()`-ed, so no inherited `NO_PROXY` can exempt loopback on
    // its own — only `.no_proxy()` in the binary can.
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
    let (port, stub) = spawn_one_shot_stub(body);
    let bogus_proxy = "http://127.0.0.1:1";

    let mut child = bin()
        .env("BRAWLER_MCP_TOKEN", "x")
        .env("BRAWLER_MCP_PORT", port.to_string())
        .env("HTTP_PROXY", bogus_proxy)
        .env("http_proxy", bogus_proxy)
        .env("ALL_PROXY", bogus_proxy)
        .env("all_proxy", bogus_proxy)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn brawler-mcp-stdio");
    send_one_request_line(&mut child);

    let output = child.wait_with_output().expect("wait for exit");
    let result = stub.join().expect("stub thread joins");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(result.path, "/mcp");
    assert_eq!(result.authorization.as_deref(), Some("Bearer x"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        format!("{body}\n"),
        "the binary must print the stub's body verbatim, proving the POST reached \
         the stub directly and not the bogus proxy"
    );
}
