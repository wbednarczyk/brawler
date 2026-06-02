use serde::Serialize;
use thiserror::Error;

const APP_SERVICE: &str = "brawler";
const GEMINI_TRANSCRIPTION_TARGET: &str = "brawler/gemini/youtube_transcription/api_key";
const GEMINI_TRANSCRIPTION_ACCOUNT: &str = "provider_gemini:youtube_transcription:api_key";
const GEMINI_TRANSCRIPTION_ENV_VAR: &str = "GEMINI_API_KEY";

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("credential value is required")]
    EmptySecret,
    #[error("credential backend did not persist the saved value")]
    PersistenceVerificationFailed,
    #[error("credential backend unavailable: {0}")]
    Backend(String),
}

#[derive(Clone, Debug)]
pub struct CredentialDescriptor {
    pub provider_id: &'static str,
    pub purpose: &'static str,
    pub secret_kind: &'static str,
    pub label: &'static str,
    target: &'static str,
    account: &'static str,
    development_env_var: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    pub provider_id: &'static str,
    pub purpose: &'static str,
    pub secret_kind: &'static str,
    pub configured: bool,
    pub storage: &'static str,
    pub label: &'static str,
    pub dev_fallback_available: bool,
    pub error: Option<String>,
}

pub fn gemini_transcription_descriptor() -> CredentialDescriptor {
    CredentialDescriptor {
        provider_id: "provider_gemini",
        purpose: "youtube_transcription",
        secret_kind: "api_key",
        label: "Gemini YouTube transcription API key",
        target: GEMINI_TRANSCRIPTION_TARGET,
        account: GEMINI_TRANSCRIPTION_ACCOUNT,
        development_env_var: Some(GEMINI_TRANSCRIPTION_ENV_VAR),
    }
}

pub fn get_gemini_transcription_credential_status() -> CredentialStatus {
    credential_status(&gemini_transcription_descriptor())
}

pub fn set_gemini_transcription_api_key(
    api_key: &str,
) -> Result<CredentialStatus, CredentialError> {
    set_credential_secret(&gemini_transcription_descriptor(), api_key)
}

pub fn clear_gemini_transcription_api_key() -> Result<CredentialStatus, CredentialError> {
    clear_credential_secret(&gemini_transcription_descriptor())
}

pub fn read_gemini_transcription_api_key() -> Result<Option<String>, CredentialError> {
    read_credential_secret(&gemini_transcription_descriptor())
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
        purpose: descriptor.purpose,
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
        let error = set_credential_secret(&gemini_transcription_descriptor(), "   ")
            .expect_err("empty secret should be rejected");

        assert!(matches!(error, CredentialError::EmptySecret));
    }

    #[test]
    fn descriptor_exposes_non_secret_metadata() {
        let descriptor = gemini_transcription_descriptor();
        let status = status(&descriptor, true, "os_keychain", false, None);

        assert_eq!(status.provider_id, "provider_gemini");
        assert_eq!(status.purpose, "youtube_transcription");
        assert_eq!(status.secret_kind, "api_key");
        assert_eq!(status.storage, "os_keychain");
        assert!(status.configured);
        assert!(status.error.is_none());
    }

    #[test]
    fn verified_save_status_rejects_missing_read_back_secret() {
        let error = verified_save_status(&gemini_transcription_descriptor(), None)
            .expect_err("missing read-back secret should fail verification");

        assert!(matches!(
            error,
            CredentialError::PersistenceVerificationFailed
        ));
    }

    #[test]
    fn verified_save_status_reports_os_keychain_when_read_back_succeeds() {
        let status = verified_save_status(
            &gemini_transcription_descriptor(),
            Some("test-secret".to_owned()),
        )
        .expect("read-back secret should verify persistence");

        assert!(status.configured);
        assert_eq!(status.storage, "os_keychain");
        assert!(!status.dev_fallback_available);
    }

    #[test]
    #[ignore = "live keyring smoke test; writes to the real OS credential store and restores the previous Gemini key"]
    fn live_keyring_persists_gemini_transcription_secret() -> Result<(), String> {
        let descriptor = gemini_transcription_descriptor();
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
