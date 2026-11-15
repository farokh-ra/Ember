//! Bounded streaming import and export cursors.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use nexus_protocol::{CHUNK_SIZE_V1, ChunkEntry, ManifestV1, ProtocolError};
use nexus_transfer::ContentHash;
use sha2::{Digest, Sha256};

/// Maximum bytes accepted by one import write or export read.
pub const MAX_STREAM_CHUNK: usize = 1024 * 1024;

/// An in-progress streaming import.
pub struct ImportSession {
    import_id: u64,
    path: PathBuf,
    file: Option<File>,
    display_name: String,
    expected_size: u64,
    written: u64,
    whole: Sha256,
    chunk: Sha256,
    chunk_len: usize,
    chunks: Vec<ChunkEntry>,
}

/// A finished import ready for store adoption.
#[derive(Debug)]
pub struct FinishedImport {
    /// Lowercase hexadecimal content ID.
    pub content_id: String,
    /// Raw SHA-256 content ID.
    pub content_hash: ContentHash,
    /// Exact file size.
    pub size: u64,
    /// Lowercase hexadecimal manifest root.
    pub manifest_root: String,
    /// Canonical manifest bytes.
    pub manifest_bytes: Vec<u8>,
    /// Fixed chunk size.
    pub chunk_size: u64,
    /// Safe display name supplied by the host.
    pub display_name: String,
    /// Temporary file that still needs to be adopted by the store.
    pub path: PathBuf,
}

impl ImportSession {
    /// Starts an import at a temporary path.
    pub fn create(
        import_id: u64,
        path: PathBuf,
        display_name: String,
        expected_size: u64,
    ) -> Result<Self, ImportError> {
        if expected_size == 0 {
            return Err(ImportError::Empty);
        }
        if expected_size > nexus_protocol::MAX_FILE_SIZE {
            return Err(ImportError::SizeExceeded {
                expected: nexus_protocol::MAX_FILE_SIZE,
                actual: expected_size,
            });
        }
        let file = OpenOptions::new().create_new(true).write(true).open(&path)?;
        Ok(Self {
            import_id,
            path,
            file: Some(file),
            display_name,
            expected_size,
            written: 0,
            whole: Sha256::new(),
            chunk: Sha256::new(),
            chunk_len: 0,
            chunks: Vec::new(),
        })
    }

    /// Import identifier.
    #[must_use]
    pub const fn import_id(&self) -> u64 {
        self.import_id
    }

    /// Temporary file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bytes written so far.
    #[must_use]
    pub const fn written(&self) -> u64 {
        self.written
    }

    /// Declared final size.
    #[must_use]
    pub const fn expected_size(&self) -> u64 {
        self.expected_size
    }

    /// Writes one bounded chunk and updates the whole-file and chunk hashes.
    pub fn write(&mut self, data: &[u8]) -> Result<(), ImportError> {
        if data.len() > MAX_STREAM_CHUNK {
            return Err(ImportError::ChunkTooLarge);
        }
        let incoming = u64::try_from(data.len()).unwrap_or(u64::MAX);
        let Some(next) = self.written.checked_add(incoming) else {
            return Err(ImportError::SizeExceeded {
                expected: self.expected_size,
                actual: u64::MAX,
            });
        };
        if next > self.expected_size {
            return Err(ImportError::SizeMismatch {
                expected: self.expected_size,
                actual: next,
            });
        }

        let file = self
            .file
            .as_mut()
            .ok_or_else(|| ImportError::Io(io::Error::other("import is already closed")))?;
        file.write_all(data)?;
        self.whole.update(data);

        let mut offset = 0;
        while offset < data.len() {
            let remaining = MAX_STREAM_CHUNK - self.chunk_len;
            let take = remaining.min(data.len() - offset);
            self.chunk.update(&data[offset..offset + take]);
            self.chunk_len += take;
            offset += take;
            if self.chunk_len == MAX_STREAM_CHUNK {
                self.flush_chunk(MAX_STREAM_CHUNK);
            }
        }
        self.written = next;
        Ok(())
    }

    /// Finalizes hashes and the manifest, closes the file, and returns the
    /// information the engine needs to adopt the bytes.
    pub fn finish(mut self) -> Result<FinishedImport, ImportError> {
        if self.written != self.expected_size {
            return Err(ImportError::SizeMismatch {
                expected: self.expected_size,
                actual: self.written,
            });
        }
        if self.written == 0 {
            return Err(ImportError::Empty);
        }
        if let Some(mut file) = self.file.take() {
            file.flush()?;
            file.sync_all()?;
        }
        if self.chunk_len > 0 {
            self.flush_chunk(self.chunk_len);
        }

        let content_hash: ContentHash =
            ContentHash::from_bytes(std::mem::replace(&mut self.whole, Sha256::new()).finalize().into());
        let chunks = std::mem::take(&mut self.chunks);
        let manifest = ManifestV1::new(*content_hash.as_bytes(), self.expected_size, chunks);
        let manifest_bytes = manifest.encode_body().map_err(ImportError::Manifest)?;
        let manifest_root = manifest.root().map_err(ImportError::Manifest)?;

        Ok(FinishedImport {
            content_id: content_hash.to_hex(),
            content_hash,
            size: self.expected_size,
            manifest_root: nexus_crypto::to_hex(&manifest_root),
            manifest_bytes,
            chunk_size: CHUNK_SIZE_V1,
            display_name: std::mem::take(&mut self.display_name),
            path: std::mem::take(&mut self.path),
        })
    }

    /// Removes the temporary file.
    pub fn abort(mut self) -> io::Result<()> {
        self.file.take();
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn flush_chunk(&mut self, length: usize) {
        let digest = std::mem::replace(&mut self.chunk, Sha256::new()).finalize();
        self.chunks
            .push(ChunkEntry::new(u64::try_from(length).unwrap_or(0), digest.into()));
        self.chunk_len = 0;
    }
}

impl Drop for ImportSession {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            drop(file);
            // An import dropped without finish or abort is cleaned up here.
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// A bounded read cursor over a verified content blob.
pub struct ExportCursor {
    path: PathBuf,
    file: File,
    size: u64,
    offset: u64,
}

impl ExportCursor {
    /// Opens a read cursor.
    pub fn open(path: PathBuf, size: u64) -> Result<Self, io::Error> {
        let file = File::open(&path)?;
        Ok(Self {
            path,
            file,
            size,
            offset: 0,
        })
    }

    /// Reads up to `max_len` bytes. An empty result means end of file.
    pub fn read(&mut self, max_len: usize) -> Result<Vec<u8>, io::Error> {
        let limit = max_len.min(MAX_STREAM_CHUNK);
        if limit == 0 || self.offset >= self.size {
            return Ok(Vec::new());
        }
        let remaining = usize::try_from(self.size - self.offset).unwrap_or(usize::MAX);
        let capacity = limit.min(remaining);
        let mut buffer = vec![0_u8; capacity];
        let read = self.file.read(&mut buffer)?;
        buffer.truncate(read);
        self.offset = self.offset.saturating_add(u64::try_from(read).unwrap_or(0));
        Ok(buffer)
    }

    /// File path, for diagnostics and tests.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bytes read so far.
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Complete file size.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }
}

/// Streaming import failure.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// A filesystem operation failed.
    #[error("streaming import I/O failed: {0}")]
    Io(#[from] io::Error),
    /// A single write exceeded 1 MiB.
    #[error("import chunk exceeds the 1 MiB limit")]
    ChunkTooLarge,
    /// The declared size did not match the bytes written.
    #[error("import size mismatch: expected {expected} bytes, observed {actual}")]
    SizeMismatch {
        /// Declared size.
        expected: u64,
        /// Observed size.
        actual: u64,
    },
    /// The declared size is outside the supported range.
    #[error("import size {actual} exceeds the supported {expected}-byte limit")]
    SizeExceeded {
        /// Maximum supported size.
        expected: u64,
        /// Requested size.
        actual: u64,
    },
    /// Empty files are not valid v1 content.
    #[error("empty files are not supported in v1")]
    Empty,
    /// The manifest could not be built.
    #[error("manifest build failed: {0}")]
    Manifest(ProtocolError),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::{ExportCursor, ImportSession, MAX_STREAM_CHUNK};
    use nexus_crypto::sha256;

    struct TempDirectory(std::path::PathBuf);

    impl TempDirectory {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "nexus-streaming-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos()
            ));
            fs::create_dir_all(&path).expect("temp directory");
            Self(path)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn import_finishes_with_raw_hash_and_manifest() {
        let directory = TempDirectory::new("finish");
        let path = directory.0.join("import.part");
        let mut session = ImportSession::create(1, path.clone(), "hello.bin".to_owned(), 5).expect("session");
        session.write(b"he").expect("write");
        session.write(b"llo").expect("write");
        let finished = session.finish().expect("finish");
        assert_eq!(finished.content_id, nexus_crypto::to_hex(&sha256(b"hello")));
        assert_eq!(finished.size, 5);
        assert!(path.is_file());
        assert_eq!(finished.manifest_bytes.len(), {
            let manifest = nexus_protocol::ManifestV1::parse_body(&finished.manifest_bytes).expect("manifest");
            manifest.encode_body().expect("canonical").len()
        });
    }

    #[test]
    fn import_rejects_size_mismatch_and_oversized_writes() {
        let directory = TempDirectory::new("reject");
        let path = directory.0.join("import.part");
        let mut session = ImportSession::create(2, path, "file.bin".to_owned(), 3).expect("session");
        assert!(session.write(&vec![0_u8; MAX_STREAM_CHUNK + 1]).is_err());
        assert!(session.write(b"four").is_err());
        let error = session.finish().expect_err("size mismatch");
        assert!(matches!(error, super::ImportError::SizeMismatch { .. }));
    }

    #[test]
    fn export_reads_bounded_chunks_and_eof() {
        let directory = TempDirectory::new("export");
        let path = directory.0.join("blob");
        let mut file = fs::File::create(&path).expect("file");
        file.write_all(b"hello world").expect("write");
        drop(file);
        let mut cursor = ExportCursor::open(path, 11).expect("cursor");
        assert_eq!(cursor.read(5).expect("read"), b"hello");
        assert_eq!(cursor.read(5).expect("read"), b" worl");
        assert_eq!(cursor.read(5).expect("read"), b"d");
        assert!(cursor.read(5).expect("read").is_empty());
        assert_eq!(cursor.offset(), 11);
    }
}
