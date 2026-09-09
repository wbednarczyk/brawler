//! `brawler-mcp-stdio` — the thin stdio↔HTTP adapter for the read-only MCP
//! server (ADR 0078 decision 6). A **dumb pipe**: it reads newline-delimited
//! JSON-RPC from stdin, POSTs each line to `http://127.0.0.1:<port>/mcp` with
//! the bearer token, and writes the response body back as one line to stdout.
//! It carries **no MCP protocol knowledge** — a malformed line is forwarded
//! as-is and the server's own parse-error response passes straight through.
//!
//! Two things the adapter must synthesize itself, because a request that never
//! reached the server can't be answered by it:
//!
//! - a JSON-RPC notification (202 / empty response) produces **no output line**;
//! - a failure to reach the server (connection refused, timeout, a non-success
//!   HTTP status like 401) becomes a proper JSON-RPC `-32603` error envelope
//!   naming the port and the enable-in-Settings remedy — so the client sees a
//!   clear message instead of the adapter dying silently on the first line.
//!
//! Config: `--port` / `--token` flags, falling back to `BRAWLER_MCP_PORT` /
//! `BRAWLER_MCP_TOKEN`. HTTP-native clients (Claude Code `--transport http`)
//! connect to the endpoint directly and never need this adapter.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

/// Default MCP port (ADR 0078 decision 4); overridable via `--port` /
/// `BRAWLER_MCP_PORT`.
const DEFAULT_PORT: u16 = 8317;

fn main() {
    std::process::exit(run());
}

/// Real entry point (returns a process exit code so `main` stays trivial).
fn run() -> i32 {
    let config = match Config::from_env_and_args(std::env::args().skip(1)) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("brawler-mcp-stdio: {message}");
            return 2;
        }
    };

    let client = reqwest::blocking::Client::new();
    let url = format!("http://127.0.0.1:{}/mcp", config.port);

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    run_loop(stdin.lock(), stdout.lock(), |body| {
        post_line(&client, &url, &config.token, config.port, body)
    })
}

/// The stdin→POST→stdout loop itself, extracted so tests can drive it over an
/// in-memory reader/writer with a stubbed `post`, without a real HTTP server
/// or the process's real stdio. Behavior is unchanged from the inline loop
/// this replaced: blank lines are skipped, a stdin read error is fatal (exit
/// 1), a write/flush error on `out` (the client went away) stops cleanly
/// (exit 0), and reaching EOF stops cleanly (exit 0).
fn run_loop(
    input: impl BufRead,
    mut out: impl Write,
    post: impl Fn(&str) -> Result<PostOutcome, String>,
) -> i32 {
    for line in input.lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("brawler-mcp-stdio: failed to read stdin: {error}");
                return 1;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response_line) = frame_line(&line, &post) {
            if writeln!(out, "{response_line}").is_err() || out.flush().is_err() {
                // stdout closed (client went away) — stop cleanly.
                return 0;
            }
        }
    }
    0
}

/// Resolved runtime configuration.
#[derive(Debug)]
struct Config {
    port: u16,
    token: String,
}

impl Config {
    /// Resolve from CLI args (`--port <n>`, `--token <s>`) with env fallbacks
    /// (`BRAWLER_MCP_PORT`, `BRAWLER_MCP_TOKEN`). A missing token is fatal — the
    /// server rejects unauthenticated requests, so there is nothing to forward.
    fn from_env_and_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut port: Option<u16> = None;
        let mut token: Option<String> = None;
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--port" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--port requires a value".to_owned())?;
                    port = Some(
                        value
                            .parse()
                            .map_err(|_| format!("invalid --port value: {value}"))?,
                    );
                }
                "--token" => {
                    token = Some(
                        args.next()
                            .ok_or_else(|| "--token requires a value".to_owned())?,
                    );
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        let port = port
            .or_else(|| {
                std::env::var("BRAWLER_MCP_PORT")
                    .ok()
                    .and_then(|value| value.parse().ok())
            })
            .unwrap_or(DEFAULT_PORT);
        let token = token
            .or_else(|| std::env::var("BRAWLER_MCP_TOKEN").ok())
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                "no MCP token: pass --token or set BRAWLER_MCP_TOKEN (copy it from Brawler → \
                 Settings → MCP server)"
                    .to_owned()
            })?;
        Ok(Self { port, token })
    }
}

/// What POSTing one line to the server returned, before framing.
enum PostOutcome {
    /// The response carried a body → emit it verbatim as one stdout line.
    Body(String),
    /// A 202 / empty response — a JSON-RPC notification was accepted → emit
    /// nothing.
    Empty,
}

/// Map one stdin line and its POST outcome to **at most one** stdout line.
///
/// The adapter has no protocol knowledge (ADR 0078 decision 6): whatever body
/// the server returns — a result *or* a `-32700` parse error for a malformed
/// line — is relayed verbatim. The single response the adapter synthesizes is
/// the connection-failure envelope, because a dead server cannot answer at all.
fn frame_line(line: &str, post: impl Fn(&str) -> Result<PostOutcome, String>) -> Option<String> {
    match post(line) {
        Ok(PostOutcome::Body(body)) => Some(body),
        Ok(PostOutcome::Empty) => None,
        Err(message) => connection_error_line(line, &message),
    }
}

/// Build the `-32603` envelope for a request that never reached the server.
///
/// A genuine JSON-RPC notification (no `id`) still gets no response, even on
/// failure. A line we cannot parse at all gets a `null`-id envelope so a dead
/// server surfaces an error instead of silence (we can't reach the server to
/// obtain its `-32700` either).
fn connection_error_line(line: &str, message: &str) -> Option<String> {
    match serde_json::from_str::<Value>(line) {
        Ok(value) => {
            let id = value.get("id")?;
            Some(error_envelope(id.clone(), message))
        }
        Err(_) => Some(error_envelope(Value::Null, message)),
    }
}

/// A JSON-RPC 2.0 internal-error (`-32603`) response with the given id.
fn error_envelope(id: Value, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32603, "message": message },
    })
    .to_string()
}

/// The user-facing message for an unreachable server: name the port and the
/// remedy (enable it in Settings) so the failure is self-explanatory.
fn connection_error_message(port: u16, detail: &str) -> String {
    format!(
        "cannot reach the Brawler MCP server at http://127.0.0.1:{port}/mcp ({detail}). \
         Enable the MCP server in Brawler → Settings → MCP server and confirm the port."
    )
}

/// POST one line to the endpoint. A transport error or any non-success,
/// non-202 status becomes `Err` (framed into a `-32603` envelope upstream); a
/// 202 becomes [`PostOutcome::Empty`]; a body-carrying success is relayed.
fn post_line(
    client: &reqwest::blocking::Client,
    url: &str,
    token: &str,
    port: u16,
    body: &str,
) -> Result<PostOutcome, String> {
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(body.to_owned())
        .send()
        .map_err(|error| connection_error_message(port, &error.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::ACCEPTED {
        // JSON-RPC notification accepted; nothing to return.
        return Ok(PostOutcome::Empty);
    }
    if !status.is_success() {
        // 401/403/405/413 etc. carry no JSON-RPC body — surface the HTTP status
        // as a reachability error rather than leaving the client hanging.
        return Err(connection_error_message(
            port,
            &format!("server returned HTTP {}", status.as_u16()),
        ));
    }
    let text = response.text().unwrap_or_default();
    if text.trim().is_empty() {
        Ok(PostOutcome::Empty)
    } else {
        Ok(PostOutcome::Body(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST_WITH_ID: &str = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    const NOTIFICATION: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;

    #[test]
    fn request_line_yields_one_response_line() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
        let out = frame_line(REQUEST_WITH_ID, |_| Ok(PostOutcome::Body(body.to_owned())));
        assert_eq!(out.as_deref(), Some(body));
    }

    #[test]
    fn notification_yields_no_line() {
        let out = frame_line(NOTIFICATION, |_| Ok(PostOutcome::Empty));
        assert_eq!(out, None, "a notification (202/empty) produces no output");
    }

    #[test]
    fn malformed_line_relays_server_parse_error_verbatim() {
        // The adapter has no protocol knowledge: it forwards the bad line and
        // relays whatever the server returns (here a -32700), unchanged.
        let parse_error =
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}"#;
        let out = frame_line("{ not json", |_| {
            Ok(PostOutcome::Body(parse_error.to_owned()))
        });
        assert_eq!(out.as_deref(), Some(parse_error));
    }

    #[test]
    fn connection_refused_yields_internal_error_envelope() {
        let message = connection_error_message(8317, "connection refused");
        let out = frame_line(REQUEST_WITH_ID, |_| Err(message.clone()))
            .expect("a request with an id must get an error envelope");
        let value: Value = serde_json::from_str(&out).expect("valid JSON envelope");
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], 1);
        assert_eq!(value["error"]["code"], -32603);
        let text = value["error"]["message"].as_str().unwrap();
        assert!(text.contains("8317"), "message names the port: {text}");
        assert!(
            text.contains("Settings"),
            "message names the remedy: {text}"
        );
    }

    #[test]
    fn connection_refused_on_notification_yields_nothing() {
        // Proper JSON-RPC: a notification gets no response even when the server
        // is unreachable.
        let out = frame_line(NOTIFICATION, |_| Err("down".to_owned()));
        assert_eq!(out, None);
    }

    #[test]
    fn connection_refused_on_unparseable_line_yields_null_id_envelope() {
        let out = frame_line("not json at all", |_| Err("down".to_owned()))
            .expect("an unparseable line still surfaces the failure");
        let value: Value = serde_json::from_str(&out).expect("valid JSON envelope");
        assert_eq!(value["id"], Value::Null);
        assert_eq!(value["error"]["code"], -32603);
    }

    #[test]
    fn config_prefers_flags_over_env() {
        let config = Config::from_env_and_args(
            ["--port", "9000", "--token", "flagtoken"]
                .into_iter()
                .map(String::from),
        )
        .expect("valid flags");
        assert_eq!(config.port, 9000);
        assert_eq!(config.token, "flagtoken");
    }

    // `config_missing_token_is_fatal` moved to `tests/mcp_stdio_cli.rs` as a
    // subprocess test: `Config::from_env_and_args` reads process env, so
    // exercising the "missing token" path belongs with the other env-mutating
    // coverage, out of this in-process unit-test module.

    // --- run_loop: the stdin→POST→stdout loop, over a real loopback stub ---

    use std::io::{BufReader, Read};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    /// One request the stub server observed.
    struct RecordedRequest {
        method: String,
        path: String,
        authorization: Option<String>,
        content_type: Option<String>,
        body: String,
    }

    /// A minimal loopback HTTP/1.1 stub: binds an ephemeral port, replies to
    /// each request with the next scripted `(status, body)` pair in order,
    /// then stops. Reads and drains the real `Content-Length` body (rather
    /// than assuming none) so `post_line`'s blocking `send()` never hangs
    /// waiting for a body the stub never wrote.
    fn spawn_stub_server(
        responses: Vec<(u16, &'static str)>,
    ) -> (u16, Arc<Mutex<Vec<RecordedRequest>>>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub listener");
        let port = listener.local_addr().expect("stub local addr").port();
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let recorded_in_thread = Arc::clone(&recorded);

        let handle = thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().expect("accept stub connection");
                let request = read_request(&mut stream);
                recorded_in_thread
                    .lock()
                    .expect("recorded lock")
                    .push(request);

                let reason = match status {
                    200 => "OK",
                    202 => "Accepted",
                    401 => "Unauthorized",
                    _ => "Status",
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write stub response");
            }
        });

        (port, recorded, handle)
    }

    /// Read one HTTP/1.1 request off `stream`: the request line, headers up
    /// to the blank line, then exactly `Content-Length` body bytes.
    fn read_request(stream: &mut TcpStream) -> RecordedRequest {
        let mut reader = BufReader::new(stream.try_clone().expect("clone stub stream"));

        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .expect("read stub request line");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_owned();
        let path = parts.next().unwrap_or_default().to_owned();

        let mut content_length = 0usize;
        let mut authorization = None;
        let mut content_type = None;
        loop {
            let mut header_line = String::new();
            reader
                .read_line(&mut header_line)
                .expect("read stub header line");
            let trimmed = header_line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                let value = value.trim().to_owned();
                match name.trim().to_ascii_lowercase().as_str() {
                    "content-length" => content_length = value.parse().unwrap_or(0),
                    "authorization" => authorization = Some(value),
                    "content-type" => content_type = Some(value),
                    _ => {}
                }
            }
        }

        let mut body_bytes = vec![0u8; content_length];
        reader.read_exact(&mut body_bytes).expect("read stub body");
        let body = String::from_utf8(body_bytes).expect("stub body is utf8");

        RecordedRequest {
            method,
            path,
            authorization,
            content_type,
            body,
        }
    }

    /// A client with a short timeout, so a test that expects "connection
    /// refused" against a closed port fails fast instead of hanging on the
    /// default (unbounded) reqwest blocking client.
    fn test_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("test client builds")
    }

    fn cursor(input: &str) -> std::io::Cursor<Vec<u8>> {
        std::io::Cursor::new(input.as_bytes().to_vec())
    }

    #[test]
    fn run_loop_relays_a_200_body_as_one_line_and_posts_correctly() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
        let (port, recorded, handle) = spawn_stub_server(vec![(200, body)]);
        let client = test_client();
        let url = format!("http://127.0.0.1:{port}/mcp");
        let mut output = Vec::new();

        let code = run_loop(cursor(REQUEST_WITH_ID), &mut output, |line| {
            post_line(&client, &url, "tok", port, line)
        });
        handle.join().expect("stub thread joins");

        assert_eq!(code, 0);
        assert_eq!(String::from_utf8(output).unwrap(), format!("{body}\n"));

        let requests = recorded.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/mcp");
        assert_eq!(requests[0].authorization.as_deref(), Some("Bearer tok"));
        assert_eq!(
            requests[0].content_type.as_deref(),
            Some("application/json")
        );
        assert_eq!(requests[0].body, REQUEST_WITH_ID);
    }

    #[test]
    fn run_loop_emits_nothing_for_a_202_with_a_nonempty_body() {
        // A non-empty body on 202 must still be swallowed — this is what kills
        // a mutant that removes the dedicated 202 branch in `post_line` (a
        // 202 is `is_success()`, so without the branch a non-empty body would
        // fall through and be relayed).
        let (port, _recorded, handle) =
            spawn_stub_server(vec![(202, r#"{"jsonrpc":"2.0","id":1,"result":{}}"#)]);
        let client = test_client();
        let url = format!("http://127.0.0.1:{port}/mcp");
        let mut output = Vec::new();

        let code = run_loop(cursor(REQUEST_WITH_ID), &mut output, |line| {
            post_line(&client, &url, "tok", port, line)
        });
        handle.join().expect("stub thread joins");

        assert_eq!(code, 0);
        assert!(output.is_empty(), "202 must yield no output line");
    }

    #[test]
    fn run_loop_emits_nothing_for_a_200_with_an_empty_body() {
        let (port, _recorded, handle) = spawn_stub_server(vec![(200, "")]);
        let client = test_client();
        let url = format!("http://127.0.0.1:{port}/mcp");
        let mut output = Vec::new();

        let code = run_loop(cursor(REQUEST_WITH_ID), &mut output, |line| {
            post_line(&client, &url, "tok", port, line)
        });
        handle.join().expect("stub thread joins");

        assert_eq!(code, 0);
        assert!(output.is_empty(), "an empty 200 body must yield no line");
    }

    #[test]
    fn run_loop_frames_a_401_as_an_internal_error_envelope() {
        let (port, _recorded, handle) = spawn_stub_server(vec![(401, "")]);
        let client = test_client();
        let url = format!("http://127.0.0.1:{port}/mcp");
        let mut output = Vec::new();

        let code = run_loop(cursor(REQUEST_WITH_ID), &mut output, |line| {
            post_line(&client, &url, "tok", port, line)
        });
        handle.join().expect("stub thread joins");

        assert_eq!(code, 0);
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 1);
        let value: Value = serde_json::from_str(lines[0]).expect("valid JSON envelope");
        assert_eq!(value["id"], 1);
        assert_eq!(value["error"]["code"], -32603);
        let message = value["error"]["message"].as_str().unwrap();
        assert!(
            message.contains(&port.to_string()),
            "message names the port: {message}"
        );
    }

    #[test]
    fn run_loop_emits_two_lines_in_order_for_two_requests() {
        let first = r#"{"jsonrpc":"2.0","id":1,"result":"first"}"#;
        let second = r#"{"jsonrpc":"2.0","id":2,"result":"second"}"#;
        let (port, _recorded, handle) = spawn_stub_server(vec![(200, first), (200, second)]);
        let client = test_client();
        let url = format!("http://127.0.0.1:{port}/mcp");
        let mut output = Vec::new();

        let input = format!(
            "{}\n{}\n",
            r#"{"jsonrpc":"2.0","id":1,"method":"a"}"#, r#"{"jsonrpc":"2.0","id":2,"method":"b"}"#
        );
        let code = run_loop(cursor(&input), &mut output, |line| {
            post_line(&client, &url, "tok", port, line)
        });
        handle.join().expect("stub thread joins");

        assert_eq!(code, 0);
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text, format!("{first}\n{second}\n"));
    }

    #[test]
    fn run_loop_skips_blank_lines_without_touching_the_server() {
        // No responses scripted: if a request were sent, `post_line` would
        // either hang (nothing to accept it) or, with the short test client
        // timeout, surface a connection error — either way `output` would be
        // non-empty. It stays empty because blank/whitespace-only lines never
        // reach `post`.
        let (port, recorded, handle) = spawn_stub_server(vec![]);
        let client = test_client();
        let url = format!("http://127.0.0.1:{port}/mcp");
        let mut output = Vec::new();

        let code = run_loop(cursor("\n   \n\t\n"), &mut output, |line| {
            post_line(&client, &url, "tok", port, line)
        });
        handle
            .join()
            .expect("stub thread joins (no accepts needed)");

        assert_eq!(code, 0);
        assert!(output.is_empty());
        assert!(recorded.lock().unwrap().is_empty(), "stub saw no requests");
    }

    /// A `BufRead` whose `read_line` always errors, to exercise the fatal
    /// stdin-read-error path without touching the real process stdin.
    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("stub read failure"))
        }
    }

    impl BufRead for FailingReader {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            Err(std::io::Error::other("stub read failure"))
        }
        fn consume(&mut self, _amt: usize) {}
    }

    #[test]
    fn run_loop_returns_1_on_a_reader_error_without_posting() {
        let code = run_loop(FailingReader, Vec::new(), |_line| {
            panic!("post must never be called: the read fails before any line is produced")
        });
        assert_eq!(code, 1);
    }

    /// A `Write` that can be told to fail on `write` or on `flush`, to
    /// exercise the "client went away" early-return-0 path.
    enum FailAt {
        Write,
        Flush,
    }

    struct FailingWriter(FailAt);

    impl Write for FailingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            match self.0 {
                FailAt::Write => Err(std::io::Error::other("stub write failure")),
                FailAt::Flush => Ok(buf.len()),
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            match self.0 {
                FailAt::Write => Ok(()),
                FailAt::Flush => Err(std::io::Error::other("stub flush failure")),
            }
        }
    }

    #[test]
    fn run_loop_returns_0_on_a_sink_write_error() {
        let code = run_loop(
            cursor(REQUEST_WITH_ID),
            FailingWriter(FailAt::Write),
            |_| Ok(PostOutcome::Body("irrelevant".to_owned())),
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn run_loop_returns_0_on_a_sink_flush_error() {
        let code = run_loop(
            cursor(REQUEST_WITH_ID),
            FailingWriter(FailAt::Flush),
            |_| Ok(PostOutcome::Body("irrelevant".to_owned())),
        );
        assert_eq!(code, 0);
    }
}
