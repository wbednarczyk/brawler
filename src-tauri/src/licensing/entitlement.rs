use semver::{Version, VersionReq};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::token::LicenseClaims;
use super::{LicenseDisplayMetadata, LicenseStatus, LicenseStatusKind};

const AUTHOR_KEY_IDS: &[&str] = &["owner_author_2026_06"];
const FRIEND_TEST_KEY_IDS: &[&str] = &["owner_friend_test_2026_06"];

#[derive(Debug, Clone)]
pub struct EntitlementContext {
    pub app_version: String,
    pub checked_at: OffsetDateTime,
}

pub trait EntitlementPolicy {
    fn evaluate(&self, claims: LicenseClaims, context: EntitlementContext) -> LicenseStatus;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LocalEntitlementPolicy;

impl EntitlementPolicy for LocalEntitlementPolicy {
    fn evaluate(&self, claims: LicenseClaims, context: EntitlementContext) -> LicenseStatus {
        let checked_at = format_time(context.checked_at);

        if !matches!(claims.channel.as_str(), "friend_test" | "author") {
            return LicenseStatus::blocked(
                LicenseStatusKind::Invalid,
                "This license channel is not supported by this app build.",
                checked_at,
            );
        }
        if !key_id_matches_channel(&claims.channel, &claims.key_id) {
            return LicenseStatus::blocked(
                LicenseStatusKind::Invalid,
                "This license signing key is not valid for its channel.",
                checked_at,
            );
        }
        if claims.channel == "author"
            && (claims.edition != "author" || !claims.features.iter().any(|feature| feature == "*"))
        {
            return LicenseStatus::blocked(
                LicenseStatusKind::Invalid,
                "This author license does not grant the expected author entitlement.",
                checked_at,
            );
        }

        let issued_at = match parse_time(&claims.issued_at) {
            Ok(value) => value,
            Err(()) => {
                return LicenseStatus::blocked(
                    LicenseStatusKind::Invalid,
                    "This license issue timestamp is invalid.",
                    checked_at,
                )
            }
        };
        if issued_at > context.checked_at {
            return LicenseStatus::blocked(
                LicenseStatusKind::Invalid,
                "This license is not valid yet.",
                checked_at,
            );
        }

        let expires_at = match parse_time(&claims.expires_at) {
            Ok(value) => value,
            Err(()) => {
                return LicenseStatus::blocked(
                    LicenseStatusKind::Invalid,
                    "This license expiry timestamp is invalid.",
                    checked_at,
                )
            }
        };
        if expires_at <= context.checked_at {
            return LicenseStatus::with_metadata(
                LicenseStatusKind::Expired,
                false,
                Some("This license has expired.".to_owned()),
                LicenseDisplayMetadata::from_claims(claims),
                checked_at,
            );
        }
        if uses_version_limit(&claims) {
            if let Some(status) = evaluate_version_limit(&claims, &context, &checked_at) {
                return status;
            }
        }

        LicenseStatus::with_metadata(
            LicenseStatusKind::Valid,
            true,
            None,
            LicenseDisplayMetadata::from_claims(claims),
            checked_at,
        )
    }
}

pub fn format_time(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

fn key_id_matches_channel(channel: &str, key_id: &str) -> bool {
    match channel {
        "author" => AUTHOR_KEY_IDS.contains(&key_id),
        "friend_test" => FRIEND_TEST_KEY_IDS.contains(&key_id),
        _ => false,
    }
}

fn uses_version_limit(claims: &LicenseClaims) -> bool {
    !matches!(claims.channel.as_str(), "author" | "friend_test")
        && claims.app_version_range.trim() != "*"
}

fn evaluate_version_limit(
    claims: &LicenseClaims,
    context: &EntitlementContext,
    checked_at: &str,
) -> Option<LicenseStatus> {
    let app_version = match Version::parse(&context.app_version) {
        Ok(value) => value,
        Err(_) => {
            return Some(LicenseStatus::blocked(
                LicenseStatusKind::Invalid,
                "The app version could not be evaluated.",
                checked_at.to_owned(),
            ))
        }
    };
    let version_req = match VersionReq::parse(&claims.app_version_range) {
        Ok(value) => value,
        Err(_) => {
            return Some(LicenseStatus::blocked(
                LicenseStatusKind::Invalid,
                "This license version range is invalid.",
                checked_at.to_owned(),
            ))
        }
    };
    if !version_req.matches(&app_version) {
        return Some(LicenseStatus::with_metadata(
            LicenseStatusKind::WrongVersion,
            false,
            Some("This license is not valid for this app version.".to_owned()),
            LicenseDisplayMetadata::from_claims(claims.clone()),
            checked_at.to_owned(),
        ));
    }

    None
}

fn parse_time(value: &str) -> Result<OffsetDateTime, ()> {
    OffsetDateTime::parse(value.trim(), &Rfc3339).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    fn claims(channel: &str, version_range: &str) -> LicenseClaims {
        LicenseClaims {
            license_id: "lic_test".to_owned(),
            holder: "Test".to_owned(),
            channel: channel.to_owned(),
            edition: "future".to_owned(),
            features: vec!["core".to_owned()],
            issued_at: "2026-01-01T00:00:00Z".to_owned(),
            expires_at: "2027-01-01T00:00:00Z".to_owned(),
            app_version_range: version_range.to_owned(),
            key_id: "future_key".to_owned(),
        }
    }

    fn context() -> EntitlementContext {
        EntitlementContext {
            app_version: "0.16.0".to_owned(),
            checked_at: datetime!(2026-06-04 12:00 UTC),
        }
    }

    #[test]
    fn current_author_and_friend_channels_do_not_use_version_limits() {
        assert!(!uses_version_limit(&claims("author", ">=999.0.0")));
        assert!(!uses_version_limit(&claims("friend_test", ">=999.0.0")));
    }

    #[test]
    fn future_channels_can_use_version_limits() {
        assert!(uses_version_limit(&claims(
            "paid_subscription",
            ">=0.16.0,<0.17.0"
        )));
        assert!(!uses_version_limit(&claims("paid_subscription", "*")));
    }

    #[test]
    fn future_version_limit_can_reject_wrong_app_version() {
        let status = evaluate_version_limit(
            &claims("paid_subscription", ">=0.18.0,<0.19.0"),
            &context(),
            "2026-06-04T12:00:00Z",
        )
        .expect("future version limit should reject wrong app version");

        assert_eq!(status.status, LicenseStatusKind::WrongVersion);
        assert!(!status.can_use_app);
    }
}
