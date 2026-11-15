//! Versioned metadata export and restore.
//!
//! An archive is a fixed header, a CBOR manifest, and a consistent SQLite
//! snapshot. File bytes and secrets are never part of the archive.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::{StoreError, StoreResult};
use crate::managed_store::hash_file;
use crate::migrations;
use crate::util::{is_content_id, now_secs, unique_suffix};

const FORMAT_NAME: &str = "NEXUS-METADATA";
const FORMAT_VERSION: u16 = 1;
const ARCHIVE_MAGIC: &[u8; 15] = b"NEXUS-METADATA\0";
const MAX_MANIFEST_BYTES: u32 = 64 * 1024;
const MAX_METADATA_DATABASE_BYTES: u64 = 1024 * 1024 * 1024;
const IO_BUFFER_BYTES: usize = 64 * 1024;

/// Result of a metadata export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportReport {
    /// Schema version in the snapshot.
    pub schema_version: i64,
    /// Snapshot size in bytes.
    pub database_size: u64,
    /// Snapshot SHA-256.
    pub database_sha256: String,
    /// Destination archive.
    pub destination: PathBuf,
}

/// Result of a metadata restore.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreReport {
    /// Schema version written by the archive.
    pub archive_schema_version: i64,
    /// Schema version after any required migrations.
    pub restored_schema_version: i64,
    /// Snapshot size in bytes.
    pub database_size: u64,
    /// Snapshot SHA-256.
    pub database_sha256: String,
    /// Archive creation time.
    pub created_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArchiveManifest {
    format: String,
    format_version: u16,
    schema_version: i64,
    created_at: u64,
    database_size: u64,
    database_sha256: String,
}

#[derive(Debug)]
struct StagedRestore {
    path: PathBuf,
    manifest: ArchiveManifest,
    restored_schema_version: i64,
}

/// Writes a consistent snapshot and CBOR manifest to `destination`.
pub(crate) fn export(database: &Database, destination: &Path) -> StoreResult<ExportReport> {
    if destination.exists() {
        return Err(StoreError::DestinationExists(destination.to_path_buf()));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| StoreError::UnsafePath("metadata export destination has no parent".to_owned()))?;
    if !parent.is_dir() {
        return Err(StoreError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "metadata export destination directory does not exist",
        )));
    }

    let snapshot = append_suffix(destination, &format!(".snapshot-{}.tmp", unique_suffix()));
    let _ = fs::remove_file(&snapshot);

    database.snapshot_to(&snapshot)?;

    let export_result = (|| -> StoreResult<ExportReport> {
        let (database_sha256, database_size) = hash_file(&snapshot)?;
        let schema_version: i64 =
            database.read(|connection| Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?))?;
        let manifest = ArchiveManifest {
            format: FORMAT_NAME.to_owned(),
            format_version: FORMAT_VERSION,
            schema_version,
            created_at: now_secs(),
            database_size,
            database_sha256: database_sha256.clone(),
        };
        let mut manifest_bytes = Vec::new();
        ciborium::ser::into_writer(&manifest, &mut manifest_bytes)
            .map_err(|error| StoreError::Serialization(error.to_string()))?;
        let manifest_length = u32::try_from(manifest_bytes.len())
            .map_err(|_| StoreError::Serialization("manifest is too large".to_owned()))?;

        let temporary = append_suffix(destination, &format!(".writing-{}.tmp", unique_suffix()));
        let write_result = (|| -> StoreResult<()> {
            let mut archive = OpenOptions::new().create_new(true).write(true).open(&temporary)?;
            archive.write_all(ARCHIVE_MAGIC)?;
            archive.write_all(&FORMAT_VERSION.to_le_bytes())?;
            archive.write_all(&manifest_length.to_le_bytes())?;
            archive.write_all(&manifest_bytes)?;

            let mut snapshot_file = File::open(&snapshot)?;
            let mut buffer = [0_u8; IO_BUFFER_BYTES];
            loop {
                let read = snapshot_file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                archive.write_all(&buffer[..read])?;
            }
            archive.sync_all()?;
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        if destination.exists() {
            let _ = fs::remove_file(&temporary);
            return Err(StoreError::DestinationExists(destination.to_path_buf()));
        }
        if let Err(error) = fs::rename(&temporary, destination) {
            let _ = fs::remove_file(&temporary);
            if destination.exists() {
                return Err(StoreError::DestinationExists(destination.to_path_buf()));
            }
            return Err(StoreError::Io(error));
        }

        Ok(ExportReport {
            schema_version,
            database_size,
            database_sha256,
            destination: destination.to_path_buf(),
        })
    })();

    let _ = fs::remove_file(&snapshot);
    export_result
}

/// Validates an archive, stages its database, and swaps it into place.
pub(crate) fn restore(database: &mut Database, archive_path: &Path) -> StoreResult<RestoreReport> {
    let staged = stage_restore(database.path(), archive_path)?;
    let restored_schema_version = staged.restored_schema_version;
    let database_size = staged.manifest.database_size;
    let database_sha256 = staged.manifest.database_sha256.clone();
    let archive_schema_version = staged.manifest.schema_version;
    let created_at = staged.manifest.created_at;
    let staged_path = staged.path;

    if let Err(error) = database.replace_with(&staged_path) {
        let _ = fs::remove_file(&staged_path);
        return Err(error);
    }

    Ok(RestoreReport {
        archive_schema_version,
        restored_schema_version,
        database_size,
        database_sha256,
        created_at,
    })
}

fn stage_restore(database_path: &Path, archive_path: &Path) -> StoreResult<StagedRestore> {
    let mut archive = File::open(archive_path)?;
    let mut magic = [0_u8; ARCHIVE_MAGIC.len()];
    archive.read_exact(&mut magic).map_err(corrupt)?;
    if &magic != ARCHIVE_MAGIC {
        return Err(StoreError::CorruptArchive("archive magic does not match".to_owned()));
    }

    let mut version_bytes = [0_u8; 2];
    archive.read_exact(&mut version_bytes).map_err(corrupt)?;
    let format_version = u16::from_le_bytes(version_bytes);
    if format_version != FORMAT_VERSION {
        return Err(StoreError::IncompatibleArchive(format!(
            "archive format version {format_version} is not supported"
        )));
    }

    let mut length_bytes = [0_u8; 4];
    archive.read_exact(&mut length_bytes).map_err(corrupt)?;
    let manifest_length = u32::from_le_bytes(length_bytes);
    if manifest_length == 0 || manifest_length > MAX_MANIFEST_BYTES {
        return Err(StoreError::CorruptArchive("manifest length is invalid".to_owned()));
    }

    let mut manifest_bytes = vec![0_u8; manifest_length as usize];
    archive.read_exact(&mut manifest_bytes).map_err(corrupt)?;
    let manifest: ArchiveManifest = ciborium::de::from_reader(manifest_bytes.as_slice())
        .map_err(|error| StoreError::CorruptArchive(format!("manifest CBOR is invalid: {error}")))?;

    validate_manifest(&manifest)?;

    let archive_size = fs::metadata(archive_path)?.len();
    let header_size = u64::try_from(ARCHIVE_MAGIC.len() + 2 + 4 + manifest_bytes.len())
        .map_err(|_| StoreError::CorruptArchive("archive size overflow".to_owned()))?;
    if archive_size < header_size {
        return Err(StoreError::CorruptArchive("archive is truncated".to_owned()));
    }
    if archive_size - header_size != manifest.database_size {
        return Err(StoreError::CorruptArchive(
            "database length does not match the manifest".to_owned(),
        ));
    }

    let staged_path = append_suffix(database_path, &format!(".restore-{}.tmp", unique_suffix()));
    let _ = fs::remove_file(&staged_path);

    let stage_result = (|| -> StoreResult<()> {
        let mut staged = OpenOptions::new().create_new(true).write(true).open(&staged_path)?;
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        let mut remaining = manifest.database_size;
        let mut buffer = [0_u8; IO_BUFFER_BYTES];
        while remaining > 0 {
            let requested = usize::try_from(remaining.min(IO_BUFFER_BYTES as u64)).unwrap_or(IO_BUFFER_BYTES);
            let read = archive.read(&mut buffer[..requested]).map_err(corrupt)?;
            if read == 0 {
                return Err(StoreError::CorruptArchive(
                    "archive database payload is truncated".to_owned(),
                ));
            }
            hasher.update(&buffer[..read]);
            staged.write_all(&buffer[..read])?;
            remaining = remaining.saturating_sub(u64::try_from(read).unwrap_or(u64::MAX));
        }
        let mut extra = [0_u8; 1];
        if archive.read(&mut extra).map_err(corrupt)? != 0 {
            return Err(StoreError::CorruptArchive("archive contains trailing data".to_owned()));
        }
        staged.sync_all()?;
        let actual = crate::util::hex_lower(hasher.finalize().as_slice());
        if actual != manifest.database_sha256 {
            return Err(StoreError::CorruptArchive(
                "database snapshot hash does not match the manifest".to_owned(),
            ));
        }
        Ok(())
    })();

    if let Err(error) = stage_result {
        let _ = fs::remove_file(&staged_path);
        return Err(error);
    }

    let restored_schema_version = match validate_staged_database(&staged_path, &manifest) {
        Ok(version) => version,
        Err(error) => {
            let _ = fs::remove_file(&staged_path);
            return Err(error);
        }
    };

    Ok(StagedRestore {
        path: staged_path,
        manifest,
        restored_schema_version,
    })
}

fn validate_manifest(manifest: &ArchiveManifest) -> StoreResult<()> {
    if manifest.format != FORMAT_NAME {
        return Err(StoreError::IncompatibleArchive(
            "manifest format name does not match".to_owned(),
        ));
    }
    if manifest.format_version != FORMAT_VERSION {
        return Err(StoreError::IncompatibleArchive(format!(
            "manifest format version {} is not supported",
            manifest.format_version
        )));
    }
    if manifest.schema_version < 1 || manifest.schema_version > migrations::LATEST_VERSION {
        return Err(StoreError::IncompatibleArchive(format!(
            "database schema version {} is not supported",
            manifest.schema_version
        )));
    }
    if manifest.database_size == 0 {
        return Err(StoreError::CorruptArchive("database snapshot is empty".to_owned()));
    }
    if manifest.database_size > MAX_METADATA_DATABASE_BYTES {
        return Err(StoreError::IncompatibleArchive(
            "database snapshot is larger than the metadata limit".to_owned(),
        ));
    }
    if !is_content_id(&manifest.database_sha256) {
        return Err(StoreError::CorruptArchive(
            "database hash is not a SHA-256 value".to_owned(),
        ));
    }
    Ok(())
}

fn validate_staged_database(staged_path: &Path, manifest: &ArchiveManifest) -> StoreResult<i64> {
    let mut connection = Connection::open_with_flags(
        staged_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;

    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(StoreError::CorruptArchive(format!(
            "database integrity check failed: {integrity}"
        )));
    }

    let mut foreign_key_violation = false;
    {
        let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        if rows.next()?.is_some() {
            foreign_key_violation = true;
        }
    }
    if foreign_key_violation {
        return Err(StoreError::CorruptArchive(
            "database contains foreign-key violations".to_owned(),
        ));
    }

    for table in [
        "schema_migrations",
        "settings",
        "contents",
        "identity_refs",
        "profiles",
        "publications",
        "tombstones",
        "content_refs",
        "downloads",
        "replicas",
        "peers",
    ] {
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        if count == 0 {
            return Err(StoreError::IncompatibleArchive(format!(
                "archive is missing required table {table}"
            )));
        }
    }

    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version != manifest.schema_version {
        return Err(StoreError::CorruptArchive(format!(
            "manifest schema version {} does not match the snapshot version {version}",
            manifest.schema_version
        )));
    }

    migrations::run(&mut connection)?;
    let restored: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    Ok(restored)
}

fn corrupt(error: std::io::Error) -> StoreError {
    StoreError::CorruptArchive(error.to_string())
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::{ARCHIVE_MAGIC, FORMAT_VERSION, append_suffix};
    use std::io::Write;

    #[test]
    fn header_constants_are_stable() {
        assert_eq!(ARCHIVE_MAGIC.len(), 15);
        assert_eq!(FORMAT_VERSION, 1);
    }

    #[test]
    fn append_suffix_keeps_the_original_name() {
        let path = std::path::Path::new("metadata.nexus");
        assert_eq!(append_suffix(path, ".tmp").to_string_lossy(), "metadata.nexus.tmp");
    }

    #[test]
    fn rejects_manifest_lengths_above_the_limit() {
        let path = std::env::temp_dir().join(format!("nexus-bad-archive-{}.bin", crate::util::unique_suffix()));
        {
            let mut file = std::fs::File::create(&path).expect("create archive");
            file.write_all(ARCHIVE_MAGIC).expect("write magic");
            file.write_all(&FORMAT_VERSION.to_le_bytes()).expect("write version");
            file.write_all(&u32::MAX.to_le_bytes()).expect("write length");
        }
        let error = crate::backup::stage_restore(std::path::Path::new("unused.sqlite3"), &path)
            .expect_err("must reject oversized manifest");
        assert!(error.to_string().contains("manifest length"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sha256_roundtrip_helper_is_stable() {
        assert_eq!(
            crate::util::sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
