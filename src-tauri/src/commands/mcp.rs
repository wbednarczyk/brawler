//! MCP server management commands (ADR 0078 decision 4, v0.52 M1).
//!
//! The read-only MCP server (ADR 0078) authenticates with a bearer token held
//! in the OS keychain under the generalized credential boundary
//! (`brawler/mcp/auth_token`). These commands manage that token's lifecycle:
//!
//! - `regenerate_mcp_token` — generates 32 random bytes (hex-encoded), stores
//!   them, and returns the plaintext **exactly once**. The token value exists
//!   only in the keychain and this one-time reveal — it is never logged (ADR
//!   0078 guardrail G-3) and never appears in any status payload.
//! - `revoke_mcp_token` — removes the token from the keychain.
//! - `mcp_token_status` — configured? which storage? dev fallback available?
//!
//! Server lifecycle (`set_mcp_enabled`, `mcp_status`) is M3's scope — not here.

use serde::Serialize;

use crate::app_state::AppState;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::mcp::lifecycle::{McpLifecycle, McpStatus};
use crate::providers::credentials::{self, CredentialDescriptor, CredentialStatus};
use crate::storage::SettingsUpdate;

/// Token size: 32 random bytes, hex-encoded to 64 chars (ADR 0078 decision 4).
const MCP_TOKEN_BYTES: usize = 32;

/// The one-time reveal returned by [`regenerate_mcp_token`]. `token` is the
/// plaintext bearer token; it is surfaced here **exactly once** — it is never
/// logged and never part of any status payload (ADR 0078 guardrail G-3).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct McpTokenGenerated {
    /// The plaintext token — shown to the user once, then only in the keychain.
    pub token: String,
    /// Post-store credential status (what the Settings pill renders).
    pub status: CredentialStatus,
}

/// Generate a fresh bearer token: 32 OS-entropy bytes, lowercase hex.
fn generate_auth_token() -> Result<String, CommandError> {
    let mut bytes = [0u8; MCP_TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|error| {
        CommandError::new(
            CommandErrorCode::Internal,
            format!("system RNG unavailable: {error}"),
        )
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Command core, parameterized by descriptor so tests and the fidelity corpus
/// exercise the identical flow against a scratch keychain slot. The plaintext
/// leaves this function only inside the returned [`McpTokenGenerated`]; no
/// tracing/log call may carry it (ADR 0078 G-3 — plan tripwire).
pub(crate) fn regenerate_mcp_token_impl(
    descriptor: &CredentialDescriptor,
) -> Result<McpTokenGenerated, CommandError> {
    let token = generate_auth_token()?;
    let status = credentials::store_credential(descriptor, &token)?;
    log::info!(
        "module=mcp stage=token_regenerated configured={} storage={}",
        status.configured,
        status.storage
    );
    Ok(McpTokenGenerated { token, status })
}

pub(crate) fn revoke_mcp_token_impl(
    descriptor: &CredentialDescriptor,
) -> Result<CredentialStatus, CommandError> {
    let status = credentials::clear_credential(descriptor)?;
    log::info!(
        "module=mcp stage=token_revoked configured={} storage={}",
        status.configured,
        status.storage
    );
    Ok(status)
}

pub(crate) fn mcp_token_status_impl(
    descriptor: &CredentialDescriptor,
) -> Result<CredentialStatus, CommandError> {
    Ok(credentials::credential_status_for(descriptor))
}

/// Command core for `set_mcp_enabled`, parameterized by descriptor so tests and
/// the fidelity corpus drive the identical flow against a scratch keychain slot.
/// Persists the `mcp.enabled` setting **and** flips the live server (no app
/// restart): enable → `ensure_started`, disable → `stop`. A bind failure or a
/// missing token surfaces in the returned [`McpStatus`], never as a crash.
pub(crate) fn set_mcp_enabled_impl(
    enabled: bool,
    state: &AppState,
    lifecycle: &McpLifecycle,
    descriptor: &CredentialDescriptor,
    kpi_descriptor: &CredentialDescriptor,
) -> Result<McpStatus, CommandError> {
    state.update_settings(SettingsUpdate {
        mcp_enabled: Some(enabled),
        ..Default::default()
    })?;
    let status = if enabled {
        lifecycle.ensure_started_with(state, descriptor, kpi_descriptor)
    } else {
        lifecycle.stop()
    };
    log::info!(
        "module=mcp stage=set_enabled enabled={} running={}",
        enabled,
        status.running
    );
    Ok(status)
}

/// Rotation composition (ADR 0099 dec. 2): generate + store, enforce the
/// read-back rule, then restart the listener from keychain truth.
///
/// The read-back rule: regenerate SUCCEEDS only when the post-store read-back
/// equals the generated plaintext — the server must never run a token other
/// than the one storage reports. The two failure causes get typed errors:
/// an environment override (read-back returns the OLD env token; dev-only —
/// rotate the env var instead) and a write-only keychain (EntryOnly dev/CI
/// Linux: the store "succeeds" but nothing can read it back).
pub(crate) fn regenerate_token_with_restart_impl(
    state: &AppState,
    lifecycle: &McpLifecycle,
    rotated: &CredentialDescriptor,
    primary: &CredentialDescriptor,
    kpi: &CredentialDescriptor,
) -> Result<McpTokenGenerated, CommandError> {
    let generated = regenerate_mcp_token_impl(rotated)?;
    let read_back = credentials::read_credential(rotated)
        .map_err(|error| CommandError::new(CommandErrorCode::Internal, error.to_string()))?;
    if read_back.as_deref() != Some(generated.token.as_str()) {
        let message = if generated.status.storage == "development_environment" {
            "this credential is provided by the environment (dev fallback) — rotate the \
             environment variable instead"
        } else {
            "the OS keychain is write-only on this platform: the stored token cannot be read \
             back, so the server would run a token storage does not report"
        };
        return Err(CommandError::new(CommandErrorCode::Conflict, message));
    }
    restart_listener(state, lifecycle, primary, kpi);
    Ok(generated)
}

/// Revocation composition (ADR 0099 dec. 2): clear, then restart from
/// keychain truth (primary revoked ⇒ the server stops with a status error;
/// acquisition revoked ⇒ the server restarts with the scope unavailable).
/// Note: revoke cannot remove an environment-provided credential (dev-only) —
/// the returned status keeps reporting `development_environment`.
pub(crate) fn revoke_token_with_restart_impl(
    state: &AppState,
    lifecycle: &McpLifecycle,
    rotated: &CredentialDescriptor,
    primary: &CredentialDescriptor,
    kpi: &CredentialDescriptor,
) -> Result<CredentialStatus, CommandError> {
    let status = revoke_mcp_token_impl(rotated)?;
    restart_listener(state, lifecycle, primary, kpi);
    Ok(status)
}

/// Stop + `ensure_started` (keychain truth): a disabled server stays cleanly
/// down; a running one picks up the rotated digests. The restart outcome is
/// NOT part of the frozen token-command response shapes — the UI fetches a
/// fresh `mcp_status` after every rotate/revoke action instead.
fn restart_listener(
    state: &AppState,
    lifecycle: &McpLifecycle,
    primary: &CredentialDescriptor,
    kpi: &CredentialDescriptor,
) {
    lifecycle.stop();
    let status = lifecycle.ensure_started_with(state, primary, kpi);
    log::info!(
        "module=mcp stage=token_rotation_restart running={} kpi_scope={}",
        status.running,
        status.kpi_acquisition_configured
    );
}

/// Persist `mcp.enabled` and start/stop the listener live; returns the fresh
/// [`McpStatus`]. Bind failure / missing token surface in the status, not a panic.
#[tauri::command]
pub fn set_mcp_enabled(
    enabled: bool,
    state: tauri::State<'_, AppState>,
    lifecycle: tauri::State<'_, McpLifecycle>,
) -> Result<McpStatus, CommandError> {
    set_mcp_enabled_impl(
        enabled,
        &state,
        &lifecycle,
        &credentials::mcp_auth_token_descriptor(),
        &credentials::mcp_kpi_acquisition_token_descriptor(),
    )
}

/// Report the MCP server's live state (`running`, bound `port`, last `error`).
#[tauri::command]
pub fn mcp_status(lifecycle: tauri::State<'_, McpLifecycle>) -> Result<McpStatus, CommandError> {
    Ok(lifecycle.status())
}

/// Generate + store the MCP bearer token, returning the plaintext exactly
/// once, then restart the listener so the new digest is live (ADR 0099).
#[tauri::command]
pub fn regenerate_mcp_token(
    state: tauri::State<'_, AppState>,
    lifecycle: tauri::State<'_, McpLifecycle>,
) -> Result<McpTokenGenerated, CommandError> {
    let primary = credentials::mcp_auth_token_descriptor();
    let kpi = credentials::mcp_kpi_acquisition_token_descriptor();
    regenerate_token_with_restart_impl(&state, &lifecycle, &primary, &primary, &kpi)
}

/// Remove the MCP bearer token from the OS keychain and restart the listener
/// (which then stops with a status error — no token).
#[tauri::command]
pub fn revoke_mcp_token(
    state: tauri::State<'_, AppState>,
    lifecycle: tauri::State<'_, McpLifecycle>,
) -> Result<CredentialStatus, CommandError> {
    let primary = credentials::mcp_auth_token_descriptor();
    let kpi = credentials::mcp_kpi_acquisition_token_descriptor();
    revoke_token_with_restart_impl(&state, &lifecycle, &primary, &primary, &kpi)
}

/// Report whether an MCP token is configured, where it lives, and whether the
/// dev env fallback (`BRAWLER_MCP_TOKEN`) is available. Never carries the token.
#[tauri::command]
pub fn mcp_token_status() -> Result<CredentialStatus, CommandError> {
    mcp_token_status_impl(&credentials::mcp_auth_token_descriptor())
}

/// Generate + store the `kpi_acquisition` bearer token (ADR 0099 dec. 2),
/// returning the plaintext exactly once, then restart the listener.
#[tauri::command]
pub fn regenerate_kpi_acquisition_token(
    state: tauri::State<'_, AppState>,
    lifecycle: tauri::State<'_, McpLifecycle>,
) -> Result<McpTokenGenerated, CommandError> {
    let primary = credentials::mcp_auth_token_descriptor();
    let kpi = credentials::mcp_kpi_acquisition_token_descriptor();
    regenerate_token_with_restart_impl(&state, &lifecycle, &kpi, &primary, &kpi)
}

/// Remove the `kpi_acquisition` token and restart the listener (the server
/// keeps running; the acquisition scope becomes unavailable).
#[tauri::command]
pub fn revoke_kpi_acquisition_token(
    state: tauri::State<'_, AppState>,
    lifecycle: tauri::State<'_, McpLifecycle>,
) -> Result<CredentialStatus, CommandError> {
    let primary = credentials::mcp_auth_token_descriptor();
    let kpi = credentials::mcp_kpi_acquisition_token_descriptor();
    revoke_token_with_restart_impl(&state, &lifecycle, &kpi, &primary, &kpi)
}

/// Report the `kpi_acquisition` credential status. Never carries the token.
#[tauri::command]
pub fn kpi_acquisition_token_status() -> Result<CredentialStatus, CommandError> {
    mcp_token_status_impl(&credentials::mcp_kpi_acquisition_token_descriptor())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::credentials;

    #[test]
    fn generated_token_is_64_lowercase_hex_chars() {
        let token = generate_auth_token().expect("system RNG should be available");

        assert_eq!(token.len(), 64, "32 bytes hex-encode to 64 chars");
        assert!(
            token
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "token must be lowercase hex: {token}"
        );
    }

    #[test]
    fn generated_tokens_are_unique_across_calls() {
        let first = generate_auth_token().expect("token generates");
        let second = generate_auth_token().expect("token generates");

        assert_ne!(first, second, "two generations must never collide");
    }

    #[test]
    fn regenerate_returns_the_plaintext_exactly_once() {
        credentials::scrub_provider_env_fallbacks();

        let generated = regenerate_mcp_token_impl(&credentials::test_mcp_auth_token_descriptor())
            .expect("regenerate should succeed on any keychain backend");

        assert_eq!(generated.token.len(), 64);
        assert!(generated
            .token
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
        // The one-time reveal is the ONLY carrier of the plaintext: the status
        // payload accompanying it must not embed the token anywhere.
        let status_json = serde_json::to_string(&generated.status).expect("status serializes");
        assert!(
            !status_json.contains(&generated.token),
            "status must never carry the plaintext token"
        );
    }

    #[test]
    fn token_status_reports_not_configured_when_absent() {
        credentials::scrub_provider_env_fallbacks();

        let status = mcp_token_status_impl(&credentials::test_mcp_auth_token_descriptor())
            .expect("status is always readable");

        assert_eq!(status.provider_id, "mcp");
        assert_eq!(status.secret_kind, "auth_token");
        assert!(!status.configured);
        assert_eq!(status.storage, "not_configured");
        assert!(!status.dev_fallback_available);
    }

    #[test]
    fn token_status_reports_dev_fallback_from_env() {
        credentials::scrub_provider_env_fallbacks();
        std::env::set_var("BRAWLER_MCP_TOKEN", "dev-token");

        let status = mcp_token_status_impl(&credentials::test_mcp_auth_token_descriptor())
            .expect("status is always readable");

        std::env::remove_var("BRAWLER_MCP_TOKEN");
        assert!(status.configured);
        assert_eq!(status.storage, "development_environment");
        assert!(status.dev_fallback_available);
    }

    #[test]
    fn set_enabled_starts_and_stops_live() {
        use crate::mcp::lifecycle::McpLifecycle;
        use crate::storage::{open_in_memory_database, AppState};

        // A readable token via the dev env fallback (portable across keychain
        // backends); nextest isolates this env mutation per process.
        credentials::scrub_provider_env_fallbacks();
        std::env::set_var("BRAWLER_MCP_TOKEN", "deadbeefdeadbeef");

        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        // A probed free port, never the 8317 default: a fixed port collides
        // with a running dev app or a parallel gate run (guardrail 2026-07-12,
        // same class as the lifecycle tests' free_port()).
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe a free port");
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        state
            .update_settings(crate::storage::SettingsUpdate {
                mcp_port: Some(port as i64),
                ..Default::default()
            })
            .expect("persist mcp port");
        let lifecycle = McpLifecycle::new();
        let descriptor = credentials::test_mcp_auth_token_descriptor();

        let kpi_descriptor = credentials::test_mcp_kpi_rotation_descriptor();
        let on = set_mcp_enabled_impl(true, &state, &lifecycle, &descriptor, &kpi_descriptor)
            .expect("enable");
        assert!(on.running, "enable starts the live server: {:?}", on.error);
        assert!(
            state.get_settings().expect("settings").mcp.enabled,
            "enable persists the setting"
        );

        let off = set_mcp_enabled_impl(false, &state, &lifecycle, &descriptor, &kpi_descriptor)
            .expect("disable");
        assert!(!off.running, "disable stops the live server");
        assert!(
            !state.get_settings().expect("settings").mcp.enabled,
            "disable persists the setting"
        );
    }

    #[test]
    fn revoke_clears_the_token_and_reports_not_configured() {
        credentials::scrub_provider_env_fallbacks();
        // Store first so revoke exercises the delete path where the backend
        // persists; on the EntryOnly mock backend this is a no-op store.
        let _ = regenerate_mcp_token_impl(&credentials::test_mcp_auth_token_descriptor());

        let status = revoke_mcp_token_impl(&credentials::test_mcp_auth_token_descriptor())
            .expect("revoking is idempotent");

        assert!(!status.configured);
        assert_eq!(status.storage, "not_configured");
    }

    /// Live-rotation harness: an enabled server on a probed free port, driven
    /// entirely through the rotation-test descriptors (their in-memory backend
    /// gives the coherent read-back the production read-back rule requires —
    /// the EntryOnly dev/CI keyring cannot).
    fn rotation_harness() -> (
        crate::storage::AppState,
        crate::mcp::lifecycle::McpLifecycle,
        CredentialDescriptor,
        CredentialDescriptor,
        u16,
    ) {
        use crate::storage::{open_in_memory_database, AppState};
        credentials::scrub_provider_env_fallbacks();
        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe a free port");
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        state
            .update_settings(crate::storage::SettingsUpdate {
                mcp_enabled: Some(true),
                mcp_port: Some(port as i64),
                ..Default::default()
            })
            .expect("persist mcp settings");
        let primary = credentials::test_mcp_rotation_descriptor();
        let kpi = credentials::test_mcp_kpi_rotation_descriptor();
        let _ = credentials::clear_credential(&primary);
        let _ = credentials::clear_credential(&kpi);
        (
            state,
            crate::mcp::lifecycle::McpLifecycle::new(),
            primary,
            kpi,
            port,
        )
    }

    /// Minimal authenticated tools-list probe; returns the HTTP status code.
    fn probe_status(port: u16, token: &str) -> u16 {
        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len(),
        );
        stream.write_all(request.as_bytes()).expect("write");
        let mut raw = Vec::new();
        let _ = stream.read_to_end(&mut raw);
        String::from_utf8_lossy(&raw)
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .expect("status code")
    }

    #[test]
    fn rotating_the_primary_token_swaps_the_live_digest() {
        // ADR 0099 dec. 2: rotation restarts the listener from keychain
        // truth — the old bearer stops authenticating, the new one works.
        let (state, lifecycle, primary, kpi, port) = rotation_harness();

        let first =
            regenerate_token_with_restart_impl(&state, &lifecycle, &primary, &primary, &kpi)
                .expect("first rotation");
        assert!(lifecycle.status().running, "rotation started the server");
        assert_eq!(probe_status(port, &first.token), 200);

        let second =
            regenerate_token_with_restart_impl(&state, &lifecycle, &primary, &primary, &kpi)
                .expect("second rotation");
        assert_eq!(probe_status(port, &first.token), 401, "old token is dead");
        assert_eq!(probe_status(port, &second.token), 200, "new token lives");
        lifecycle.stop();
    }

    #[test]
    fn rotating_the_acquisition_token_swaps_the_scope_digest() {
        let (state, lifecycle, primary, kpi, port) = rotation_harness();
        state
            .update_settings(crate::storage::SettingsUpdate {
                kpi_acquisition_enabled: Some(true),
                ..Default::default()
            })
            .expect("enable the acquisition gate");
        let _ = regenerate_token_with_restart_impl(&state, &lifecycle, &primary, &primary, &kpi)
            .expect("primary token");

        let first = regenerate_token_with_restart_impl(&state, &lifecycle, &kpi, &primary, &kpi)
            .expect("first acquisition rotation");
        assert!(lifecycle.status().kpi_acquisition_configured);
        assert_eq!(probe_status(port, &first.token), 200);

        let second = regenerate_token_with_restart_impl(&state, &lifecycle, &kpi, &primary, &kpi)
            .expect("second acquisition rotation");
        assert_eq!(probe_status(port, &first.token), 401, "old token is dead");
        assert_eq!(probe_status(port, &second.token), 200, "new token lives");
        lifecycle.stop();
    }

    #[test]
    fn rotation_while_disabled_succeeds_without_starting_the_server() {
        let (state, lifecycle, primary, kpi, _port) = rotation_harness();
        state
            .update_settings(crate::storage::SettingsUpdate {
                mcp_enabled: Some(false),
                ..Default::default()
            })
            .expect("disable mcp");

        let generated =
            regenerate_token_with_restart_impl(&state, &lifecycle, &primary, &primary, &kpi)
                .expect("rotation succeeds while disabled");
        assert!(generated.status.configured);
        let status = lifecycle.status();
        assert!(!status.running, "disabled stays down: {:?}", status.error);
    }

    #[test]
    fn revoking_the_primary_stops_the_server_and_status_says_so() {
        let (state, lifecycle, primary, kpi, port) = rotation_harness();
        let generated =
            regenerate_token_with_restart_impl(&state, &lifecycle, &primary, &primary, &kpi)
                .expect("rotation");
        assert_eq!(probe_status(port, &generated.token), 200);

        let status = revoke_token_with_restart_impl(&state, &lifecycle, &primary, &primary, &kpi)
            .expect("revoke");
        assert!(!status.configured);
        let live = lifecycle.status();
        assert!(!live.running, "no token — the restart refuses");
        assert!(live.error.is_some(), "and the status names the refusal");
    }

    #[test]
    fn a_bind_failure_after_rotation_surfaces_in_status_not_success() {
        let (state, lifecycle, primary, kpi, port) = rotation_harness();
        // Occupy the configured port so the post-rotation restart must fail.
        let blocker = std::net::TcpListener::bind(("127.0.0.1", port)).expect("occupy the port");

        let generated =
            regenerate_token_with_restart_impl(&state, &lifecycle, &primary, &primary, &kpi)
                .expect("the token command itself succeeds");
        assert!(generated.status.configured);
        let live = lifecycle.status();
        assert!(!live.running, "bind failure — not running");
        assert!(live.error.is_some(), "the bind error is in the status");
        drop(blocker);
    }

    #[test]
    fn regenerate_refuses_when_the_environment_overrides_the_credential() {
        // Read-back rule (ADR 0099 dec. 2): with a dev env fallback active,
        // the post-store read-back returns the OLD env token — the command
        // must refuse instead of running a token storage does not report.
        // (Deterministic on the EntryOnly dev/CI keyring, like the rest of
        // this suite: the scratch slot's store persists nothing.)
        let (state, lifecycle, _primary, kpi, _port) = rotation_harness();
        std::env::set_var("BRAWLER_MCP_TOKEN", "env-owned-token");
        let scratch = credentials::test_mcp_auth_token_descriptor();

        let error =
            regenerate_token_with_restart_impl(&state, &lifecycle, &scratch, &scratch, &kpi)
                .expect_err("env-provided credentials cannot be rotated");

        std::env::remove_var("BRAWLER_MCP_TOKEN");
        assert_eq!(error.code, CommandErrorCode::Conflict);
        assert!(error.message.contains("environment"), "{}", error.message);
    }

    #[test]
    fn regenerate_refuses_on_a_write_only_keychain() {
        // EntryOnly (dev/CI Linux): the store "succeeds" but nothing can read
        // it back — the server must never run a phantom token (ADR 0099).
        let (state, lifecycle, _primary, kpi, _port) = rotation_harness();
        let scratch = credentials::test_mcp_auth_token_descriptor();
        let _ = credentials::clear_credential(&scratch);

        let error =
            regenerate_token_with_restart_impl(&state, &lifecycle, &scratch, &scratch, &kpi)
                .expect_err("a write-only keychain cannot satisfy the read-back rule");

        assert_eq!(error.code, CommandErrorCode::Conflict);
        assert!(error.message.contains("write-only"), "{}", error.message);
    }
}
