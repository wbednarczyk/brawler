use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const TOKEN_PREFIX_V1: &str = "BRAWLER-LIC-1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicenseClaims {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLicenseToken {
    pub claims: LicenseClaims,
    pub signed_message: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenParseError {
    #[error("license key is required")]
    Missing,
    #[error("license key version is not supported")]
    UnsupportedVersion,
    #[error("license key format is not recognized")]
    Malformed,
    #[error("license key payload is not valid base64url")]
    InvalidPayloadEncoding,
    #[error("license key signature is not valid base64url")]
    InvalidSignatureEncoding,
    #[error("license key payload is not valid JSON")]
    InvalidPayloadJson,
}

pub fn parse_license_token(token: &str) -> Result<ParsedLicenseToken, TokenParseError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(TokenParseError::Missing);
    }

    let mut parts = token.split('.');
    let prefix = parts.next().ok_or(TokenParseError::Malformed)?;
    let payload = parts.next().ok_or(TokenParseError::Malformed)?;
    let signature = parts.next().ok_or(TokenParseError::Malformed)?;
    if parts.next().is_some() || payload.is_empty() || signature.is_empty() {
        return Err(TokenParseError::Malformed);
    }

    if prefix != TOKEN_PREFIX_V1 {
        return if prefix.starts_with("BRAWLER-LIC-") {
            Err(TokenParseError::UnsupportedVersion)
        } else {
            Err(TokenParseError::Malformed)
        };
    }

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| TokenParseError::InvalidPayloadEncoding)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| TokenParseError::InvalidSignatureEncoding)?;
    let claims =
        serde_json::from_slice(&payload_bytes).map_err(|_| TokenParseError::InvalidPayloadJson)?;

    Ok(ParsedLicenseToken {
        claims,
        signed_message: format!("{prefix}.{payload}"),
        signature,
    })
}

#[cfg(test)]
pub fn build_test_token(claims: &LicenseClaims, signing_key: &ed25519_dalek::SigningKey) -> String {
    use ed25519_dalek::Signer;

    let payload = serde_json::to_vec(claims).expect("test claims should serialize");
    let payload = URL_SAFE_NO_PAD.encode(payload);
    let signed_message = format!("{TOKEN_PREFIX_V1}.{payload}");
    let signature = signing_key.sign(signed_message.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(signature.to_bytes());

    format!("{signed_message}.{signature}")
}
