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
) -> Result<McpStatus, CommandError> {
    state.update_settings(SettingsUpdate {
        mcp_enabled: Some(enabled),
        ..Default::default()
    })?;
    let status = if enabled {
        lifecycle.ensure_started_with(state, descriptor)
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
    )
}

/// Report the MCP server's live state (`running`, bound `port`, last `error`).
#[tauri::command]
pub fn mcp_status(lifecycle: tauri::State<'_, McpLifecycle>) -> Result<McpStatus, CommandError> {
    Ok(lifecycle.status())
}

/// Generate + store the MCP bearer token, returning the plaintext exactly once.
#[tauri::command]
pub fn regenerate_mcp_token() -> Result<McpTokenGenerated, CommandError> {
    regenerate_mcp_token_impl(&credentials::mcp_auth_token_descriptor())
}

/// Remove the MCP bearer token from the OS keychain.
#[tauri::command]
pub fn revoke_mcp_token() -> Result<CredentialStatus, CommandError> {
    revoke_mcp_token_impl(&credentials::mcp_auth_token_descriptor())
}

/// Report whether an MCP token is configured, where it lives, and whether the
/// dev env fallback (`BRAWLER_MCP_TOKEN`) is available. Never carries the token.
#[tauri::command]
pub fn mcp_token_status() -> Result<CredentialStatus, CommandError> {
    mcp_token_status_impl(&credentials::mcp_auth_token_descriptor())
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

        let on = set_mcp_enabled_impl(true, &state, &lifecycle, &descriptor).expect("enable");
        assert!(on.running, "enable starts the live server: {:?}", on.error);
        assert!(
            state.get_settings().expect("settings").mcp.enabled,
            "enable persists the setting"
        );

        let off = set_mcp_enabled_impl(false, &state, &lifecycle, &descriptor).expect("disable");
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
}
