//! Signed record schemas and wire validation.
//!
//! Signed records use canonical CBOR bodies wrapped in an exact-byte envelope
//! `{1: body bstr, 2: sig bstr}`. A signature covers `domain_tag || body`.
//! Verification always uses the stored body bytes; it never re-encodes them.

mod envelope;
mod error;
mod fields;
mod manifest;
mod profile;
mod publication;
mod tombstone;

pub use envelope::{MAX_SIGNED_BODY_BYTES, MAX_SIGNED_RECORD_BYTES, SignedRecord};
pub use error::ProtocolError;
pub use manifest::{ChunkEntry, MANIFEST_SCHEMA_VERSION, MAX_CHUNKS, MAX_MANIFEST_BYTES, ManifestV1, manifest_root};
pub use profile::{
    MAX_BIO_CHARS, MAX_DISPLAY_NAME_CHARS, MAX_PROFILE_PUBLICATION_REFS, MAX_USERNAME_CHARS, MIN_DISPLAY_NAME_CHARS,
    MIN_USERNAME_CHARS, PROFILE_SCHEMA_VERSION, ProfileV1, SignedProfile,
};
pub use publication::{
    CHUNK_SIZE_V1, Category, MAX_APP_VERSION_BYTES, MAX_DESCRIPTION_BYTES, MAX_DISPLAY_FILENAME_BYTES, MAX_FILE_SIZE,
    MAX_TAG_BYTES, MAX_TAGS, PUBLICATION_SCHEMA_VERSION, PublicationV1, SignedPublication, publication_id,
};
pub use tombstone::{SignedTombstone, TOMBSTONE_SCHEMA_VERSION, TombstoneV1};

/// Returns the architecture component name for diagnostics.
#[must_use]
pub const fn component_name() -> &'static str {
    "nexus-protocol"
}
