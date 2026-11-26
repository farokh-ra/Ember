//! Bounded request/response bodies for `/nexus/file/1`.
//!
//! The file protocol carries a metadata request (manifest and, when known, the
//! signed publication) and bounded chunk slices. File bytes never enter the
//! control envelope or the DHT: every response is length-bounded and verified
//! by the downloader before it is written to disk.
//!
//! Chunk slices are deliberately much smaller than one v1 chunk so the same
//! messages fit through a relayed circuit with a small byte limit. A client
//! assembles and verifies one complete 1 MiB chunk in memory before writing it.

use nexus_crypto::{CborLimits, Value, decode};

use crate::error::ProtocolError;
use crate::publication::CHUNK_SIZE_V1;

/// Wire version for file metadata and chunk messages.
pub const FILE_WIRE_VERSION: u64 = 1;
/// Maximum encoded metadata or chunk message.
pub const MAX_FILE_MESSAGE_BYTES: usize = 512 * 1024;
/// Maximum bytes returned by one chunk request.
pub const MAX_CHUNK_SLICE_BYTES: usize = 64 * 1024;
/// Maximum typed error code length.
pub const MAX_FILE_ERROR_BYTES: usize = 128;

const KIND_METADATA: u64 = 0;
const KIND_CHUNK: u64 = 1;
const SELECTOR_CONTENT: u64 = 0;
const SELECTOR_PUBLICATION: u64 = 1;
const LIMITS: CborLimits = CborLimits::new(4, 16, MAX_FILE_MESSAGE_BYTES);

/// How a metadata request identifies the desired content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileSelector {
    /// Request by the SHA-256 of the complete file.
    ContentId([u8; 32]),
    /// Request by a signed publication ID.
    PublicationId([u8; 32]),
}

impl FileSelector {
    /// Returns the 32-byte selector value.
    #[must_use]
    pub const fn id(self) -> [u8; 32] {
        match self {
            Self::ContentId(id) | Self::PublicationId(id) => id,
        }
    }

    const fn kind(self) -> u64 {
        match self {
            Self::ContentId(_) => SELECTOR_CONTENT,
            Self::PublicationId(_) => SELECTOR_PUBLICATION,
        }
    }
}

/// A bounded request for one chunk slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkRequest {
    /// SHA-256 of the complete file.
    pub content_id: [u8; 32],
    /// Zero-based chunk index in the manifest.
    pub index: u32,
    /// Byte offset inside the chunk.
    pub offset: u32,
    /// Requested slice length, at most [`MAX_CHUNK_SLICE_BYTES`].
    pub length: u32,
}

/// A metadata or chunk request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileRequest {
    /// Fetch the manifest and, when known, the signed publication.
    Metadata(FileSelector),
    /// Fetch one bounded slice of one chunk.
    Chunk(ChunkRequest),
}

/// A metadata or chunk response.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct FileResponse {
    /// Exact canonical manifest bytes for metadata responses.
    pub manifest: Option<Vec<u8>>,
    /// Exact signed publication bytes when the source has one.
    pub publication: Option<Vec<u8>>,
    /// Chunk slice bytes for chunk responses.
    pub data: Option<Vec<u8>>,
    /// Stable typed error when the request was rejected.
    pub error: Option<String>,
}

/// Encodes a file request as canonical CBOR.
pub fn encode_file_request(request: &FileRequest) -> Result<Vec<u8>, ProtocolError> {
    let entries = match request {
        FileRequest::Metadata(selector) => vec![
            (Value::Unsigned(1), Value::Unsigned(FILE_WIRE_VERSION)),
            (Value::Unsigned(2), Value::Unsigned(KIND_METADATA)),
            (Value::Unsigned(3), Value::Unsigned(selector.kind())),
            (Value::Unsigned(4), Value::Bytes(selector.id().to_vec())),
        ],
        FileRequest::Chunk(chunk) => {
            if chunk.length == 0
                || usize::try_from(chunk.length).map_or(true, |length| length > MAX_CHUNK_SLICE_BYTES)
                || u64::from(chunk.offset).saturating_add(u64::from(chunk.length)) > CHUNK_SIZE_V1
            {
                return Err(ProtocolError::InvalidWireMessage);
            }
            vec![
                (Value::Unsigned(1), Value::Unsigned(FILE_WIRE_VERSION)),
                (Value::Unsigned(2), Value::Unsigned(KIND_CHUNK)),
                (Value::Unsigned(4), Value::Bytes(chunk.content_id.to_vec())),
                (Value::Unsigned(5), Value::Unsigned(u64::from(chunk.index))),
                (Value::Unsigned(6), Value::Unsigned(u64::from(chunk.offset))),
                (Value::Unsigned(7), Value::Unsigned(u64::from(chunk.length))),
            ]
        }
    };
    encode(Value::Map(entries))
}

/// Decodes a file request and rejects unknown shapes, versions, and sizes.
pub fn decode_file_request(bytes: &[u8]) -> Result<FileRequest, ProtocolError> {
    let entries = decode_map(bytes)?;
    let mut version = None;
    let mut kind = None;
    let mut selector = None;
    let mut id = None;
    let mut index = None;
    let mut offset = None;
    let mut length = None;
    for (key, value) in entries {
        match key.as_unsigned() {
            Some(1) => version = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(2) => kind = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(3) => selector = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(4) => {
                id = Some(
                    value
                        .as_bytes()
                        .ok_or(ProtocolError::InvalidWireMessage)?
                        .try_into()
                        .map_err(|_| ProtocolError::InvalidWireMessage)?,
                );
            }
            Some(5) => index = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(6) => offset = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(7) => length = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            _ => return Err(ProtocolError::InvalidWireMessage),
        }
    }
    if version != Some(FILE_WIRE_VERSION) {
        return Err(ProtocolError::InvalidWireMessage);
    }
    let id: [u8; 32] = id.ok_or(ProtocolError::InvalidWireMessage)?;
    match kind {
        Some(KIND_METADATA) => {
            if index.is_some() || offset.is_some() || length.is_some() {
                return Err(ProtocolError::InvalidWireMessage);
            }
            let selector = match selector {
                Some(SELECTOR_CONTENT) => FileSelector::ContentId(id),
                Some(SELECTOR_PUBLICATION) => FileSelector::PublicationId(id),
                _ => return Err(ProtocolError::InvalidWireMessage),
            };
            Ok(FileRequest::Metadata(selector))
        }
        Some(KIND_CHUNK) => {
            if selector.is_some() {
                return Err(ProtocolError::InvalidWireMessage);
            }
            let index = u32::try_from(index.ok_or(ProtocolError::InvalidWireMessage)?)
                .map_err(|_| ProtocolError::InvalidWireMessage)?;
            let offset = u32::try_from(offset.ok_or(ProtocolError::InvalidWireMessage)?)
                .map_err(|_| ProtocolError::InvalidWireMessage)?;
            let length = u32::try_from(length.ok_or(ProtocolError::InvalidWireMessage)?)
                .map_err(|_| ProtocolError::InvalidWireMessage)?;
            if length == 0
                || usize::try_from(length).map_or(true, |length| length > MAX_CHUNK_SLICE_BYTES)
                || u64::from(offset).saturating_add(u64::from(length)) > CHUNK_SIZE_V1
            {
                return Err(ProtocolError::InvalidWireMessage);
            }
            Ok(FileRequest::Chunk(ChunkRequest {
                content_id: id,
                index,
                offset,
                length,
            }))
        }
        _ => Err(ProtocolError::InvalidWireMessage),
    }
}

/// Encodes a successful metadata, successful chunk, or typed error response.
pub fn encode_file_response(response: &FileResponse) -> Result<Vec<u8>, ProtocolError> {
    let entries = match (&response.manifest, &response.data, &response.error) {
        (Some(manifest), None, None) => {
            if manifest.len() > crate::manifest::MAX_MANIFEST_BYTES {
                return Err(ProtocolError::InvalidWireMessage);
            }
            let mut entries = vec![
                (Value::Unsigned(1), Value::Unsigned(FILE_WIRE_VERSION)),
                (Value::Unsigned(2), Value::Unsigned(KIND_METADATA)),
                (Value::Unsigned(3), Value::Bytes(manifest.clone())),
            ];
            if let Some(publication) = &response.publication {
                if publication.len() > crate::envelope::MAX_SIGNED_RECORD_BYTES {
                    return Err(ProtocolError::InvalidWireMessage);
                }
                entries.push((Value::Unsigned(4), Value::Bytes(publication.clone())));
            }
            entries
        }
        (None, Some(data), None) => {
            if data.is_empty() || data.len() > MAX_CHUNK_SLICE_BYTES {
                return Err(ProtocolError::InvalidWireMessage);
            }
            vec![
                (Value::Unsigned(1), Value::Unsigned(FILE_WIRE_VERSION)),
                (Value::Unsigned(2), Value::Unsigned(KIND_CHUNK)),
                (Value::Unsigned(5), Value::Bytes(data.clone())),
            ]
        }
        (None, None, Some(error)) => {
            if error.is_empty() || error.len() > MAX_FILE_ERROR_BYTES || !error.is_ascii() {
                return Err(ProtocolError::InvalidWireMessage);
            }
            vec![
                (Value::Unsigned(1), Value::Unsigned(FILE_WIRE_VERSION)),
                (Value::Unsigned(6), Value::Text(error.clone())),
            ]
        }
        _ => return Err(ProtocolError::InvalidWireMessage),
    };
    encode(Value::Map(entries))
}

/// Decodes a file response, rejecting mixed or unbounded shapes.
pub fn decode_file_response(bytes: &[u8]) -> Result<FileResponse, ProtocolError> {
    let entries = decode_map(bytes)?;
    let mut version = None;
    let mut kind = None;
    let mut manifest = None;
    let mut publication = None;
    let mut data = None;
    let mut error = None;
    for (key, value) in entries {
        match key.as_unsigned() {
            Some(1) => version = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(2) => kind = Some(value.as_unsigned().ok_or(ProtocolError::InvalidWireMessage)?),
            Some(3) => {
                manifest = Some(value.as_bytes().ok_or(ProtocolError::InvalidWireMessage)?.to_vec());
            }
            Some(4) => {
                publication = Some(value.as_bytes().ok_or(ProtocolError::InvalidWireMessage)?.to_vec());
            }
            Some(5) => data = Some(value.as_bytes().ok_or(ProtocolError::InvalidWireMessage)?.to_vec()),
            Some(6) => {
                error = Some(value.as_text().ok_or(ProtocolError::InvalidWireMessage)?.to_owned());
            }
            _ => return Err(ProtocolError::InvalidWireMessage),
        }
    }
    if version != Some(FILE_WIRE_VERSION) {
        return Err(ProtocolError::InvalidWireMessage);
    }
    match (kind, manifest, publication, data, error) {
        (Some(KIND_METADATA), Some(manifest), publication, None, None)
            if manifest.len() <= crate::manifest::MAX_MANIFEST_BYTES
                && publication
                    .as_ref()
                    .is_none_or(|publication| publication.len() <= crate::envelope::MAX_SIGNED_RECORD_BYTES) =>
        {
            Ok(FileResponse {
                manifest: Some(manifest),
                publication,
                data: None,
                error: None,
            })
        }
        (Some(KIND_CHUNK), None, None, Some(data), None) if !data.is_empty() && data.len() <= MAX_CHUNK_SLICE_BYTES => {
            Ok(FileResponse {
                manifest: None,
                publication: None,
                data: Some(data),
                error: None,
            })
        }
        (None, None, None, None, Some(error))
            if !error.is_empty() && error.len() <= MAX_FILE_ERROR_BYTES && error.is_ascii() =>
        {
            Ok(FileResponse {
                manifest: None,
                publication: None,
                data: None,
                error: Some(error),
            })
        }
        _ => Err(ProtocolError::InvalidWireMessage),
    }
}

fn encode(value: Value) -> Result<Vec<u8>, ProtocolError> {
    let bytes = value.to_canonical_bytes_with_limits(LIMITS)?;
    if bytes.len() > MAX_FILE_MESSAGE_BYTES {
        return Err(ProtocolError::InvalidWireMessage);
    }
    Ok(bytes)
}

fn decode_map(bytes: &[u8]) -> Result<Vec<(Value, Value)>, ProtocolError> {
    if bytes.is_empty() || bytes.len() > MAX_FILE_MESSAGE_BYTES {
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
        ChunkRequest, FileRequest, FileResponse, FileSelector, MAX_CHUNK_SLICE_BYTES, MAX_FILE_MESSAGE_BYTES,
        decode_file_request, decode_file_response, encode_file_request, encode_file_response,
    };

    #[test]
    fn metadata_request_round_trips_both_selectors() {
        for selector in [
            FileSelector::ContentId([7_u8; 32]),
            FileSelector::PublicationId([9_u8; 32]),
        ] {
            let request = FileRequest::Metadata(selector);
            let bytes = encode_file_request(&request).expect("encode");
            assert_eq!(decode_file_request(&bytes).expect("decode"), request);
        }
    }

    #[test]
    fn chunk_request_round_trips_within_one_chunk() {
        let request = FileRequest::Chunk(ChunkRequest {
            content_id: [3_u8; 32],
            index: 41,
            offset: 64 * 1024,
            length: 32 * 1024,
        });
        let bytes = encode_file_request(&request).expect("encode");
        assert_eq!(decode_file_request(&bytes).expect("decode"), request);
    }

    #[test]
    fn oversized_or_offset_chunk_slices_are_rejected() {
        let too_large = FileRequest::Chunk(ChunkRequest {
            content_id: [3_u8; 32],
            index: 0,
            offset: 0,
            length: u32::try_from(MAX_CHUNK_SLICE_BYTES + 1).expect("fits"),
        });
        assert!(encode_file_request(&too_large).is_err());

        let past_chunk = FileRequest::Chunk(ChunkRequest {
            content_id: [3_u8; 32],
            index: 0,
            offset: u32::try_from(1024 * 1024).expect("fits"),
            length: 1,
        });
        assert!(encode_file_request(&past_chunk).is_err());
    }

    #[test]
    fn metadata_and_chunk_responses_round_trip() {
        let metadata = FileResponse {
            manifest: Some(vec![1, 2, 3]),
            publication: Some(vec![4, 5]),
            data: None,
            error: None,
        };
        let bytes = encode_file_response(&metadata).expect("encode");
        assert_eq!(decode_file_response(&bytes).expect("decode"), metadata);

        let chunk = FileResponse {
            manifest: None,
            publication: None,
            data: Some(vec![9; MAX_CHUNK_SLICE_BYTES]),
            error: None,
        };
        let bytes = encode_file_response(&chunk).expect("encode");
        assert_eq!(decode_file_response(&bytes).expect("decode"), chunk);
    }

    #[test]
    fn typed_errors_round_trip_and_oversized_errors_fail() {
        let error = FileResponse {
            manifest: None,
            publication: None,
            data: None,
            error: Some("content_unavailable".to_owned()),
        };
        let bytes = encode_file_response(&error).expect("encode");
        assert_eq!(decode_file_response(&bytes).expect("decode"), error);

        let oversized = FileResponse {
            manifest: None,
            publication: None,
            data: None,
            error: Some("x".repeat(super::MAX_FILE_ERROR_BYTES + 1)),
        };
        assert!(encode_file_response(&oversized).is_err());
    }

    #[test]
    fn mixed_or_unknown_shapes_are_rejected() {
        let mixed = FileResponse {
            manifest: Some(vec![1]),
            publication: None,
            data: Some(vec![2]),
            error: None,
        };
        assert!(encode_file_response(&mixed).is_err());

        let request = encode_file_request(&FileRequest::Metadata(FileSelector::ContentId([1; 32]))).expect("encode");
        let mut truncated = request.clone();
        truncated.pop();
        assert!(decode_file_request(&truncated).is_err());

        let mut trailing = request;
        trailing.push(0);
        assert!(decode_file_request(&trailing).is_err());
    }

    #[test]
    fn malformed_inputs_never_panic_or_decode() {
        let request = encode_file_request(&FileRequest::Chunk(ChunkRequest {
            content_id: [5; 32],
            index: 3,
            offset: 64 * 1024,
            length: 4096,
        }))
        .expect("encode");

        for length in 0..request.len() {
            assert!(
                decode_file_request(&request[..length]).is_err(),
                "truncated length {length} decoded"
            );
        }
        for index in 0..request.len() {
            let mut mutated = request.clone();
            mutated[index] ^= 0xff;
            let _ = decode_file_request(&mutated);
        }
        assert!(decode_file_request(&vec![0_u8; MAX_FILE_MESSAGE_BYTES + 1]).is_err());
        assert!(decode_file_response(&[0xff_u8; 64]).is_err());
    }
}
