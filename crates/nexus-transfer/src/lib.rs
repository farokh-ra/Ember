//! Streaming SHA-256 hashing, flat chunk manifests, and file verification.
//!
//! The transfer crate owns the byte-level work between a file on disk and the
//! signed publication metadata in [`nexus_protocol`]:
//!
//! - raw SHA-256 content IDs over exact file bytes;
//! - fixed 1 MiB chunk manifests with one SHA-256 per chunk;
//! - streaming verification with bounded memory for files up to 4 GiB;
//! - manifest parsing bound to a signed publication root.
//!
//! No function in this crate holds more than one chunk and one hash buffer in
//! memory, and no function writes or mutates signed record bytes.
//!
//! Stage 17 adds [`scheduler`], the pure multi-source chunk policy. It has no
//! I/O and no identity input; callers perform transfers and report outcomes.

pub mod scheduler;

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use nexus_protocol::{
    CHUNK_SIZE_V1, ChunkEntry, MAX_FILE_SIZE, ManifestV1, ProtocolError, SignedPublication, manifest_root,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Read buffer used by whole-file hashing.
const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// A raw SHA-256 content ID.
///
/// This is the plain SHA-256 digest of the complete file bytes. It is not
/// domain-separated and is not the manifest root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Wraps a 32-byte SHA-256 digest.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Parses exactly 64 lowercase or uppercase hexadecimal characters.
    pub fn parse_hex(text: &str) -> Result<Self, TransferError> {
        if text.len() != 64 {
            return Err(TransferError::InvalidContentHash);
        }
        let mut bytes = [0_u8; 32];
        let (pairs, remainder) = text.as_bytes().as_chunks::<2>();
        if !remainder.is_empty() {
            return Err(TransferError::InvalidContentHash);
        }
        for (index, pair) in pairs.iter().enumerate() {
            let high = hex_value(pair[0]).ok_or(TransferError::InvalidContentHash)?;
            let low = hex_value(pair[1]).ok_or(TransferError::InvalidContentHash)?;
            bytes[index] = (high << 4) | low;
        }
        Ok(Self(bytes))
    }

    /// Returns lowercase hexadecimal.
    #[must_use]
    pub fn to_hex(self) -> String {
        nexus_crypto::to_hex(&self.0)
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

/// A built manifest and the content identity it describes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestedContent {
    content_hash: ContentHash,
    size: u64,
    manifest: ManifestV1,
}

impl ManifestedContent {
    /// Returns the raw SHA-256 content ID.
    #[must_use]
    pub const fn content_hash(&self) -> ContentHash {
        self.content_hash
    }

    /// Returns the complete file size in bytes.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Returns the parsed manifest.
    #[must_use]
    pub const fn manifest(&self) -> &ManifestV1 {
        &self.manifest
    }

    /// Returns the canonical manifest body bytes.
    pub fn manifest_bytes(&self) -> Result<Vec<u8>, TransferError> {
        Ok(self.manifest.encode_body()?)
    }

    /// Returns the domain-separated manifest root.
    pub fn manifest_root(&self) -> Result<[u8; 32], TransferError> {
        Ok(self.manifest.root()?)
    }
}

/// A hashing, manifest, or verification failure.
#[derive(Debug, Error)]
pub enum TransferError {
    /// A filesystem or reader operation failed.
    #[error("transfer I/O failed: {0}")]
    Io(#[from] io::Error),
    /// A manifest or signed record did not satisfy the protocol schema.
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    /// Empty files cannot be published in v1.
    #[error("file is empty; v1 publishes only non-empty files")]
    EmptyFile,
    /// The file exceeded the 4 GiB v1 limit.
    #[error("file exceeds the 4 GiB v1 limit")]
    FileTooLarge,
    /// A content-ID string was not 64 hexadecimal characters.
    #[error("content ID must be 64 hexadecimal characters")]
    InvalidContentHash,
    /// A chunk index was outside the manifest.
    #[error("chunk index {index} is outside the manifest")]
    ChunkIndexOutOfRange {
        /// Requested index.
        index: usize,
    },
    /// A chunk did not have the length recorded in the manifest.
    #[error("chunk {index} has {actual} bytes; the manifest records {expected}")]
    ChunkLengthMismatch {
        /// Chunk index.
        index: usize,
        /// Manifest length.
        expected: u64,
        /// Received length.
        actual: u64,
    },
    /// A chunk's SHA-256 did not match the manifest.
    #[error("chunk {index} does not match its manifest hash")]
    ChunkHashMismatch {
        /// Chunk index.
        index: usize,
    },
    /// The complete file hash did not match the manifest content ID.
    #[error("content hash mismatch: expected {expected}, computed {actual}")]
    ContentHashMismatch {
        /// Expected content ID.
        expected: ContentHash,
        /// Computed content ID.
        actual: ContentHash,
    },
    /// The complete file size did not match the manifest.
    #[error("content size mismatch: expected {expected} bytes, read {actual}")]
    SizeMismatch {
        /// Expected size.
        expected: u64,
        /// Observed size.
        actual: u64,
    },
    /// The supplied manifest bytes did not hash to the expected root.
    #[error("manifest root mismatch: expected {expected}, computed {actual}")]
    ManifestRootMismatch {
        /// Expected root as lowercase hexadecimal.
        expected: String,
        /// Computed root as lowercase hexadecimal.
        actual: String,
    },
}

/// Streams a reader and returns the raw SHA-256 content ID.
pub fn hash_reader<R: Read>(reader: &mut R) -> Result<ContentHash, TransferError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(ContentHash(hasher.finalize().into()))
}

/// Hashes a file and returns its raw SHA-256 content ID.
pub fn hash_file(path: impl AsRef<Path>) -> Result<ContentHash, TransferError> {
    let mut file = File::open(path)?;
    hash_reader(&mut file)
}

/// Streams a reader and builds a v1 manifest with per-chunk SHA-256 hashes.
///
/// The reader is consumed once with a single 1 MiB chunk buffer. Empty files
/// and files above [`MAX_FILE_SIZE`] are rejected.
pub fn build_manifest<R: Read>(reader: &mut R) -> Result<ManifestedContent, TransferError> {
    let mut whole = Sha256::new();
    let mut chunks = Vec::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; usize::try_from(CHUNK_SIZE_V1).unwrap_or(usize::MAX)];

    loop {
        let mut filled = 0;
        while filled < buffer.len() {
            let read = reader.read(&mut buffer[filled..])?;
            if read == 0 {
                break;
            }
            filled += read;
        }
        if filled == 0 {
            break;
        }

        total = total
            .checked_add(u64::try_from(filled).unwrap_or(u64::MAX))
            .ok_or(TransferError::FileTooLarge)?;
        if total > MAX_FILE_SIZE {
            return Err(TransferError::FileTooLarge);
        }

        let chunk = &buffer[..filled];
        whole.update(chunk);
        let mut chunk_hasher = Sha256::new();
        chunk_hasher.update(chunk);
        chunks.push(ChunkEntry::new(
            u64::try_from(filled).unwrap_or(u64::MAX),
            chunk_hasher.finalize().into(),
        ));

        if filled < buffer.len() {
            break;
        }
    }

    if total == 0 {
        return Err(TransferError::EmptyFile);
    }

    let content_hash = ContentHash(whole.finalize().into());
    let manifest = ManifestV1::new(*content_hash.as_bytes(), total, chunks);
    // Validates chunk count, lengths, total size, chunk size, and 256 KiB cap.
    let _ = manifest.encode_body()?;

    Ok(ManifestedContent {
        content_hash,
        size: total,
        manifest,
    })
}

/// Builds a manifest for a file on disk.
pub fn build_manifest_for_file(path: impl AsRef<Path>) -> Result<ManifestedContent, TransferError> {
    let mut file = File::open(path)?;
    build_manifest(&mut file)
}

/// Verifies one chunk against its manifest entry.
pub fn verify_chunk(manifest: &ManifestV1, index: usize, chunk: &[u8]) -> Result<(), TransferError> {
    let entry = manifest
        .chunk(index)
        .ok_or(TransferError::ChunkIndexOutOfRange { index })?;
    let actual = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
    if actual != entry.length {
        return Err(TransferError::ChunkLengthMismatch {
            index,
            expected: entry.length,
            actual,
        });
    }
    let mut hasher = Sha256::new();
    hasher.update(chunk);
    let digest: [u8; 32] = hasher.finalize().into();
    if digest != entry.sha256 {
        return Err(TransferError::ChunkHashMismatch { index });
    }
    Ok(())
}

/// Parses manifest bytes and checks them against a trusted manifest root.
pub fn verify_manifest_bytes(body: &[u8], expected_root: [u8; 32]) -> Result<ManifestV1, TransferError> {
    let manifest = ManifestV1::parse_body(body)?;
    let actual = manifest_root(body);
    if actual != expected_root {
        return Err(TransferError::ManifestRootMismatch {
            expected: nexus_crypto::to_hex(&expected_root),
            actual: nexus_crypto::to_hex(&actual),
        });
    }
    Ok(manifest)
}

/// Verifies manifest bytes against a signed publication record.
///
/// This checks the signed root, parses the manifest, and confirms that the
/// content ID, size, chunk size, and root all match the publication.
pub fn verify_publication_manifest(publication: &SignedPublication, body: &[u8]) -> Result<ManifestV1, TransferError> {
    let metadata = publication.publication();
    let manifest = verify_manifest_bytes(body, metadata.manifest_root)?;
    manifest.validate_for_publication(metadata)?;
    Ok(manifest)
}

/// An in-order streaming verifier for downloaded chunks.
///
/// A verifier consumes chunks in manifest order, hashes them into the
/// whole-file SHA-256, and only succeeds once every chunk has been seen and the
/// content ID matches.
#[derive(Clone, Debug)]
pub struct ManifestVerifier {
    manifest: ManifestV1,
    next_index: usize,
    verified_bytes: u64,
    hasher: Sha256,
}

impl ManifestVerifier {
    /// Creates a verifier for a parsed manifest.
    #[must_use]
    pub fn new(manifest: ManifestV1) -> Self {
        Self {
            manifest,
            next_index: 0,
            verified_bytes: 0,
            hasher: Sha256::new(),
        }
    }

    /// Returns the manifest under verification.
    #[must_use]
    pub const fn manifest(&self) -> &ManifestV1 {
        &self.manifest
    }

    /// Returns the next expected chunk index.
    #[must_use]
    pub const fn next_index(&self) -> usize {
        self.next_index
    }

    /// Returns how many bytes have passed chunk verification.
    #[must_use]
    pub const fn verified_bytes(&self) -> u64 {
        self.verified_bytes
    }

    /// Returns whether every chunk has been verified.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.next_index == self.manifest.chunk_count()
    }

    /// Verifies and consumes the next chunk in manifest order.
    pub fn verify_next(&mut self, chunk: &[u8]) -> Result<(), TransferError> {
        let index = self.next_index;
        verify_chunk(&self.manifest, index, chunk)?;
        self.hasher.update(chunk);
        self.verified_bytes = self
            .verified_bytes
            .checked_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX))
            .ok_or(TransferError::FileTooLarge)?;
        self.next_index = index.checked_add(1).ok_or(TransferError::FileTooLarge)?;
        Ok(())
    }

    /// Completes verification after every chunk has been consumed.
    pub fn finish(self) -> Result<ContentHash, TransferError> {
        if !self.is_complete() {
            return Err(TransferError::ChunkIndexOutOfRange { index: self.next_index });
        }
        if self.verified_bytes != self.manifest.total_size {
            return Err(TransferError::SizeMismatch {
                expected: self.manifest.total_size,
                actual: self.verified_bytes,
            });
        }
        let actual = ContentHash(self.hasher.finalize().into());
        let expected = ContentHash(self.manifest.content_id);
        if actual != expected {
            return Err(TransferError::ContentHashMismatch { expected, actual });
        }
        Ok(actual)
    }
}

/// Verifies a complete reader against a manifest and returns its content ID.
///
/// The reader must supply exactly the chunk lengths recorded in the manifest
/// and no trailing bytes. Verification is chunk-by-chunk with bounded memory.
pub fn verify_file<R: Read>(reader: &mut R, manifest: &ManifestV1) -> Result<ContentHash, TransferError> {
    let mut verifier = ManifestVerifier::new(manifest.clone());
    let mut buffer = vec![0_u8; usize::try_from(CHUNK_SIZE_V1).unwrap_or(usize::MAX)];

    for index in 0..manifest.chunk_count() {
        let entry = manifest
            .chunk(index)
            .ok_or(TransferError::ChunkIndexOutOfRange { index })?;
        let length = usize::try_from(entry.length).map_err(|_| TransferError::ChunkLengthMismatch {
            index,
            expected: entry.length,
            actual: 0,
        })?;
        if length > buffer.len() {
            return Err(TransferError::ChunkLengthMismatch {
                index,
                expected: entry.length,
                actual: u64::try_from(buffer.len()).unwrap_or(u64::MAX),
            });
        }
        if let Err(error) = reader.read_exact(&mut buffer[..length]) {
            if error.kind() == io::ErrorKind::UnexpectedEof {
                return Err(TransferError::SizeMismatch {
                    expected: manifest.total_size,
                    actual: verifier.verified_bytes(),
                });
            }
            return Err(TransferError::Io(error));
        }
        verifier.verify_next(&buffer[..length])?;
    }

    let mut trailing = [0_u8; 1];
    if reader.read(&mut trailing)? != 0 {
        return Err(TransferError::SizeMismatch {
            expected: manifest.total_size,
            actual: manifest.total_size.saturating_add(1),
        });
    }

    verifier.finish()
}

/// Verifies a file on disk against a manifest and returns its content ID.
pub fn verify_file_path(path: impl AsRef<Path>, manifest: &ManifestV1) -> Result<ContentHash, TransferError> {
    let mut file = File::open(path)?;
    verify_file(&mut file, manifest)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        ContentHash, ManifestVerifier, TransferError, build_manifest, hash_reader, verify_chunk, verify_file,
        verify_manifest_bytes,
    };
    use nexus_crypto::sha256;

    #[test]
    fn hash_reader_matches_known_vectors() {
        let mut empty = Cursor::new(Vec::new());
        assert_eq!(
            hash_reader(&mut empty).expect("hash").to_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        let mut abc = Cursor::new(b"abc".to_vec());
        assert_eq!(
            hash_reader(&mut abc).expect("hash").to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn content_hash_hex_round_trips() {
        let hash = ContentHash::from_bytes([0xab_u8; 32]);
        let text = hash.to_hex();
        assert_eq!(text, "abababababababababababababababababababababababababababababababab");
        assert_eq!(ContentHash::parse_hex(&text).expect("parse"), hash);
        assert!(ContentHash::parse_hex("ab").is_err());
        assert!(ContentHash::parse_hex(&"zz".repeat(32)).is_err());
    }

    #[test]
    fn build_manifest_round_trips_through_verification() {
        let data = b"nexus streaming transfer".repeat(40_000);
        let built = build_manifest(&mut Cursor::new(data.clone())).expect("build");
        assert_eq!(built.size(), u64::try_from(data.len()).expect("size"));
        assert_eq!(built.content_hash().as_bytes(), &sha256(&data));
        assert_eq!(built.manifest().chunk_count(), 1);
        assert_eq!(built.manifest().chunk(0).expect("chunk").sha256, sha256(&data));

        let body = built.manifest_bytes().expect("body");
        verify_manifest_bytes(&body, built.manifest_root().expect("root")).expect("manifest root");

        let mut verifier = ManifestVerifier::new(built.manifest().clone());
        verifier.verify_next(&data).expect("chunk");
        assert!(verifier.is_complete());
        assert_eq!(verifier.finish().expect("finish"), built.content_hash());
        assert_eq!(
            verify_file(&mut Cursor::new(data), built.manifest()).expect("file"),
            built.content_hash()
        );
    }

    #[test]
    fn empty_files_are_rejected() {
        let error = build_manifest(&mut Cursor::new(Vec::new())).expect_err("empty");
        assert!(matches!(error, TransferError::EmptyFile));
    }

    #[test]
    fn chunk_corruption_and_length_changes_are_rejected() {
        let first = vec![0x11_u8; 1024];
        let second = vec![0x22_u8; 512];
        let built = build_manifest(&mut Cursor::new([first.clone(), second.clone()].concat())).expect("build");
        let manifest = built.manifest();
        assert_eq!(manifest.chunk_count(), 1);

        verify_chunk(manifest, 0, &[first.clone(), second.clone()].concat()).expect("valid chunk");

        let mut corrupted = [first, second].concat();
        corrupted[0] ^= 0xff;
        assert!(matches!(
            verify_chunk(manifest, 0, &corrupted),
            Err(TransferError::ChunkHashMismatch { index: 0 })
        ));

        assert!(matches!(
            verify_chunk(manifest, 0, &corrupted[..100]),
            Err(TransferError::ChunkLengthMismatch { index: 0, .. })
        ));
        assert!(matches!(
            verify_chunk(manifest, 1, &corrupted),
            Err(TransferError::ChunkIndexOutOfRange { index: 1 })
        ));
    }

    #[test]
    fn verifier_rejects_missing_trailing_and_wrong_order_data() {
        let data = vec![0x5a_u8; 2048];
        let built = build_manifest(&mut Cursor::new(data.clone())).expect("build");
        let manifest = built.manifest();

        let mut incomplete = ManifestVerifier::new(manifest.clone());
        assert!(incomplete.clone().finish().is_err());
        incomplete.verify_next(&data).expect("chunk");
        assert_eq!(incomplete.finish().expect("finish"), built.content_hash());

        let mut extra = data.clone();
        extra.push(0);
        assert!(matches!(
            verify_file(&mut Cursor::new(extra), manifest),
            Err(TransferError::SizeMismatch { .. })
        ));

        let mut truncated = data;
        truncated.pop();
        assert!(matches!(
            verify_file(&mut Cursor::new(truncated), manifest),
            Err(TransferError::SizeMismatch { .. })
        ));
    }

    #[test]
    fn manifest_root_mismatch_is_rejected() {
        let built = build_manifest(&mut Cursor::new(b"root check".to_vec())).expect("build");
        let body = built.manifest_bytes().expect("body");
        assert!(matches!(
            verify_manifest_bytes(&body, [0x99_u8; 32]),
            Err(TransferError::ManifestRootMismatch { .. })
        ));
    }
}
