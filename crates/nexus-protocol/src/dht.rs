//! DHT key derivation and bounded record validation.
//!
//! DHT values for publications, profiles, and tombstones are exact signed
//! envelopes. A record is accepted only when its signature verifies and the
//! domain-separated key derived from the signed identifier matches the DHT key
//! it was stored under. Provider keys are raw 32-byte content IDs.
//!
//! This module owns key derivation and record shape. It does not perform network
//! lookups, cache results, or decide publication policy.

use nexus_crypto::DomainTag;
use sha2::{Digest, Sha256};

use crate::envelope::SignedRecord;
use crate::error::ProtocolError;
use crate::profile::SignedProfile;
use crate::publication::SignedPublication;
use crate::tombstone::SignedTombstone;

/// Size of a DHT key and a provider content ID in bytes.
pub const DHT_KEY_BYTES: usize = 32;
/// Maximum DHT value size, matching the signed-envelope cap.
pub const DHT_MAX_VALUE_BYTES: usize = crate::MAX_SIGNED_RECORD_BYTES;
/// Maximum provider addresses accepted for one provider record.
pub const DHT_MAX_PROVIDER_ADDRESSES: usize = 8;
/// Maximum length of one provider multiaddress.
pub const DHT_MAX_PROVIDER_ADDRESS_BYTES: usize = 256;

/// The signed record families that may be published as DHT values.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DhtRecordKind {
    /// A signed publication record.
    Publication,
    /// A signed profile record.
    Profile,
    /// A signed tombstone record.
    Tombstone,
}

impl DhtRecordKind {
    /// Stable wire and diagnostics name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Publication => "publication",
            Self::Profile => "profile",
            Self::Tombstone => "tombstone",
        }
    }

    /// Parses a stable wire name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "publication" => Some(Self::Publication),
            "profile" => Some(Self::Profile),
            "tombstone" => Some(Self::Tombstone),
            _ => None,
        }
    }
}

/// A verified signed record paired with its derived DHT key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DhtRecord {
    kind: DhtRecordKind,
    key: [u8; DHT_KEY_BYTES],
    revision: u64,
    raw: Vec<u8>,
}

impl DhtRecord {
    /// Parses and verifies one exact signed envelope, then derives its DHT key.
    ///
    /// The record is accepted only when it is a valid v1 publication, profile,
    /// or tombstone whose signature verifies against the identity it names.
    pub fn parse(raw: &[u8]) -> Result<Self, ProtocolError> {
        if raw.is_empty() {
            return Err(ProtocolError::InvalidDhtRecord);
        }
        if raw.len() > DHT_MAX_VALUE_BYTES {
            return Err(ProtocolError::RecordTooLarge);
        }
        let record = SignedRecord::parse(raw)?;
        if let Ok(publication) = SignedPublication::from_record(record.clone()) {
            return Ok(Self {
                kind: DhtRecordKind::Publication,
                key: publication_dht_key(publication.publication_id()),
                revision: publication.publication().revision,
                raw: raw.to_vec(),
            });
        }
        if let Ok(tombstone) = SignedTombstone::from_record(record.clone()) {
            return Ok(Self {
                kind: DhtRecordKind::Tombstone,
                key: tombstone_dht_key(tombstone.tombstone().publication_id),
                revision: tombstone.tombstone().revision,
                raw: raw.to_vec(),
            });
        }
        if let Ok(profile) = SignedProfile::from_record(record) {
            return Ok(Self {
                kind: DhtRecordKind::Profile,
                key: profile_dht_key(profile.profile().identity_id.as_str()),
                revision: profile.profile().revision,
                raw: raw.to_vec(),
            });
        }
        Err(ProtocolError::InvalidDhtRecord)
    }

    /// Verifies the envelope and requires it to belong under `key`.
    pub fn verify_for_key(key: &[u8], raw: &[u8]) -> Result<Self, ProtocolError> {
        if key.len() != DHT_KEY_BYTES {
            return Err(ProtocolError::DhtKeyMismatch);
        }
        let record = Self::parse(raw)?;
        if record.key.as_slice() != key {
            return Err(ProtocolError::DhtKeyMismatch);
        }
        Ok(record)
    }

    /// Record family.
    #[must_use]
    pub const fn kind(&self) -> DhtRecordKind {
        self.kind
    }

    /// Derived DHT key.
    #[must_use]
    pub const fn key(&self) -> &[u8; DHT_KEY_BYTES] {
        &self.key
    }

    /// Signed revision of the record, always at least 1.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Exact signed envelope bytes, preserved without re-encoding.
    #[must_use]
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Classifies an inbound record against the record already stored under the
    /// same key.
    ///
    /// Revisions are monotonic per DHT key. A newer revision replaces the
    /// stored record, an identical revision is idempotent, and an older or
    /// conflicting revision is rejected so a peer cannot roll back or shadow a
    /// record with stale or divergent signed bytes.
    #[must_use]
    pub fn classify_update(existing: Option<&Self>, incoming: &Self) -> DhtRecordDecision {
        let Some(existing) = existing else {
            return DhtRecordDecision::Store;
        };
        if existing.key != incoming.key {
            return DhtRecordDecision::Conflict;
        }
        match incoming.revision.cmp(&existing.revision) {
            std::cmp::Ordering::Greater => DhtRecordDecision::Store,
            std::cmp::Ordering::Less => DhtRecordDecision::Stale,
            std::cmp::Ordering::Equal if incoming.raw == existing.raw => DhtRecordDecision::AlreadyStored,
            std::cmp::Ordering::Equal => DhtRecordDecision::Conflict,
        }
    }
}

/// Decision for an inbound record that may already be stored under its key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DhtRecordDecision {
    /// The incoming record is newer and should replace the stored bytes.
    Store,
    /// The incoming record is byte-identical to the stored record.
    AlreadyStored,
    /// The incoming record is older than the stored record.
    Stale,
    /// The incoming record has the same revision but different bytes, or the
    /// keys differ. It must not replace the stored record.
    Conflict,
}

/// Derives the DHT key for a publication ID.
#[must_use]
pub fn publication_dht_key(publication_id: [u8; DHT_KEY_BYTES]) -> [u8; DHT_KEY_BYTES] {
    hashed_key(DomainTag::DhtPublication, &publication_id)
}

/// Derives the DHT key for a tombstone under its publication ID.
#[must_use]
pub fn tombstone_dht_key(publication_id: [u8; DHT_KEY_BYTES]) -> [u8; DHT_KEY_BYTES] {
    hashed_key(DomainTag::DhtTombstone, &publication_id)
}

/// Derives the DHT key for a profile under its canonical identity ID text.
#[must_use]
pub fn profile_dht_key(identity_id: &str) -> [u8; DHT_KEY_BYTES] {
    hashed_key(DomainTag::DhtProfile, identity_id.as_bytes())
}

/// Returns the provider key for a content ID. Provider records use the raw
/// 32-byte content ID as required by the architecture.
#[must_use]
pub const fn content_provider_key(content_id: [u8; DHT_KEY_BYTES]) -> [u8; DHT_KEY_BYTES] {
    content_id
}

/// Validates a provider key: a raw 32-byte content ID, never a hashed value key.
pub fn validate_provider_key(key: &[u8]) -> Result<(), ProtocolError> {
    if key.len() == DHT_KEY_BYTES {
        Ok(())
    } else {
        Err(ProtocolError::InvalidDhtProvider)
    }
}

/// Validates a provider address count and per-address length.
///
/// An empty list is allowed: provider identity is authoritative, and addresses
/// are advisory routing hints that a lookup may resolve from the routing table.
pub fn validate_provider_addresses(lengths: &[usize]) -> Result<(), ProtocolError> {
    if lengths.len() > DHT_MAX_PROVIDER_ADDRESSES {
        return Err(ProtocolError::InvalidDhtProvider);
    }
    if lengths
        .iter()
        .any(|length| *length == 0 || *length > DHT_MAX_PROVIDER_ADDRESS_BYTES)
    {
        return Err(ProtocolError::InvalidDhtProvider);
    }
    Ok(())
}

fn hashed_key(tag: DomainTag, identifier: &[u8]) -> [u8; DHT_KEY_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(tag.as_bytes());
    hasher.update(identifier);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use nexus_crypto::{IdentityId, KeyPair};

    use super::{
        DHT_MAX_VALUE_BYTES, DhtRecord, DhtRecordDecision, DhtRecordKind, content_provider_key, profile_dht_key,
        publication_dht_key, tombstone_dht_key, validate_provider_addresses, validate_provider_key,
    };
    use crate::profile::{ProfileV1, SignedProfile};
    use crate::publication::{Category, PublicationV1, SignedPublication};
    use crate::tombstone::{SignedTombstone, TombstoneV1};

    fn publication() -> (SignedPublication, KeyPair) {
        let keypair = KeyPair::from_seed([71_u8; 32]);
        let identity_id = IdentityId::from_public_key(&keypair.public_key());
        let body = PublicationV1::new(
            identity_id,
            [7_u8; 32],
            1_024,
            "notes.txt",
            "A small note",
            Category::Documents,
            [8_u8; 32],
            1,
            1_700_000_000_000,
            "0.1.0",
        );
        (SignedPublication::sign(&body, &keypair).expect("sign"), keypair)
    }

    #[test]
    fn signed_records_derive_kind_and_key() {
        let (signed, keypair) = publication();
        let record = DhtRecord::parse(signed.raw()).expect("parse");
        assert_eq!(record.kind(), DhtRecordKind::Publication);
        assert_eq!(record.key(), &publication_dht_key(signed.publication_id()));
        assert_eq!(record.raw(), signed.raw());

        let tombstone = TombstoneV1::new(
            signed.publication().identity_id.clone(),
            signed.publication_id(),
            [7_u8; 32],
            1,
            1_700_000_100_000,
        );
        let signed_tombstone = SignedTombstone::sign(&tombstone, &keypair).expect("tombstone sign");
        let record = DhtRecord::parse(signed_tombstone.raw()).expect("tombstone parse");
        assert_eq!(record.kind(), DhtRecordKind::Tombstone);
        assert_eq!(record.key(), &tombstone_dht_key(signed.publication_id()));

        let profile_keypair = KeyPair::from_seed([72_u8; 32]);
        let identity_id = IdentityId::from_public_key(&profile_keypair.public_key());
        let profile = ProfileV1::new(identity_id.clone(), "Ada", "Local-first", 1, 1_700_000_000_000);
        let signed_profile = SignedProfile::sign(&profile, &profile_keypair).expect("profile sign");
        let record = DhtRecord::parse(signed_profile.raw()).expect("profile parse");
        assert_eq!(record.kind(), DhtRecordKind::Profile);
        assert_eq!(record.key(), &profile_dht_key(identity_id.as_str()));
    }

    #[test]
    fn rejects_wrong_keys_bad_signatures_and_malformed_values() {
        let (signed, _) = publication();
        let mut wrong_key = publication_dht_key(signed.publication_id());
        wrong_key[0] ^= 0xff;
        assert!(DhtRecord::verify_for_key(&wrong_key, signed.raw()).is_err());

        let mut tampered = signed.raw().to_vec();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        assert!(DhtRecord::parse(&tampered).is_err());
        assert!(DhtRecord::parse(&[]).is_err());
        assert!(DhtRecord::parse(&vec![0_u8; DHT_MAX_VALUE_BYTES + 1]).is_err());
    }

    #[test]
    fn revision_is_exposed_and_updates_are_monotonic() {
        let (signed, keypair) = publication();
        let first = DhtRecord::parse(signed.raw()).expect("parse");
        assert_eq!(first.revision(), 1);
        assert_eq!(DhtRecord::classify_update(None, &first), DhtRecordDecision::Store);
        assert_eq!(
            DhtRecord::classify_update(Some(&first), &first),
            DhtRecordDecision::AlreadyStored
        );

        let mut body = signed.publication().clone();
        body.revision = 2;
        body.supersedes = Some(signed.publication_id());
        let second = DhtRecord::parse(SignedPublication::sign(&body, &keypair).expect("sign revision").raw())
            .expect("parse revision");
        assert_eq!(second.revision(), 2);
        // A publication revision changes the signed body, so it gets a new
        // publication ID and therefore a distinct DHT key.
        assert_ne!(second.key(), first.key());

        let profile_keypair = KeyPair::from_seed([73_u8; 32]);
        let identity_id = IdentityId::from_public_key(&profile_keypair.public_key());
        let one = ProfileV1::new(identity_id.clone(), "Ada", "One", 1, 1_700_000_000_000);
        let two = ProfileV1::new(identity_id.clone(), "Ada", "Two", 2, 1_700_000_000_001);
        let one = DhtRecord::parse(SignedProfile::sign(&one, &profile_keypair).expect("one").raw()).expect("parse one");
        let two = DhtRecord::parse(SignedProfile::sign(&two, &profile_keypair).expect("two").raw()).expect("parse two");
        assert_eq!(one.key(), two.key());
        assert_eq!(DhtRecord::classify_update(Some(&one), &two), DhtRecordDecision::Store);
        assert_eq!(DhtRecord::classify_update(Some(&two), &one), DhtRecordDecision::Stale);

        let conflict = ProfileV1::new(identity_id, "Ada", "Other", 1, 1_700_000_000_002);
        let conflict = DhtRecord::parse(
            SignedProfile::sign(&conflict, &profile_keypair)
                .expect("conflict")
                .raw(),
        )
        .expect("parse conflict");
        assert_eq!(
            DhtRecord::classify_update(Some(&one), &conflict),
            DhtRecordDecision::Conflict
        );
    }

    #[test]
    fn provider_keys_use_raw_content_ids() {
        let content_id = [3_u8; 32];
        assert_eq!(content_provider_key(content_id), content_id);
        assert!(validate_provider_key(&content_id).is_ok());
        assert!(validate_provider_key(&[3_u8; 31]).is_err());
        assert!(validate_provider_addresses(&[10, 20]).is_ok());
        assert!(validate_provider_addresses(&[]).is_ok());
        assert!(validate_provider_addresses(&[0]).is_err());
    }
}
