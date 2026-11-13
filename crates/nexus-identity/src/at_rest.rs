//! Encrypted identity-at-rest file format.

use nexus_crypto::{AEAD_NONCE_LENGTH, AEAD_TAG_LENGTH, DomainTag, decrypt, encrypt, hkdf_sha256_32, random_array};

use crate::error::IdentityError;
use crate::identity::Identity;
use crate::local_key::LocalEncryptionKey;
use crate::payload::{PAYLOAD_LENGTH, decode as decode_payload, encode as encode_payload};

/// Identity-file magic.
pub const AT_REST_MAGIC: [u8; 8] = *b"NEXIDNT1";
/// Identity-file format version.
pub const AT_REST_FORMAT_VERSION: u16 = 1;
/// KDF identifier for HKDF-SHA256.
pub const AT_REST_KDF_HKDF_SHA256: u8 = 1;
/// HKDF salt length.
pub const AT_REST_SALT_LENGTH: usize = 32;
/// Header length in bytes.
pub const AT_REST_HEADER_LENGTH: usize = 8 + 2 + 1 + 1 + AT_REST_SALT_LENGTH + AEAD_NONCE_LENGTH;
/// Maximum accepted identity-file size.
pub const AT_REST_MAX_FILE_LENGTH: usize = 4_096;

const VERSION_OFFSET: usize = 8;
const KDF_OFFSET: usize = 10;
const FLAGS_OFFSET: usize = 11;
const SALT_OFFSET: usize = 12;
const NONCE_OFFSET: usize = 44;

/// Encrypts an identity into the exact `secrets/identity.enc` byte format.
pub fn encrypt_identity(identity: &Identity, lek: &LocalEncryptionKey) -> Result<Vec<u8>, IdentityError> {
    let salt: [u8; AT_REST_SALT_LENGTH] = random_array()?;
    let nonce: [u8; AEAD_NONCE_LENGTH] = random_array()?;
    encrypt_identity_with_material(identity, lek, &salt, &nonce)
}

/// Encrypts with explicit salt and nonce for deterministic vectors.
///
/// Production callers must use [`encrypt_identity`], which generates fresh
/// random material for every write.
pub fn encrypt_identity_with_material(
    identity: &Identity,
    lek: &LocalEncryptionKey,
    salt: &[u8; AT_REST_SALT_LENGTH],
    nonce: &[u8; AEAD_NONCE_LENGTH],
) -> Result<Vec<u8>, IdentityError> {
    let mut header = Vec::with_capacity(AT_REST_HEADER_LENGTH);
    header.extend_from_slice(&AT_REST_MAGIC);
    header.extend_from_slice(&AT_REST_FORMAT_VERSION.to_le_bytes());
    header.push(AT_REST_KDF_HKDF_SHA256);
    header.push(0);
    header.extend_from_slice(salt);
    header.extend_from_slice(nonce);
    debug_assert_eq!(header.len(), AT_REST_HEADER_LENGTH);

    let key = hkdf_sha256_32(lek.expose(), salt, DomainTag::IdentityKey.as_bytes())?;
    let payload = encode_payload(identity);
    let ciphertext = encrypt(&key, nonce, &header, &payload)?;

    let mut file = Vec::with_capacity(header.len() + ciphertext.len());
    file.extend_from_slice(&header);
    file.extend_from_slice(&ciphertext);
    if file.len() > AT_REST_MAX_FILE_LENGTH {
        return Err(IdentityError::InvalidIdentityFile);
    }
    Ok(file)
}

/// Decrypts and validates an identity-at-rest file.
pub fn decrypt_identity(file: &[u8], lek: &LocalEncryptionKey) -> Result<Identity, IdentityError> {
    if file.len() > AT_REST_MAX_FILE_LENGTH || file.len() < AT_REST_HEADER_LENGTH + PAYLOAD_LENGTH + AEAD_TAG_LENGTH {
        return Err(IdentityError::InvalidIdentityFile);
    }
    if file[..8] != AT_REST_MAGIC {
        return Err(IdentityError::InvalidIdentityFile);
    }
    let version = u16::from_le_bytes([file[VERSION_OFFSET], file[VERSION_OFFSET + 1]]);
    if version != AT_REST_FORMAT_VERSION {
        return Err(IdentityError::UnsupportedFormat);
    }
    if file[KDF_OFFSET] != AT_REST_KDF_HKDF_SHA256 || file[FLAGS_OFFSET] != 0 {
        return Err(IdentityError::UnsupportedFormat);
    }

    let header = &file[..AT_REST_HEADER_LENGTH];
    let salt: [u8; AT_REST_SALT_LENGTH] = header[SALT_OFFSET..SALT_OFFSET + AT_REST_SALT_LENGTH]
        .try_into()
        .map_err(|_| IdentityError::InvalidIdentityFile)?;
    let nonce: [u8; AEAD_NONCE_LENGTH] = header[NONCE_OFFSET..NONCE_OFFSET + AEAD_NONCE_LENGTH]
        .try_into()
        .map_err(|_| IdentityError::InvalidIdentityFile)?;
    let key = hkdf_sha256_32(lek.expose(), &salt, DomainTag::IdentityKey.as_bytes())?;
    let plaintext =
        decrypt(&key, &nonce, header, &file[AT_REST_HEADER_LENGTH..]).map_err(|_| IdentityError::DecryptionFailed)?;
    let payload = decode_payload(&plaintext)?;
    Ok(Identity::from_seed(
        *payload.keypair.secret_seed(),
        payload.created_at_ms,
    ))
}

#[cfg(test)]
mod tests {
    use super::{AT_REST_HEADER_LENGTH, decrypt_identity, encrypt_identity};
    use crate::identity::Identity;
    use crate::local_key::LocalEncryptionKey;

    #[test]
    fn round_trips_and_rejects_header_and_lek_changes() {
        let identity = Identity::from_seed([11_u8; 32], 123);
        let lek = LocalEncryptionKey::new([12_u8; 32]);
        let file = encrypt_identity(&identity, &lek).expect("encrypt");
        assert_eq!(&file[..8], b"NEXIDNT1");
        let restored = decrypt_identity(&file, &lek).expect("decrypt");
        assert_eq!(restored.identity_id(), identity.identity_id());
        assert_eq!(restored.created_at_ms(), 123);

        let mut changed_header = file.clone();
        changed_header[AT_REST_HEADER_LENGTH - 1] ^= 0x01;
        assert!(decrypt_identity(&changed_header, &lek).is_err());

        let mut truncated = file;
        truncated.truncate(AT_REST_HEADER_LENGTH + 1);
        assert!(decrypt_identity(&truncated, &lek).is_err());

        assert!(decrypt_identity(&changed_header, &LocalEncryptionKey::new([13_u8; 32])).is_err());
    }
}
