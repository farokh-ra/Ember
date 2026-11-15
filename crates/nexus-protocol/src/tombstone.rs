//! Signed tombstone schema v1.
//!
//! A tombstone is signed by the original uploader and references the withdrawn
//! publication ID and content ID. It never deletes another peer's copy or
//! rewrites the original signed publication record.

use nexus_crypto::{CborLimits, DomainTag, IdentityId, KeyPair, Value, decode_signed_record, normalize_text_bytes};

use crate::envelope::{MAX_SIGNED_BODY_BYTES, SignedRecord};
use crate::error::ProtocolError;
use crate::fields::{bytes32, nonnegative_keys, text, unsigned};
use crate::publication::SignedPublication;

/// Tombstone schema major version.
pub const TOMBSTONE_SCHEMA_VERSION: u64 = 1;

const MAX_KNOWN_FIELD: u64 = 6;

/// A tombstone body with the v1 logical fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TombstoneV1 {
    /// Public identity ID of the original uploader.
    pub identity_id: IdentityId,
    /// Publication being withdrawn.
    pub publication_id: [u8; 32],
    /// Content ID of the withdrawn publication.
    pub content_id: [u8; 32],
    /// Tombstone revision, at least 1.
    pub revision: u64,
    /// Informational unpublish time in Unix milliseconds.
    pub unpublished_at_ms: u64,
    unknown_fields: Vec<(u64, Value)>,
}

impl TombstoneV1 {
    /// Creates a minimal v1 tombstone body.
    #[must_use]
    pub fn new(
        identity_id: IdentityId,
        publication_id: [u8; 32],
        content_id: [u8; 32],
        revision: u64,
        unpublished_at_ms: u64,
    ) -> Self {
        Self {
            identity_id,
            publication_id,
            content_id,
            revision,
            unpublished_at_ms,
            unknown_fields: Vec::new(),
        }
    }

    /// Returns preserved unknown fields from a newer valid minor record.
    #[must_use]
    pub fn unknown_fields(&self) -> &[(u64, Value)] {
        &self.unknown_fields
    }

    /// Encodes the canonical tombstone body.
    pub fn encode_body(&self) -> Result<Vec<u8>, ProtocolError> {
        if self.revision == 0 {
            return Err(ProtocolError::InvalidField(5));
        }
        let identity_id =
            normalize_text_bytes(self.identity_id.as_str(), 128).map_err(|_| ProtocolError::InvalidField(2))?;
        let mut entries = vec![
            (Value::Unsigned(1), Value::Unsigned(TOMBSTONE_SCHEMA_VERSION)),
            (Value::Unsigned(2), Value::Text(identity_id)),
            (Value::Unsigned(3), Value::Bytes(self.publication_id.to_vec())),
            (Value::Unsigned(4), Value::Bytes(self.content_id.to_vec())),
            (Value::Unsigned(5), Value::Unsigned(self.revision)),
            (Value::Unsigned(6), Value::Unsigned(self.unpublished_at_ms)),
        ];
        append_unknown_fields(&mut entries, &self.unknown_fields)?;

        let body = Value::Map(entries).to_canonical_bytes_with_limits(CborLimits::signed_record())?;
        if body.len() > MAX_SIGNED_BODY_BYTES {
            return Err(ProtocolError::RecordTooLarge);
        }
        Ok(body)
    }

    /// Parses a canonical v1 tombstone body and preserves unknown minor fields.
    pub fn parse_body(body: &[u8]) -> Result<Self, ProtocolError> {
        let value = decode_signed_record(body)?;
        let keys = nonnegative_keys(&value)?;
        let schema = unsigned(&value, 1)?;
        if schema != TOMBSTONE_SCHEMA_VERSION {
            return Err(ProtocolError::UnsupportedVersion(schema));
        }

        let identity_id_text = text(&value, 2)?;
        let identity_id = IdentityId::parse(identity_id_text).map_err(|_| ProtocolError::InvalidField(2))?;
        let publication_id = bytes32(&value, 3)?;
        let content_id = bytes32(&value, 4)?;
        let revision = unsigned(&value, 5)?;
        if revision == 0 {
            return Err(ProtocolError::InvalidField(5));
        }
        let unpublished_at_ms = unsigned(&value, 6)?;

        let mut unknown_fields = Vec::new();
        for key in keys {
            if key > MAX_KNOWN_FIELD {
                let value = value.get_key(key).ok_or(ProtocolError::MissingField(key))?.clone();
                unknown_fields.push((key, value));
            }
        }

        Ok(Self {
            identity_id,
            publication_id,
            content_id,
            revision,
            unpublished_at_ms,
            unknown_fields,
        })
    }

    /// Signs this tombstone and returns the exact-byte record.
    pub fn sign(&self, keypair: &KeyPair) -> Result<SignedTombstone, ProtocolError> {
        if IdentityId::from_public_key(&keypair.public_key()) != self.identity_id {
            return Err(ProtocolError::IdentityKeyMismatch);
        }
        let body = self.encode_body()?;
        let record = SignedRecord::sign(body, DomainTag::Tombstone, keypair)?;
        let tombstone = Self::parse_body(record.body_bytes())?;
        Ok(SignedTombstone { record, tombstone })
    }
}

/// A verified tombstone record that keeps its exact envelope bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedTombstone {
    record: SignedRecord,
    tombstone: TombstoneV1,
}

impl SignedTombstone {
    /// Signs a tombstone body.
    pub fn sign(tombstone: &TombstoneV1, keypair: &KeyPair) -> Result<Self, ProtocolError> {
        tombstone.sign(keypair)
    }

    /// Parses and verifies a tombstone record from exact envelope bytes.
    pub fn parse(raw: &[u8]) -> Result<Self, ProtocolError> {
        let record = SignedRecord::parse(raw)?;
        Self::from_record(record)
    }

    /// Verifies an already-parsed signed record.
    pub fn from_record(record: SignedRecord) -> Result<Self, ProtocolError> {
        let tombstone = TombstoneV1::parse_body(record.body_bytes())?;
        let public_key = tombstone
            .identity_id
            .public_key()
            .map_err(|_| ProtocolError::InvalidField(2))?;
        record.verify(DomainTag::Tombstone, &public_key)?;
        Ok(Self { record, tombstone })
    }

    /// Returns the verified tombstone body.
    #[must_use]
    pub fn tombstone(&self) -> &TombstoneV1 {
        &self.tombstone
    }

    /// Returns the exact signed envelope.
    #[must_use]
    pub fn record(&self) -> &SignedRecord {
        &self.record
    }

    /// Returns the exact envelope bytes for storage or re-serving.
    #[must_use]
    pub fn raw(&self) -> &[u8] {
        self.record.raw()
    }

    /// Verifies that this tombstone withdraws the supplied publication under
    /// the original uploader's identity and content ID.
    pub fn validate_for(&self, publication: &SignedPublication) -> Result<(), ProtocolError> {
        if self.tombstone.identity_id != publication.publication().identity_id
            || self.tombstone.publication_id != publication.publication_id()
            || self.tombstone.content_id != publication.publication().content_id
        {
            return Err(ProtocolError::TombstoneMismatch);
        }
        Ok(())
    }

    /// Verifies that this tombstone is a newer revision of an earlier verdict.
    ///
    /// Revisions may skip numbers because a peer can observe only part of a
    /// tombstone history, but the same publication, uploader, and content ID
    /// must be preserved and the revision must increase.
    pub fn validate_successor_of(&self, previous: &Self) -> Result<(), ProtocolError> {
        if self.tombstone.identity_id != previous.tombstone.identity_id
            || self.tombstone.publication_id != previous.tombstone.publication_id
            || self.tombstone.content_id != previous.tombstone.content_id
            || self.tombstone.revision <= previous.tombstone.revision
        {
            return Err(ProtocolError::RevisionChainMismatch);
        }
        Ok(())
    }
}

fn append_unknown_fields(
    entries: &mut Vec<(Value, Value)>,
    unknown_fields: &[(u64, Value)],
) -> Result<(), ProtocolError> {
    let mut seen = std::collections::BTreeSet::new();
    for (key, value) in unknown_fields {
        if *key <= MAX_KNOWN_FIELD || !seen.insert(*key) {
            return Err(ProtocolError::InvalidFieldKey);
        }
        entries.push((Value::Unsigned(*key), value.clone()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SignedTombstone, TombstoneV1};
    use crate::publication::{Category, PublicationV1, SignedPublication};
    use nexus_crypto::{IdentityId, KeyPair};

    fn keypair() -> KeyPair {
        KeyPair::from_seed([7_u8; 32])
    }

    fn identity() -> IdentityId {
        IdentityId::from_public_key(&keypair().public_key())
    }

    fn tombstone() -> TombstoneV1 {
        TombstoneV1::new(identity(), [0x11_u8; 32], [0x22_u8; 32], 1, 1_700_000_000_000)
    }

    #[test]
    fn tombstone_signs_and_verifies_with_exact_bytes() {
        let signed = SignedTombstone::sign(&tombstone(), &keypair()).expect("sign");
        let parsed = SignedTombstone::parse(signed.raw()).expect("verify");
        assert_eq!(parsed.tombstone(), signed.tombstone());
        assert_eq!(parsed.raw(), signed.raw());
    }

    #[test]
    fn tombstone_must_match_the_publication_it_withdraws() {
        let publication = PublicationV1::new(
            identity(),
            [0x22_u8; 32],
            1,
            "notes.txt",
            "",
            Category::Documents,
            [0x33_u8; 32],
            1,
            1_700_000_000_000,
            "0.1.0",
        );
        let signed_publication = SignedPublication::sign(&publication, &keypair()).expect("sign publication");

        let matching = TombstoneV1::new(
            identity(),
            signed_publication.publication_id(),
            [0x22_u8; 32],
            1,
            1_700_000_000_000,
        );
        let signed_tombstone = SignedTombstone::sign(&matching, &keypair()).expect("sign tombstone");
        signed_tombstone
            .validate_for(&signed_publication)
            .expect("matching tombstone");

        let other = KeyPair::from_seed([8_u8; 32]);
        let other_tombstone = TombstoneV1::new(
            IdentityId::from_public_key(&other.public_key()),
            signed_publication.publication_id(),
            [0x22_u8; 32],
            1,
            1_700_000_000_000,
        );
        let other_signed = SignedTombstone::sign(&other_tombstone, &other).expect("sign other");
        assert!(other_signed.validate_for(&signed_publication).is_err());
    }

    #[test]
    fn tombstone_revisions_must_increase_without_changing_attribution() {
        let first = SignedTombstone::sign(&tombstone(), &keypair()).expect("first");
        let mut newer = tombstone();
        newer.revision = 3;
        let newer = SignedTombstone::sign(&newer, &keypair()).expect("newer");
        newer.validate_successor_of(&first).expect("newer revision");

        let mut changed_content = tombstone();
        changed_content.revision = 2;
        changed_content.content_id = [0x44_u8; 32];
        let changed_content = SignedTombstone::sign(&changed_content, &keypair()).expect("changed");
        assert!(changed_content.validate_successor_of(&first).is_err());

        let mut same_revision = tombstone();
        same_revision.unpublished_at_ms += 1;
        let same_revision = SignedTombstone::sign(&same_revision, &keypair()).expect("same");
        assert!(same_revision.validate_successor_of(&first).is_err());
    }
}
