//! Ed25519 key generation, signing, and strict verification.

use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use zeroize::Zeroizing;

use crate::domain::{DomainTag, signing_message};
use crate::error::CryptoError;
use crate::hash::to_hex;
use crate::random::fill_random;

/// Length of an Ed25519 public key.
pub const PUBLIC_KEY_LENGTH: usize = 32;
/// Length of an Ed25519 secret seed.
pub const SECRET_KEY_LENGTH: usize = 32;
/// Length of an Ed25519 signature.
pub const SIGNATURE_LENGTH: usize = 64;

/// A validated Ed25519 public key.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct PublicKey([u8; PUBLIC_KEY_LENGTH]);

impl PublicKey {
    /// Validates and wraps a public key.
    pub fn from_bytes(bytes: [u8; PUBLIC_KEY_LENGTH]) -> Result<Self, CryptoError> {
        VerifyingKey::from_bytes(&bytes).map_err(|_| CryptoError::InvalidPublicKey)?;
        Ok(Self(bytes))
    }

    /// Validates and copies a public-key slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, CryptoError> {
        let bytes: [u8; PUBLIC_KEY_LENGTH] = bytes.try_into().map_err(|_| CryptoError::InvalidPublicKey)?;
        Self::from_bytes(bytes)
    }

    /// Returns the raw public-key bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LENGTH] {
        &self.0
    }

    /// Returns lowercase hexadecimal for display and test vectors.
    #[must_use]
    pub fn to_hex(&self) -> String {
        to_hex(&self.0)
    }

    /// Verifies a signature over `domain_tag || message`.
    pub fn verify_domain(&self, tag: DomainTag, message: &[u8], signature: &SignatureBytes) -> Result<(), CryptoError> {
        let verifying_key = VerifyingKey::from_bytes(&self.0).map_err(|_| CryptoError::InvalidPublicKey)?;
        let signature = Signature::from_bytes(signature.as_bytes());
        verifying_key
            .verify_strict(&signing_message(tag, message), &signature)
            .map_err(|_| CryptoError::SignatureVerification)
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("PublicKey").field(&self.to_hex()).finish()
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

/// A validated Ed25519 signature.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct SignatureBytes([u8; SIGNATURE_LENGTH]);

impl SignatureBytes {
    /// Wraps an exact 64-byte signature.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; SIGNATURE_LENGTH]) -> Self {
        Self(bytes)
    }

    /// Copies a signature slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, CryptoError> {
        let bytes: [u8; SIGNATURE_LENGTH] = bytes.try_into().map_err(|_| CryptoError::InvalidSignature)?;
        Ok(Self(bytes))
    }

    /// Returns the raw signature bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; SIGNATURE_LENGTH] {
        &self.0
    }

    /// Returns lowercase hexadecimal for display and test vectors.
    #[must_use]
    pub fn to_hex(&self) -> String {
        to_hex(&self.0)
    }
}

impl fmt::Debug for SignatureBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("SignatureBytes").field(&self.to_hex()).finish()
    }
}

/// An Ed25519 signing keypair.
///
/// The secret seed is zeroized when the keypair is dropped. `Debug` output is
/// redacted so key material cannot leak through logs or crash reports.
pub struct KeyPair {
    signing_key: SigningKey,
}

impl KeyPair {
    /// Generates a new keypair from the operating-system CSPRNG.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut seed = Zeroizing::new([0_u8; SECRET_KEY_LENGTH]);
        fill_random(seed.as_mut())?;
        Ok(Self::from_seed(*seed))
    }

    /// Creates a keypair from a 32-byte Ed25519 seed.
    #[must_use]
    pub fn from_seed(seed: [u8; SECRET_KEY_LENGTH]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    /// Creates a keypair from an exact 32-byte seed slice.
    pub fn from_seed_slice(seed: &[u8]) -> Result<Self, CryptoError> {
        let seed: [u8; SECRET_KEY_LENGTH] = seed.try_into().map_err(|_| CryptoError::InvalidKeyMaterial)?;
        Ok(Self::from_seed(seed))
    }

    /// Returns the public key.
    #[must_use]
    pub fn public_key(&self) -> PublicKey {
        let bytes = self.signing_key.verifying_key().to_bytes();
        PublicKey(bytes)
    }

    /// Signs `domain_tag || message`.
    #[must_use]
    pub fn sign_domain(&self, tag: DomainTag, message: &[u8]) -> SignatureBytes {
        let signature = self.signing_key.sign(&signing_message(tag, message));
        SignatureBytes(signature.to_bytes())
    }

    /// Exports the seed for encryption into an identity file or backup.
    ///
    /// The returned buffer is zeroized on drop. Callers must never log,
    /// display, or persist it without encryption.
    #[must_use]
    pub fn secret_seed(&self) -> Zeroizing<[u8; SECRET_KEY_LENGTH]> {
        Zeroizing::new(self.signing_key.to_bytes())
    }
}

impl fmt::Debug for KeyPair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeyPair")
            .field("public_key", &self.public_key())
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyPair, PublicKey, SignatureBytes};
    use crate::domain::DomainTag;

    #[test]
    fn sign_and_verify_with_domain_separation() {
        let keypair = KeyPair::from_seed([7_u8; 32]);
        let signature = keypair.sign_domain(DomainTag::Publication, b"hello");
        keypair
            .public_key()
            .verify_domain(DomainTag::Publication, b"hello", &signature)
            .expect("valid signature");
        assert!(
            keypair
                .public_key()
                .verify_domain(DomainTag::Profile, b"hello", &signature)
                .is_err()
        );
        assert!(
            keypair
                .public_key()
                .verify_domain(DomainTag::Publication, b"hello!", &signature)
                .is_err()
        );
    }

    #[test]
    fn changed_signature_bytes_fail() {
        let keypair = KeyPair::from_seed([9_u8; 32]);
        let signature = keypair.sign_domain(DomainTag::Profile, b"message");
        let mut bytes = *signature.as_bytes();
        bytes[0] ^= 0x01;
        let changed = SignatureBytes::from_bytes(bytes);
        assert!(
            keypair
                .public_key()
                .verify_domain(DomainTag::Profile, b"message", &changed)
                .is_err()
        );
    }

    #[test]
    fn public_key_rejects_invalid_length_and_debug_is_safe() {
        assert!(PublicKey::from_slice(&[0_u8; 31]).is_err());
        let keypair = KeyPair::from_seed([1_u8; 32]);
        let debug = format!("{keypair:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("0101010101010101"));
    }

    #[test]
    fn committed_publication_signature_vector_is_exact() {
        let keypair = KeyPair::from_seed([0_u8; 32]);
        let signature = keypair.sign_domain(DomainTag::Publication, b"nexus-stage-06");
        assert_eq!(
            signature.to_hex(),
            "3fe32d30a11f1d3e0c9da80cb9f82d84b5fb517a1d5fddf952736fe785cdb779ea6db58a773b9d2bc92d527f97f69d81278dcfb636a3ee55c0368fa03f019e00"
        );
        keypair
            .public_key()
            .verify_domain(DomainTag::Publication, b"nexus-stage-06", &signature)
            .expect("committed vector verifies");
    }

    #[test]
    fn deterministic_seed_produces_deterministic_signature() {
        let first = KeyPair::from_seed([42_u8; 32]);
        let second = KeyPair::from_seed([42_u8; 32]);
        assert_eq!(first.public_key(), second.public_key());
        assert_eq!(
            first.sign_domain(DomainTag::Publication, b"vector"),
            second.sign_domain(DomainTag::Publication, b"vector")
        );
    }
}
