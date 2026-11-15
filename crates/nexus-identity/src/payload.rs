//! Versioned plaintext payload shared by identity-at-rest and backup files.

use nexus_crypto::{KeyPair, PUBLIC_KEY_LENGTH, PublicKey, SECRET_KEY_LENGTH};
use zeroize::Zeroizing;

use crate::error::IdentityError;
use crate::identity::Identity;

/// Plaintext payload magic.
pub const PAYLOAD_MAGIC: [u8; 8] = *b"NEXKEY01";
/// Plaintext payload format version.
pub const PAYLOAD_VERSION: u16 = 1;
/// Key type identifier for Ed25519.
pub const PAYLOAD_KEY_TYPE_ED25519: u8 = 1;
/// Exact payload length in bytes.
pub const PAYLOAD_LENGTH: usize = 8 + 2 + 1 + 1 + 8 + PUBLIC_KEY_LENGTH + SECRET_KEY_LENGTH;

const VERSION_OFFSET: usize = 8;
const KEY_TYPE_OFFSET: usize = 10;
const FLAGS_OFFSET: usize = 11;
const CREATED_AT_OFFSET: usize = 12;
const PUBLIC_KEY_OFFSET: usize = 20;
const SECRET_SEED_OFFSET: usize = 52;

pub(crate) struct Payload {
    pub created_at_ms: u64,
    pub keypair: KeyPair,
}

pub(crate) fn encode(identity: &Identity) -> Zeroizing<Vec<u8>> {
    let mut payload = Zeroizing::new(Vec::with_capacity(PAYLOAD_LENGTH));
    payload.extend_from_slice(&PAYLOAD_MAGIC);
    payload.extend_from_slice(&PAYLOAD_VERSION.to_le_bytes());
    payload.push(PAYLOAD_KEY_TYPE_ED25519);
    payload.push(0);
    payload.extend_from_slice(&identity.created_at_ms().to_le_bytes());
    payload.extend_from_slice(identity.public_key().as_bytes());
    payload.extend_from_slice(identity.keypair().secret_seed().as_ref());
    payload
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Payload, IdentityError> {
    if bytes.len() != PAYLOAD_LENGTH {
        return Err(IdentityError::InvalidMetadata);
    }
    if bytes[..8] != PAYLOAD_MAGIC {
        return Err(IdentityError::InvalidMetadata);
    }
    let version = u16::from_le_bytes([bytes[VERSION_OFFSET], bytes[VERSION_OFFSET + 1]]);
    if version != PAYLOAD_VERSION {
        return Err(IdentityError::UnsupportedFormat);
    }
    if bytes[KEY_TYPE_OFFSET] != PAYLOAD_KEY_TYPE_ED25519 || bytes[FLAGS_OFFSET] != 0 {
        return Err(IdentityError::InvalidMetadata);
    }

    let created_at_ms = u64::from_le_bytes(
        bytes[CREATED_AT_OFFSET..CREATED_AT_OFFSET + 8]
            .try_into()
            .map_err(|_| IdentityError::InvalidMetadata)?,
    );
    let public_key = PublicKey::from_slice(&bytes[PUBLIC_KEY_OFFSET..PUBLIC_KEY_OFFSET + PUBLIC_KEY_LENGTH])
        .map_err(|_| IdentityError::InvalidMetadata)?;
    let keypair = KeyPair::from_seed_slice(&bytes[SECRET_SEED_OFFSET..SECRET_SEED_OFFSET + SECRET_KEY_LENGTH])
        .map_err(|_| IdentityError::InvalidMetadata)?;
    if keypair.public_key() != public_key {
        return Err(IdentityError::InvalidMetadata);
    }

    Ok(Payload { created_at_ms, keypair })
}

#[cfg(test)]
mod tests {
    use super::{PAYLOAD_LENGTH, decode, encode};
    use crate::identity::Identity;

    #[test]
    fn payload_round_trips_and_rejects_public_key_mismatch() {
        let identity = Identity::from_seed([3_u8; 32], 1_700_000_000_000);
        let encoded = encode(&identity);
        assert_eq!(encoded.len(), PAYLOAD_LENGTH);
        let decoded = decode(&encoded).expect("decode");
        assert_eq!(decoded.created_at_ms, 1_700_000_000_000);
        assert_eq!(decoded.keypair.public_key(), identity.public_key());

        let mut changed = encoded.to_vec();
        let last = changed.len() - 1;
        changed[last] ^= 0x01;
        assert!(decode(&changed).is_err());
    }
}
