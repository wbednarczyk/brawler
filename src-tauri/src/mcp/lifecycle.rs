//! App-open-only lifecycle for the read-only MCP server (ADR 0078 decision 4, M3).
//!
//! The transport ([`super::server::McpServerHandle`]) knows how to bind and
//! serve; it does not know *when* to run. This controller owns that policy:
//!
//! - Start only when Settings say `mcp.enabled` **and** a bearer token exists
//!   in the keychain (or the dev env fallback) — a missing token or a disabled
//!   toggle is a **clean refusal**, not a crash.
//! - A bind failure (port already in use) is **captured into status**, never a
//!   panic — the app keeps running with the server reported as down + an error.
//! - `enable`/`disable` take effect **live** (no app restart): the Settings
//!   command flips the running server through [`Self::stop`] /
//!   [`Self::ensure_started`].
//!
//! Lifetime is app-open-only: it is spawned in the `lib.rs` setup closure
//! beside the scheduler and torn down with the process. The handle lives behind
//! a `Mutex` so the Tauri command layer (single shared `State`) can reach it.
//!
//! **Port-change semantics** (ADR 0078 decision 4, "editable in Settings"):
//! changing `mcp.port` via `update_settings` while the server is running does
//! **not** hot-rebind — the change takes effect on the next start (disable then
//! enable, or the next app open). `mcp_status` always reports the port the
//! listener is *actually* bound to, so the UI never claims a port the server is
//! not serving. This is the simplest honest behavior and is documented in
//! `docs/contracts.md` § MCP.

use std::sync::Mutex;

use serde::Serialize;

use crate::app_state::AppState;
use crate::mcp::server::McpServerHandle;
use crate::providers::credentials::{self, CredentialDescriptor};

/// Snapshot of the MCP server's live state for the Settings status pill and the
/// `mcp_status` command (ADR 0078 decision 4). `error` carries the last
/// start-refusal reason (bind failure, missing token) — never a crash.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    /// Is the loopback listener currently serving?
    pub running: bool,
    /// The port the listener is bound to when running, else the configured
    /// port that the next start will attempt.
    pub port: u16,
    /// Last start-refusal reason (bind-in-use, missing token). `None` when
    /// running or when cleanly disabled.
    pub error: Option<String>,
    /// Whether the running listener authenticates the `kpi_acquisition`
    /// credential (ADR 0099 dec. 2): false when not running, when the token
    /// is absent/unreadable, or when its digest collides with the primary.
    pub kpi_acquisition_configured: bool,
}

/// Mutable interior guarded by the controller's `Mutex`.
struct Inner {
    /// `Some` exactly when the listener thread is alive.
    handle: Option<McpServerHandle>,
    /// The actual bound port while running.
    running_port: u16,
    /// The last-read Settings port — the port the next start will attempt.
    configured_port: u16,
    /// Last start-refusal reason surfaced in [`McpStatus::error`].
    error: Option<String>,
}

/// App-open-only lifecycle controller for the MCP server. Managed as a Tauri
/// `State` so `set_mcp_enabled` / `mcp_status` reach the single live instance.
pub struct McpLifecycle {
    inner: Mutex<Inner>,
}

impl McpLifecycle {
    /// A fresh, stopped controller. `configured_port` seeds to the settings
    /// default until the first start reads the live Settings value.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                handle: None,
                running_port: 0,
                configured_port: crate::storage::MCP_PORT_DEFAULT as u16,
                error: None,
            }),
        }
    }

    /// Start the server iff Settings enable it and a token is resolvable, using
    /// the production keychain descriptors. Idempotent, never panics — see
    /// [`Self::ensure_started_with`].
    pub fn ensure_started(&self, state: &AppState) -> McpStatus {
        self.ensure_started_with(
            state,
            &credentials::mcp_auth_token_descriptor(),
            &credentials::mcp_kpi_acquisition_token_descriptor(),
        )
    }

    /// Core start logic, parameterized by credential descriptors so tests and
    /// the fidelity corpus drive the identical flow against scratch slots.
    ///
    /// Refusals are clean, not crashes:
    /// - disabled → stop (if running), no error;
    /// - enabled + no primary token → not running, `error` set;
    /// - enabled + token but bind fails → not running, `error` = the bind error;
    /// - already running → no-op (idempotent), current status;
    /// - stalled-processor cap reached → not running, `error` set (ADR 0099).
    ///
    /// The acquisition token is OPTIONAL (ADR 0099 dec. 2 truth table):
    /// absent or unreadable means the scope is unavailable — the server still
    /// starts and `kpi_acquisition_configured` reports it.
    pub(crate) fn ensure_started_with(
        &self,
        state: &AppState,
        descriptor: &CredentialDescriptor,
        kpi_descriptor: &CredentialDescriptor,
    ) -> McpStatus {
        let mut inner = self.inner.lock().expect("mcp lifecycle mutex not poisoned");

        // Read the live Settings toggle + port. A read failure is captured, not
        // propagated — the server simply stays down with an error.
        let settings = match state.get_settings() {
            Ok(settings) => settings.mcp,
            Err(error) => {
                inner.error = Some(format!("failed to read MCP settings: {error}"));
                return status_of(&inner);
            }
        };
        inner.configured_port = settings.port;

        if !settings.enabled {
            // Clean refusal: ensure stopped, no error.
            stop_inner(&mut inner);
            inner.error = None;
            return status_of(&inner);
        }

        // Already running → idempotent no-op.
        if inner.handle.is_some() {
            inner.error = None;
            return status_of(&inner);
        }

        // Stalled-processor latch (ADR 0099 dec. 2): at the cap, refuse to
        // spawn another acceptor/processor pair until a parked processor
        // exits. The error persists through status()/ensure_started() until
        // the count drops — then a plain retry starts normally.
        if crate::mcp::server::stalled_processor_count()
            >= crate::mcp::server::STALLED_PROCESSOR_CAP
        {
            inner.error = Some(
                "MCP server restart deferred: stalled connections from a previous instance — \
                 retry after they close (or restart the app)"
                    .to_owned(),
            );
            return status_of(&inner);
        }

        // Resolve the bearer token (keychain, then dev env fallback). Absent →
        // clean refusal with an error surfaced in status.
        let token = match credentials::read_credential(descriptor) {
            Ok(Some(token)) => token,
            Ok(None) => {
                inner.error = Some("MCP server auth token is not configured".to_owned());
                return status_of(&inner);
            }
            Err(error) => {
                inner.error = Some(format!("failed to read MCP auth token: {error}"));
                return status_of(&inner);
            }
        };

        // The acquisition token is optional: absent/unreadable ⇒ the scope is
        // simply unavailable, the server still starts (ADR 0099 dec. 2).
        let kpi_token = credentials::read_credential(kpi_descriptor).ok().flatten();

        // Bind. A failure (port in use) is captured, never a panic.
        //
        // Bounded retry on a busy port: tiny_http's internal accept thread is
        // not joined by its Drop, so immediately after OUR OWN stop() the old
        // listener FD can outlive the handle by a beat — a quick
        // disable→enable toggle on the same port (and the rebind test under
        // llvm-cov timing) hits it. Ten 50 ms attempts cover that window; a
        // port genuinely held by another process still surfaces the same
        // truthful error, just ~0.5 s later.
        let mut outcome =
            McpServerHandle::start(state.clone(), &token, kpi_token.as_deref(), settings.port);
        let mut attempts = 0;
        while attempts < 10 {
            match &outcome {
                Err(error) if error.contains("failed to bind") => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    attempts += 1;
                    outcome = McpServerHandle::start(
                        state.clone(),
                        &token,
                        kpi_token.as_deref(),
                        settings.port,
                    );
                }
                _ => break,
            }
        }
        match outcome {
            Ok(handle) => {
                inner.running_port = handle.local_addr().port();
                inner.handle = Some(handle);
                inner.error = None;
            }
            Err(error) => {
                inner.error = Some(error);
            }
        }
        status_of(&inner)
    }

    /// Stop the listener if running (unblock + join). Idempotent; clears any
    /// prior error so a later start reports fresh.
    pub fn stop(&self) -> McpStatus {
        let mut inner = self.inner.lock().expect("mcp lifecycle mutex not poisoned");
        stop_inner(&mut inner);
        inner.error = None;
        status_of(&inner)
    }

    /// Current live status without touching the server.
    pub fn status(&self) -> McpStatus {
        let inner = self.inner.lock().expect("mcp lifecycle mutex not poisoned");
        status_of(&inner)
    }
}

impl Default for McpLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

/// Stop and join the listener thread if one is running. Idempotent.
fn stop_inner(inner: &mut Inner) {
    if let Some(handle) = inner.handle.take() {
        handle.stop();
    }
}

/// Build a status snapshot from the guarded interior. The reported port is the
/// actual bound port while running, else the configured next-start port.
fn status_of(inner: &Inner) -> McpStatus {
    let running = inner.handle.is_some();
    McpStatus {
        running,
        port: if running {
            inner.running_port
        } else {
            inner.configured_port
        },
        error: inner.error.clone(),
        kpi_acquisition_configured: inner
            .handle
            .as_ref()
            .is_some_and(McpServerHandle::kpi_acquisition_active),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::credentials;
    use crate::storage::{open_in_memory_database, AppState, SettingsUpdate};
    use std::net::TcpListener;

    /// The dev env fallback the MCP descriptor reads when the keychain has no
    /// entry — the portable way to make a token *readable* in a test regardless
    /// of the keychain backend (the Linux CI mock backend is EntryOnly and does
    /// not read a stored secret back). nextest runs each test in its own
    /// process, so this env mutation is isolated.
    const TOKEN_ENV: &str = "BRAWLER_MCP_TOKEN";

    /// A concrete free loopback port probed per test. Tests must NEVER pass
    /// port `0` expecting an ephemeral bind: the settings clamp maps `0` to the
    /// 1024 floor, so every such test would race for the SAME port under
    /// nextest's parallelism (this exact collision reddened the W7 commit gate
    /// intermittently — guardrail 2026-07-12).
    fn free_port() -> u16 {
        let probe = TcpListener::bind("127.0.0.1:0").expect("probe a free port");
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        port
    }

    fn state_with(enabled: bool, port: u16) -> AppState {
        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        state
            .update_settings(SettingsUpdate {
                mcp_enabled: Some(enabled),
                mcp_port: Some(port as i64),
                ..Default::default()
            })
            .expect("persist mcp settings");
        state
    }

    fn descriptor() -> CredentialDescriptor {
        credentials::test_mcp_auth_token_descriptor()
    }

    fn kpi_descriptor() -> CredentialDescriptor {
        credentials::test_mcp_kpi_rotation_descriptor()
    }

    fn with_token() {
        credentials::scrub_provider_env_fallbacks();
        std::env::set_var(TOKEN_ENV, "deadbeefdeadbeef");
    }

    #[test]
    fn refuses_when_disabled() {
        with_token(); // token present, but the toggle is off
        let state = state_with(false, 0);
        let lifecycle = McpLifecycle::new();

        let status = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());

        assert!(!status.running, "disabled must not run");
        assert!(
            status.error.is_none(),
            "disabled is a clean refusal, not an error: {status:?}"
        );
    }

    #[test]
    fn refuses_to_start_without_token() {
        // Hermeticity (docs/testing.md): scrub the dev env fallback so an
        // ambient BRAWLER_MCP_TOKEN cannot flip this missing-token assertion.
        credentials::scrub_provider_env_fallbacks();
        let state = state_with(true, 0);
        let lifecycle = McpLifecycle::new();

        let status = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());

        assert!(!status.running, "no token means no server");
        assert!(
            status.error.is_some(),
            "missing token surfaces as a status error, never a crash"
        );
    }

    #[test]
    fn start_stop_is_idempotent() {
        with_token();
        let state = state_with(true, free_port());
        let lifecycle = McpLifecycle::new();

        let first = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());
        assert!(first.running, "first start binds: {:?}", first.error);
        let port = first.port;

        // Second ensure is a no-op: same running port, no rebind, no error.
        let second = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());
        assert!(second.running);
        assert_eq!(second.port, port, "idempotent start keeps the same port");
        assert!(second.error.is_none());

        // Stop is idempotent too.
        let stopped = lifecycle.stop();
        assert!(!stopped.running);
        let stopped_again = lifecycle.stop();
        assert!(!stopped_again.running);
    }

    #[test]
    fn status_reflects_running_port() {
        with_token();
        let state = state_with(true, free_port());
        let lifecycle = McpLifecycle::new();

        let started = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());
        assert!(started.running, "start binds: {:?}", started.error);

        let status = lifecycle.status();
        assert!(status.running);
        assert_eq!(
            status.port, started.port,
            "status mirrors the actual bound port"
        );

        lifecycle.stop();
    }

    #[test]
    fn stop_releases_port_for_rebind() {
        with_token();
        // A concrete free port so the rebind targets the SAME port.
        let port = free_port();
        let state = state_with(true, port);
        let lifecycle = McpLifecycle::new();

        let started = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());
        assert!(started.running, "first bind: {:?}", started.error);
        assert_eq!(started.port, port);

        lifecycle.stop();

        // The same port must be immediately rebindable after a graceful stop.
        let restarted = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());
        assert!(
            restarted.running,
            "rebind after stop: {:?}",
            restarted.error
        );
        assert_eq!(restarted.port, port);

        lifecycle.stop();
    }

    #[test]
    fn port_in_use_reports_error_not_crash() {
        with_token();
        // Occupy a port so the MCP bind must fail.
        let blocker = TcpListener::bind("127.0.0.1:0").expect("bind a blocker socket");
        let port = blocker.local_addr().unwrap().port();
        let state = state_with(true, port);
        let lifecycle = McpLifecycle::new();

        let status = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());

        assert!(!status.running, "cannot run on an occupied port");
        assert!(
            status.error.is_some(),
            "bind failure is captured in status, never a panic"
        );
        drop(blocker);
    }

    #[test]
    fn starts_without_the_acquisition_token_and_reports_the_scope_off() {
        // ADR 0099 dec. 2 truth table: the acquisition token is OPTIONAL —
        // absent means the scope is unavailable, never a start failure.
        with_token();
        let _ = credentials::clear_credential(&kpi_descriptor());
        let state = state_with(true, free_port());
        let lifecycle = McpLifecycle::new();

        let status = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());

        assert!(
            status.running,
            "primary-only start runs: {:?}",
            status.error
        );
        assert!(!status.kpi_acquisition_configured);
        lifecycle.stop();
    }

    #[test]
    fn starts_with_both_tokens_and_reports_the_scope_configured() {
        with_token();
        credentials::store_credential(&kpi_descriptor(), "kpi-secret-1")
            .expect("store acquisition token in the rotation slot");
        let state = state_with(true, free_port());
        let lifecycle = McpLifecycle::new();

        let status = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());

        assert!(status.running, "dual-token start runs: {:?}", status.error);
        assert!(status.kpi_acquisition_configured);
        lifecycle.stop();
        let _ = credentials::clear_credential(&kpi_descriptor());
    }

    #[test]
    fn colliding_tokens_report_the_scope_unavailable() {
        // Fail-closed misconfiguration guard (ADR 0099 dec. 2): equal secrets
        // must not grant the acquisition credential the Full surface.
        with_token();
        credentials::store_credential(&kpi_descriptor(), "deadbeefdeadbeef")
            .expect("store the SAME secret as the primary env token");
        let state = state_with(true, free_port());
        let lifecycle = McpLifecycle::new();

        let status = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());

        assert!(status.running, "the server itself runs: {:?}", status.error);
        assert!(
            !status.kpi_acquisition_configured,
            "digest collision disables the scope"
        );
        lifecycle.stop();
        let _ = credentials::clear_credential(&kpi_descriptor());
    }

    #[test]
    fn the_stalled_processor_latch_refuses_starts_until_the_count_drops() {
        // ADR 0099 dec. 2: at the cap the lifecycle refuses to spawn another
        // acceptor/processor pair; the error persists until a parked
        // processor exits — then a plain retry starts normally.
        use std::sync::mpsc;
        with_token();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = std::sync::Arc::new(std::sync::Mutex::new(release_rx));
        for _ in 0..crate::mcp::server::STALLED_PROCESSOR_CAP {
            let rx = std::sync::Arc::clone(&release_rx);
            crate::mcp::server::park_stalled_for_tests(std::thread::spawn(move || {
                let _ = rx.lock().expect("release rx").recv();
            }));
        }
        let state = state_with(true, free_port());
        let lifecycle = McpLifecycle::new();

        let refused = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());
        assert!(!refused.running, "at the cap the start is refused");
        assert!(
            refused
                .error
                .as_deref()
                .is_some_and(|error| error.contains("stalled connections")),
            "the latch names its cause: {refused:?}"
        );
        // The error persists through a plain status read.
        assert!(lifecycle.status().error.is_some());

        // Release the parked threads — the count drops, a retry starts.
        drop(release_tx);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while crate::mcp::server::stalled_processor_count() > 0 {
            assert!(std::time::Instant::now() < deadline, "parked threads exit");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let recovered = lifecycle.ensure_started_with(&state, &descriptor(), &kpi_descriptor());
        assert!(
            recovered.running,
            "below the cap the start succeeds: {:?}",
            recovered.error
        );
        lifecycle.stop();
    }
}
