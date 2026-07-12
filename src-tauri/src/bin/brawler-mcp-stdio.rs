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
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
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
        let post = |body: &str| post_line(&client, &url, &config.token, config.port, body);
        if let Some(response_line) = frame_line(&line, post) {
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

    #[test]
    fn config_missing_token_is_fatal() {
        // No --token and (in this hermetic call) no env var provided.
        let err = Config::from_env_and_args(["--port", "9000"].into_iter().map(String::from))
            .expect_err("a missing token must be rejected");
        assert!(
            err.contains("token"),
            "error explains the missing token: {err}"
        );
    }
}
