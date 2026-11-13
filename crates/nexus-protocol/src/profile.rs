//! Signed profile record schema v1.

use nexus_crypto::{CborLimits, DomainTag, IdentityId, KeyPair, Value, decode_signed_record, normalize_text_chars};

use crate::envelope::{MAX_SIGNED_BODY_BYTES, SignedRecord};
use crate::error::ProtocolError;
use crate::fields::{array, nonnegative_keys, nullable_bytes32, text, unsigned};

/// Profile schema major version.
pub const PROFILE_SCHEMA_VERSION: u64 = 1;
/// Minimum display-name length in Unicode scalar values.
pub const MIN_DISPLAY_NAME_CHARS: usize = 1;
/// Maximum display-name length in Unicode scalar values.
pub const MAX_DISPLAY_NAME_CHARS: usize = 64;
/// Minimum username-label length in Unicode scalar values.
pub const MIN_USERNAME_CHARS: usize = 3;
/// Maximum username-label length in Unicode scalar values.
pub const MAX_USERNAME_CHARS: usize = 32;
/// Maximum bio length in Unicode scalar values.
pub const MAX_BIO_CHARS: usize = 500;
/// Maximum publication references in one profile.
pub const MAX_PROFILE_PUBLICATION_REFS: usize = 64;

const MAX_KNOWN_FIELD: u64 = 9;

/// A profile body with the v1 logical fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileV1 {
    /// Public identity ID of the profile owner.
    pub identity_id: IdentityId,
    /// Display name, 1-64 Unicode scalar values.
    pub display_name: String,
    /// Optional non-unique username label, 3-32 Unicode scalar values.
    pub username: Option<String>,
    /// Bio, up to 500 Unicode scalar values.
    pub bio: String,
    /// Optional avatar content ID.
    pub avatar_content_id: Option<[u8; 32]>,
    /// Publication IDs referenced by this profile.
    pub publication_refs: Vec<[u8; 32]>,
    /// Starts at 1 and increases for profile edits.
    pub revision: u64,
    /// Informational update time in Unix milliseconds.
    pub updated_at_ms: u64,
    unknown_fields: Vec<(u64, Value)>,
}

impl ProfileV1 {
    /// Creates a minimal v1 profile body.
    #[must_use]
    pub fn new(
        identity_id: IdentityId,
        display_name: impl Into<String>,
        bio: impl Into<String>,
        revision: u64,
        updated_at_ms: u64,
    ) -> Self {
        Self {
            identity_id,
            display_name: display_name.into(),
            username: None,
            bio: bio.into(),
            avatar_content_id: None,
            publication_refs: Vec::new(),
            revision,
            updated_at_ms,
            unknown_fields: Vec::new(),
        }
    }

    /// Sets the optional username label.
    #[must_use]
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Sets the optional avatar content ID.
    #[must_use]
    pub fn with_avatar(mut self, content_id: [u8; 32]) -> Self {
        self.avatar_content_id = Some(content_id);
        self
    }

    /// Sets publication references.
    #[must_use]
    pub fn with_publications(mut self, publication_refs: Vec<[u8; 32]>) -> Self {
        self.publication_refs = publication_refs;
        self
    }

    /// Returns preserved unknown fields from a newer valid minor record.
    #[must_use]
    pub fn unknown_fields(&self) -> &[(u64, Value)] {
        &self.unknown_fields
    }

    /// Encodes the canonical profile body.
    pub fn encode_body(&self) -> Result<Vec<u8>, ProtocolError> {
        if self.revision == 0 {
            return Err(ProtocolError::InvalidField(8));
        }
        let display_name =
            normalize_required_chars(&self.display_name, 3, MIN_DISPLAY_NAME_CHARS, MAX_DISPLAY_NAME_CHARS)?;
        let username = match &self.username {
            Some(username) => Some(normalize_required_chars(
                username,
                4,
                MIN_USERNAME_CHARS,
                MAX_USERNAME_CHARS,
            )?),
            None => None,
        };
        let bio = normalize_optional_chars(&self.bio, 5, MAX_BIO_CHARS)?;
        if self.publication_refs.len() > MAX_PROFILE_PUBLICATION_REFS {
            return Err(ProtocolError::InvalidField(7));
        }

        let mut entries = vec![
            (Value::Unsigned(1), Value::Unsigned(PROFILE_SCHEMA_VERSION)),
            (Value::Unsigned(2), Value::Text(self.identity_id.as_str().to_owned())),
            (Value::Unsigned(3), Value::Text(display_name)),
            (Value::Unsigned(4), username.map_or(Value::Null, Value::Text)),
            (Value::Unsigned(5), Value::Text(bio)),
            (
                Value::Unsigned(6),
                self.avatar_content_id
                    .map_or(Value::Null, |content_id| Value::Bytes(content_id.to_vec())),
            ),
            (
                Value::Unsigned(7),
                Value::Array(
                    self.publication_refs
                        .iter()
                        .map(|reference| Value::Bytes(reference.to_vec()))
                        .collect(),
                ),
            ),
            (Value::Unsigned(8), Value::Unsigned(self.revision)),
            (Value::Unsigned(9), Value::Unsigned(self.updated_at_ms)),
        ];
        append_unknown_fields(&mut entries, &self.unknown_fields, MAX_KNOWN_FIELD)?;

        let body = Value::Map(entries).to_canonical_bytes_with_limits(CborLimits::signed_record())?;
        if body.len() > MAX_SIGNED_BODY_BYTES {
            return Err(ProtocolError::RecordTooLarge);
        }
        Ok(body)
    }

    /// Parses a canonical v1 profile body and preserves unknown minor fields.
    pub fn parse_body(body: &[u8]) -> Result<Self, ProtocolError> {
        let value = decode_signed_record(body)?;
        let keys = nonnegative_keys(&value)?;
        let schema = unsigned(&value, 1)?;
        if schema != PROFILE_SCHEMA_VERSION {
            return Err(ProtocolError::UnsupportedVersion(schema));
        }

        let identity_id_text = text(&value, 2)?;
        let identity_id = IdentityId::parse(identity_id_text).map_err(|_| ProtocolError::InvalidField(2))?;

        let display_name = parse_text_chars(&value, 3, MIN_DISPLAY_NAME_CHARS, MAX_DISPLAY_NAME_CHARS)?;
        let username = match value.get_key(4).ok_or(ProtocolError::MissingField(4))? {
            Value::Null => None,
            Value::Text(username) => {
                let normalized = normalize_chars(username, 4, MIN_USERNAME_CHARS, MAX_USERNAME_CHARS)?;
                if normalized != *username {
                    return Err(ProtocolError::InvalidField(4));
                }
                Some(username.clone())
            }
            _ => return Err(ProtocolError::InvalidField(4)),
        };
        let bio = parse_optional_chars(&value, 5, MAX_BIO_CHARS)?;
        let avatar_content_id = nullable_bytes32(&value, 6)?;
        let references = array(&value, 7)?;
        if references.len() > MAX_PROFILE_PUBLICATION_REFS {
            return Err(ProtocolError::InvalidField(7));
        }
        let mut publication_refs = Vec::with_capacity(references.len());
        for reference in references {
            let bytes = reference.as_bytes().ok_or(ProtocolError::InvalidField(7))?;
            publication_refs.push(bytes.try_into().map_err(|_| ProtocolError::InvalidField(7))?);
        }
        let revision = unsigned(&value, 8)?;
        if revision == 0 {
            return Err(ProtocolError::InvalidField(8));
        }
        let updated_at_ms = unsigned(&value, 9)?;

        let mut unknown_fields = Vec::new();
        for key in keys {
            if key > MAX_KNOWN_FIELD {
                let value = value.get_key(key).ok_or(ProtocolError::MissingField(key))?.clone();
                unknown_fields.push((key, value));
            }
        }

        Ok(Self {
            identity_id,
            display_name,
            username,
            bio,
            avatar_content_id,
            publication_refs,
            revision,
            updated_at_ms,
            unknown_fields,
        })
    }

    /// Signs this profile and returns the exact-byte record.
    pub fn sign(&self, keypair: &KeyPair) -> Result<SignedProfile, ProtocolError> {
        if IdentityId::from_public_key(&keypair.public_key()) != self.identity_id {
            return Err(ProtocolError::IdentityKeyMismatch);
        }
        let body = self.encode_body()?;
        let record = SignedRecord::sign(body, DomainTag::Profile, keypair)?;
        let profile = Self::parse_body(record.body_bytes())?;
        Ok(SignedProfile { record, profile })
    }
}

/// A verified profile record that keeps its exact envelope bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedProfile {
    record: SignedRecord,
    profile: ProfileV1,
}

impl SignedProfile {
    /// Signs a profile body.
    pub fn sign(profile: &ProfileV1, keypair: &KeyPair) -> Result<Self, ProtocolError> {
        profile.sign(keypair)
    }

    /// Parses and verifies a profile record from exact envelope bytes.
    pub fn parse(raw: &[u8]) -> Result<Self, ProtocolError> {
        let record = SignedRecord::parse(raw)?;
        Self::from_record(record)
    }

    /// Verifies an already-parsed signed record.
    pub fn from_record(record: SignedRecord) -> Result<Self, ProtocolError> {
        let profile = ProfileV1::parse_body(record.body_bytes())?;
        let public_key = profile
            .identity_id
            .public_key()
            .map_err(|_| ProtocolError::InvalidField(2))?;
        record.verify(DomainTag::Profile, &public_key)?;
        Ok(Self { record, profile })
    }

    /// Returns the verified profile body.
    #[must_use]
    pub fn profile(&self) -> &ProfileV1 {
        &self.profile
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
}

fn normalize_required_chars(value: &str, field: u64, minimum: usize, maximum: usize) -> Result<String, ProtocolError> {
    let normalized = normalize_chars(value, field, minimum, maximum)?;
    Ok(normalized)
}

fn normalize_optional_chars(value: &str, field: u64, maximum: usize) -> Result<String, ProtocolError> {
    let normalized = normalize_text_chars(value, maximum).map_err(|_| ProtocolError::InvalidField(field))?;
    Ok(normalized)
}

fn normalize_chars(value: &str, field: u64, minimum: usize, maximum: usize) -> Result<String, ProtocolError> {
    let normalized = normalize_text_chars(value, maximum).map_err(|_| ProtocolError::InvalidField(field))?;
    if normalized.chars().count() < minimum {
        return Err(ProtocolError::InvalidField(field));
    }
    Ok(normalized)
}

fn parse_text_chars(value: &Value, field: u64, minimum: usize, maximum: usize) -> Result<String, ProtocolError> {
    let text = text(value, field)?;
    let normalized = normalize_chars(text, field, minimum, maximum)?;
    if normalized != text {
        return Err(ProtocolError::InvalidField(field));
    }
    Ok(text.to_owned())
}

fn parse_optional_chars(value: &Value, field: u64, maximum: usize) -> Result<String, ProtocolError> {
    let text = text(value, field)?;
    let normalized = normalize_optional_chars(text, field, maximum)?;
    if normalized != text {
        return Err(ProtocolError::InvalidField(field));
    }
    Ok(text.to_owned())
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
    use nexus_crypto::{IdentityId, KeyPair, Value};

    use super::{ProfileV1, SignedProfile};

    fn profile() -> (ProfileV1, KeyPair) {
        let keypair = KeyPair::from_seed([51_u8; 32]);
        let identity_id = IdentityId::from_public_key(&keypair.public_key());
        let profile = ProfileV1::new(identity_id, "Ada", "Local-first builder", 1, 1_700_000_000_000)
            .with_username("ada.l")
            .with_publications(vec![[9_u8; 32]]);
        (profile, keypair)
    }

    #[test]
    fn profile_signs_and_verifies_with_exact_bytes() {
        let (profile, keypair) = profile();
        let signed = SignedProfile::sign(&profile, &keypair).expect("sign");
        let parsed = SignedProfile::parse(signed.raw()).expect("verify");
        assert_eq!(parsed.profile(), &profile);
        assert_eq!(parsed.raw(), signed.raw());
    }

    #[test]
    fn profile_rejects_identity_key_mismatch_and_domain_replay() {
        let (profile, keypair) = profile();
        let other = KeyPair::from_seed([52_u8; 32]);
        assert!(profile.sign(&other).is_err());

        let body = profile.encode_body().expect("body");
        let replay =
            nexus_crypto::KeyPair::from_seed([53_u8; 32]).sign_domain(nexus_crypto::DomainTag::Publication, &body);
        assert!(
            keypair
                .public_key()
                .verify_domain(nexus_crypto::DomainTag::Profile, &body, &replay)
                .is_err()
        );
    }

    #[test]
    fn unknown_minor_fields_are_preserved() {
        let (profile, keypair) = profile();
        let mut body = profile.encode_body().expect("body");
        let value = nexus_crypto::decode_signed_record(&body).expect("decode");
        let mut entries = value.as_map().expect("map").to_vec();
        entries.push((Value::Unsigned(100), Value::Text("future".to_owned())));
        body = Value::Map(entries).to_canonical_bytes().expect("canonical");
        let record =
            crate::envelope::SignedRecord::sign(body, nexus_crypto::DomainTag::Profile, &keypair).expect("sign");
        let signed = SignedProfile::from_record(record).expect("verify");
        assert_eq!(signed.profile().unknown_fields().len(), 1);
        assert_eq!(signed.profile().unknown_fields()[0].0, 100);
    }

    #[test]
    fn rejects_unknown_major_and_non_integer_key() {
        let (profile, keypair) = profile();
        let body = profile.encode_body().expect("body");
        let value = nexus_crypto::decode_signed_record(&body).expect("decode");
        let mut entries = value.as_map().expect("map").to_vec();
        for entry in &mut entries {
            if entry.0 == Value::Unsigned(1) {
                entry.1 = Value::Unsigned(2);
            }
        }
        let changed = Value::Map(entries).to_canonical_bytes().expect("canonical");
        let record =
            crate::envelope::SignedRecord::sign(changed, nexus_crypto::DomainTag::Profile, &keypair).expect("sign");
        assert!(SignedProfile::from_record(record).is_err());

        let mut entries = value.as_map().expect("map").to_vec();
        entries.push((Value::Text("bad".to_owned()), Value::Null));
        let changed = Value::Map(entries).to_canonical_bytes().expect("canonical");
        let record =
            crate::envelope::SignedRecord::sign(changed, nexus_crypto::DomainTag::Profile, &keypair).expect("sign");
        assert!(SignedProfile::from_record(record).is_err());
    }

    #[test]
    fn enforces_display_and_tag_bounds() {
        let (profile, keypair) = profile();
        let mut too_long = profile.clone();
        too_long.display_name = "x".repeat(65);
        assert!(too_long.sign(&keypair).is_err());
        let mut short_username = profile;
        short_username.username = Some("ab".to_owned());
        assert!(short_username.sign(&keypair).is_err());
    }
}
