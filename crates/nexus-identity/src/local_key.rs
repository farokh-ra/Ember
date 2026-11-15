//! Host-supplied local encryption key (LEK).

use std::fmt;

use nexus_crypto::{CryptoError, fill_random};
use zeroize::Zeroize;

/// Length of the local encryption key.
pub const LOCAL_ENCRYPTION_KEY_LENGTH: usize = 32;

/// A 32-byte local encryption key supplied by the platform secure store.
///
/// The value is zeroized on drop and never appears in `Debug` output. This
/// type is a transport for the LEK inside the core; it is not a passphrase and
/// must not be persisted in plaintext.
pub struct LocalEncryptionKey([u8; LOCAL_ENCRYPTION_KEY_LENGTH]);

impl LocalEncryptionKey {
    /// Wraps an existing 32-byte LEK.
    #[must_use]
    pub const fn new(bytes: [u8; LOCAL_ENCRYPTION_KEY_LENGTH]) -> Self {
        Self(bytes)
    }

    /// Generates a random LEK.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut bytes = [0_u8; LOCAL_ENCRYPTION_KEY_LENGTH];
        fill_random(&mut bytes)?;
        Ok(Self(bytes))
    }

    /// Copies an LEK slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, CryptoError> {
        let bytes: [u8; LOCAL_ENCRYPTION_KEY_LENGTH] = bytes.try_into().map_err(|_| CryptoError::InvalidKeyMaterial)?;
        Ok(Self(bytes))
    }

    /// Returns the raw key bytes for key derivation.
    ///
    /// Callers must not log, display, or persist the returned bytes.
    #[must_use]
    pub const fn expose(&self) -> &[u8; LOCAL_ENCRYPTION_KEY_LENGTH] {
        &self.0
    }
}

impl Drop for LocalEncryptionKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for LocalEncryptionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LocalEncryptionKey([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::LocalEncryptionKey;

    #[test]
    fn debug_is_redacted() {
        let key = LocalEncryptionKey::new([7_u8; 32]);
        let debug = format!("{key:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("07070707"));
    }
}
