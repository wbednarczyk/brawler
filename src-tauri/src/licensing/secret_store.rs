use thiserror::Error;

const APP_SERVICE: &str = "brawler";
const LICENSE_TOKEN_TARGET: &str = "brawler/license/friend_test/token";
const LICENSE_TOKEN_ACCOUNT: &str = "licensing:friend_test:token";

#[derive(Debug, Error)]
pub enum LicenseSecretStoreError {
    #[error("license key is required")]
    EmptyToken,
    #[error("license keychain backend unavailable: {0}")]
    Backend(String),
}

pub trait LicenseTokenStore {
    fn read_token(&self) -> Result<Option<String>, LicenseSecretStoreError>;
    fn save_token(&self, token: &str) -> Result<(), LicenseSecretStoreError>;
    fn clear_token(&self) -> Result<(), LicenseSecretStoreError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OsKeychainLicenseTokenStore;

impl LicenseTokenStore for OsKeychainLicenseTokenStore {
    fn read_token(&self) -> Result<Option<String>, LicenseSecretStoreError> {
        match entry()?.get_password() {
            Ok(token) if token.trim().is_empty() => Ok(None),
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(LicenseSecretStoreError::Backend(error.to_string())),
        }
    }

    fn save_token(&self, token: &str) -> Result<(), LicenseSecretStoreError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(LicenseSecretStoreError::EmptyToken);
        }

        entry()?
            .set_password(token)
            .map_err(|error| LicenseSecretStoreError::Backend(error.to_string()))
    }

    fn clear_token(&self) -> Result<(), LicenseSecretStoreError> {
        match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(LicenseSecretStoreError::Backend(error.to_string())),
        }
    }
}

fn entry() -> Result<keyring::Entry, LicenseSecretStoreError> {
    keyring::Entry::new_with_target(LICENSE_TOKEN_TARGET, APP_SERVICE, LICENSE_TOKEN_ACCOUNT)
        .map_err(|error| LicenseSecretStoreError::Backend(error.to_string()))
}

#[cfg(test)]
#[derive(Debug, Default)]
pub struct MemoryLicenseTokenStore {
    token: std::sync::Mutex<Option<String>>,
}

#[cfg(test)]
impl LicenseTokenStore for MemoryLicenseTokenStore {
    fn read_token(&self) -> Result<Option<String>, LicenseSecretStoreError> {
        Ok(self.token.lock().expect("test mutex poisoned").clone())
    }

    fn save_token(&self, token: &str) -> Result<(), LicenseSecretStoreError> {
        *self.token.lock().expect("test mutex poisoned") = Some(token.to_owned());
        Ok(())
    }

    fn clear_token(&self) -> Result<(), LicenseSecretStoreError> {
        *self.token.lock().expect("test mutex poisoned") = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_token_store_round_trips_tokens() {
        let store = MemoryLicenseTokenStore::default();

        assert_eq!(store.read_token().expect("read token"), None);

        store.save_token("token").expect("save token");
        assert_eq!(
            store.read_token().expect("read saved token"),
            Some("token".to_owned())
        );

        store.clear_token().expect("clear token");
        assert_eq!(store.read_token().expect("read cleared token"), None);
    }
}
