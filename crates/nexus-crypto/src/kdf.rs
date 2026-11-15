//! HKDF-SHA256 and Argon2id key derivation.

use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::CryptoError;

/// Default key length produced by the Ember KDF helpers.
pub const DERIVED_KEY_LENGTH: usize = 32;
/// Length of an Argon2id backup salt.
pub const ARGON2_SALT_LENGTH: usize = 16;
/// Maximum memory cost accepted from an untrusted backup header.
pub const MAX_ARGON2_MEMORY_KIB: u32 = 1_048_576;
/// Maximum iterations accepted from an untrusted backup header.
pub const MAX_ARGON2_ITERATIONS: u32 = 16;
/// Maximum parallelism accepted from an untrusted backup header.
pub const MAX_ARGON2_PARALLELISM: u32 = 16;

/// Expands key material with HKDF-SHA256.
pub fn hkdf_sha256(input_key_material: &[u8], salt: &[u8], info: &[u8], output: &mut [u8]) -> Result<(), CryptoError> {
    let hkdf = Hkdf::<Sha256>::new(Some(salt), input_key_material);
    hkdf.expand(info, output).map_err(|_| CryptoError::KeyDerivation)
}

/// Derives a 32-byte key with HKDF-SHA256.
pub fn hkdf_sha256_32(
    input_key_material: &[u8],
    salt: &[u8],
    info: &[u8],
) -> Result<Zeroizing<[u8; DERIVED_KEY_LENGTH]>, CryptoError> {
    let mut output = Zeroizing::new([0_u8; DERIVED_KEY_LENGTH]);
    hkdf_sha256(input_key_material, salt, info, &mut output[..])?;
    Ok(output)
}

/// Versioned Argon2id cost parameters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Argon2idParams {
    /// Memory cost in kibibytes.
    pub memory_kib: u32,
    /// Number of passes.
    pub iterations: u32,
    /// Number of lanes.
    pub parallelism: u32,
}

impl Argon2idParams {
    /// Creates a parameter set.
    #[must_use]
    pub const fn new(memory_kib: u32, iterations: u32, parallelism: u32) -> Self {
        Self {
            memory_kib,
            iterations,
            parallelism,
        }
    }

    /// Validates bounds before allocating memory from an untrusted header.
    pub fn validate(&self) -> Result<(), CryptoError> {
        if self.memory_kib < 8
            || self.memory_kib > MAX_ARGON2_MEMORY_KIB
            || self.iterations == 0
            || self.iterations > MAX_ARGON2_ITERATIONS
            || self.parallelism == 0
            || self.parallelism > MAX_ARGON2_PARALLELISM
            || self.memory_kib < self.parallelism.saturating_mul(8)
        {
            return Err(CryptoError::KeyDerivation);
        }
        Ok(())
    }
}

/// Derives a 32-byte key with Argon2id v1.3.
pub fn argon2id_key(
    passphrase: &[u8],
    salt: &[u8; ARGON2_SALT_LENGTH],
    params: Argon2idParams,
) -> Result<Zeroizing<[u8; DERIVED_KEY_LENGTH]>, CryptoError> {
    params.validate()?;
    let argon2_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(DERIVED_KEY_LENGTH),
    )
    .map_err(|_| CryptoError::KeyDerivation)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params);
    let mut output = Zeroizing::new([0_u8; DERIVED_KEY_LENGTH]);
    argon2
        .hash_password_into(passphrase, salt, &mut output[..])
        .map_err(|_| CryptoError::KeyDerivation)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{Argon2idParams, argon2id_key, hkdf_sha256_32};

    #[test]
    fn hkdf_matches_rfc_5869_case_one_prefix() {
        let input_key_material = [0x0b_u8; 22];
        let salt = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
        ];
        let info = [0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9];
        let output = hkdf_sha256_32(&input_key_material, &salt, &info).expect("hkdf");
        assert_eq!(
            hex::encode(&output[..]),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"
        );
    }

    #[test]
    fn argon2id_is_deterministic_with_fixed_parameters() {
        let params = Argon2idParams::new(8, 1, 1);
        let salt = [7_u8; 16];
        let first = argon2id_key(b"correct horse battery staple", &salt, params).expect("kdf");
        let second = argon2id_key(b"correct horse battery staple", &salt, params).expect("kdf");
        assert_eq!(&first[..], &second[..]);
        let other = argon2id_key(b"correct horse battery stapl3", &salt, params).expect("kdf");
        assert_ne!(&first[..], &other[..]);
    }

    #[test]
    fn rejects_untrusted_extreme_parameters() {
        assert!(Argon2idParams::new(u32::MAX, 1, 1).validate().is_err());
        assert!(Argon2idParams::new(8, 100, 1).validate().is_err());
        assert!(Argon2idParams::new(8, 1, 0).validate().is_err());
    }
}
