mod entitlement;
mod secret_store;
mod token;
mod verifier;

use serde::Serialize;
use time::OffsetDateTime;

pub use entitlement::{EntitlementContext, EntitlementPolicy, LocalEntitlementPolicy};
pub use secret_store::{LicenseSecretStoreError, LicenseTokenStore, OsKeychainLicenseTokenStore};
pub use token::{parse_license_token, LicenseClaims, TokenParseError};
pub use verifier::{
    local_license_verifier, Ed25519LicenseVerifier, LicenseVerificationError, LicenseVerifier,
    VerificationKey,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatusKind {
    Valid,
    Missing,
    Invalid,
    Expired,
    WrongVersion,
    UnsupportedVersion,
    StorageError,
}

impl LicenseStatusKind {
    pub fn as_str(self) -> &'static str {
        match self {
            LicenseStatusKind::Valid => "valid",
            LicenseStatusKind::Missing => "missing",
            LicenseStatusKind::Invalid => "invalid",
            LicenseStatusKind::Expired => "expired",
            LicenseStatusKind::WrongVersion => "wrong_version",
            LicenseStatusKind::UnsupportedVersion => "unsupported_version",
            LicenseStatusKind::StorageError => "storage_error",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    pub status: LicenseStatusKind,
    pub can_use_app: bool,
    pub reason: Option<String>,
    pub license: Option<LicenseDisplayMetadata>,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LicenseDisplayMetadata {
    pub license_id: String,
    pub holder: String,
    pub channel: String,
    pub edition: String,
    pub features: Vec<String>,
    pub issued_at: String,
    pub expires_at: String,
    pub app_version_range: String,
    pub key_id: String,
}

impl LicenseDisplayMetadata {
    pub fn from_claims(claims: LicenseClaims) -> Self {
        Self {
            license_id: claims.license_id,
            holder: claims.holder,
            channel: claims.channel,
            edition: claims.edition,
            features: claims.features,
            issued_at: claims.issued_at,
            expires_at: claims.expires_at,
            app_version_range: claims.app_version_range,
            key_id: claims.key_id,
        }
    }
}

impl LicenseStatus {
    pub fn missing() -> Self {
        Self {
            status: LicenseStatusKind::Missing,
            can_use_app: true,
            reason: Some(
                "Core features are available without a license. Add a license only for gated entitlements."
                    .to_owned(),
            ),
            license: None,
            checked_at: entitlement::format_time(OffsetDateTime::now_utc()),
        }
    }

    pub fn blocked(
        status: LicenseStatusKind,
        reason: impl Into<String>,
        checked_at: String,
    ) -> Self {
        Self {
            status,
            can_use_app: true,
            reason: Some(reason.into()),
            license: None,
            checked_at,
        }
    }

    pub fn storage_error(reason: impl Into<String>) -> Self {
        Self::blocked(
            LicenseStatusKind::StorageError,
            reason,
            entitlement::format_time(OffsetDateTime::now_utc()),
        )
    }

    pub fn with_metadata(
        status: LicenseStatusKind,
        can_use_app: bool,
        reason: Option<String>,
        metadata: LicenseDisplayMetadata,
        checked_at: String,
    ) -> Self {
        Self {
            status,
            can_use_app,
            reason,
            license: Some(metadata),
            checked_at,
        }
    }
}

pub fn evaluate_local_license_token(token: &str) -> LicenseStatus {
    let context = EntitlementContext {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        checked_at: OffsetDateTime::now_utc(),
    };

    evaluate_license_token(
        token,
        context,
        &local_license_verifier(),
        &LocalEntitlementPolicy,
    )
}

pub fn evaluate_friend_test_license_token(token: &str) -> LicenseStatus {
    evaluate_local_license_token(token)
}

pub fn evaluate_license_token(
    token: &str,
    context: EntitlementContext,
    verifier: &dyn LicenseVerifier,
    policy: &dyn EntitlementPolicy,
) -> LicenseStatus {
    let checked_at = entitlement::format_time(context.checked_at);
    let parsed = match parse_license_token(token) {
        Ok(parsed) => parsed,
        Err(TokenParseError::Missing) => return LicenseStatus::missing(),
        Err(TokenParseError::UnsupportedVersion) => {
            return LicenseStatus::blocked(
                LicenseStatusKind::UnsupportedVersion,
                "This license key was made for an unsupported license format.",
                checked_at,
            )
        }
        Err(_) => {
            return LicenseStatus::blocked(
                LicenseStatusKind::Invalid,
                "This license key format is not recognized.",
                checked_at,
            )
        }
    };

    if let Err(error) = verifier.verify(
        &parsed.claims.key_id,
        parsed.signed_message.as_bytes(),
        &parsed.signature,
    ) {
        return LicenseStatus::blocked(
            LicenseStatusKind::Invalid,
            format!("This license key could not be verified: {error}."),
            checked_at,
        );
    }

    policy.evaluate(parsed.claims, context)
}

pub fn redact_license_token(_token: &str) -> &'static str {
    "[redacted-license-token]"
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use ed25519_dalek::SigningKey;
    use time::{macros::datetime, OffsetDateTime};

    use super::entitlement::EntitlementContext;
    use super::token::build_test_token;
    use super::verifier::{Ed25519LicenseVerifier, VerificationKey};
    use super::*;

    const TEST_FRIEND_KEY_ID: &str = "owner_friend_test_2026_06";
    const TEST_AUTHOR_KEY_ID: &str = "owner_author_2026_06";

    fn test_friend_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7_u8; 32])
    }

    fn test_author_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[11_u8; 32])
    }

    fn test_verifier() -> Ed25519LicenseVerifier {
        let friend_public_key =
            STANDARD.encode(test_friend_signing_key().verifying_key().to_bytes());
        let friend_public_key = Box::leak(friend_public_key.into_boxed_str());
        let author_public_key =
            STANDARD.encode(test_author_signing_key().verifying_key().to_bytes());
        let author_public_key = Box::leak(author_public_key.into_boxed_str());
        let keys = Box::leak(Box::new([
            VerificationKey {
                key_id: TEST_FRIEND_KEY_ID,
                public_key_base64: friend_public_key,
            },
            VerificationKey {
                key_id: TEST_AUTHOR_KEY_ID,
                public_key_base64: author_public_key,
            },
        ]));

        Ed25519LicenseVerifier::new(keys)
    }

    fn claims() -> LicenseClaims {
        LicenseClaims {
            license_id: "lic_test_001".to_owned(),
            holder: "Test Friend".to_owned(),
            channel: "friend_test".to_owned(),
            edition: "friend".to_owned(),
            features: vec!["core".to_owned()],
            issued_at: "2026-01-01T00:00:00Z".to_owned(),
            expires_at: "2027-01-01T00:00:00Z".to_owned(),
            app_version_range: "*".to_owned(),
            key_id: TEST_FRIEND_KEY_ID.to_owned(),
        }
    }

    fn author_claims() -> LicenseClaims {
        LicenseClaims {
            license_id: "lic_author_001".to_owned(),
            holder: "Project Author".to_owned(),
            channel: "author".to_owned(),
            edition: "author".to_owned(),
            features: vec!["*".to_owned()],
            issued_at: "2026-01-01T00:00:00Z".to_owned(),
            expires_at: "2099-01-01T00:00:00Z".to_owned(),
            app_version_range: "*".to_owned(),
            key_id: TEST_AUTHOR_KEY_ID.to_owned(),
        }
    }

    fn context() -> EntitlementContext {
        EntitlementContext {
            app_version: "0.16.0".to_owned(),
            checked_at: datetime!(2026-06-04 12:00 UTC),
        }
    }

    fn evaluate(claims: LicenseClaims) -> LicenseStatus {
        let token = build_test_token(&claims, &test_friend_signing_key());
        evaluate_license_token(&token, context(), &test_verifier(), &LocalEntitlementPolicy)
    }

    fn evaluate_with_key(claims: LicenseClaims, signing_key: &SigningKey) -> LicenseStatus {
        let token = build_test_token(&claims, signing_key);
        evaluate_license_token(&token, context(), &test_verifier(), &LocalEntitlementPolicy)
    }

    #[test]
    fn accepts_valid_friend_test_token() {
        let status = evaluate(claims());

        assert_eq!(status.status, LicenseStatusKind::Valid);
        assert!(status.can_use_app);
        assert_eq!(
            status
                .license
                .as_ref()
                .map(|license| license.license_id.as_str()),
            Some("lic_test_001")
        );
    }

    #[test]
    fn accepts_valid_author_token_signed_with_author_key() {
        let status = evaluate_with_key(author_claims(), &test_author_signing_key());

        assert_eq!(status.status, LicenseStatusKind::Valid);
        assert!(status.can_use_app);
        let license = status.license.expect("author metadata");
        assert_eq!(license.channel, "author");
        assert_eq!(license.edition, "author");
        assert_eq!(license.features, vec!["*".to_owned()]);
    }

    #[test]
    fn rejects_author_token_signed_with_friend_key() {
        let status = evaluate_with_key(author_claims(), &test_friend_signing_key());

        assert_eq!(status.status, LicenseStatusKind::Invalid);
        assert!(status.can_use_app);
    }

    #[test]
    fn rejects_author_claims_that_name_the_friend_key() {
        let mut claims = author_claims();
        claims.key_id = TEST_FRIEND_KEY_ID.to_owned();

        let status = evaluate_with_key(claims, &test_friend_signing_key());

        assert_eq!(status.status, LicenseStatusKind::Invalid);
        assert!(status.can_use_app);
    }

    #[test]
    fn rejects_author_channel_without_author_entitlement() {
        let mut claims = author_claims();
        claims.features = vec!["core".to_owned()];

        let status = evaluate_with_key(claims, &test_author_signing_key());

        assert_eq!(status.status, LicenseStatusKind::Invalid);
        assert!(status.can_use_app);
    }

    #[test]
    fn rejects_unsupported_license_channel() {
        let mut claims = claims();
        claims.channel = "paid_subscription".to_owned();

        let status = evaluate(claims);

        assert_eq!(status.status, LicenseStatusKind::Invalid);
        assert!(status.can_use_app);
    }

    #[test]
    fn rejects_missing_token() {
        let status =
            evaluate_license_token("   ", context(), &test_verifier(), &LocalEntitlementPolicy);

        assert_eq!(status.status, LicenseStatusKind::Missing);
        assert!(status.can_use_app);
    }

    #[test]
    fn rejects_tampered_token() {
        let token = build_test_token(&claims(), &test_friend_signing_key());
        let mut tampered = token.clone();
        tampered.push('A');
        let status = evaluate_license_token(
            &tampered,
            context(),
            &test_verifier(),
            &LocalEntitlementPolicy,
        );

        assert_eq!(status.status, LicenseStatusKind::Invalid);
        assert!(status.can_use_app);
    }

    #[test]
    fn rejects_expired_token() {
        let mut claims = claims();
        claims.expires_at = "2026-01-02T00:00:00Z".to_owned();

        let status = evaluate(claims);

        assert_eq!(status.status, LicenseStatusKind::Expired);
        assert!(status.can_use_app);
    }

    #[test]
    fn author_token_is_not_version_bounded() {
        let mut claims = author_claims();
        claims.app_version_range = ">=0.18.0,<0.19.0".to_owned();

        let status = evaluate_with_key(claims, &test_author_signing_key());

        assert_eq!(status.status, LicenseStatusKind::Valid);
        assert!(status.can_use_app);
    }

    #[test]
    fn friend_test_token_is_not_version_bounded() {
        let mut claims = claims();
        claims.app_version_range = ">=0.18.0,<0.19.0".to_owned();

        let status = evaluate(claims);

        assert_eq!(status.status, LicenseStatusKind::Valid);
        assert!(status.can_use_app);
    }

    #[test]
    fn rejects_unsupported_token_version() {
        let token = build_test_token(&claims(), &test_friend_signing_key())
            .replace("BRAWLER-LIC-1", "BRAWLER-LIC-99");

        let status =
            evaluate_license_token(&token, context(), &test_verifier(), &LocalEntitlementPolicy);

        assert_eq!(status.status, LicenseStatusKind::UnsupportedVersion);
        assert!(status.can_use_app);
    }

    #[test]
    fn rejects_unknown_key_id() {
        let mut claims = claims();
        claims.key_id = "other_key".to_owned();

        let status = evaluate(claims);

        assert_eq!(status.status, LicenseStatusKind::Invalid);
        assert!(status.can_use_app);
    }

    #[test]
    fn formats_redacted_license_tokens_without_leaking_input() {
        assert_eq!(
            redact_license_token("BRAWLER-LIC-1.secret.signature"),
            "[redacted-license-token]"
        );
    }

    #[test]
    fn checked_at_uses_context_time() {
        let status = evaluate_license_token(
            &build_test_token(&claims(), &test_friend_signing_key()),
            EntitlementContext {
                app_version: "0.16.0".to_owned(),
                checked_at: OffsetDateTime::UNIX_EPOCH,
            },
            &test_verifier(),
            &LocalEntitlementPolicy,
        );

        assert_eq!(status.checked_at, "1970-01-01T00:00:00Z");
    }
}
