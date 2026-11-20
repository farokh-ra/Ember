//! Inbound file-transfer provider contract.
//!
//! `nexus-net` owns the transport and the wire bounds; the core owns storage
//! and decides which verified content it can serve. The driver calls this trait
//! only after decoding and rate-limiting an inbound request, and it never
//! retains the returned bytes beyond the response.

use nexus_protocol::FileSelector;

/// Metadata returned for a `/nexus/file/1` metadata request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    /// Exact canonical manifest bytes.
    pub manifest: Vec<u8>,
    /// Exact signed publication bytes when the source has one.
    pub publication: Option<Vec<u8>>,
}

/// Storage-backed source for inbound file requests.
///
/// Implementations must return `None` for unknown, incomplete, or unverified
/// content. They must never read outside the managed store or return more than
/// the requested slice.
pub trait FileProvider: Send + Sync {
    /// Returns metadata for a content ID or publication ID.
    fn metadata(&self, selector: FileSelector) -> Option<FileMetadata>;

    /// Returns exactly `length` bytes at `offset` inside `index`, or `None`.
    fn chunk(&self, content_id: &[u8; 32], index: u32, offset: u32, length: u32) -> Option<Vec<u8>>;
}

/// Provider used when the host has no storage attached.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EmptyFileProvider;

impl FileProvider for EmptyFileProvider {
    fn metadata(&self, _selector: FileSelector) -> Option<FileMetadata> {
        None
    }

    fn chunk(&self, _content_id: &[u8; 32], _index: u32, _offset: u32, _length: u32) -> Option<Vec<u8>> {
        None
    }
}
