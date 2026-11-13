//! Identity keypair and user-facing identity statements.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use nexus_crypto::{DomainTag, IdentityId, KeyPair, PublicKey, SECRET_KEY_LENGTH, SignatureBytes};

use crate::error::IdentityError;

/// User-facing warning that key loss means identity loss.
pub const KEY_LOSS_WARNING: &str = "Losing this identity key loses the identity. NEXUS has no central account, password reset, or recovery service. Keep an encrypted backup and its passphrase somewhere safe.";

/// Status label that never claims real-world identity verification.
pub const SIGNATURE_STATUS_LABEL: &str = "Signed record verified";

/// Explanation of what a valid signature proves and does not prove.
pub const CRYPTOGRAPHIC_IDENTITY_NOTICE: &str = "A valid signature proves control of this identity key. It does not prove a real name, legal authorship, or that content is legal.";

/// Explanation of v1 key lifecycle limits.
pub const KEY_LIMITS_NOTICE: &str = "This version has no automatic key rotation or central revocation. If the key is compromised, create and republish under a new identity; old records remain valid under the old key.";

/// A local Ed25519 identity.
///
/// The keypair is generated locally and never leaves the core in plaintext.
/// `Debug` output contains only public metadata.
pub struct Identity {
    keypair: KeyPair,
    created_at_ms: u64,
}

impl Identity {
    /// Generates a new identity with the operating-system CSPRNG.
    pub fn create() -> Result<Self, IdentityError> {
        Ok(Self {
            keypair: KeyPair::generate()?,
            created_at_ms: now_ms(),
        })
    }

    /// Recreates an identity from a 32-byte seed and creation timestamp.
    ///
    /// This is a core-internal restore path. The seed is secret material; it
    /// must come from a decrypted payload, never from a user interface.
    #[must_use]
    pub fn from_seed(seed: [u8; SECRET_KEY_LENGTH], created_at_ms: u64) -> Self {
        Self {
            keypair: KeyPair::from_seed(seed),
            created_at_ms,
        }
    }

    /// Returns the public key.
    #[must_use]
    pub fn public_key(&self) -> PublicKey {
        self.keypair.public_key()
    }

    /// Returns the stable public identity ID.
    #[must_use]
    pub fn identity_id(&self) -> IdentityId {
        IdentityId::from_public_key(&self.public_key())
    }

    /// Returns the local creation timestamp in Unix milliseconds.
    #[must_use]
    pub const fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }

    /// Signs `domain_tag || message`.
    #[must_use]
    pub fn sign_domain(&self, tag: DomainTag, message: &[u8]) -> SignatureBytes {
        self.keypair.sign_domain(tag, message)
    }

    pub(crate) fn keypair(&self) -> &KeyPair {
        &self.keypair
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Identity")
            .field("identity_id", &self.identity_id())
            .field("created_at_ms", &self.created_at_ms)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::{CRYPTOGRAPHIC_IDENTITY_NOTICE, KEY_LIMITS_NOTICE, KEY_LOSS_WARNING, SIGNATURE_STATUS_LABEL};

    #[test]
    fn user_facing_statements_do_not_overclaim() {
        assert_eq!(SIGNATURE_STATUS_LABEL, "Signed record verified");
        assert!(!SIGNATURE_STATUS_LABEL.to_lowercase().contains("identity verified"));
        assert!(KEY_LOSS_WARNING.contains("no central account"));
        assert!(KEY_LOSS_WARNING.contains("no central account, password reset, or recovery"));
        assert!(CRYPTOGRAPHIC_IDENTITY_NOTICE.contains("does not prove a real name"));
        assert!(KEY_LIMITS_NOTICE.contains("no automatic key rotation"));
        assert!(KEY_LIMITS_NOTICE.contains("central revocation"));
    }
}
