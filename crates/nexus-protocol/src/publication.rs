//! Signed publication record schema v1.

use nexus_crypto::{
    CborLimits, DomainTag, IdentityId, KeyPair, Value, decode_signed_record, domain_sha256, normalize_text_bytes,
};

use crate::envelope::{MAX_SIGNED_BODY_BYTES, SignedRecord};
use crate::error::ProtocolError;
use crate::fields::{array, bytes32, nonnegative_keys, nullable_bytes32, text, unsigned};
use crate::manifest::ManifestV1;

/// Publication schema major version.
pub const PUBLICATION_SCHEMA_VERSION: u64 = 1;
/// Fixed v1 chunk size: 1 MiB.
pub const CHUNK_SIZE_V1: u64 = 1_048_576;
/// Maximum file size: 4 GiB.
pub const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024 * 1024;
/// Maximum display-filename length in UTF-8 bytes.
pub const MAX_DISPLAY_FILENAME_BYTES: usize = 255;
/// Maximum description length in UTF-8 bytes.
pub const MAX_DESCRIPTION_BYTES: usize = 4_096;
/// Maximum number of tags.
pub const MAX_TAGS: usize = 10;
/// Maximum tag length in UTF-8 bytes.
pub const MAX_TAG_BYTES: usize = 32;
/// Maximum app-version length in UTF-8 bytes.
pub const MAX_APP_VERSION_BYTES: usize = 64;

const MAX_KNOWN_FIELD: u64 = 16;

/// Closed v1 publication category taxonomy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum Category {
    /// Documents, PDFs, office files, e-books.
    Documents = 0,
    /// Photos, graphics, diagrams.
    Images = 1,
    /// Music, speech, sound recordings.
    Audio = 2,
    /// Films, clips, recordings.
    Video = 3,
    /// Applications, installers, scripts, source archives.
    Software = 4,
    /// Datasets, databases, structured exports.
    Data = 5,
    /// Compressed bundles and backups.
    Archives = 6,
    /// Content outside the other categories.
    Other = 7,
}

impl Category {
    /// Returns the stable wire code.
    #[must_use]
    pub const fn code(self) -> u64 {
        self as u64
    }

    /// Maps a wire code to the closed v1 enum.
    #[must_use]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Documents),
            1 => Some(Self::Images),
            2 => Some(Self::Audio),
            3 => Some(Self::Video),
            4 => Some(Self::Software),
            5 => Some(Self::Data),
            6 => Some(Self::Archives),
            7 => Some(Self::Other),
            _ => None,
        }
    }
}

/// A publication body with the v1 logical fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationV1 {
    /// Public identity ID of the uploader.
    pub identity_id: IdentityId,
    /// SHA-256 of the complete file bytes.
    pub content_id: [u8; 32],
    /// Complete file size in bytes.
    pub size: u64,
    /// Display filename, at most 255 UTF-8 bytes.
    pub display_filename: String,
    /// Description, at most 4,096 UTF-8 bytes.
    pub description: String,
    /// Closed v1 category.
    pub category: Category,
    /// Up to ten tags, each at most 32 UTF-8 bytes.
    pub tags: Vec<String>,
    /// Informational publication time in Unix milliseconds.
    pub published_at_ms: u64,
    /// Domain-separated chunk-manifest root.
    pub manifest_root: [u8; 32],
    /// Fixed v1 chunk size.
    pub chunk_size: u64,
    /// Starts at 1 and increases for metadata edits.
    pub revision: u64,
    /// Previous publication ID for a revision.
    pub supersedes: Option<[u8; 32]>,
    /// Optional modified-from content ID, informational only.
    pub parent_content_id: Option<[u8; 32]>,
    /// Optional modified-from publication ID, informational only.
    pub parent_publication_id: Option<[u8; 32]>,
    /// Record-producing app semantic version.
    pub app_version: String,
    unknown_fields: Vec<(u64, Value)>,
}

impl PublicationV1 {
    /// Creates a minimal v1 publication body.
    ///
    /// The argument count mirrors the v1 record fields so callers cannot
    /// accidentally omit required signed metadata.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity_id: IdentityId,
        content_id: [u8; 32],
        size: u64,
        display_filename: impl Into<String>,
        description: impl Into<String>,
        category: Category,
        manifest_root: [u8; 32],
        revision: u64,
        published_at_ms: u64,
        app_version: impl Into<String>,
    ) -> Self {
        Self {
            identity_id,
            content_id,
            size,
            display_filename: display_filename.into(),
            description: description.into(),
            category,
            tags: Vec::new(),
            published_at_ms,
            manifest_root,
            chunk_size: CHUNK_SIZE_V1,
            revision,
            supersedes: None,
            parent_content_id: None,
            parent_publication_id: None,
            app_version: app_version.into(),
            unknown_fields: Vec::new(),
        }
    }

    /// Sets tags.
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Sets the superseded publication ID.
    #[must_use]
    pub fn with_supersedes(mut self, publication_id: [u8; 32]) -> Self {
        self.supersedes = Some(publication_id);
        self
    }

    /// Sets the optional parent references.
    #[must_use]
    pub fn with_parents(mut self, content_id: Option<[u8; 32]>, publication_id: Option<[u8; 32]>) -> Self {
        self.parent_content_id = content_id;
        self.parent_publication_id = publication_id;
        self
    }

    /// Returns preserved unknown fields from a newer valid minor record.
    #[must_use]
    pub fn unknown_fields(&self) -> &[(u64, Value)] {
        &self.unknown_fields
    }

    /// Encodes the canonical publication body.
    pub fn encode_body(&self) -> Result<Vec<u8>, ProtocolError> {
        if self.size == 0 || self.size > MAX_FILE_SIZE {
            return Err(ProtocolError::InvalidField(4));
        }
        if self.chunk_size != CHUNK_SIZE_V1 {
            return Err(ProtocolError::InvalidField(11));
        }
        if self.revision == 0 {
            return Err(ProtocolError::InvalidField(12));
        }
        if (self.revision == 1 && self.supersedes.is_some()) || (self.revision > 1 && self.supersedes.is_none()) {
            return Err(ProtocolError::InvalidField(13));
        }
        let display_filename = normalize_required_bytes(&self.display_filename, 5, MAX_DISPLAY_FILENAME_BYTES)?;
        let description = normalize_optional_bytes(&self.description, 6, MAX_DESCRIPTION_BYTES)?;
        if self.tags.len() > MAX_TAGS {
            return Err(ProtocolError::InvalidField(8));
        }
        let mut tags = Vec::with_capacity(self.tags.len());
        for tag in &self.tags {
            tags.push(Value::Text(normalize_required_bytes(tag, 8, MAX_TAG_BYTES)?));
        }
        let app_version = normalize_required_bytes(&self.app_version, 16, MAX_APP_VERSION_BYTES)?;

        let mut entries = vec![
            (Value::Unsigned(1), Value::Unsigned(PUBLICATION_SCHEMA_VERSION)),
            (Value::Unsigned(2), Value::Text(self.identity_id.as_str().to_owned())),
            (Value::Unsigned(3), Value::Bytes(self.content_id.to_vec())),
            (Value::Unsigned(4), Value::Unsigned(self.size)),
            (Value::Unsigned(5), Value::Text(display_filename)),
            (Value::Unsigned(6), Value::Text(description)),
            (Value::Unsigned(7), Value::Unsigned(self.category.code())),
            (Value::Unsigned(8), Value::Array(tags)),
            (Value::Unsigned(9), Value::Unsigned(self.published_at_ms)),
            (Value::Unsigned(10), Value::Bytes(self.manifest_root.to_vec())),
            (Value::Unsigned(11), Value::Unsigned(self.chunk_size)),
            (Value::Unsigned(12), Value::Unsigned(self.revision)),
            (Value::Unsigned(13), optional_bytes32(self.supersedes)),
            (Value::Unsigned(14), optional_bytes32(self.parent_content_id)),
            (Value::Unsigned(15), optional_bytes32(self.parent_publication_id)),
            (Value::Unsigned(16), Value::Text(app_version)),
        ];
        append_unknown_fields(&mut entries, &self.unknown_fields, MAX_KNOWN_FIELD)?;

        let body = Value::Map(entries).to_canonical_bytes_with_limits(CborLimits::signed_record())?;
        if body.len() > MAX_SIGNED_BODY_BYTES {
            return Err(ProtocolError::RecordTooLarge);
        }
        Ok(body)
    }

    /// Parses a canonical v1 publication body and preserves unknown minor fields.
    pub fn parse_body(body: &[u8]) -> Result<Self, ProtocolError> {
        let value = decode_signed_record(body)?;
        let keys = nonnegative_keys(&value)?;
        let schema = unsigned(&value, 1)?;
        if schema != PUBLICATION_SCHEMA_VERSION {
            return Err(ProtocolError::UnsupportedVersion(schema));
        }

        let identity_id_text = text(&value, 2)?;
        let identity_id = IdentityId::parse(identity_id_text).map_err(|_| ProtocolError::InvalidField(2))?;
        let content_id = bytes32(&value, 3)?;
        let size = unsigned(&value, 4)?;
        if size == 0 || size > MAX_FILE_SIZE {
            return Err(ProtocolError::InvalidField(4));
        }
        let display_filename = parse_required_bytes(&value, 5, MAX_DISPLAY_FILENAME_BYTES)?;
        let description = parse_optional_bytes(&value, 6, MAX_DESCRIPTION_BYTES)?;
        let category = Category::from_code(unsigned(&value, 7)?).ok_or(ProtocolError::InvalidField(7))?;
        let tag_values = array(&value, 8)?;
        if tag_values.len() > MAX_TAGS {
            return Err(ProtocolError::InvalidField(8));
        }
        let mut tags = Vec::with_capacity(tag_values.len());
        for tag in tag_values {
            let tag = tag.as_text().ok_or(ProtocolError::InvalidField(8))?;
            let normalized = normalize_text_bytes(tag, MAX_TAG_BYTES).map_err(|_| ProtocolError::InvalidField(8))?;
            if normalized.is_empty() || normalized != tag {
                return Err(ProtocolError::InvalidField(8));
            }
            tags.push(tag.to_owned());
        }
        let published_at_ms = unsigned(&value, 9)?;
        let manifest_root = bytes32(&value, 10)?;
        let chunk_size = unsigned(&value, 11)?;
        if chunk_size != CHUNK_SIZE_V1 {
            return Err(ProtocolError::InvalidField(11));
        }
        let revision = unsigned(&value, 12)?;
        if revision == 0 {
            return Err(ProtocolError::InvalidField(12));
        }
        let supersedes = nullable_bytes32(&value, 13)?;
        if (revision == 1 && supersedes.is_some()) || (revision > 1 && supersedes.is_none()) {
            return Err(ProtocolError::InvalidField(13));
        }
        let parent_content_id = nullable_bytes32(&value, 14)?;
        let parent_publication_id = nullable_bytes32(&value, 15)?;
        let app_version = parse_required_bytes(&value, 16, MAX_APP_VERSION_BYTES)?;

        let mut unknown_fields = Vec::new();
        for key in keys {
            if key > MAX_KNOWN_FIELD {
                let value = value.get_key(key).ok_or(ProtocolError::MissingField(key))?.clone();
                unknown_fields.push((key, value));
            }
        }

        Ok(Self {
            identity_id,
            content_id,
            size,
            display_filename,
            description,
            category,
            tags,
            published_at_ms,
            manifest_root,
            chunk_size,
            revision,
            supersedes,
            parent_content_id,
            parent_publication_id,
            app_version,
            unknown_fields,
        })
    }

    /// Verifies that this publication is a metadata-only successor of
    /// `previous`.
    ///
    /// A revision keeps the same uploader, content ID, size, chunk size, and
    /// manifest root. It must increase the revision by exactly one and name the
    /// previous publication ID in `supersedes`.
    pub fn validate_successor_of(&self, previous: &SignedPublication) -> Result<(), ProtocolError> {
        let prior = previous.publication();
        let expected_revision = prior
            .revision
            .checked_add(1)
            .ok_or(ProtocolError::RevisionChainMismatch)?;
        if self.identity_id != prior.identity_id
            || self.content_id != prior.content_id
            || self.size != prior.size
            || self.manifest_root != prior.manifest_root
            || self.chunk_size != prior.chunk_size
            || self.revision != expected_revision
            || self.supersedes != Some(previous.publication_id())
        {
            return Err(ProtocolError::RevisionChainMismatch);
        }
        Ok(())
    }

    /// Verifies that a parsed manifest matches this publication's signed
    /// content ID, size, chunk size, and manifest root.
    pub fn validate_manifest(&self, manifest: &ManifestV1) -> Result<(), ProtocolError> {
        manifest.validate_for_publication(self)
    }

    /// Signs this publication and returns the exact-byte record.
    pub fn sign(&self, keypair: &KeyPair) -> Result<SignedPublication, ProtocolError> {
        if IdentityId::from_public_key(&keypair.public_key()) != self.identity_id {
            return Err(ProtocolError::IdentityKeyMismatch);
        }
        let body = self.encode_body()?;
        let record = SignedRecord::sign(body, DomainTag::Publication, keypair)?;
        let publication = Self::parse_body(record.body_bytes())?;
        Ok(SignedPublication { record, publication })
    }
}

/// A verified publication record that keeps its exact envelope bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedPublication {
    record: SignedRecord,
    publication: PublicationV1,
}

impl SignedPublication {
    /// Signs a publication body.
    pub fn sign(publication: &PublicationV1, keypair: &KeyPair) -> Result<Self, ProtocolError> {
        publication.sign(keypair)
    }

    /// Parses and verifies a publication record from exact envelope bytes.
    pub fn parse(raw: &[u8]) -> Result<Self, ProtocolError> {
        let record = SignedRecord::parse(raw)?;
        Self::from_record(record)
    }

    /// Verifies an already-parsed signed record.
    pub fn from_record(record: SignedRecord) -> Result<Self, ProtocolError> {
        let publication = PublicationV1::parse_body(record.body_bytes())?;
        let public_key = publication
            .identity_id
            .public_key()
            .map_err(|_| ProtocolError::InvalidField(2))?;
        record.verify(DomainTag::Publication, &public_key)?;
        Ok(Self { record, publication })
    }

    /// Returns the verified publication body.
    #[must_use]
    pub fn publication(&self) -> &PublicationV1 {
        &self.publication
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

    /// Returns the stable publication ID over the exact signed body bytes.
    #[must_use]
    pub fn publication_id(&self) -> [u8; 32] {
        publication_id(self.record.body_bytes())
    }
}

/// Hashes exact canonical publication body bytes with the publication-ID tag.
#[must_use]
pub fn publication_id(body: &[u8]) -> [u8; 32] {
    domain_sha256(DomainTag::PublicationId, body)
}

fn normalize_required_bytes(value: &str, field: u64, maximum: usize) -> Result<String, ProtocolError> {
    let normalized = normalize_text_bytes(value, maximum).map_err(|_| ProtocolError::InvalidField(field))?;
    if normalized.is_empty() {
        return Err(ProtocolError::InvalidField(field));
    }
    Ok(normalized)
}

fn normalize_optional_bytes(value: &str, field: u64, maximum: usize) -> Result<String, ProtocolError> {
    normalize_text_bytes(value, maximum).map_err(|_| ProtocolError::InvalidField(field))
}

fn parse_required_bytes(value: &Value, field: u64, maximum: usize) -> Result<String, ProtocolError> {
    let text = text(value, field)?;
    let normalized = normalize_text_bytes(text, maximum).map_err(|_| ProtocolError::InvalidField(field))?;
    if normalized.is_empty() || normalized != text {
        return Err(ProtocolError::InvalidField(field));
    }
    Ok(text.to_owned())
}

fn parse_optional_bytes(value: &Value, field: u64, maximum: usize) -> Result<String, ProtocolError> {
    let text = text(value, field)?;
    let normalized = normalize_text_bytes(text, maximum).map_err(|_| ProtocolError::InvalidField(field))?;
    if normalized != text {
        return Err(ProtocolError::InvalidField(field));
    }
    Ok(text.to_owned())
}

fn optional_bytes32(value: Option<[u8; 32]>) -> Value {
    value.map_or(Value::Null, |bytes| Value::Bytes(bytes.to_vec()))
}

fn append_unknown_fields(
    entries: &mut Vec<(Value, Value)>,
    unknown_fields: &[(u64, Value)],
    max_known: u64,
) -> Result<(), ProtocolError> {
    let mut seen = std::collections::BTreeSet::new();
    for (key, value) in unknown_fields {
        if *key <= max_known || !seen.insert(*key) {
            return Err(ProtocolError::InvalidFieldKey);
        }
        entries.push((Value::Unsigned(*key), value.clone()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use nexus_crypto::{IdentityId, KeyPair};

    use super::{Category, MAX_TAG_BYTES, PublicationV1, SignedPublication};

    fn publication() -> (PublicationV1, KeyPair) {
        let keypair = KeyPair::from_seed([61_u8; 32]);
        let identity_id = IdentityId::from_public_key(&keypair.public_key());
        let publication = PublicationV1::new(
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
        )
        .with_tags(vec!["notes".to_owned(), "local-first".to_owned()]);
        (publication, keypair)
    }

    #[test]
    fn publication_signs_and_verifies_with_stable_publication_id() {
        let (publication, keypair) = publication();
        let signed = SignedPublication::sign(&publication, &keypair).expect("sign");
        let id = signed.publication_id();
        let parsed = SignedPublication::parse(signed.raw()).expect("verify");
        assert_eq!(parsed.publication(), &publication);
        assert_eq!(parsed.publication_id(), id);
        assert_eq!(parsed.raw(), signed.raw());
    }

    #[test]
    fn rejects_bad_size_chunk_revision_category_and_tags() {
        let (publication, keypair) = publication();
        let mut bad = publication.clone();
        bad.size = 0;
        assert!(bad.sign(&keypair).is_err());
        let mut bad = publication.clone();
        bad.size = super::MAX_FILE_SIZE + 1;
        assert!(bad.sign(&keypair).is_err());
        let mut bad = publication.clone();
        bad.chunk_size = 1;
        assert!(bad.sign(&keypair).is_err());
        let mut bad = publication.clone();
        bad.revision = 0;
        assert!(bad.sign(&keypair).is_err());
        let mut bad = publication;
        bad.tags = vec!["x".repeat(MAX_TAG_BYTES + 1)];
        assert!(bad.sign(&keypair).is_err());
    }

    #[test]
    fn rejects_changed_body_with_old_signature() {
        let (publication, keypair) = publication();
        let signed = SignedPublication::sign(&publication, &keypair).expect("sign");
        let mut body = signed.record().body_bytes().to_vec();
        let position = body
            .windows(9)
            .position(|window| window == b"notes.txt")
            .expect("filename");
        body[position] = b'm';
        let envelope = nexus_crypto::Value::Map(vec![
            (nexus_crypto::Value::Unsigned(1), nexus_crypto::Value::Bytes(body)),
            (
                nexus_crypto::Value::Unsigned(2),
                nexus_crypto::Value::Bytes(signed.record().signature().as_bytes().to_vec()),
            ),
        ])
        .to_canonical_bytes()
        .expect("envelope");
        let record = crate::envelope::SignedRecord::parse(&envelope).expect("parse");
        assert!(SignedPublication::from_record(record).is_err());
    }
}
