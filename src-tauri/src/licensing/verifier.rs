use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub struct VerificationKey {
    pub key_id: &'static str,
    pub public_key_base64: &'static str,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LicenseVerificationError {
    #[error("license signing key is not recognized")]
    UnknownKey,
    #[error("embedded license public key is invalid")]
    InvalidPublicKey,
    #[error("license signature has invalid length")]
    InvalidSignature,
    #[error("license signature could not be verified")]
    SignatureMismatch,
}

pub trait LicenseVerifier {
    fn verify(
        &self,
        key_id: &str,
        signed_message: &[u8],
        signature: &[u8],
    ) -> Result<(), LicenseVerificationError>;
}

#[derive(Debug, Clone, Copy)]
pub struct Ed25519LicenseVerifier {
    keys: &'static [VerificationKey],
}

impl Ed25519LicenseVerifier {
    pub const fn new(keys: &'static [VerificationKey]) -> Self {
        Self { keys }
    }

    fn find_key(&self, key_id: &str) -> Option<&VerificationKey> {
        self.keys.iter().find(|key| key.key_id == key_id)
    }
}

impl LicenseVerifier for Ed25519LicenseVerifier {
    fn verify(
        &self,
        key_id: &str,
        signed_message: &[u8],
        signature: &[u8],
    ) -> Result<(), LicenseVerificationError> {
        let key = self
            .find_key(key_id)
            .ok_or(LicenseVerificationError::UnknownKey)?;
        let public_key = STANDARD
            .decode(key.public_key_base64)
            .map_err(|_| LicenseVerificationError::InvalidPublicKey)?;
        let public_key: [u8; 32] = public_key
            .try_into()
            .map_err(|_| LicenseVerificationError::InvalidPublicKey)?;
        let verifying_key = VerifyingKey::from_bytes(&public_key)
            .map_err(|_| LicenseVerificationError::InvalidPublicKey)?;
        let signature = Signature::from_slice(signature)
            .map_err(|_| LicenseVerificationError::InvalidSignature)?;

        verifying_key
            .verify(signed_message, &signature)
            .map_err(|_| LicenseVerificationError::SignatureMismatch)
    }
}

pub const LOCAL_LICENSE_PUBLIC_KEYS: &[VerificationKey] = &[
    VerificationKey {
        key_id: "owner_friend_test_2026_06",
        public_key_base64: "KGAU6G/ZyBBvuclZikXO2wqeaCNMkdPMHt41ucjCnnU=",
    },
    VerificationKey {
        key_id: "owner_author_2026_06",
        public_key_base64: "TJJfAfzkpH9nV4KCpVluwtSgEcUfUpVHeY6bzOXAtwM=",
    },
];

pub fn local_license_verifier() -> Ed25519LicenseVerifier {
    Ed25519LicenseVerifier::new(LOCAL_LICENSE_PUBLIC_KEYS)
}
