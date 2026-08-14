use serde::Serialize;
use thiserror::Error;

const APP_SERVICE: &str = "brawler";

// One API-key credential per provider. Purpose (analysis vs transcription) is a
// usage decided by settings, not part of the credential's identity (ADR 0028).
const GEMINI_TARGET: &str = "brawler/provider_gemini/api_key";
const GEMINI_ACCOUNT: &str = "provider_gemini:api_key";
const GEMINI_ENV_VAR: &str = "GEMINI_API_KEY";

// The Claude / OpenAI / OpenAI-compatible / Mistral credentials are removed
// (ADR 0084 decision 7): Gemini, which powers **transcription only**, is the
// last provider key the app asks for. Existing OS-keychain entries for the
// removed providers are deliberately NOT deleted (outside app scope, harmless).

// The MCP server bearer token (ADR 0078 decision 4) generalizes the credential
// boundary beyond AI providers: same keychain service, same descriptor flow,
// a non-provider identity. The dev env fallback keeps parity with API keys
// (and feeds the stdio adapter, decision 6).
const MCP_AUTH_TOKEN_TARGET: &str = "brawler/mcp/auth_token";
const MCP_AUTH_TOKEN_ACCOUNT: &str = "mcp:auth_token";
const MCP_AUTH_TOKEN_ENV_VAR: &str = "BRAWLER_MCP_TOKEN";

// The acquisition-scoped MCP credential (ADR 0099 decision 2): a second bearer
// whose scope sees only the KPI-ingest workflow tools.
const MCP_KPI_ACQUISITION_TOKEN_TARGET: &str = "brawler/mcp/kpi_acquisition_token";
const MCP_KPI_ACQUISITION_TOKEN_ACCOUNT: &str = "mcp:kpi_acquisition_token";
const MCP_KPI_ACQUISITION_TOKEN_ENV_VAR: &str = "BRAWLER_MCP_KPI_ACQUISITION_TOKEN";

/// Test support: remove every provider dev-fallback env var so hermetic tests
/// asserting missing-credential behavior cannot inherit the developer's shell
/// (guardrail 2026-07-11: an exported `GEMINI_API_KEY` turned three `jobs::`
/// tests red on one machine and green on another). Call this at the top of any
/// test whose assertions depend on a credential being absent. Safe under
/// nextest (process per test); under plain `cargo test` it narrows, not widens,
/// the environment.
#[cfg(test)]
pub(crate) fn scrub_provider_env_fallbacks() {
    for var in [
        GEMINI_ENV_VAR,
        // Not a provider key, but the same dev-fallback class: an exported
        // MCP token would flip missing-token assertions (ADR 0078 M1).
        MCP_AUTH_TOKEN_ENV_VAR,
        MCP_KPI_ACQUISITION_TOKEN_ENV_VAR,
    ] {
        std::env::remove_var(var);
    }
}

// Legacy purpose-scoped entries (ADR 0028): best-effort cleared once so no
// orphaned secrets linger; never read or fallen back to.
const LEGACY_GEMINI_PURPOSE_TARGET: &str = "brawler/gemini/youtube_transcription/api_key";
const LEGACY_GEMINI_PURPOSE_ACCOUNT: &str = "provider_gemini:youtube_transcription:api_key";

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("credential value is required")]
    EmptySecret,
    #[error("unknown AI provider: {0}")]
    UnknownProvider(String),
    #[error("credential backend did not persist the saved value")]
    PersistenceVerificationFailed,
    #[error("credential backend unavailable: {0}")]
    Backend(String),
}

#[derive(Clone, Debug)]
pub struct CredentialDescriptor {
    pub provider_id: &'static str,
    pub secret_kind: &'static str,
    pub label: &'static str,
    target: &'static str,
    account: &'static str,
    development_env_var: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    pub provider_id: &'static str,
    pub secret_kind: &'static str,
    pub configured: bool,
    pub storage: &'static str,
    pub label: &'static str,
    pub dev_fallback_available: bool,
    pub error: Option<String>,
}

/// The provider ids that authenticate with a single OS-keychain API key.
pub fn credentialed_provider_ids() -> &'static [&'static str] {
    &["provider_gemini"]
}

/// Every provider id `provider_credential_descriptor` resolves — the canonical
/// enumeration behind the "every credential is enterable in Settings" guardrail
/// (owner report 2026-07-14: the twelvedata-eod key had no Settings field, so
/// it was physically un-enterable). Adding a descriptor arm below REQUIRES
/// adding the id here: a test pins this list to the shared fixture
/// `src/test/scenarios/credentialProviders.json`, and the frontend test checks
/// every fixture id has a Settings form entry.
pub const CREDENTIAL_PROVIDER_IDS: &[&str] = &["provider_gemini"];

/// Resolve the credential descriptor for a provider id, if it uses a credential.
pub fn provider_credential_descriptor(provider_id: &str) -> Option<CredentialDescriptor> {
    match provider_id {
        "provider_gemini" => Some(CredentialDescriptor {
            provider_id: "provider_gemini",
            secret_kind: "api_key",
            // Transcription only, since ADR 0084 — the analysis adapters are gone.
            label: "Gemini API key",
            target: GEMINI_TARGET,
            account: GEMINI_ACCOUNT,
            development_env_var: Some(GEMINI_ENV_VAR),
        }),
        _ => None,
    }
}

/// Read the credential status for a provider, or `None` if the id is keyless.
pub fn provider_credential_status(provider_id: &str) -> Option<CredentialStatus> {
    provider_credential_descriptor(provider_id).map(|descriptor| credential_status(&descriptor))
}

/// The MCP server bearer-token descriptor (ADR 0078 decision 4) — the first
/// non-AI-provider credential; the same store/read/clear/status flow applies.
pub fn mcp_auth_token_descriptor() -> CredentialDescriptor {
    CredentialDescriptor {
        provider_id: "mcp",
        secret_kind: "auth_token",
        label: "MCP server auth token",
        target: MCP_AUTH_TOKEN_TARGET,
        account: MCP_AUTH_TOKEN_ACCOUNT,
        development_env_var: Some(MCP_AUTH_TOKEN_ENV_VAR),
    }
}

/// The acquisition-scoped MCP bearer-token descriptor (ADR 0099 decision 2).
/// Distinct `secret_kind` so the frontend can tell the two token statuses
/// apart; same store/read/clear/status flow as the primary.
pub fn mcp_kpi_acquisition_token_descriptor() -> CredentialDescriptor {
    CredentialDescriptor {
        provider_id: "mcp",
        secret_kind: "kpi_acquisition_token",
        label: "MCP acquisition token",
        target: MCP_KPI_ACQUISITION_TOKEN_TARGET,
        account: MCP_KPI_ACQUISITION_TOKEN_ACCOUNT,
        development_env_var: Some(MCP_KPI_ACQUISITION_TOKEN_ENV_VAR),
    }
}

/// Test support: an MCP descriptor pointing at a scratch keychain slot, so a
/// test/corpus run on a machine with a persistent keychain backend (native
/// Windows) never reads or clobbers the owner's real MCP token. Same identity
/// (`provider_id`/`secret_kind`) and env fallback, different target/account.
#[cfg(test)]
pub(crate) fn test_mcp_auth_token_descriptor() -> CredentialDescriptor {
    CredentialDescriptor {
        provider_id: "mcp",
        secret_kind: "auth_token",
        label: "MCP server auth token",
        target: "brawler/test/mcp/auth_token",
        account: "test:mcp:auth_token",
        development_env_var: Some(MCP_AUTH_TOKEN_ENV_VAR),
    }
}

/// Test support: rotation-test descriptors backed by the in-memory test
/// backend (target prefix routes them there). The real keyring on dev/CI
/// Linux is EntryOnly (write-only), so live-rotation tests — which must read
/// back the token they just generated — need a persistent seam. The scratch
/// descriptor above deliberately STAYS on the real keyring: it is the
/// EntryOnly-truthfulness coverage.
#[cfg(test)]
pub(crate) fn test_mcp_rotation_descriptor() -> CredentialDescriptor {
    CredentialDescriptor {
        provider_id: "mcp",
        secret_kind: "auth_token",
        label: "MCP server auth token",
        target: "brawler/test-memory/mcp/auth_token",
        account: "test-memory:mcp:auth_token",
        development_env_var: Some(MCP_AUTH_TOKEN_ENV_VAR),
    }
}

#[cfg(test)]
pub(crate) fn test_mcp_kpi_rotation_descriptor() -> CredentialDescriptor {
    CredentialDescriptor {
        provider_id: "mcp",
        secret_kind: "kpi_acquisition_token",
        label: "MCP acquisition token",
        target: "brawler/test-memory/mcp/kpi_acquisition_token",
        account: "test-memory:mcp:kpi_acquisition_token",
        development_env_var: Some(MCP_KPI_ACQUISITION_TOKEN_ENV_VAR),
    }
}

/// Store a secret under an arbitrary credential descriptor (the generalized,
/// descriptor-first API — the provider-id functions above stay as the
/// AI-provider convenience layer).
pub fn store_credential(
    descriptor: &CredentialDescriptor,
    secret: &str,
) -> Result<CredentialStatus, CredentialError> {
    set_credential_secret(descriptor, secret)
}

/// Clear the stored secret for an arbitrary credential descriptor.
pub fn clear_credential(
    descriptor: &CredentialDescriptor,
) -> Result<CredentialStatus, CredentialError> {
    clear_credential_secret(descriptor)
}

/// Read the status for an arbitrary credential descriptor.
pub fn credential_status_for(descriptor: &CredentialDescriptor) -> CredentialStatus {
    credential_status(descriptor)
}

/// Read the stored secret for an arbitrary credential descriptor (OS keychain,
/// then the dev env fallback). Returns `Ok(None)` when nothing is configured.
/// The descriptor-first read path for non-AI-provider credentials (the MCP
/// bearer token, ADR 0078 decision 4); mirrors [`read_provider_api_key`].
pub fn read_credential(
    descriptor: &CredentialDescriptor,
) -> Result<Option<String>, CredentialError> {
    read_credential_secret(descriptor)
}

/// Store the API key for a provider in the OS keychain.
pub fn set_provider_api_key(
    provider_id: &str,
    api_key: &str,
) -> Result<CredentialStatus, CredentialError> {
    let descriptor = provider_credential_descriptor(provider_id)
        .ok_or_else(|| CredentialError::UnknownProvider(provider_id.to_owned()))?;
    set_credential_secret(&descriptor, api_key)
}

/// Clear the stored API key for a provider.
pub fn clear_provider_api_key(provider_id: &str) -> Result<CredentialStatus, CredentialError> {
    let descriptor = provider_credential_descriptor(provider_id)
        .ok_or_else(|| CredentialError::UnknownProvider(provider_id.to_owned()))?;
    clear_credential_secret(&descriptor)
}

/// Read the configured API key for a provider (keychain, then dev env fallback).
/// Returns `Ok(None)` for keyless providers and for configured providers with no key.
pub fn read_provider_api_key(provider_id: &str) -> Result<Option<String>, CredentialError> {
    match provider_credential_descriptor(provider_id) {
        Some(descriptor) => read_credential_secret(&descriptor),
        None => Ok(None),
    }
}

/// Best-effort, one-time removal of legacy purpose-scoped keychain entries
/// (ADR 0028). Errors are ignored.
pub fn clear_legacy_credentials() {
    if let Ok(entry) = keyring::Entry::new_with_target(
        LEGACY_GEMINI_PURPOSE_TARGET,
        APP_SERVICE,
        LEGACY_GEMINI_PURPOSE_ACCOUNT,
    ) {
        let _ = entry.delete_credential();
    }
}

/// Whether a descriptor routes to the in-memory test backend (rotation-test
/// slots under `brawler/test-memory/` — a persistent read-back seam the
/// EntryOnly dev/CI keyring cannot provide). Production descriptors never
/// carry the prefix; every call site is `#[cfg(test)]`-gated.
#[cfg(test)]
fn uses_test_memory_backend(descriptor: &CredentialDescriptor) -> bool {
    descriptor.target.starts_with("brawler/test-memory/")
}

#[cfg(test)]
mod test_memory_backend {
    use std::collections::HashMap;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static STORE: OnceLock<Mutex<HashMap<&'static str, String>>> = OnceLock::new();

    pub(super) fn map() -> MutexGuard<'static, HashMap<&'static str, String>> {
        STORE
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .expect("test memory credential store poisoned")
    }
}

#[cfg(test)]
fn test_memory_secret(descriptor: &CredentialDescriptor) -> Option<String> {
    test_memory_backend::map().get(descriptor.target).cloned()
}

fn credential_status(descriptor: &CredentialDescriptor) -> CredentialStatus {
    #[cfg(test)]
    if uses_test_memory_backend(descriptor) {
        return match test_memory_secret(descriptor) {
            Some(_) => status(descriptor, true, "os_keychain", false, None),
            None => {
                let dev = development_env_secret(descriptor).is_some();
                status(
                    descriptor,
                    dev,
                    if dev {
                        "development_environment"
                    } else {
                        "not_configured"
                    },
                    dev,
                    None,
                )
            }
        };
    }
    match read_os_keychain_secret(descriptor) {
        Ok(Some(_)) => status(descriptor, true, "os_keychain", false, None),
        Ok(None) => {
            let dev_fallback_available = development_env_secret(descriptor).is_some();
            status(
                descriptor,
                dev_fallback_available,
                if dev_fallback_available {
                    "development_environment"
                } else {
                    "not_configured"
                },
                dev_fallback_available,
                None,
            )
        }
        Err(error) => {
            let dev_fallback_available = development_env_secret(descriptor).is_some();
            if dev_fallback_available {
                status(
                    descriptor,
                    true,
                    "development_environment",
                    true,
                    Some(error.to_string()),
                )
            } else {
                status(
                    descriptor,
                    false,
                    "os_keychain_unavailable",
                    false,
                    Some(error.to_string()),
                )
            }
        }
    }
}

/// Whether the active keyring backend persists a secret beyond the entry that
/// stored it. On dev/CI Linux (no secret-service feature) keyring falls back
/// to its mock backend with `EntryOnly` persistence: a fresh `Entry` can never
/// read a stored secret back, so read-back verification would always fail.
fn keychain_persists_across_entries() -> bool {
    !matches!(
        keyring::default::default_credential_builder().persistence(),
        keyring::credential::CredentialPersistence::EntryOnly
    )
}

fn set_credential_secret(
    descriptor: &CredentialDescriptor,
    secret: &str,
) -> Result<CredentialStatus, CredentialError> {
    let secret = secret.trim();
    if secret.is_empty() {
        return Err(CredentialError::EmptySecret);
    }

    #[cfg(test)]
    if uses_test_memory_backend(descriptor) {
        test_memory_backend::map().insert(descriptor.target, secret.to_owned());
        return Ok(credential_status(descriptor));
    }

    os_keychain_entry(descriptor)
        .and_then(|entry| {
            entry
                .set_password(secret)
                .map_err(|error| CredentialError::Backend(error.to_string()))
        })
        .and_then(|()| {
            if keychain_persists_across_entries() {
                verified_save_status(descriptor, read_os_keychain_secret(descriptor)?)
            } else {
                // EntryOnly backend (dev/CI): nothing persisted, and read-back
                // verification cannot succeed by construction. Report the
                // truthful status (not_configured / development_environment)
                // instead of a spurious PersistenceVerificationFailed.
                Ok(credential_status(descriptor))
            }
        })
}

fn verified_save_status(
    descriptor: &CredentialDescriptor,
    read_back_secret: Option<String>,
) -> Result<CredentialStatus, CredentialError> {
    if read_back_secret.is_some() {
        Ok(status(descriptor, true, "os_keychain", false, None))
    } else {
        Err(CredentialError::PersistenceVerificationFailed)
    }
}

fn clear_credential_secret(
    descriptor: &CredentialDescriptor,
) -> Result<CredentialStatus, CredentialError> {
    #[cfg(test)]
    if uses_test_memory_backend(descriptor) {
        test_memory_backend::map().remove(descriptor.target);
        return Ok(credential_status(descriptor));
    }
    let entry = os_keychain_entry(descriptor)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(credential_status(descriptor)),
        Err(error) => Err(CredentialError::Backend(error.to_string())),
    }
}

fn read_credential_secret(
    descriptor: &CredentialDescriptor,
) -> Result<Option<String>, CredentialError> {
    #[cfg(test)]
    if uses_test_memory_backend(descriptor) {
        return Ok(test_memory_secret(descriptor).or_else(|| development_env_secret(descriptor)));
    }
    match read_os_keychain_secret(descriptor) {
        Ok(Some(secret)) => Ok(Some(secret)),
        Ok(None) => Ok(development_env_secret(descriptor)),
        Err(error) => {
            if let Some(secret) = development_env_secret(descriptor) {
                Ok(Some(secret))
            } else {
                Err(error)
            }
        }
    }
}

fn read_os_keychain_secret(
    descriptor: &CredentialDescriptor,
) -> Result<Option<String>, CredentialError> {
    let entry = os_keychain_entry(descriptor)?;
    match entry.get_password() {
        Ok(secret) if secret.trim().is_empty() => Ok(None),
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(CredentialError::Backend(error.to_string())),
    }
}

fn os_keychain_entry(descriptor: &CredentialDescriptor) -> Result<keyring::Entry, CredentialError> {
    keyring::Entry::new_with_target(descriptor.target, APP_SERVICE, descriptor.account)
        .map_err(|error| CredentialError::Backend(error.to_string()))
}

fn development_env_secret(descriptor: &CredentialDescriptor) -> Option<String> {
    descriptor
        .development_env_var
        .and_then(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn status(
    descriptor: &CredentialDescriptor,
    configured: bool,
    storage: &'static str,
    dev_fallback_available: bool,
    error: Option<String>,
) -> CredentialStatus {
    CredentialStatus {
        provider_id: descriptor.provider_id,
        secret_kind: descriptor.secret_kind,
        configured,
        storage,
        label: descriptor.label,
        dev_fallback_available,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Guardrail (owner report 2026-07-14, twelvedata-eod was un-enterable):
    // every credential-bearing provider id is enumerated, resolvable, and
    // pinned byte-for-byte in the shared fixture the frontend test reads to
    // assert each one has a Settings form. Adding a descriptor without
    // updating the fixture (and then the Settings list) reddens here or there.
    #[test]
    fn every_credential_provider_id_resolves_and_matches_the_shared_fixture() {
        for provider_id in CREDENTIAL_PROVIDER_IDS {
            assert!(
                provider_credential_descriptor(provider_id).is_some(),
                "{provider_id} is enumerated but resolves no descriptor"
            );
        }
        let fixture: Vec<String> = serde_json::from_str(include_str!(concat!(
            env!("BRAWLER_SCENARIOS_DIR"),
            "/credentialProviders.json"
        )))
        .expect("credentialProviders.json parses");
        assert_eq!(
            fixture, CREDENTIAL_PROVIDER_IDS,
            "src/test/scenarios/credentialProviders.json drifted from \
             CREDENTIAL_PROVIDER_IDS — update BOTH deliberately (and give any \
             new provider a Settings credential form)"
        );
    }

    #[test]
    fn empty_secret_is_rejected_before_keychain_access() {
        let error = set_provider_api_key("provider_gemini", "   ")
            .expect_err("empty secret should be rejected");

        assert!(matches!(error, CredentialError::EmptySecret));
    }

    #[test]
    fn unknown_provider_is_rejected() {
        let error = set_provider_api_key("provider_unknown", "key")
            .expect_err("unknown provider should be rejected");

        assert!(matches!(error, CredentialError::UnknownProvider(_)));
        assert!(provider_credential_status("provider_unknown").is_none());
        assert_eq!(
            read_provider_api_key("test_sample").expect("keyless read is ok"),
            None
        );
    }

    #[test]
    fn descriptors_exist_for_all_credentialed_providers() {
        for provider_id in credentialed_provider_ids() {
            let descriptor = provider_credential_descriptor(provider_id)
                .unwrap_or_else(|| panic!("descriptor missing for {provider_id}"));
            assert_eq!(descriptor.secret_kind, "api_key");
            assert_eq!(&descriptor.provider_id, provider_id);
        }
    }

    #[test]
    fn descriptor_exposes_non_secret_metadata() {
        let descriptor = provider_credential_descriptor("provider_gemini").expect("gemini exists");
        let status = status(&descriptor, true, "os_keychain", false, None);

        assert_eq!(status.provider_id, "provider_gemini");
        assert_eq!(status.secret_kind, "api_key");
        assert_eq!(status.storage, "os_keychain");
        assert!(status.configured);
        assert!(status.error.is_none());
    }

    #[test]
    fn verified_save_status_rejects_missing_read_back_secret() {
        let descriptor = provider_credential_descriptor("provider_gemini").expect("gemini exists");
        let error = verified_save_status(&descriptor, None)
            .expect_err("missing read-back secret should fail verification");

        assert!(matches!(
            error,
            CredentialError::PersistenceVerificationFailed
        ));
    }

    #[test]
    fn verified_save_status_reports_os_keychain_when_read_back_succeeds() {
        let descriptor = provider_credential_descriptor("provider_gemini").expect("exists");
        let status = verified_save_status(&descriptor, Some("test-secret".to_owned()))
            .expect("read-back secret should verify persistence");

        assert!(status.configured);
        assert_eq!(status.storage, "os_keychain");
        assert!(!status.dev_fallback_available);
    }

    #[test]
    fn mcp_auth_token_descriptor_targets_the_mcp_keychain_slot() {
        // ADR 0078 decision 4: the MCP bearer token generalizes the credential
        // boundary beyond the AI-provider list — pinned constants are the wire
        // contract between the commands, the server auth (M2), and the stdio
        // adapter's env fallback (M5).
        let descriptor = mcp_auth_token_descriptor();

        assert_eq!(descriptor.provider_id, "mcp");
        assert_eq!(descriptor.secret_kind, "auth_token");
        assert_eq!(descriptor.target, "brawler/mcp/auth_token");
        assert_eq!(descriptor.account, "mcp:auth_token");
        assert_eq!(descriptor.development_env_var, Some("BRAWLER_MCP_TOKEN"));
    }

    #[test]
    fn test_mcp_descriptor_uses_a_scratch_slot_but_the_same_identity() {
        // The corpus/test descriptor must never collide with the real keychain
        // slot (a native-Windows nextest run would otherwise clobber the
        // owner's real MCP token) while keeping the same status identity.
        let descriptor = test_mcp_auth_token_descriptor();

        assert_eq!(descriptor.provider_id, "mcp");
        assert_eq!(descriptor.secret_kind, "auth_token");
        assert_ne!(descriptor.target, mcp_auth_token_descriptor().target);
        assert_ne!(descriptor.account, mcp_auth_token_descriptor().account);
    }

    #[test]
    fn kpi_acquisition_descriptor_targets_its_own_keychain_slot() {
        // ADR 0099 decision 2: the acquisition credential is a distinct
        // keychain identity — a distinct secret_kind so the frontend can tell
        // the two token statuses apart, and its own env fallback for stdio.
        let descriptor = mcp_kpi_acquisition_token_descriptor();

        assert_eq!(descriptor.provider_id, "mcp");
        assert_eq!(descriptor.secret_kind, "kpi_acquisition_token");
        assert_eq!(descriptor.target, "brawler/mcp/kpi_acquisition_token");
        assert_eq!(descriptor.account, "mcp:kpi_acquisition_token");
        assert_eq!(
            descriptor.development_env_var,
            Some("BRAWLER_MCP_KPI_ACQUISITION_TOKEN")
        );
        assert_ne!(descriptor.target, mcp_auth_token_descriptor().target);
    }

    #[test]
    fn rotation_descriptors_round_trip_through_the_memory_backend() {
        // The rotation slots must behave like a PERSISTENT keychain (store →
        // read-back → clear), which the EntryOnly dev/CI keyring cannot do —
        // that is their whole reason to exist (ADR 0099 dec. 2 test seam).
        scrub_provider_env_fallbacks();
        let descriptor = test_mcp_rotation_descriptor();

        let stored = store_credential(&descriptor, "rotation-secret").expect("store succeeds");
        assert!(stored.configured);
        assert_eq!(stored.storage, "os_keychain");
        assert_eq!(
            read_credential(&descriptor).expect("read succeeds"),
            Some("rotation-secret".to_owned())
        );

        let cleared = clear_credential(&descriptor).expect("clear succeeds");
        assert!(!cleared.configured);
        assert_eq!(read_credential(&descriptor).expect("read succeeds"), None);
    }

    #[test]
    fn scrub_provider_env_fallbacks_clears_the_mcp_token_var() {
        // Scrub-hermeticity rule (docs/testing.md): a test asserting
        // missing-token behavior must not inherit BRAWLER_MCP_TOKEN from the
        // developer's shell, so the shared scrub helper covers it too.
        std::env::set_var("BRAWLER_MCP_TOKEN", "shell-leaked-token");

        scrub_provider_env_fallbacks();

        assert!(std::env::var("BRAWLER_MCP_TOKEN").is_err());
    }

    #[test]
    fn mcp_token_status_reports_dev_fallback_when_env_var_set() {
        scrub_provider_env_fallbacks();
        std::env::set_var("BRAWLER_MCP_TOKEN", "deadbeef");

        let status = credential_status(&test_mcp_auth_token_descriptor());

        std::env::remove_var("BRAWLER_MCP_TOKEN");
        assert!(status.configured);
        assert_eq!(status.storage, "development_environment");
        assert!(status.dev_fallback_available);
        assert_eq!(status.provider_id, "mcp");
        assert_eq!(status.secret_kind, "auth_token");
    }

    #[test]
    fn clearing_the_mcp_token_reports_not_configured() {
        scrub_provider_env_fallbacks();

        let status = clear_credential(&test_mcp_auth_token_descriptor())
            .expect("clearing an absent token is idempotent");

        assert!(!status.configured);
        assert_eq!(status.storage, "not_configured");
        assert!(!status.dev_fallback_available);
    }

    #[test]
    fn storing_on_an_entry_only_backend_reports_truthful_status() {
        // On the dev/CI mock keyring backend (EntryOnly persistence) a fresh
        // entry cannot read a stored secret back, so the verified read-back
        // must be skipped and the store must report the truthful status
        // instead of failing with PersistenceVerificationFailed.
        if keychain_persists_across_entries() {
            return; // real OS keychain — covered by the #[ignore] live test
        }
        scrub_provider_env_fallbacks();

        let status = store_credential(&test_mcp_auth_token_descriptor(), "scratch-token")
            .expect("storing on the mock backend must not fail verification");

        assert!(
            !status.configured,
            "EntryOnly backend persists nothing — status must say so"
        );
        assert_eq!(status.storage, "not_configured");
    }

    #[test]
    #[ignore = "live keyring smoke test; writes to the real OS credential store and restores the previous Gemini key"]
    fn live_keyring_persists_provider_secret() -> Result<(), String> {
        let descriptor = provider_credential_descriptor("provider_gemini").expect("gemini exists");
        let original = read_os_keychain_secret(&descriptor).map_err(|error| error.to_string())?;
        let smoke_secret = std::env::var("BRAWLER_KEYRING_SMOKE_SECRET")
            .unwrap_or_else(|_| "brawler-keyring-smoke-secret".to_owned());

        let smoke_result = (|| {
            set_credential_secret(&descriptor, &smoke_secret)?;
            let read_back = read_os_keychain_secret(&descriptor)?;
            if read_back.as_deref() != Some(smoke_secret.as_str()) {
                return Err(CredentialError::PersistenceVerificationFailed);
            }

            clear_credential_secret(&descriptor)?;
            let after_clear = read_os_keychain_secret(&descriptor)?;
            if after_clear.is_some() {
                return Err(CredentialError::PersistenceVerificationFailed);
            }

            Ok(())
        })();

        let restore_result = match original {
            Some(secret) => set_credential_secret(&descriptor, &secret).map(|_| ()),
            None => clear_credential_secret(&descriptor).map(|_| ()),
        };

        smoke_result
            .and(restore_result)
            .map_err(|error| error.to_string())
    }
}
