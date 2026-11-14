//! Immutable content-addressed blob store with an exclusive process lock.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use nexus_crypto::{DomainTag, domain_sha256};
use nexus_protocol::{MAX_MANIFEST_BYTES, ManifestV1};
use sha2::{Digest, Sha256};

use crate::error::{StoreError, StoreResult};
use crate::paths::validate_store_component;
use crate::util::{hex_lower, is_content_id, unique_suffix};

const IO_BUFFER_BYTES: usize = 64 * 1024;

/// A fully written temporary import awaiting atomic promotion.
pub(crate) struct StagedBlob {
    pub(crate) path: PathBuf,
    pub(crate) content_id: String,
    pub(crate) size_bytes: u64,
}

/// A promoted content-addressed blob.
pub(crate) struct BlobInfo {
    pub(crate) content_id: String,
    pub(crate) size_bytes: u64,
}

/// A file encountered while scanning a generated directory.
pub(crate) struct ScanEntry {
    pub(crate) path: PathBuf,
    pub(crate) content_id: Option<String>,
}

/// The managed store rooted at an app-private directory.
pub struct ManagedStore {
    root: PathBuf,
    blobs: PathBuf,
    manifests: PathBuf,
    partials: PathBuf,
    temporary: PathBuf,
    quarantine: PathBuf,
    max_file_size: u64,
    lock_file: File,
}

impl ManagedStore {
    /// Creates the layout and takes an exclusive store lock.
    pub fn open(root: &Path, max_file_size: u64) -> StoreResult<Self> {
        if !root.is_absolute() {
            return Err(StoreError::UnsafePath("managed store root must be absolute".to_owned()));
        }
        fs::create_dir_all(root)?;
        reject_symlink(root)?;

        let blobs = root.join("blobs");
        let manifests = root.join("manifests");
        let partials = root.join("partials");
        let temporary = root.join("tmp");
        let quarantine = root.join("quarantine");
        for directory in [&blobs, &manifests, &partials, &temporary, &quarantine] {
            fs::create_dir_all(directory)?;
            reject_symlink(directory)?;
        }

        let lock_path = root.join(".lock");
        let lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock_path)?;
        lock_file.try_lock_exclusive().map_err(|_| StoreError::StoreLocked)?;

        Ok(Self {
            root: root.to_path_buf(),
            blobs,
            manifests,
            partials,
            temporary,
            quarantine,
            max_file_size,
            lock_file,
        })
    }

    /// Managed-store root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Blob directory, exposed for diagnostics and reconciliation tests.
    #[must_use]
    pub fn blobs_directory(&self) -> &Path {
        &self.blobs
    }

    /// Partials directory.
    #[must_use]
    pub fn partials_directory(&self) -> &Path {
        &self.partials
    }

    /// Quarantine directory.
    #[must_use]
    pub fn quarantine_directory(&self) -> &Path {
        &self.quarantine
    }

    /// Bytes currently available on the store volume.
    pub fn available_space(&self) -> StoreResult<u64> {
        fs2::available_space(&self.root).map_err(StoreError::Io)
    }

    /// Generated blob path for a content ID.
    pub fn blob_path(&self, content_id: &str) -> StoreResult<PathBuf> {
        if !is_content_id(content_id) {
            return Err(StoreError::InvalidContentId(content_id.to_owned()));
        }
        Ok(self
            .blobs
            .join(&content_id[0..2])
            .join(&content_id[2..4])
            .join(content_id))
    }

    /// Generated relative blob path stored in the database.
    pub fn blob_relative_path(content_id: &str) -> StoreResult<String> {
        if !is_content_id(content_id) {
            return Err(StoreError::InvalidContentId(content_id.to_owned()));
        }
        Ok(format!(
            "blobs/{}/{}/{}",
            &content_id[0..2],
            &content_id[2..4],
            content_id
        ))
    }

    /// Generated partial path for a download ID.
    pub fn partial_path(&self, download_id: &str) -> StoreResult<PathBuf> {
        validate_store_component(download_id, "download ID")?;
        Ok(self.partials.join(format!("{download_id}.part")))
    }

    /// Generated relative partial path stored in the database.
    pub fn partial_relative_path(download_id: &str) -> StoreResult<String> {
        validate_store_component(download_id, "download ID")?;
        Ok(format!("partials/{download_id}.part"))
    }

    /// Generated manifest path for a trusted manifest root.
    pub fn manifest_path(&self, root: &str) -> StoreResult<PathBuf> {
        if !is_content_id(root) {
            return Err(StoreError::InvalidContentId(root.to_owned()));
        }
        Ok(self.manifests.join(format!("{root}.cbor")))
    }

    /// Stores exact canonical manifest bytes under their trusted root.
    ///
    /// Identical bytes are idempotent. A corrupt file occupying the same name
    /// is quarantined before verified bytes are written.
    pub(crate) fn write_manifest(&self, expected_root: &str, bytes: &[u8]) -> StoreResult<()> {
        let path = self.manifest_path(expected_root)?;
        let Some(root_length) = u64::try_from(MAX_MANIFEST_BYTES).ok() else {
            return Err(StoreError::InvalidManifest(expected_root.to_owned()));
        };
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > root_length {
            return Err(StoreError::InvalidManifest(expected_root.to_owned()));
        }
        let actual = hex_lower(&domain_sha256(DomainTag::Manifest, bytes));
        if actual != expected_root {
            return Err(StoreError::InvalidManifest(expected_root.to_owned()));
        }

        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                self.quarantine_file(&path, expected_root)?;
            } else if fs::read(&path).is_ok_and(|existing| existing == bytes) {
                return Ok(());
            } else {
                self.quarantine_file(&path, expected_root)?;
            }
        }

        let temporary = self.temporary.join(format!("manifest-{}.part", unique_suffix()));
        let result = (|| -> StoreResult<()> {
            let mut file = OpenOptions::new().create_new(true).write(true).open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &path)?;
            sync_directory(&self.manifests);
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    /// Reads and verifies exact manifest bytes by their trusted root.
    pub(crate) fn read_manifest(&self, root: &str) -> StoreResult<Vec<u8>> {
        let path = self.manifest_path(root)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(StoreError::ManifestMissing(root.to_owned()));
            }
            Err(error) => return Err(StoreError::Io(error)),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(StoreError::InvalidManifest(root.to_owned()));
        }
        if metadata.len() > u64::try_from(MAX_MANIFEST_BYTES).unwrap_or(u64::MAX) {
            return Err(StoreError::InvalidManifest(root.to_owned()));
        }
        let bytes = fs::read(&path)?;
        let actual = hex_lower(&domain_sha256(DomainTag::Manifest, &bytes));
        if actual != root {
            return Err(StoreError::InvalidManifest(root.to_owned()));
        }
        Ok(bytes)
    }

    /// Verifies a named manifest file during reconciliation.
    pub(crate) fn verify_manifest_file(&self, path: &Path, root: &str) -> StoreResult<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > u64::try_from(MAX_MANIFEST_BYTES).unwrap_or(u64::MAX)
        {
            return Err(StoreError::InvalidManifest(root.to_owned()));
        }
        let bytes = fs::read(path)?;
        let actual = hex_lower(&domain_sha256(DomainTag::Manifest, &bytes));
        if actual != root {
            return Err(StoreError::InvalidManifest(root.to_owned()));
        }
        ManifestV1::parse_body(&bytes).map_err(|_| StoreError::InvalidManifest(root.to_owned()))?;
        Ok(())
    }

    /// Copies a source file into a temporary file while hashing it.
    ///
    /// The source is opened read-only and is never modified or deleted.
    pub(crate) fn stage_file(&self, source: &Path) -> StoreResult<StagedBlob> {
        let metadata = fs::symlink_metadata(source)?;
        if metadata.file_type().is_symlink() {
            return Err(StoreError::UnsafePath(
                "source is a symbolic link or reparse point".to_owned(),
            ));
        }
        if !metadata.is_file() {
            return Err(StoreError::InvalidFilename("source must be a regular file".to_owned()));
        }
        if metadata.len() > self.max_file_size {
            return Err(StoreError::FileTooLarge {
                actual: metadata.len(),
                limit: self.max_file_size,
            });
        }

        let staged_path = self.temporary.join(format!("import-{}.part", unique_suffix()));
        let mut source_file = File::open(source)?;
        let mut staged_file = OpenOptions::new().create_new(true).write(true).open(&staged_path)?;
        let mut hasher = Sha256::new();
        let mut size_bytes = 0_u64;
        let mut buffer = [0_u8; IO_BUFFER_BYTES];

        let result = (|| -> StoreResult<()> {
            loop {
                let read = source_file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                size_bytes = size_bytes.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
                if size_bytes > self.max_file_size {
                    return Err(StoreError::FileTooLarge {
                        actual: size_bytes,
                        limit: self.max_file_size,
                    });
                }
                hasher.update(&buffer[..read]);
                staged_file.write_all(&buffer[..read])?;
            }
            staged_file.sync_all()?;
            Ok(())
        })();

        if let Err(error) = result {
            drop(staged_file);
            let _ = fs::remove_file(&staged_path);
            return Err(error);
        }

        Ok(StagedBlob {
            path: staged_path,
            content_id: hex_lower(hasher.finalize().as_slice()),
            size_bytes,
        })
    }

    /// Atomically promotes a staged file into the content-addressed layout.
    ///
    /// If identical bytes already exist, the staged copy is discarded and the
    /// existing immutable blob is kept.
    pub(crate) fn promote(&self, staged: StagedBlob) -> StoreResult<BlobInfo> {
        let destination = self.blob_path(&staged.content_id)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        if destination.is_file() {
            if self.hash_matches(&destination, &staged.content_id, staged.size_bytes)? {
                let _ = fs::remove_file(&staged.path);
                return Ok(BlobInfo {
                    content_id: staged.content_id,
                    size_bytes: staged.size_bytes,
                });
            }
            self.quarantine_file(&destination, &staged.content_id)?;
        }

        fs::rename(&staged.path, &destination)?;
        if let Some(parent) = destination.parent() {
            sync_directory(parent);
        }
        Ok(BlobInfo {
            content_id: staged.content_id,
            size_bytes: staged.size_bytes,
        })
    }

    /// Verifies a blob's size and SHA-256 content ID.
    pub fn verify_blob(&self, content_id: &str, expected_size: u64) -> StoreResult<()> {
        let path = self.blob_path(content_id)?;
        let metadata = fs::metadata(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StoreError::ContentMissing(content_id.to_owned())
            } else {
                StoreError::Io(error)
            }
        })?;
        if metadata.len() != expected_size {
            return Err(StoreError::IntegrityMismatch(content_id.to_owned()));
        }
        if !self.hash_matches(&path, content_id, expected_size)? {
            return Err(StoreError::IntegrityMismatch(content_id.to_owned()));
        }
        Ok(())
    }

    /// Deletes a blob if it is present.
    pub(crate) fn remove_blob(&self, content_id: &str) -> StoreResult<()> {
        let path = self.blob_path(content_id)?;
        if path.exists() {
            fs::remove_file(path)?;
            if let Some(parent) = self.blobs.join(&content_id[0..2]).join(&content_id[2..4]).parent() {
                sync_directory(parent);
            }
        }
        Ok(())
    }

    /// Copies a verified blob to a new destination file.
    pub fn copy_blob_to(&self, content_id: &str, destination: &Path) -> StoreResult<u64> {
        let source_path = self.blob_path(content_id)?;
        let mut source = File::open(&source_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StoreError::ContentMissing(content_id.to_owned())
            } else {
                StoreError::Io(error)
            }
        })?;
        let mut destination_file = OpenOptions::new().create_new(true).write(true).open(destination)?;
        let mut hasher = Sha256::new();
        let mut size_bytes = 0_u64;
        let mut buffer = [0_u8; IO_BUFFER_BYTES];

        let result = (|| -> StoreResult<()> {
            loop {
                let read = source.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                size_bytes = size_bytes.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
                hasher.update(&buffer[..read]);
                destination_file.write_all(&buffer[..read])?;
            }
            destination_file.sync_all()?;
            Ok(())
        })();

        if let Err(error) = result {
            drop(destination_file);
            let _ = fs::remove_file(destination);
            return Err(error);
        }

        let actual = hex_lower(hasher.finalize().as_slice());
        if actual != content_id {
            drop(destination_file);
            let _ = fs::remove_file(destination);
            return Err(StoreError::IntegrityMismatch(content_id.to_owned()));
        }
        Ok(size_bytes)
    }

    /// Creates or truncates the partial file for a download.
    pub(crate) fn reset_partial(&self, download_id: &str) -> StoreResult<()> {
        let path = self.partial_path(download_id)?;
        let file = OpenOptions::new().create(true).write(true).truncate(true).open(path)?;
        file.sync_all()?;
        Ok(())
    }

    /// Truncates a partial file to its verified prefix.
    pub(crate) fn truncate_partial(&self, download_id: &str, prefix: u64) -> StoreResult<()> {
        let path = self.partial_path(download_id)?;
        let file = OpenOptions::new().write(true).open(path)?;
        file.set_len(prefix)?;
        file.sync_all()?;
        Ok(())
    }

    /// Reads the current length of a partial file.
    pub(crate) fn partial_length(&self, download_id: &str) -> StoreResult<u64> {
        let path = self.partial_path(download_id)?;
        Ok(fs::metadata(path)?.len())
    }

    /// Removes a partial file if present.
    pub(crate) fn remove_partial(&self, download_id: &str) -> StoreResult<()> {
        let path = self.partial_path(download_id)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Promotes a completed partial file into the blob layout without copying.
    pub(crate) fn promote_partial(
        &self,
        download_id: &str,
        content_id: &str,
        expected_size: u64,
    ) -> StoreResult<BlobInfo> {
        let partial = self.partial_path(download_id)?;
        if !self.hash_matches(&partial, content_id, expected_size)? {
            return Err(StoreError::IntegrityMismatch(content_id.to_owned()));
        }

        let destination = self.blob_path(content_id)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        if destination.is_file() {
            if self.hash_matches(&destination, content_id, expected_size)? {
                fs::remove_file(partial)?;
                return Ok(BlobInfo {
                    content_id: content_id.to_owned(),
                    size_bytes: expected_size,
                });
            }
            self.quarantine_file(&destination, content_id)?;
        }
        fs::rename(&partial, &destination)?;
        if let Some(parent) = destination.parent() {
            sync_directory(parent);
        }
        Ok(BlobInfo {
            content_id: content_id.to_owned(),
            size_bytes: expected_size,
        })
    }

    /// Lists every file in the generated blob directory.
    pub(crate) fn scan_blobs(&self) -> StoreResult<Vec<ScanEntry>> {
        let mut entries = Vec::new();
        for path in files_under(&self.blobs)? {
            let symlink = fs::symlink_metadata(&path)?.file_type().is_symlink();
            let content_id = if symlink {
                None
            } else {
                path.strip_prefix(&self.blobs).ok().and_then(valid_blob_relative_path)
            };
            entries.push(ScanEntry { path, content_id });
        }
        Ok(entries)
    }

    /// Lists every file in the partials directory.
    pub(crate) fn scan_partials(&self) -> StoreResult<Vec<PathBuf>> {
        files_under(&self.partials)
    }

    /// Lists every file in the manifests directory.
    pub(crate) fn scan_manifests(&self) -> StoreResult<Vec<ScanEntry>> {
        let mut entries = Vec::new();
        for path in files_under(&self.manifests)? {
            let symlink = fs::symlink_metadata(&path)?.file_type().is_symlink();
            let content_id = if symlink {
                None
            } else {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| name.strip_suffix(".cbor"))
                    .filter(|stem| is_content_id(stem))
                    .map(str::to_owned)
            };
            entries.push(ScanEntry { path, content_id });
        }
        Ok(entries)
    }

    /// Removes every entry in the temporary import directory.
    pub(crate) fn clear_temporary(&self) -> StoreResult<(usize, u64)> {
        let mut removed = 0_usize;
        let mut bytes = 0_u64;
        for entry in fs::read_dir(&self.temporary)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() || file_type.is_file() {
                bytes = bytes.saturating_add(entry.metadata()?.len());
                fs::remove_file(entry.path())?;
                removed += 1;
            } else if file_type.is_dir() {
                fs::remove_dir_all(entry.path())?;
                removed += 1;
            }
        }
        Ok((removed, bytes))
    }

    /// Moves an unexpected or failed file into quarantine.
    pub(crate) fn quarantine_file(&self, path: &Path, label: &str) -> StoreResult<PathBuf> {
        let safe_label: String = label
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                    character
                } else {
                    '_'
                }
            })
            .take(64)
            .collect();
        let destination = self.quarantine.join(format!(
            "{}-{}",
            if safe_label.is_empty() {
                "quarantined"
            } else {
                &safe_label
            },
            unique_suffix()
        ));
        fs::rename(path, &destination)?;
        Ok(destination)
    }

    /// Removes a file if it exists.
    pub(crate) fn remove_file(&self, path: &Path) -> StoreResult<bool> {
        if path.is_file() {
            fs::remove_file(path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn hash_matches(&self, path: &Path, expected_content_id: &str, expected_size: u64) -> StoreResult<bool> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(StoreError::Io(error)),
        };
        if metadata.len() != expected_size {
            return Ok(false);
        }
        let (actual, size) = hash_file(path)?;
        Ok(size == expected_size && actual == expected_content_id)
    }
}

/// Hashes a file with bounded memory.
pub(crate) fn hash_file(path: &Path) -> StoreResult<(String, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;
    let mut buffer = [0_u8; IO_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        size_bytes = size_bytes.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        hasher.update(&buffer[..read]);
    }
    Ok((hex_lower(hasher.finalize().as_slice()), size_bytes))
}

fn files_under(root: &Path) -> StoreResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                // Never recurse through a link or follow it during cleanup.
                files.push(entry.path());
            } else if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                files.push(entry.path());
            }
        }
    }
    Ok(files)
}

fn valid_blob_relative_path(relative: &Path) -> Option<String> {
    let components: Vec<&str> = relative
        .iter()
        .map(|component| component.to_str())
        .collect::<Option<Vec<_>>>()?;
    if components.len() != 3 {
        return None;
    }
    let (first, second, name) = (components[0], components[1], components[2]);
    if first.len() != 2 || second.len() != 2 || !is_content_id(name) {
        return None;
    }
    if !first.bytes().all(is_lower_hex) || !second.bytes().all(is_lower_hex) {
        return None;
    }
    if name.starts_with(first) && name[2..].starts_with(second) {
        Some(name.to_owned())
    } else {
        None
    }
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn reject_symlink(path: &Path) -> StoreResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(StoreError::UnsafePath(format!(
            "{} is a symbolic link or reparse point",
            path.display()
        )));
    }
    Ok(())
}

fn sync_directory(path: &Path) {
    if let Ok(directory) = File::open(path) {
        let _ = directory.sync_all();
    }
}

impl Drop for ManagedStore {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.lock_file);
    }
}

#[cfg(test)]
mod tests {
    use super::ManagedStore;
    use std::fs;
    use std::path::PathBuf;

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("nexus-store-unit-{}-{label}", crate::util::unique_suffix()));
        fs::create_dir_all(&root).expect("create temporary root");
        root
    }

    #[test]
    fn writes_blobs_under_generated_paths() {
        let root = temporary_root("blob-path");
        let store = ManagedStore::open(&root, 1024).expect("open store");
        let content_id = "ab".repeat(32);
        let path = store.blob_path(&content_id).expect("blob path");
        assert!(path.ends_with(format!("ab/{}/{}", &content_id[2..4], content_id)));
        assert!(path.starts_with(store.blobs_directory()));

        let outside = store.blob_path("../escape").expect_err("reject traversal");
        assert!(outside.to_string().contains("invalid content ID"));
        drop(store);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn rejects_a_second_store_on_the_same_root() {
        let root = temporary_root("lock");
        let first = ManagedStore::open(&root, 1024).expect("first store");
        let second = ManagedStore::open(&root, 1024);
        assert!(matches!(second, Err(crate::StoreError::StoreLocked)));
        drop(first);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn rejects_relative_roots() {
        let result = ManagedStore::open(std::path::Path::new("relative/store"), 1024);
        assert!(matches!(result, Err(crate::StoreError::UnsafePath(_))));
    }
}
