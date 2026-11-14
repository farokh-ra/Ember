//! Canonical chunk-manifest schema v1.
//!
//! v1 uses a flat, manifest-authenticated chunk list. The publication record
//! signs the domain-separated manifest root; the manifest itself carries the
//! content ID, total size, fixed chunk size, and one `(length, SHA-256)` entry
//! per chunk. A manifest never contains file bytes.

use nexus_crypto::{CborLimits, DomainTag, Value, decode, domain_sha256};

use crate::error::ProtocolError;
use crate::fields::{array, bytes32, nonnegative_keys, unsigned};
use crate::publication::{CHUNK_SIZE_V1, MAX_FILE_SIZE, PublicationV1};

/// Manifest schema major version.
pub const MANIFEST_SCHEMA_VERSION: u64 = 1;
/// Maximum canonical manifest size: 256 KiB.
pub const MAX_MANIFEST_BYTES: usize = 262_144;
/// Maximum chunk entries for a 4 GiB file at the v1 chunk size.
pub const MAX_CHUNKS: usize = 4_096;

const MAX_KNOWN_FIELD: u64 = 5;
const MANIFEST_LIMITS: CborLimits = CborLimits::new(8, 20_000, MAX_MANIFEST_BYTES);

/// One ordered chunk entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkEntry {
    /// Chunk length in bytes. Every chunk except the last is exactly 1 MiB.
    pub length: u64,
    /// SHA-256 of the exact chunk bytes.
    pub sha256: [u8; 32],
}

impl ChunkEntry {
    /// Creates a chunk entry.
    #[must_use]
    pub const fn new(length: u64, sha256: [u8; 32]) -> Self {
        Self { length, sha256 }
    }
}

/// A chunk manifest body with the v1 logical fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestV1 {
    /// SHA-256 of the complete file bytes.
    pub content_id: [u8; 32],
    /// Complete file size in bytes.
    pub total_size: u64,
    /// Fixed v1 chunk size.
    pub chunk_size: u64,
    /// Ordered chunk entries.
    pub chunks: Vec<ChunkEntry>,
    unknown_fields: Vec<(u64, Value)>,
}

impl ManifestV1 {
    /// Creates a manifest with the fixed v1 chunk size.
    #[must_use]
    pub fn new(content_id: [u8; 32], total_size: u64, chunks: Vec<ChunkEntry>) -> Self {
        Self {
            content_id,
            total_size,
            chunk_size: CHUNK_SIZE_V1,
            chunks,
            unknown_fields: Vec::new(),
        }
    }

    /// Returns preserved unknown fields from a newer valid minor record.
    #[must_use]
    pub fn unknown_fields(&self) -> &[(u64, Value)] {
        &self.unknown_fields
    }

    /// Returns the number of chunk entries.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Returns one chunk entry by index.
    #[must_use]
    pub fn chunk(&self, index: usize) -> Option<&ChunkEntry> {
        self.chunks.get(index)
    }

    /// Encodes the canonical manifest body.
    pub fn encode_body(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate_shape()?;
        let chunks = self
            .chunks
            .iter()
            .map(|entry| Value::Array(vec![Value::Unsigned(entry.length), Value::Bytes(entry.sha256.to_vec())]))
            .collect();
        let mut entries = vec![
            (Value::Unsigned(1), Value::Unsigned(MANIFEST_SCHEMA_VERSION)),
            (Value::Unsigned(2), Value::Bytes(self.content_id.to_vec())),
            (Value::Unsigned(3), Value::Unsigned(self.total_size)),
            (Value::Unsigned(4), Value::Unsigned(self.chunk_size)),
            (Value::Unsigned(5), Value::Array(chunks)),
        ];
        append_unknown_fields(&mut entries, &self.unknown_fields)?;

        let body = Value::Map(entries).to_canonical_bytes_with_limits(MANIFEST_LIMITS)?;
        if body.len() > MAX_MANIFEST_BYTES {
            return Err(ProtocolError::ManifestTooLarge);
        }
        Ok(body)
    }

    /// Parses a canonical v1 manifest body and preserves unknown minor fields.
    pub fn parse_body(body: &[u8]) -> Result<Self, ProtocolError> {
        if body.len() > MAX_MANIFEST_BYTES {
            return Err(ProtocolError::ManifestTooLarge);
        }
        let value = decode(body, MANIFEST_LIMITS)?;
        let keys = nonnegative_keys(&value)?;
        let schema = unsigned(&value, 1)?;
        if schema != MANIFEST_SCHEMA_VERSION {
            return Err(ProtocolError::UnsupportedVersion(schema));
        }

        let content_id = bytes32(&value, 2)?;
        let total_size = unsigned(&value, 3)?;
        let chunk_size = unsigned(&value, 4)?;
        let chunk_values = array(&value, 5)?;
        if chunk_values.len() > MAX_CHUNKS {
            return Err(ProtocolError::InvalidField(5));
        }
        let mut chunks = Vec::with_capacity(chunk_values.len());
        for chunk in chunk_values {
            let items = chunk.as_array().ok_or(ProtocolError::InvalidField(5))?;
            if items.len() != 2 {
                return Err(ProtocolError::InvalidField(5));
            }
            let length = items[0].as_unsigned().ok_or(ProtocolError::InvalidField(5))?;
            let sha256 = items[1]
                .as_bytes()
                .ok_or(ProtocolError::InvalidField(5))?
                .try_into()
                .map_err(|_| ProtocolError::InvalidField(5))?;
            chunks.push(ChunkEntry { length, sha256 });
        }

        let mut unknown_fields = Vec::new();
        for key in keys {
            if key > MAX_KNOWN_FIELD {
                let value = value.get_key(key).ok_or(ProtocolError::MissingField(key))?.clone();
                unknown_fields.push((key, value));
            }
        }

        let manifest = Self {
            content_id,
            total_size,
            chunk_size,
            chunks,
            unknown_fields,
        };
        manifest.validate_shape()?;
        Ok(manifest)
    }

    /// Returns the domain-separated root of the canonical manifest bytes.
    pub fn root(&self) -> Result<[u8; 32], ProtocolError> {
        Ok(manifest_root(&self.encode_body()?))
    }

    /// Verifies the computed root against a publication record value.
    pub fn verify_root(&self, expected: [u8; 32]) -> Result<(), ProtocolError> {
        if self.root()? != expected {
            return Err(ProtocolError::ManifestMismatch);
        }
        Ok(())
    }

    /// Verifies that this manifest matches the signed publication metadata.
    pub fn validate_for_publication(&self, publication: &PublicationV1) -> Result<(), ProtocolError> {
        if self.content_id != publication.content_id
            || self.total_size != publication.size
            || self.chunk_size != publication.chunk_size
            || self.root()? != publication.manifest_root
        {
            return Err(ProtocolError::ManifestMismatch);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), ProtocolError> {
        if self.total_size == 0 || self.total_size > MAX_FILE_SIZE {
            return Err(ProtocolError::InvalidField(3));
        }
        if self.chunk_size != CHUNK_SIZE_V1 {
            return Err(ProtocolError::InvalidField(4));
        }
        if self.chunks.is_empty() || self.chunks.len() > MAX_CHUNKS {
            return Err(ProtocolError::InvalidField(5));
        }
        let expected_chunks = self.total_size.div_ceil(CHUNK_SIZE_V1);
        if u64::try_from(self.chunks.len()).ok() != Some(expected_chunks) {
            return Err(ProtocolError::InvalidField(5));
        }

        let mut total = 0_u64;
        let last = self.chunks.len() - 1;
        for (index, entry) in self.chunks.iter().enumerate() {
            if entry.length == 0 || entry.length > CHUNK_SIZE_V1 {
                return Err(ProtocolError::InvalidField(5));
            }
            if index != last && entry.length != CHUNK_SIZE_V1 {
                return Err(ProtocolError::InvalidField(5));
            }
            total = total.checked_add(entry.length).ok_or(ProtocolError::InvalidField(5))?;
        }
        if total != self.total_size {
            return Err(ProtocolError::InvalidField(5));
        }
        Ok(())
    }
}

/// Hashes exact canonical manifest bytes with the manifest-root tag.
#[must_use]
pub fn manifest_root(body: &[u8]) -> [u8; 32] {
    domain_sha256(DomainTag::Manifest, body)
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
    use nexus_crypto::{sha256, to_hex};

    use super::{ChunkEntry, MAX_CHUNKS, ManifestV1, manifest_root};
    use crate::publication::{Category, PublicationV1};
    use nexus_crypto::{IdentityId, KeyPair};

    fn chunks() -> Vec<ChunkEntry> {
        vec![
            ChunkEntry::new(super::CHUNK_SIZE_V1, sha256(b"a")),
            ChunkEntry::new(5, sha256(b"bcdef")),
        ]
    }

    #[test]
    fn manifest_round_trips_and_binds_its_publication() {
        let manifest = ManifestV1::new([0x11_u8; 32], super::CHUNK_SIZE_V1 + 5, chunks());
        let body = manifest.encode_body().expect("encode");
        let parsed = ManifestV1::parse_body(&body).expect("parse");
        assert_eq!(parsed, manifest);
        assert_eq!(manifest.root().expect("root"), manifest_root(&body));
        manifest
            .verify_root(manifest.root().expect("root"))
            .expect("root match");

        let keypair = KeyPair::from_seed([7_u8; 32]);
        let identity = IdentityId::from_public_key(&keypair.public_key());
        let publication = PublicationV1::new(
            identity,
            [0x11_u8; 32],
            super::CHUNK_SIZE_V1 + 5,
            "chunked.bin",
            "",
            Category::Data,
            manifest.root().expect("root"),
            1,
            1_700_000_000_000,
            "0.1.0",
        );
        manifest
            .validate_for_publication(&publication)
            .expect("publication match");
    }

    #[test]
    fn rejects_bad_chunk_counts_lengths_and_sizes() {
        let root = [0x22_u8; 32];
        let mut bad = ManifestV1::new([0x11_u8; 32], 1, Vec::new());
        assert!(bad.encode_body().is_err());
        bad.chunks = vec![ChunkEntry::new(1, root)];
        assert!(bad.encode_body().is_ok());

        let bad = ManifestV1::new([0x11_u8; 32], 2, vec![ChunkEntry::new(1, root)]);
        assert!(bad.encode_body().is_err());

        let bad = ManifestV1::new(
            [0x11_u8; 32],
            super::CHUNK_SIZE_V1 + 1,
            vec![ChunkEntry::new(super::CHUNK_SIZE_V1, root), ChunkEntry::new(2, root)],
        );
        assert!(bad.encode_body().is_err());

        let bad = ManifestV1::new(
            [0x11_u8; 32],
            super::CHUNK_SIZE_V1,
            vec![ChunkEntry::new(super::CHUNK_SIZE_V1 - 1, root)],
        );
        assert!(bad.encode_body().is_err());

        let too_many = ManifestV1::new(
            [0x11_u8; 32],
            u64::try_from(MAX_CHUNKS + 1).expect("count") * super::CHUNK_SIZE_V1,
            vec![ChunkEntry::new(super::CHUNK_SIZE_V1, root); MAX_CHUNKS + 1],
        );
        assert!(too_many.encode_body().is_err());
    }

    #[test]
    fn exact_manifest_bytes_are_stable() {
        let manifest = ManifestV1::new([0x11_u8; 32], 5, vec![ChunkEntry::new(5, [0x33_u8; 32])]);
        let body = manifest.encode_body().expect("encode");
        assert_eq!(
            to_hex(&body),
            "a5010102582011111111111111111111111111111111111111111111111111111111111111110305041a001000000581820558203333333333333333333333333333333333333333333333333333333333333333"
        );
        assert_eq!(
            to_hex(&manifest.root().expect("root")),
            "257324e565dba2a5440600ac15ad3ab1ce28bb1efad8a32c52c5d073a572b16f"
        );
    }
}
