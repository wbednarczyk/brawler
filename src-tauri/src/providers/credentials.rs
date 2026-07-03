use serde::Serialize;
use thiserror::Error;

const APP_SERVICE: &str = "brawler";

// One API-key credential per provider. Purpose (analysis vs transcription) is a
// usage decided by settings, not part of the credential's identity (ADR 0028).
const GEMINI_TARGET: &str = "brawler/provider_gemini/api_key";
const GEMINI_ACCOUNT: &str = "provider_gemini:api_key";
const GEMINI_ENV_VAR: &str = "GEMINI_API_KEY";

const ANTHROPIC_TARGET: &str = "brawler/provider_anthropic/api_key";
const ANTHROPIC_ACCOUNT: &str = "provider_anthropic:api_key";
const ANTHROPIC_ENV_VAR: &str = "ANTHROPIC_API_KEY";

const OPENAI_TARGET: &str = "brawler/provider_openai/api_key";
const OPENAI_ACCOUNT: &str = "provider_openai:api_key";
const OPENAI_ENV_VAR: &str = "OPENAI_API_KEY";

const OPENAI_COMPATIBLE_TARGET: &str = "brawler/provider_openai_compatible/api_key";
const OPENAI_COMPATIBLE_ACCOUNT: &str = "provider_openai_compatible:api_key";
const OPENAI_COMPATIBLE_ENV_VAR: &str = "OPENAI_COMPATIBLE_API_KEY";

// Legacy purpose-scoped entries removed in ADR 0028. They are best-effort
// cleared once so no orphaned secrets linger; we never read or fall back to them.
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
    &[
        "provider_gemini",
        "provider_anthropic",
        "provider_openai",
        "provider_openai_compatible",
    ]
}

/// Resolve the credential descriptor for a provider id, if it uses a credential.
pub fn provider_credential_descriptor(provider_id: &str) -> Option<CredentialDescriptor> {
    match provider_id {
        "provider_gemini" => Some(CredentialDescriptor {
            provider_id: "provider_gemini",
            secret_kind: "api_key",
            label: "Gemini API key",
            target: GEMINI_TARGET,
            account: GEMINI_ACCOUNT,
            development_env_var: Some(GEMINI_ENV_VAR),
        }),
        "provider_anthropic" => Some(CredentialDescriptor {
            provider_id: "provider_anthropic",
            secret_kind: "api_key",
            label: "Anthropic (Claude) API key",
            target: ANTHROPIC_TARGET,
            account: ANTHROPIC_ACCOUNT,
            development_env_var: Some(ANTHROPIC_ENV_VAR),
        }),
        "provider_openai" => Some(CredentialDescriptor {
            provider_id: "provider_openai",
            secret_kind: "api_key",
            label: "OpenAI API key",
            target: OPENAI_TARGET,
            account: OPENAI_ACCOUNT,
            development_env_var: Some(OPENAI_ENV_VAR),
        }),
        "provider_openai_compatible" => Some(CredentialDescriptor {
            provider_id: "provider_openai_compatible",
            secret_kind: "api_key",
            label: "OpenAI-compatible API key",
            target: OPENAI_COMPATIBLE_TARGET,
            account: OPENAI_COMPATIBLE_ACCOUNT,
            development_env_var: Some(OPENAI_COMPATIBLE_ENV_VAR),
        }),
        _ => None,
    }
}

/// Read the credential status for a provider, or `None` if the id is keyless.
pub fn provider_credential_status(provider_id: &str) -> Option<CredentialStatus> {
    provider_credential_descriptor(provider_id).map(|descriptor| credential_status(&descriptor))
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
/// replaced by the one-key-per-provider model. Errors are ignored.
pub fn clear_legacy_credentials() {
    if let Ok(entry) = keyring::Entry::new_with_target(
        LEGACY_GEMINI_PURPOSE_TARGET,
        APP_SERVICE,
        LEGACY_GEMINI_PURPOSE_ACCOUNT,
    ) {
        let _ = entry.delete_credential();
    }
}

fn credential_status(descriptor: &CredentialDescriptor) -> CredentialStatus {
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

fn set_credential_secret(
    descriptor: &CredentialDescriptor,
    secret: &str,
) -> Result<CredentialStatus, CredentialError> {
    let secret = secret.trim();
    if secret.is_empty() {
        return Err(CredentialError::EmptySecret);
    }

    os_keychain_entry(descriptor)
        .and_then(|entry| {
            entry
                .set_password(secret)
                .map_err(|error| CredentialError::Backend(error.to_string()))
        })
        .and_then(|()| verified_save_status(descriptor, read_os_keychain_secret(descriptor)?))
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
    let entry = os_keychain_entry(descriptor)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(credential_status(descriptor)),
        Err(error) => Err(CredentialError::Backend(error.to_string())),
    }
}

fn read_credential_secret(
    descriptor: &CredentialDescriptor,
) -> Result<Option<String>, CredentialError> {
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
        let descriptor = provider_credential_descriptor("provider_anthropic").expect("exists");
        let status = verified_save_status(&descriptor, Some("test-secret".to_owned()))
            .expect("read-back secret should verify persistence");

        assert!(status.configured);
        assert_eq!(status.storage, "os_keychain");
        assert!(!status.dev_fallback_available);
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
