//! Exact-byte signed-record envelope.

use nexus_crypto::{CborLimits, DomainTag, KeyPair, PublicKey, SignatureBytes, Value, decode_signed_record};

use crate::error::ProtocolError;

/// Maximum complete signed-envelope size.
pub const MAX_SIGNED_RECORD_BYTES: usize = 65_536;
/// Maximum signed body size, leaving envelope headroom under the cap.
pub const MAX_SIGNED_BODY_BYTES: usize = MAX_SIGNED_RECORD_BYTES - 96;

/// A signed envelope that preserves the exact body bytes it was given.
///
/// The body is never re-encoded during verification. A replica can serve
/// [`SignedRecord::raw`] unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedRecord {
    raw: Vec<u8>,
    body: Vec<u8>,
    signature: SignatureBytes,
}

impl SignedRecord {
    /// Signs canonical body bytes with a domain tag.
    pub fn sign(body: Vec<u8>, tag: DomainTag, keypair: &KeyPair) -> Result<Self, ProtocolError> {
        if body.is_empty() || body.len() > MAX_SIGNED_BODY_BYTES {
            return Err(ProtocolError::RecordTooLarge);
        }
        let signature = keypair.sign_domain(tag, &body);
        Self::from_parts(body, signature)
    }

    /// Builds an envelope from already-signed body bytes and a signature.
    ///
    /// The identity vault signs domain-separated messages without exposing its
    /// keypair. A record built here is not trusted until the caller verifies it
    /// against the public key named by the body.
    pub fn from_parts(body: Vec<u8>, signature: SignatureBytes) -> Result<Self, ProtocolError> {
        if body.is_empty() || body.len() > MAX_SIGNED_BODY_BYTES {
            return Err(ProtocolError::RecordTooLarge);
        }
        let envelope = Value::Map(vec![
            (Value::Unsigned(1), Value::Bytes(body.clone())),
            (Value::Unsigned(2), Value::Bytes(signature.as_bytes().to_vec())),
        ]);
        let raw = envelope.to_canonical_bytes_with_limits(CborLimits::signed_record())?;
        if raw.len() > MAX_SIGNED_RECORD_BYTES {
            return Err(ProtocolError::RecordTooLarge);
        }
        Ok(Self { raw, body, signature })
    }

    /// Parses a canonical `{1: body, 2: sig}` envelope and keeps its raw bytes.
    pub fn parse(raw: &[u8]) -> Result<Self, ProtocolError> {
        if raw.len() > MAX_SIGNED_RECORD_BYTES {
            return Err(ProtocolError::RecordTooLarge);
        }
        let value = decode_signed_record(raw)?;
        let entries = value.as_map().ok_or(ProtocolError::InvalidEnvelope)?;
        if entries.len() != 2 {
            return Err(ProtocolError::InvalidEnvelope);
        }
        let body = value
            .get_key(1)
            .and_then(Value::as_bytes)
            .ok_or(ProtocolError::InvalidEnvelope)?;
        let signature = value
            .get_key(2)
            .and_then(Value::as_bytes)
            .ok_or(ProtocolError::InvalidEnvelope)?;
        if body.is_empty() || body.len() > MAX_SIGNED_BODY_BYTES {
            return Err(ProtocolError::RecordTooLarge);
        }
        let signature = SignatureBytes::from_slice(signature).map_err(|_| ProtocolError::InvalidEnvelope)?;
        Ok(Self {
            raw: raw.to_vec(),
            body: body.to_vec(),
            signature,
        })
    }

    /// Verifies the signature over the exact stored body bytes.
    pub fn verify(&self, tag: DomainTag, public_key: &PublicKey) -> Result<(), ProtocolError> {
        public_key
            .verify_domain(tag, &self.body, &self.signature)
            .map_err(|_| ProtocolError::VerificationFailed)
    }

    /// Returns the exact envelope bytes for storage or re-serving.
    #[must_use]
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Returns the exact signed body bytes.
    #[must_use]
    pub fn body_bytes(&self) -> &[u8] {
        &self.body
    }

    /// Returns the signature.
    #[must_use]
    pub const fn signature(&self) -> &SignatureBytes {
        &self.signature
    }
}

#[cfg(test)]
mod tests {
    use nexus_crypto::{DomainTag, KeyPair, Value};

    use super::{MAX_SIGNED_RECORD_BYTES, SignedRecord};

    #[test]
    fn envelope_round_trips_exact_bytes() {
        let keypair = KeyPair::from_seed([1_u8; 32]);
        let body = Value::Map(vec![
            (Value::Unsigned(1), Value::Unsigned(1)),
            (Value::Unsigned(2), Value::Text("body".to_owned())),
        ])
        .to_canonical_bytes()
        .expect("body");
        let record = SignedRecord::sign(body.clone(), DomainTag::Profile, &keypair).expect("sign");
        let parsed = SignedRecord::parse(record.raw()).expect("parse");
        assert_eq!(parsed.raw(), record.raw());
        assert_eq!(parsed.body_bytes(), body);
        parsed
            .verify(DomainTag::Profile, &keypair.public_key())
            .expect("verify");
        assert!(parsed.verify(DomainTag::Publication, &keypair.public_key()).is_err());
    }

    #[test]
    fn envelope_can_be_built_from_external_signing() {
        let keypair = KeyPair::from_seed([3_u8; 32]);
        let body = Value::Map(vec![(Value::Unsigned(1), Value::Text("profile".to_owned()))])
            .to_canonical_bytes()
            .expect("body");
        let signature = keypair.sign_domain(DomainTag::Profile, &body);
        let record = SignedRecord::from_parts(body.clone(), signature).expect("envelope");
        record
            .verify(DomainTag::Profile, &keypair.public_key())
            .expect("verify");
        assert_eq!(record.body_bytes(), body);

        let other = KeyPair::from_seed([4_u8; 32]).sign_domain(DomainTag::Profile, &body);
        let wrong = SignedRecord::from_parts(body, other).expect("envelope");
        assert!(wrong.verify(DomainTag::Profile, &keypair.public_key()).is_err());
        assert!(SignedRecord::from_parts(Vec::new(), signature).is_err());
    }

    #[test]
    fn rejects_extra_fields_and_oversize_input() {
        let keypair = KeyPair::from_seed([2_u8; 32]);
        let signature = keypair.sign_domain(DomainTag::Profile, b"body");
        let extra = Value::Map(vec![
            (Value::Unsigned(1), Value::Bytes(b"body".to_vec())),
            (Value::Unsigned(2), Value::Bytes(signature.as_bytes().to_vec())),
            (Value::Unsigned(3), Value::Null),
        ])
        .to_canonical_bytes()
        .expect("envelope");
        assert!(SignedRecord::parse(&extra).is_err());

        assert!(SignedRecord::parse(&vec![0_u8; MAX_SIGNED_RECORD_BYTES + 1]).is_err());
    }
}
