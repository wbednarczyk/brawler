//! Subprocess coverage for the `brawler-mcp-stdio` binary's config resolution
//! and exit codes (ADR 0078 decision 6). Runs the real compiled binary via
//! `CARGO_BIN_EXE_brawler-mcp-stdio` so `Config::from_env_and_args`'s env
//! fallbacks are exercised against a real, `.env_clear()`-ed process
//! environment (see [`bin`]) rather than mutating the test process's own env
//! — the bin's `#[cfg(test)]` unit tests stay hermetic (no
//! `std::env::set_var`).

use std::io::Write;
use std::net::TcpListener;
use std::process::{Command, Stdio};

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
fn env_config_with_empty_stdin_exits_0_at_eof() {
    let status = bin()
        .env("BRAWLER_MCP_TOKEN", "envtoken")
        .env("BRAWLER_MCP_PORT", "9001")
        .stdin(Stdio::null())
        .status()
        .expect("spawn brawler-mcp-stdio");

    assert_eq!(status.code(), Some(0), "config from env is accepted");
}

#[test]
fn flags_win_over_env_and_the_error_names_the_flag_port() {
    let port = closed_port();
    let mut child = bin()
        // Env would point elsewhere; the flags must win.
        .env("BRAWLER_MCP_TOKEN", "envtoken")
        .env("BRAWLER_MCP_PORT", "1")
        .args(["--port", &port.to_string(), "--token", "flagtoken"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn brawler-mcp-stdio");

    {
        let stdin = child.stdin.as_mut().expect("piped stdin");
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list"}}"#)
            .expect("write request line");
    }
    // Drop stdin (EOF) by taking it, then wait for the process to exit.
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for exit");
    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "one connection-error envelope: {stdout}");
    assert!(
        lines[0].contains(&port.to_string()),
        "error names the flag port ({port}), proving flags won over env: {stdout}"
    );
}
