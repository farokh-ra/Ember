//! Bounded request/response bodies for optional `/nexus/index/1` providers.
//!
//! Search is local-first. A peer may opt in as an index provider, but its
//! responses are only hints: every returned profile or publication is the
//! exact signed envelope and must be verified again by the requester before it
//! is cached, indexed, or displayed. The protocol never carries unsigned
//! metadata, ranking, popularity, or a global result set.

use nexus_crypto::{CborLimits, DomainTag, Value, decode};
use sha2::{Digest, Sha256};

use crate::error::ProtocolError;
use crate::publication::Category;

/// Wire version for index-provider messages.
pub const INDEX_WIRE_VERSION: u64 = 1;
/// Maximum encoded index request or response.
pub const MAX_INDEX_MESSAGE_BYTES: usize = 1024 * 1024;
/// Maximum query text in UTF-8 bytes.
pub const MAX_INDEX_QUERY_BYTES: usize = 128;
/// Maximum tag filters in one request.
pub const MAX_INDEX_TAG_FILTERS: usize = 5;
/// Maximum bytes in one tag filter.
pub const MAX_INDEX_TAG_BYTES: usize = 32;
/// Maximum records in one response page.
pub const MAX_INDEX_PAGE_SIZE: usize = 20;
/// Maximum opaque cursor bytes.
pub const MAX_INDEX_CURSOR_BYTES: usize = 128;
/// Maximum provider error text bytes.
pub const MAX_INDEX_ERROR_BYTES: usize = 128;

const KIND_PROFILE: u64 = 0;
const KIND_PUBLICATION: u64 = 1;
const LIMITS: CborLimits = CborLimits::new(5, 2048, MAX_INDEX_MESSAGE_BYTES);

/// The signed record kind returned by an index provider.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IndexKind {
    /// A signed public profile.
    Profile,
    /// A signed file publication.
    Publication,
}

impl IndexKind {
    /// Returns the stable wire and storage code.
    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::Profile => KIND_PROFILE,
            Self::Publication => KIND_PUBLICATION,
        }
    }

    /// Returns the stable lowercase display name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Publication => "publication",
        }
    }

    /// Parses a stable lowercase kind name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "profile" => Some(Self::Profile),
            "publication" => Some(Self::Publication),
            _ => None,
        }
    }

    fn from_code(code: u64) -> Option<Self> {
        match code {
            KIND_PROFILE => Some(Self::Profile),
            KIND_PUBLICATION => Some(Self::Publication),
            _ => None,
        }
    }
}

/// A bounded local or provider search request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexRequest {
    /// Query text, at most [`MAX_INDEX_QUERY_BYTES`] UTF-8 bytes.
    pub query: String,
    /// Optional restriction to one signed record kind.
    pub kind: Option<IndexKind>,
    /// Optional canonical category filter for publications.
    pub category: Option<Category>,
    /// Exact tag filters; all listed tags must match.
    pub tags: Vec<String>,
    /// Opaque provider cursor.
    pub cursor: Option<String>,
    /// Requested page size, 1 through [`MAX_INDEX_PAGE_SIZE`].
    pub limit: u16,
}

impl IndexRequest {
    /// Validates all request bounds before encoding or querying.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.query.len() > MAX_INDEX_QUERY_BYTES || self.query.chars().any(char::is_control) {
            return Err(ProtocolError::InvalidWireMessage);
        }
        if self.tags.len() > MAX_INDEX_TAG_FILTERS {
            return Err(ProtocolError::InvalidWireMessage);
        }
        for tag in &self.tags {
            if tag.is_empty() || tag.len() > MAX_INDEX_TAG_BYTES || tag.chars().any(char::is_control) {
                return Err(ProtocolError::InvalidWireMessage);
            }
        }
        if let Some(cursor) = &self.cursor {
            validate_cursor(cursor)?;
        }
        if self.limit == 0 || usize::from(self.limit) > MAX_INDEX_PAGE_SIZE {
            return Err(ProtocolError::InvalidWireMessage);
        }
        Ok(())
    }
}

/// One exact signed record returned by an index provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexRecord {
    /// Signed record kind.
    pub kind: IndexKind,
    /// Exact signed profile or publication envelope bytes.
    pub signed: Vec<u8>,
}

/// A bounded provider response.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct IndexResponse {
    /// Verified signed records known to the provider.
    pub records: Vec<IndexRecord>,
    /// Opaque cursor for the next provider page.
    pub next_cursor: Option<String>,
    /// Whether the provider reached the end of its local matches.
    pub complete: bool,
    /// Stable typed error when the provider rejected the request.
    pub error: Option<String>,
    /// When the provider last rebuilt or refreshed its local index.
    pub indexed_at_ms: Option<u64>,
}

/// Derives the DHT provider key used to discover optional index providers.
///
/// The key is a fixed domain-separated value, not a content ID. It identifies
/// a service role, never an identity or an authority.
#[must_use]
pub fn index_provider_key() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DomainTag::DhtIndex.as_bytes());
    hasher.finalize().into()
}

/// Encodes an index request as canonical CBOR.
pub fn encode_index_request(request: &IndexRequest) -> Result<Vec<u8>, ProtocolError> {
    request.validate()?;
    let mut entries = vec![
        (Value::Unsigned(1), Value::Unsigned(INDEX_WIRE_VERSION)),
        (Value::Unsigned(2), Value::Text(request.query.clone())),
    ];
    if let Some(kind) = request.kind {
        entries.push((Value::Unsigned(3), Value::Unsigned(kind.code())));
    }
    if let Some(category) = request.category {
        entries.push((Value::Unsigned(4), Value::Text(category.as_str().to_owned())));
    }
    if !request.tags.is_empty() {
        entries.push((
            Value::Unsigned(5),
            Value::Array(request.tags.iter().cloned().map(Value::Text).collect()),
        ));
    }
    if let Some(cursor) = &request.cursor {
        entries.push((Value::Unsigned(6), Value::Text(cursor.clone())));
    }
    entries.push((Value::Unsigned(7), Value::Unsigned(u64::from(request.limit))));
    encode(Value::Map(entries))
}

/// Decodes an index request and rejects unknown shapes, versions, and sizes.
pub fn decode_index_request(bytes: &[u8]) -> Result<IndexRequest, ProtocolError> {
    let entries = decode_map(bytes)?;
    let mut version = None;
    let mut query = None;
    let mut kind = None;
    let mut category = None;
    let mut tags = None;
    let mut cursor = None;
    let mut limit = None;
    for (key, value) in entries {
        match key.as_unsigned() {
            Some(1) => version = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(2) => {
                query = Some(value.as_text().ok_or(ProtocolError::InvalidWireMessage)?.to_owned());
            }
            Some(3) => {
                let code = value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?;
                kind = Some(IndexKind::from_code(code).ok_or(ProtocolError::InvalidWireMessage)?);
            }
            Some(4) => {
                let text = value.as_text().ok_or(ProtocolError::InvalidWireMessage)?;
                category = Some(Category::parse(text).ok_or(ProtocolError::InvalidWireMessage)?);
            }
            Some(5) => {
                let values = value.as_array().ok_or(ProtocolError::InvalidWireMessage)?;
                let mut parsed = Vec::with_capacity(values.len());
                for value in values {
                    parsed.push(value.as_text().ok_or(ProtocolError::InvalidWireMessage)?.to_owned());
                }
                tags = Some(parsed);
            }
            Some(6) => {
                cursor = Some(value.as_text().ok_or(ProtocolError::InvalidWireMessage)?.to_owned());
            }
            Some(7) => limit = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            _ => return Err(ProtocolError::InvalidWireMessage),
        }
    }
    if version != Some(INDEX_WIRE_VERSION) {
        return Err(ProtocolError::InvalidWireMessage);
    }
    let request = IndexRequest {
        query: query.ok_or(ProtocolError::InvalidWireMessage)?,
        kind,
        category,
        tags: tags.unwrap_or_default(),
        cursor,
        limit: u16::try_from(limit.ok_or(ProtocolError::InvalidWireMessage)?)
            .map_err(|_| ProtocolError::InvalidWireMessage)?,
    };
    request.validate()?;
    Ok(request)
}

/// Encodes a provider response as canonical CBOR.
///
/// A success response may carry records, a cursor, and freshness metadata. An
/// error response carries only a bounded ASCII error code.
pub fn encode_index_response(response: &IndexResponse) -> Result<Vec<u8>, ProtocolError> {
    if response.error.is_some() {
        if !response.records.is_empty() || response.next_cursor.is_some() || response.complete {
            return Err(ProtocolError::InvalidWireMessage);
        }
        let error = response.error.as_deref().unwrap_or_default();
        if error.is_empty() || error.len() > MAX_INDEX_ERROR_BYTES || !error.is_ascii() {
            return Err(ProtocolError::InvalidWireMessage);
        }
        return encode(Value::Map(vec![
            (Value::Unsigned(1), Value::Unsigned(INDEX_WIRE_VERSION)),
            (Value::Unsigned(6), Value::Text(error.to_owned())),
        ]));
    }
    if response.records.len() > MAX_INDEX_PAGE_SIZE {
        return Err(ProtocolError::InvalidWireMessage);
    }
    if let Some(cursor) = &response.next_cursor {
        validate_cursor(cursor)?;
    }
    let mut records = Vec::with_capacity(response.records.len());
    for record in &response.records {
        if record.signed.is_empty() || record.signed.len() > crate::envelope::MAX_SIGNED_RECORD_BYTES {
            return Err(ProtocolError::InvalidWireMessage);
        }
        records.push(Value::Map(vec![
            (Value::Unsigned(1), Value::Unsigned(record.kind.code())),
            (Value::Unsigned(2), Value::Bytes(record.signed.clone())),
        ]));
    }
    let mut entries = vec![
        (Value::Unsigned(1), Value::Unsigned(INDEX_WIRE_VERSION)),
        (Value::Unsigned(2), Value::Array(records)),
        (Value::Unsigned(4), Value::Bool(response.complete)),
    ];
    if let Some(cursor) = &response.next_cursor {
        entries.push((Value::Unsigned(3), Value::Text(cursor.clone())));
    }
    if let Some(indexed_at_ms) = response.indexed_at_ms {
        entries.push((Value::Unsigned(5), Value::Unsigned(indexed_at_ms)));
    }
    encode(Value::Map(entries))
}

/// Decodes a provider response, rejecting mixed or unbounded shapes.
pub fn decode_index_response(bytes: &[u8]) -> Result<IndexResponse, ProtocolError> {
    let entries = decode_map(bytes)?;
    let mut version = None;
    let mut records = None;
    let mut next_cursor = None;
    let mut complete = None;
    let mut error = None;
    let mut indexed_at_ms = None;
    for (key, value) in entries {
        match key.as_unsigned() {
            Some(1) => version = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(2) => {
                let values = value.as_array().ok_or(ProtocolError::InvalidWireMessage)?;
                let mut parsed = Vec::with_capacity(values.len());
                for value in values {
                    let fields = value.as_map().ok_or(ProtocolError::InvalidWireMessage)?;
                    if fields.len() != 2 {
                        return Err(ProtocolError::InvalidWireMessage);
                    }
                    let mut kind = None;
                    let mut signed = None;
                    for (field, value) in fields {
                        match field.as_unsigned() {
                            Some(1) => {
                                kind =
                                    IndexKind::from_code(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?);
                                if kind.is_none() {
                                    return Err(ProtocolError::InvalidWireMessage);
                                }
                            }
                            Some(2) => {
                                signed = Some(value.as_bytes().ok_or(ProtocolError::InvalidWireMessage)?.to_vec());
                            }
                            _ => return Err(ProtocolError::InvalidWireMessage),
                        }
                    }
                    parsed.push(IndexRecord {
                        kind: kind.ok_or(ProtocolError::InvalidWireMessage)?,
                        signed: signed.ok_or(ProtocolError::InvalidWireMessage)?,
                    });
                }
                records = Some(parsed);
            }
            Some(3) => {
                next_cursor = Some(value.as_text().ok_or(ProtocolError::InvalidWireMessage)?.to_owned());
            }
            Some(4) => complete = Some(value.as_bool().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(5) => {
                indexed_at_ms = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?);
            }
            Some(6) => {
                error = Some(value.as_text().ok_or(ProtocolError::InvalidWireMessage)?.to_owned());
            }
            _ => return Err(ProtocolError::InvalidWireMessage),
        }
    }
    if version != Some(INDEX_WIRE_VERSION) {
        return Err(ProtocolError::InvalidWireMessage);
    }
    if let Some(error) = error {
        if records.is_some() || next_cursor.is_some() || complete.is_some() || indexed_at_ms.is_some() {
            return Err(ProtocolError::InvalidWireMessage);
        }
        if error.is_empty() || error.len() > MAX_INDEX_ERROR_BYTES || !error.is_ascii() {
            return Err(ProtocolError::InvalidWireMessage);
        }
        return Ok(IndexResponse {
            error: Some(error),
            ..IndexResponse::default()
        });
    }
    let records = records.ok_or(ProtocolError::InvalidWireMessage)?;
    if records.len() > MAX_INDEX_PAGE_SIZE {
        return Err(ProtocolError::InvalidWireMessage);
    }
    for record in &records {
        if record.signed.is_empty() || record.signed.len() > crate::envelope::MAX_SIGNED_RECORD_BYTES {
            return Err(ProtocolError::InvalidWireMessage);
        }
    }
    if let Some(cursor) = &next_cursor {
        validate_cursor(cursor)?;
    }
    Ok(IndexResponse {
        records,
        next_cursor,
        complete: complete.ok_or(ProtocolError::InvalidWireMessage)?,
        error: None,
        indexed_at_ms,
    })
}

fn validate_cursor(cursor: &str) -> Result<(), ProtocolError> {
    if cursor.is_empty()
        || cursor.len() > MAX_INDEX_CURSOR_BYTES
        || !cursor
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_'))
    {
        return Err(ProtocolError::InvalidWireMessage);
    }
    Ok(())
}

fn encode(value: Value) -> Result<Vec<u8>, ProtocolError> {
    let bytes = value.to_canonical_bytes_with_limits(LIMITS)?;
    if bytes.len() > MAX_INDEX_MESSAGE_BYTES {
        return Err(ProtocolError::InvalidWireMessage);
    }
    Ok(bytes)
}

fn decode_map(bytes: &[u8]) -> Result<Vec<(Value, Value)>, ProtocolError> {
    if bytes.is_empty() || bytes.len() > MAX_INDEX_MESSAGE_BYTES {
        return Err(ProtocolError::InvalidWireMessage);
    }
    match decode(bytes, LIMITS)? {
        Value::Map(entries) => Ok(entries),
        _ => Err(ProtocolError::InvalidWireMessage),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IndexKind, IndexRecord, IndexRequest, IndexResponse, MAX_INDEX_PAGE_SIZE, MAX_INDEX_QUERY_BYTES,
        decode_index_request, decode_index_response, encode_index_request, encode_index_response, index_provider_key,
    };
    use crate::publication::Category;

    fn request() -> IndexRequest {
        IndexRequest {
            query: "field notes".to_owned(),
            kind: Some(IndexKind::Publication),
            category: Some(Category::Documents),
            tags: vec!["notes".to_owned(), "stage16".to_owned()],
            cursor: Some("v1:42:0:abcd1234".to_owned()),
            limit: 20,
        }
    }

    #[test]
    fn requests_round_trip_with_and_without_filters() {
        let request = request();
        let bytes = encode_index_request(&request).expect("encode");
        assert_eq!(decode_index_request(&bytes).expect("decode"), request);

        let minimal = IndexRequest {
            query: "rust".to_owned(),
            kind: None,
            category: None,
            tags: Vec::new(),
            cursor: None,
            limit: 1,
        };
        let bytes = encode_index_request(&minimal).expect("encode");
        assert_eq!(decode_index_request(&bytes).expect("decode"), minimal);
    }

    #[test]
    fn oversized_or_control_queries_and_tags_are_rejected() {
        let mut bad = request();
        bad.query = "x".repeat(MAX_INDEX_QUERY_BYTES + 1);
        assert!(encode_index_request(&bad).is_err());

        let mut bad = request();
        bad.query = "bad\nquery".to_owned();
        assert!(encode_index_request(&bad).is_err());

        let mut bad = request();
        bad.tags = vec!["x".repeat(super::MAX_INDEX_TAG_BYTES + 1)];
        assert!(encode_index_request(&bad).is_err());

        let mut bad = request();
        bad.tags = (0..=super::MAX_INDEX_TAG_FILTERS)
            .map(|index| format!("tag{index}"))
            .collect();
        assert!(encode_index_request(&bad).is_err());

        let mut bad = request();
        bad.limit = 0;
        assert!(encode_index_request(&bad).is_err());
    }

    #[test]
    fn responses_round_trip_and_bound_page_size() {
        let response = IndexResponse {
            records: vec![IndexRecord {
                kind: IndexKind::Profile,
                signed: vec![1, 2, 3],
            }],
            next_cursor: Some("v1:42:20:abcd1234".to_owned()),
            complete: false,
            error: None,
            indexed_at_ms: Some(1_700_000_000_000),
        };
        let bytes = encode_index_response(&response).expect("encode");
        assert_eq!(decode_index_response(&bytes).expect("decode"), response);

        let too_many = IndexResponse {
            records: (0..=MAX_INDEX_PAGE_SIZE)
                .map(|_| IndexRecord {
                    kind: IndexKind::Publication,
                    signed: vec![9],
                })
                .collect(),
            ..IndexResponse::default()
        };
        assert!(encode_index_response(&too_many).is_err());
    }

    #[test]
    fn error_responses_are_exclusive_and_bounded() {
        let error = IndexResponse {
            error: Some("rate_limited".to_owned()),
            ..IndexResponse::default()
        };
        let bytes = encode_index_response(&error).expect("encode");
        assert_eq!(decode_index_response(&bytes).expect("decode"), error);

        let mixed = IndexResponse {
            records: vec![IndexRecord {
                kind: IndexKind::Profile,
                signed: vec![1],
            }],
            error: Some("bad".to_owned()),
            ..IndexResponse::default()
        };
        assert!(encode_index_response(&mixed).is_err());

        let oversized = IndexResponse {
            error: Some("x".repeat(super::MAX_INDEX_ERROR_BYTES + 1)),
            ..IndexResponse::default()
        };
        assert!(encode_index_response(&oversized).is_err());
    }

    #[test]
    fn unknown_fields_versions_and_trailing_bytes_are_rejected() {
        let bytes = encode_index_request(&request()).expect("encode");
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode_index_request(&trailing).is_err());

        let mut truncated = bytes;
        truncated.pop();
        assert!(decode_index_request(&truncated).is_err());

        let unknown = nexus_crypto::Value::Map(vec![
            (nexus_crypto::Value::Unsigned(1), nexus_crypto::Value::Unsigned(1)),
            (
                nexus_crypto::Value::Unsigned(2),
                nexus_crypto::Value::Text("x".to_owned()),
            ),
            (nexus_crypto::Value::Unsigned(7), nexus_crypto::Value::Unsigned(1)),
            (nexus_crypto::Value::Unsigned(9), nexus_crypto::Value::Bool(true)),
        ])
        .to_canonical_bytes()
        .expect("encode");
        assert!(decode_index_request(&unknown).is_err());
    }

    #[test]
    fn provider_key_is_stable_and_domain_separated() {
        let key = index_provider_key();
        assert_eq!(key, index_provider_key());
        assert_ne!(key, [0_u8; 32]);
        assert_eq!(key.len(), 32);
    }
}
