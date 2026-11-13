//! Stable public identity IDs.
//!
//! An identity ID is a Bech32m string with human-readable prefix `nx`. The
//! payload is the multicodec varint for an Ed25519 public key (`0xed 0x01`)
//! followed by the 32-byte public key. The ID is derived only from the public
//! key; there is no registry, account, or centrally assigned name.

use std::fmt;

use bech32::{Bech32m, Hrp};

use crate::error::CryptoError;
use crate::key::{PUBLIC_KEY_LENGTH, PublicKey};

/// Human-readable prefix for NEXUS identity IDs.
pub const IDENTITY_ID_HRP: &str = "nx";
/// Multicodec varint for an Ed25519 public key.
pub const ED25519_PUBLIC_KEY_MULTICODEC: [u8; 2] = [0xed, 0x01];
/// Length of the Bech32m payload: multicodec plus public key.
pub const IDENTITY_ID_PAYLOAD_LENGTH: usize = ED25519_PUBLIC_KEY_MULTICODEC.len() + PUBLIC_KEY_LENGTH;

/// A canonical `nx1...` public identity ID.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdentityId(String);

impl IdentityId {
    /// Derives the canonical identity ID from a public key.
    #[must_use]
    pub fn from_public_key(public_key: &PublicKey) -> Self {
        let mut payload = Vec::with_capacity(IDENTITY_ID_PAYLOAD_LENGTH);
        payload.extend_from_slice(&ED25519_PUBLIC_KEY_MULTICODEC);
        payload.extend_from_slice(public_key.as_bytes());
        let hrp = Hrp::parse_unchecked(IDENTITY_ID_HRP);
        let encoded = bech32::encode::<Bech32m>(hrp, &payload)
            .expect("constant HRP and 34-byte payload always encode as canonical Bech32m");
        Self(encoded)
    }

    /// Parses a canonical lowercase `nx1...` Bech32m identity ID.
    pub fn parse(value: &str) -> Result<Self, CryptoError> {
        let (hrp, payload) = bech32::decode(value).map_err(|_| CryptoError::InvalidIdentityId)?;
        if hrp.as_str() != IDENTITY_ID_HRP {
            return Err(CryptoError::InvalidIdentityId);
        }
        if payload.len() != IDENTITY_ID_PAYLOAD_LENGTH {
            return Err(CryptoError::InvalidIdentityId);
        }
        if payload[..ED25519_PUBLIC_KEY_MULTICODEC.len()] != ED25519_PUBLIC_KEY_MULTICODEC {
            return Err(CryptoError::InvalidIdentityId);
        }
        let public_key = PublicKey::from_slice(&payload[ED25519_PUBLIC_KEY_MULTICODEC.len()..])?;
        let identity_id = Self::from_public_key(&public_key);

        // Re-encoding rejects the Bech32 checksum variant, uppercase input,
        // mixed case, and any other non-canonical spelling.
        if identity_id.as_str() != value {
            return Err(CryptoError::InvalidIdentityId);
        }
        Ok(identity_id)
    }

    /// Decodes the embedded Ed25519 public key.
    pub fn public_key(&self) -> Result<PublicKey, CryptoError> {
        let (hrp, payload) = bech32::decode(&self.0).map_err(|_| CryptoError::InvalidIdentityId)?;
        if hrp.as_str() != IDENTITY_ID_HRP
            || payload.len() != IDENTITY_ID_PAYLOAD_LENGTH
            || payload[..ED25519_PUBLIC_KEY_MULTICODEC.len()] != ED25519_PUBLIC_KEY_MULTICODEC
        {
            return Err(CryptoError::InvalidIdentityId);
        }
        PublicKey::from_slice(&payload[ED25519_PUBLIC_KEY_MULTICODEC.len()..])
    }

    /// Returns the canonical string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdentityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for IdentityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("IdentityId").field(&self.0).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{IDENTITY_ID_HRP, IdentityId};
    use crate::key::KeyPair;

    #[test]
    fn round_trips_public_key() {
        let keypair = KeyPair::from_seed([5_u8; 32]);
        let identity_id = IdentityId::from_public_key(&keypair.public_key());
        assert!(identity_id.as_str().starts_with(&format!("{IDENTITY_ID_HRP}1")));
        assert_eq!(
            IdentityId::parse(identity_id.as_str())
                .expect("parse")
                .public_key()
                .expect("public key"),
            keypair.public_key()
        );
    }

    #[test]
    fn rejects_wrong_prefix_checksum_variant_and_case() {
        let keypair = KeyPair::from_seed([6_u8; 32]);
        let identity_id = IdentityId::from_public_key(&keypair.public_key());
        let payload = bech32::decode(identity_id.as_str()).expect("decode").1;

        let wrong_prefix =
            bech32::encode::<bech32::Bech32m>(bech32::Hrp::parse_unchecked("ny"), &payload).expect("encode");
        assert!(IdentityId::parse(&wrong_prefix).is_err());

        let bech32_checksum =
            bech32::encode::<bech32::Bech32>(bech32::Hrp::parse_unchecked("nx"), &payload).expect("encode");
        assert!(IdentityId::parse(&bech32_checksum).is_err());

        assert!(IdentityId::parse(&identity_id.as_str().to_uppercase()).is_err());
    }

    #[test]
    fn rejects_wrong_payload_and_checksum() {
        let keypair = KeyPair::from_seed([8_u8; 32]);
        let identity_id = IdentityId::from_public_key(&keypair.public_key());
        let mut payload = bech32::decode(identity_id.as_str()).expect("decode").1;
        payload[0] = 0xee;
        let wrong_codec =
            bech32::encode::<bech32::Bech32m>(bech32::Hrp::parse_unchecked("nx"), &payload).expect("encode");
        assert!(IdentityId::parse(&wrong_codec).is_err());

        let mut chars: Vec<char> = identity_id.as_str().chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'q' { 'p' } else { 'q' };
        let corrupted: String = chars.into_iter().collect();
        assert!(IdentityId::parse(&corrupted).is_err());
    }
}
