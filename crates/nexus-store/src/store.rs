//! Public storage facade used by the core and later feature crates.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::types::Type;
use rusqlite::{OptionalExtension, Row, params};
use sha2::{Digest, Sha256};

use nexus_protocol::{CHUNK_SIZE_V1, Category, ChunkEntry, MAX_TAG_BYTES, MAX_TAGS, ManifestV1, SignedPublication};

use crate::backup::{self, ExportReport, RestoreReport};
use crate::chunk_map::ChunkMap;
use crate::db::Database;
use crate::error::{StoreError, StoreResult};
use crate::managed_store::ManagedStore;
use crate::model::{
    AbuseCategory, AbuseReport, BlockReason, BlockedPeer, ContentRecord, ContentReference, ContentState,
    DownloadRecord, DownloadState, HostingLimits, IdentityReference, ImportedContent, MAX_ABUSE_REPORTS,
    MAX_BLOCKED_PEERS, MAX_SECURITY_NOTE_BYTES, PeerObservation, ProfileCacheOutcome, ProfileConflict,
    ProfileFreshness, ProfileRecord, ProfileState, PublicationAvailability, PublicationCacheOutcome, PublicationRecord,
    ReferenceType, ReleaseOutcome, ReplicaRecord, ReplicaState, ReplicaUsage, TombstoneRecord,
};
use crate::paths::{safe_display_name, unique_export_path, validate_store_component};
use crate::quota::{MAX_FILE_SIZE, QuotaPolicy, QuotaUsage};
use crate::util::{hex_lower, is_content_id, is_safe_id, now_ms, now_secs};

const CONTENT_COLUMNS: &str = "content_id, size_bytes, state, blob_path, verified_at, created_at, updated_at";
const DOWNLOAD_COLUMNS: &str = "download_id, content_id, display_name, expected_size, verified_prefix, state, partial_file, created_at, updated_at, last_error, manifest_root, source_peer, verified_bytes, chunk_map";
pub(crate) const PROFILE_COLUMNS: &str = "identity_id, revision, record_bytes, display_name, username, bio, avatar_content_id, updated_at, updated_at_ms, state, published_at, last_checked_at, freshness";
pub(crate) const PUBLICATION_COLUMNS: &str = "publication_id, identity_id, content_id, revision, record_bytes, title, description, filename, published_at, updated_at, tombstoned, category, size_bytes, local, serving, announced, availability, last_checked_at";
const REPLICA_COLUMNS: &str = "replica_id, content_id, publication_id, state, source_peer, verified_size_bytes, last_verified_at, last_checked_at, last_seen_at, failure_count, last_error, announced, created_at, updated_at";
const QUOTA_SETTING: &str = "storage.quota_bytes";
const UPLOAD_LIMIT_SETTING: &str = "hosting.upload_bytes_per_second";
const ALLOW_METERED_SETTING: &str = "hosting.allow_metered";
const BATTERY_MIN_SETTING: &str = "hosting.battery_min_percent";

/// Tunable storage limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreConfig {
    /// Managed quota and free-space reserve.
    pub quota: QuotaPolicy,
    /// Maximum single file size.
    pub max_file_size: u64,
    /// SQLite busy timeout.
    pub busy_timeout: Duration,
    /// SQLite page cache in KiB.
    pub cache_size_kib: i64,
    /// Maximum locally blocked peers.
    pub max_blocked_peers: usize,
    /// Maximum local abuse reports.
    pub max_abuse_reports: usize,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            quota: QuotaPolicy::default(),
            max_file_size: MAX_FILE_SIZE,
            busy_timeout: Duration::from_secs(5),
            cache_size_kib: 4 * 1024,
            max_blocked_peers: MAX_BLOCKED_PEERS,
            max_abuse_reports: MAX_ABUSE_REPORTS,
        }
    }
}

/// A managed file exported to a host-selected directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportedContent {
    /// Collision-free destination path.
    pub path: PathBuf,
    /// Bytes written.
    pub size_bytes: u64,
}

/// Summary of startup reconciliation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReconciliationReport {
    /// Blobs whose size and hash verified.
    pub verified_blobs: usize,
    /// Named manifests whose root and canonical encoding verified.
    pub verified_manifests: usize,
    /// Completed-download crashes adopted into content records.
    pub adopted_downloads: usize,
    /// Unexpected or invalid files moved to quarantine.
    pub quarantined_files: usize,
    /// Generated blobs with no matching record removed.
    pub deleted_orphan_blobs: usize,
    /// Content rows with no remaining reference removed.
    pub removed_unreferenced_contents: usize,
    /// Content rows whose bytes are absent.
    pub missing_contents: usize,
    /// Partials truncated to their verified prefix.
    pub truncated_partials: usize,
    /// Downloads marked failed because their partial was absent.
    pub failed_downloads: usize,
    /// Unreferenced partial files left for the retention policy.
    pub orphan_partials: usize,
    /// Temporary import files removed.
    pub removed_temp_files: usize,
    /// Bytes reclaimed by cleanup.
    pub bytes_reclaimed: u64,
}

/// SQLite metadata plus the managed content store.
pub struct Store {
    root: PathBuf,
    pub(crate) db: Database,
    managed: ManagedStore,
    config: StoreConfig,
    quota: Mutex<QuotaPolicy>,
    reconciliation: ReconciliationReport,
}

impl Store {
    /// Opens the layout, runs migrations, and reconciles startup state.
    pub fn open(root: impl AsRef<Path>) -> StoreResult<Self> {
        Self::open_with_config(root, StoreConfig::default())
    }

    /// Opens with explicit limits and runtime settings.
    pub fn open_with_config(root: impl AsRef<Path>, config: StoreConfig) -> StoreResult<Self> {
        let root = root.as_ref();
        if !root.is_absolute() {
            return Err(StoreError::UnsafePath("store root must be absolute".to_owned()));
        }
        if config.max_file_size == 0 || config.max_file_size > MAX_FILE_SIZE {
            return Err(StoreError::InvalidConfig(format!(
                "maximum file size must be between 1 and {MAX_FILE_SIZE} bytes"
            )));
        }
        if config.cache_size_kib < 256 {
            return Err(StoreError::InvalidConfig(
                "SQLite cache must be at least 256 KiB".to_owned(),
            ));
        }
        if config.max_blocked_peers == 0 || config.max_blocked_peers > 65_536 {
            return Err(StoreError::InvalidConfig(
                "blocked-peer cap must be between 1 and 65536".to_owned(),
            ));
        }
        if config.max_abuse_reports == 0 || config.max_abuse_reports > 65_536 {
            return Err(StoreError::InvalidConfig(
                "abuse-report cap must be between 1 and 65536".to_owned(),
            ));
        }

        fs::create_dir_all(root)?;
        let managed = ManagedStore::open(&root.join("store"), config.max_file_size)?;
        let database_path = root.join("db").join("nexus.sqlite3");
        let db = Database::open(&database_path, config.busy_timeout, config.cache_size_kib)?;

        // A quota changed through `store.set_quota` persists independently of
        // the host-provided config. An invalid persisted value falls back to
        // the validated config instead of preventing the store from opening.
        let persisted_quota = db
            .read(|connection| {
                Ok(connection
                    .query_row("SELECT value FROM settings WHERE key = ?1", [QUOTA_SETTING], |row| {
                        row.get::<_, String>(0)
                    })
                    .optional()?)
            })
            .ok()
            .flatten()
            .and_then(|value| value.parse::<u64>().ok())
            .and_then(|bytes| QuotaPolicy::new(bytes).ok());
        let quota = persisted_quota.unwrap_or(config.quota);

        let mut store = Self {
            root: root.to_path_buf(),
            db,
            managed,
            config,
            quota: Mutex::new(quota),
            reconciliation: ReconciliationReport::default(),
        };
        store.reconciliation = store.reconcile_internal()?;
        Ok(store)
    }

    /// App-private root supplied by the platform shell.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Absolute SQLite path.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        self.db.path()
    }

    /// Absolute managed-store path.
    #[must_use]
    pub fn store_path(&self) -> &Path {
        self.managed.root()
    }

    /// Absolute temporary directory used by streaming imports.
    #[must_use]
    pub fn temporary_directory(&self) -> &Path {
        self.managed.temporary_directory()
    }

    /// Active limits, including the effective quota.
    #[must_use]
    pub fn config(&self) -> StoreConfig {
        let mut config = self.config;
        config.quota = self.quota_policy();
        config
    }

    /// Current quota policy.
    #[must_use]
    pub fn quota_policy(&self) -> QuotaPolicy {
        *self.quota.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Changes the managed quota and persists it for later opens.
    ///
    /// Existing usage is never evicted. The returned policy is effective even
    /// when current usage already exceeds it.
    pub fn set_quota(&self, quota_bytes: u64) -> StoreResult<QuotaPolicy> {
        let policy = QuotaPolicy::new(quota_bytes)?;
        self.set_setting(QUOTA_SETTING, &quota_bytes.to_string())?;
        *self.quota.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = policy;
        Ok(policy)
    }

    /// Reconciliation result from the most recent open or explicit call.
    #[must_use]
    pub const fn last_reconciliation(&self) -> &ReconciliationReport {
        &self.reconciliation
    }

    /// Current database file size.
    pub fn database_size_bytes(&self) -> StoreResult<u64> {
        Ok(fs::metadata(self.db.path())?.len())
    }

    /// Current schema version.
    pub fn schema_version(&self) -> StoreResult<i64> {
        self.db
            .read(|connection| Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?))
    }

    /// Bytes available on the store volume.
    pub fn available_space(&self) -> StoreResult<u64> {
        self.managed.available_space()
    }

    /// Current quota accounting.
    pub fn usage(&self) -> StoreResult<QuotaUsage> {
        self.db.read(|connection| {
            let managed: i64 = connection.query_row(
                "SELECT COALESCE(SUM(size_bytes), 0) FROM contents WHERE state IN ('pending', 'verified')",
                [],
                |row| row.get(0),
            )?;
            let partial: i64 = connection.query_row(
                "SELECT COALESCE(SUM(verified_bytes), 0) FROM downloads WHERE state IN ('pending', 'downloading', 'paused', 'failed', 'cancelled')",
                [],
                |row| row.get(0),
            )?;
            Ok(QuotaUsage {
                managed_bytes: u64::try_from(managed).unwrap_or(0),
                partial_bytes: u64::try_from(partial).unwrap_or(0),
            })
        })
    }

    /// Verifies that `incoming` more bytes fit the quota and reserve.
    pub fn ensure_capacity(&self, incoming: u64) -> StoreResult<()> {
        let quota = self.quota_policy();
        quota.check_file_size(incoming, self.config.max_file_size)?;
        let usage = self.usage()?;
        let available = self.managed.available_space()?;
        quota.check(usage, incoming, available)
    }

    /// Verifies that `incoming` more bytes fit the managed quota.
    ///
    /// This is used after bytes are already staged on the same volume, where
    /// charging the free-space reserve a second time would double-count them.
    pub fn ensure_quota(&self, incoming: u64) -> StoreResult<()> {
        let quota = self.quota_policy();
        quota.check_file_size(incoming, self.config.max_file_size)?;
        let usage = self.usage()?;
        quota.check(usage, incoming, u64::MAX)
    }

    /// Imports a source file as a publication original.
    pub fn import_publication_original(&self, source: &Path, publication_id: &str) -> StoreResult<ImportedContent> {
        self.import_with_reference(source, ReferenceType::PublicationOriginal, publication_id)
    }

    /// Imports a source file as an actively hosted replica.
    pub fn import_replica(&self, source: &Path, replica_id: &str) -> StoreResult<ImportedContent> {
        self.import_with_reference(source, ReferenceType::ActiveReplica, replica_id)
    }

    /// Imports a source file as a retained download.
    pub fn import_retained_download(&self, source: &Path, download_id: &str) -> StoreResult<ImportedContent> {
        self.import_with_reference(source, ReferenceType::RetainedDownload, download_id)
    }

    /// Promotes a fully written streaming import without copying it again.
    ///
    /// The source must be inside the managed temporary directory. The store
    /// re-verifies the exact bytes and retains them with a `local_import`
    /// reference until a later publication or download takes ownership.
    pub fn import_local_staged(
        &self,
        source: &Path,
        content_id: &str,
        size_bytes: u64,
        owner_id: &str,
    ) -> StoreResult<ImportedContent> {
        validate_content_id(content_id)?;
        validate_entity_id(owner_id, "owner ID")?;
        self.config
            .quota
            .check_file_size(size_bytes, self.config.max_file_size)?;

        let already_present = self.content_exists(content_id)?;
        if already_present && self.content(content_id)?.size_bytes != size_bytes {
            return Err(StoreError::IntegrityMismatch(content_id.to_owned()));
        }
        // A database row alone is not enough: the blob may be missing or
        // corrupt. Reuse only a blob that passes the verified-path check.
        let already_verified = already_present && self.content_path(content_id).is_ok();
        let charge = if already_verified { 0 } else { size_bytes };
        if charge > 0 {
            self.ensure_quota(charge)?;
        }

        if already_verified {
            // The bytes are already stored and verified. The staged copy is
            // ours to discard; the reference below keeps the existing blob.
            match fs::remove_file(source) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(StoreError::Io(error)),
            }
        } else {
            self.managed.adopt_staged_file(source, content_id, size_bytes)?;
        }

        let blob_path = ManagedStore::blob_relative_path(content_id)?;
        let owner = owner_id.to_owned();
        let row_content_id = content_id.to_owned();
        let ref_content_id = content_id.to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "INSERT INTO contents (content_id, size_bytes, state, blob_path, verified_at, created_at, updated_at)
                 VALUES (?1, ?2, 'verified', ?3, ?4, ?4, ?4)
                 ON CONFLICT(content_id) DO UPDATE SET
                     size_bytes = excluded.size_bytes,
                     state = 'verified',
                     blob_path = excluded.blob_path,
                     verified_at = excluded.verified_at,
                     updated_at = excluded.updated_at",
                params![row_content_id, size_bytes, blob_path, now],
            )?;
            transaction.execute(
                "INSERT INTO content_refs (content_id, ref_type, owner_id, created_at)
                 VALUES (?1, 'local_import', ?2, ?3)
                 ON CONFLICT(content_id, ref_type, owner_id) DO NOTHING",
                params![ref_content_id, owner, now],
            )?;
            transaction.commit()?;
            Ok(())
        })?;

        Ok(ImportedContent {
            content_id: content_id.to_owned(),
            size_bytes,
            already_present,
        })
    }

    /// Returns the absolute path of a locally verified content blob.
    ///
    /// The record must be verified and the file size must match the metadata.
    /// The caller owns only a read handle; it must never modify or move the
    /// managed file.
    pub fn content_path(&self, content_id: &str) -> StoreResult<PathBuf> {
        let record = self.content(content_id)?;
        if record.state != ContentState::Verified {
            return Err(StoreError::ContentMissing(content_id.to_owned()));
        }
        let path = self.managed.blob_path(content_id)?;
        let metadata = fs::metadata(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StoreError::ContentMissing(content_id.to_owned())
            } else {
                StoreError::Io(error)
            }
        })?;
        if metadata.len() != record.size_bytes {
            return Err(StoreError::IntegrityMismatch(content_id.to_owned()));
        }
        Ok(path)
    }

    /// Flushes the SQLite write-ahead log to durable storage.
    pub fn checkpoint(&self) -> StoreResult<()> {
        self.db.write(|connection| {
            connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
            Ok(())
        })
    }

    fn import_with_reference(
        &self,
        source: &Path,
        ref_type: ReferenceType,
        owner_id: &str,
    ) -> StoreResult<ImportedContent> {
        validate_entity_id(owner_id, "owner ID")?;
        let source_size = fs::metadata(source)?.len();
        let quota = self.quota_policy();
        quota.check_file_size(source_size, self.config.max_file_size)?;

        // If the quota is already full, the source may still be bytes we
        // already hold. Stage and hash it before refusing so identical bytes
        // are not charged twice. Free space is checked before that copy.
        let preflight = self.ensure_capacity(source_size);
        if let Err(error) = &preflight {
            if !matches!(error, StoreError::QuotaExceeded { .. }) {
                return Err(preflight.expect_err("checked failure"));
            }
            let available = self.managed.available_space()?;
            if available.saturating_sub(source_size) < quota.reserve_bytes() {
                return Err(StoreError::FreeSpaceReserve {
                    available,
                    incoming: source_size,
                    reserve: quota.reserve_bytes(),
                });
            }
        }

        let staged = self.managed.stage_file(source)?;
        let content_id = staged.content_id.clone();
        let size_bytes = staged.size_bytes;
        let already_present = self.content_exists(&content_id)?;
        let charge = if already_present { 0 } else { size_bytes };
        if let Err(error) = self.ensure_capacity(charge) {
            let _ = fs::remove_file(&staged.path);
            return Err(error);
        }

        self.managed.promote(staged)?;
        let blob_path = ManagedStore::blob_relative_path(&content_id)?;
        let owner = owner_id.to_owned();
        let reference_type = ref_type.as_str().to_owned();
        let row_content_id = content_id.clone();
        let ref_content_id = content_id.clone();
        let now = now_secs();

        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "INSERT INTO contents (content_id, size_bytes, state, blob_path, verified_at, created_at, updated_at)
                 VALUES (?1, ?2, 'verified', ?3, ?4, ?4, ?4)
                 ON CONFLICT(content_id) DO UPDATE SET
                     size_bytes = excluded.size_bytes,
                     state = 'verified',
                     blob_path = excluded.blob_path,
                     verified_at = excluded.verified_at,
                     updated_at = excluded.updated_at",
                params![row_content_id, size_bytes, blob_path, now],
            )?;
            transaction.execute(
                "INSERT INTO content_refs (content_id, ref_type, owner_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(content_id, ref_type, owner_id) DO NOTHING",
                params![ref_content_id, reference_type, owner, now],
            )?;
            transaction.commit()?;
            Ok(())
        })?;

        Ok(ImportedContent {
            content_id,
            size_bytes,
            already_present,
        })
    }

    /// Returns a content record or a missing-content error.
    pub fn content(&self, content_id: &str) -> StoreResult<ContentRecord> {
        validate_content_id(content_id)?;
        let content_id = content_id.to_owned();
        self.db.read(move |connection| {
            connection
                .query_row(
                    &format!("SELECT {CONTENT_COLUMNS} FROM contents WHERE content_id = ?1"),
                    [&content_id],
                    content_from_row,
                )
                .optional()?
                .ok_or(StoreError::ContentMissing(content_id))
        })
    }

    /// Returns whether a content record exists.
    pub fn content_exists(&self, content_id: &str) -> StoreResult<bool> {
        validate_content_id(content_id)?;
        let content_id = content_id.to_owned();
        self.db.read(move |connection| {
            let count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM contents WHERE content_id = ?1",
                [content_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Lists all content records.
    pub fn contents(&self) -> StoreResult<Vec<ContentRecord>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(&format!("SELECT {CONTENT_COLUMNS} FROM contents"))?;
            let rows = statement.query_map([], content_from_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Re-verifies a content blob against its recorded size.
    pub fn verify_content(&self, content_id: &str) -> StoreResult<()> {
        let record = self.content(content_id)?;
        self.managed.verify_blob(content_id, record.size_bytes)?;
        if record.state != ContentState::Verified {
            self.update_content_state(content_id, ContentState::Verified)?;
        }
        Ok(())
    }

    /// Exports a content blob under a safe, non-overwriting display name.
    pub fn export_content(
        &self,
        content_id: &str,
        destination_directory: &Path,
        display_name: &str,
    ) -> StoreResult<ExportedContent> {
        validate_content_id(content_id)?;
        let store_root = self.managed.root().canonicalize()?;
        let destination_directory = destination_directory.canonicalize()?;
        if destination_directory.starts_with(&store_root) {
            return Err(StoreError::UnsafePath(
                "exports must be written outside the managed store".to_owned(),
            ));
        }
        if !destination_directory.is_dir() {
            return Err(StoreError::UnsafePath(
                "export destination must be a directory".to_owned(),
            ));
        }
        let path = unique_export_path(&destination_directory, display_name)?;
        let size_bytes = self.managed.copy_blob_to(content_id, &path)?;
        Ok(ExportedContent { path, size_bytes })
    }

    /// Adds a reference to an existing content record. Re-adding the same
    /// reference is idempotent.
    pub fn add_reference(&self, content_id: &str, ref_type: ReferenceType, owner_id: &str) -> StoreResult<()> {
        validate_content_id(content_id)?;
        validate_entity_id(owner_id, "owner ID")?;
        if !self.content_exists(content_id)? {
            return Err(StoreError::ContentMissing(content_id.to_owned()));
        }
        let content_id = content_id.to_owned();
        let owner_id = owner_id.to_owned();
        let reference_type = ref_type.as_str().to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "INSERT INTO content_refs (content_id, ref_type, owner_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(content_id, ref_type, owner_id) DO NOTHING",
                params![content_id, reference_type, owner_id, now],
            )?;
            Ok(())
        })
    }

    /// Number of references retaining a content blob.
    pub fn reference_count(&self, content_id: &str) -> StoreResult<usize> {
        validate_content_id(content_id)?;
        let content_id = content_id.to_owned();
        self.db.read(move |connection| {
            let count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM content_refs WHERE content_id = ?1",
                [content_id],
                |row| row.get(0),
            )?;
            Ok(usize::try_from(count).unwrap_or(usize::MAX))
        })
    }

    /// Removes a reference. Bytes are deleted only when no reference remains.
    ///
    /// Removing the last publication-original or active-replica reference
    /// requires `confirmed = true`; otherwise the reference is left intact.
    pub fn release_reference(
        &self,
        content_id: &str,
        ref_type: ReferenceType,
        owner_id: &str,
        confirmed: bool,
    ) -> StoreResult<ReleaseOutcome> {
        validate_content_id(content_id)?;
        validate_entity_id(owner_id, "owner ID")?;
        let content = content_id.to_owned();
        let owner = owner_id.to_owned();
        let reference_type = ref_type.as_str().to_owned();

        let outcome = self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            let matching: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM content_refs
                 WHERE content_id = ?1 AND ref_type = ?2 AND owner_id = ?3",
                params![&content, &reference_type, &owner],
                |row| row.get(0),
            )?;
            if matching == 0 {
                return Ok(ReleaseOutcome::NotFound);
            }

            let total: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM content_refs WHERE content_id = ?1",
                [&content],
                |row| row.get(0),
            )?;
            if total == 1 && ref_type.requires_confirmation() && !confirmed {
                return Err(StoreError::ConfirmationRequired(ref_type.label()));
            }

            transaction.execute(
                "DELETE FROM content_refs
                 WHERE content_id = ?1 AND ref_type = ?2 AND owner_id = ?3",
                params![&content, &reference_type, &owner],
            )?;

            let remaining = total.saturating_sub(1);
            if remaining > 0 {
                transaction.commit()?;
                return Ok(ReleaseOutcome::Retained {
                    remaining: usize::try_from(remaining).unwrap_or(usize::MAX),
                });
            }

            transaction.execute("DELETE FROM contents WHERE content_id = ?1", [&content])?;
            transaction.commit()?;
            Ok(ReleaseOutcome::BytesDeleted)
        })?;

        if outcome == ReleaseOutcome::BytesDeleted {
            return match self.managed.remove_blob(content_id) {
                Ok(()) => Ok(ReleaseOutcome::BytesDeleted),
                Err(_) => Ok(ReleaseOutcome::BytesPendingCleanup),
            };
        }
        Ok(outcome)
    }

    /// Lists all content references.
    pub fn references(&self) -> StoreResult<Vec<ContentReference>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(
                "SELECT content_id, ref_type, owner_id, created_at
                 FROM content_refs ORDER BY ref_id ASC",
            )?;
            let rows = statement.query_map([], |row| {
                let ref_type: String = row.get(1)?;
                Ok(ContentReference {
                    content_id: row.get(0)?,
                    ref_type: ReferenceType::parse(&ref_type)
                        .ok_or_else(|| invalid_enum(1, "reference type", &ref_type))?,
                    owner_id: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Registers a resumable download and creates its empty partial file.
    pub fn begin_download(
        &self,
        download_id: &str,
        content_id: &str,
        expected_size: u64,
        display_name: &str,
    ) -> StoreResult<DownloadRecord> {
        self.begin_download_with_source(download_id, content_id, expected_size, display_name, None, None)
    }

    /// Registers a download and remembers its trusted manifest root and source.
    pub fn begin_download_with_source(
        &self,
        download_id: &str,
        content_id: &str,
        expected_size: u64,
        display_name: &str,
        manifest_root: Option<&str>,
        source_peer: Option<&str>,
    ) -> StoreResult<DownloadRecord> {
        validate_store_component(download_id, "download ID")?;
        validate_content_id(content_id)?;
        if let Some(root) = manifest_root {
            validate_content_id(root)?;
        }
        let source_peer = validate_source_peer(source_peer)?;
        self.config
            .quota
            .check_file_size(expected_size, self.config.max_file_size)?;
        self.ensure_capacity(expected_size)?;

        let download_id_owned = download_id.to_owned();
        let content_id_owned = content_id.to_owned();
        let display_name = safe_display_name(display_name);
        let manifest_root = manifest_root.map(str::to_owned);
        let partial_file = ManagedStore::partial_relative_path(download_id)?;
        let chunk_map = ChunkMap::new(expected_size).bytes().to_vec();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "INSERT INTO downloads
                 (download_id, content_id, display_name, expected_size, verified_prefix, state,
                  partial_file, created_at, updated_at, last_error, manifest_root, source_peer,
                  verified_bytes, chunk_map)
                 VALUES (?1, ?2, ?3, ?4, 0, 'downloading', ?5, ?6, ?6, NULL, ?7, ?8, 0, ?9)",
                params![
                    download_id_owned,
                    content_id_owned,
                    display_name,
                    expected_size,
                    partial_file,
                    now,
                    manifest_root,
                    source_peer,
                    chunk_map
                ],
            )?;
            Ok(())
        })?;

        if let Err(error) = self.managed.reset_partial(download_id) {
            let _ = self.delete_download_row(download_id);
            return Err(error);
        }
        self.download(download_id)
    }

    /// Updates the trusted manifest root and preferred source for a download.
    pub fn set_download_source(
        &self,
        download_id: &str,
        manifest_root: Option<&str>,
        source_peer: Option<&str>,
    ) -> StoreResult<DownloadRecord> {
        let record = self.download(download_id)?;
        if matches!(record.state, DownloadState::Completed | DownloadState::Cancelled) {
            return Err(StoreError::Unsupported("download is terminal".to_owned()));
        }
        if let Some(root) = manifest_root {
            validate_content_id(root)?;
        }
        let source_peer = validate_source_peer(source_peer)?;
        let download_id_owned = download_id.to_owned();
        let manifest_root = manifest_root.map(str::to_owned);
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE downloads
                 SET manifest_root = ?2, source_peer = ?3, updated_at = ?4
                 WHERE download_id = ?1",
                params![download_id_owned, manifest_root, source_peer, now],
            )?;
            Ok(())
        })?;
        self.download(download_id)
    }

    /// Marks a download paused without discarding verified bytes.
    pub fn pause_download(&self, download_id: &str, reason: Option<&str>) -> StoreResult<DownloadRecord> {
        let record = self.download(download_id)?;
        if record.state == DownloadState::Completed {
            return Err(StoreError::Unsupported("download is already complete".to_owned()));
        }
        let reason = reason.map(|value| value.chars().take(256).collect::<String>());
        let download_id_owned = download_id.to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE downloads SET state = 'paused', last_error = ?2, updated_at = ?3
                 WHERE download_id = ?1",
                params![download_id_owned, reason, now],
            )?;
            Ok(())
        })?;
        self.download(download_id)
    }

    /// Marks a download failed and records a bounded error message.
    pub fn fail_download(&self, download_id: &str, message: &str) -> StoreResult<DownloadRecord> {
        let record = self.download(download_id)?;
        if matches!(record.state, DownloadState::Completed | DownloadState::Cancelled) {
            return Err(StoreError::Unsupported("download is terminal".to_owned()));
        }
        let message = message.chars().take(256).collect::<String>();
        let download_id_owned = download_id.to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE downloads SET state = 'failed', last_error = ?2, updated_at = ?3
                 WHERE download_id = ?1",
                params![download_id_owned, message, now],
            )?;
            Ok(())
        })?;
        self.download(download_id)
    }

    /// Returns a paused, failed, or kept-cancelled download to `downloading`.
    pub fn resume_download(&self, download_id: &str) -> StoreResult<DownloadRecord> {
        let record = self.download(download_id)?;
        if record.state == DownloadState::Completed {
            return Err(StoreError::Unsupported("download is already complete".to_owned()));
        }
        let download_id_owned = download_id.to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE downloads SET state = 'downloading', last_error = NULL, updated_at = ?2
                 WHERE download_id = ?1",
                params![download_id_owned, now],
            )?;
            Ok(())
        })?;
        self.download(download_id)
    }

    /// Marks a download cancelled while keeping its verified partial bytes.
    pub fn cancel_download_keep(&self, download_id: &str) -> StoreResult<DownloadRecord> {
        let record = self.download(download_id)?;
        if record.state == DownloadState::Completed {
            return Err(StoreError::Unsupported("download is already complete".to_owned()));
        }
        let download_id_owned = download_id.to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE downloads SET state = 'cancelled', updated_at = ?2 WHERE download_id = ?1",
                params![download_id_owned, now],
            )?;
            Ok(())
        })?;
        self.download(download_id)
    }

    /// Pauses every download left in `downloading` by a previous process.
    pub fn pause_interrupted_downloads(&self) -> StoreResult<usize> {
        let now = now_secs();
        self.db.write(move |connection| {
            let changed = connection.execute(
                "UPDATE downloads
                 SET state = 'paused', last_error = 'interrupted by restart', updated_at = ?1
                 WHERE state = 'downloading'",
                [now],
            )?;
            Ok(changed)
        })
    }

    /// Returns one download record.
    pub fn download(&self, download_id: &str) -> StoreResult<DownloadRecord> {
        validate_store_component(download_id, "download ID")?;
        let download_id = download_id.to_owned();
        self.db.read(move |connection| {
            connection
                .query_row(
                    &format!("SELECT {DOWNLOAD_COLUMNS} FROM downloads WHERE download_id = ?1"),
                    [&download_id],
                    download_from_row,
                )
                .optional()?
                .ok_or_else(|| StoreError::InvalidId(format!("unknown download {download_id}")))
        })
    }

    /// Lists all downloads, including terminal records retained for history.
    pub fn downloads(&self) -> StoreResult<Vec<DownloadRecord>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {DOWNLOAD_COLUMNS} FROM downloads ORDER BY created_at ASC"
            ))?;
            let rows = statement.query_map([], download_from_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Generated partial path for a download, for the transfer layer.
    pub fn partial_path(&self, download_id: &str) -> StoreResult<PathBuf> {
        self.managed.partial_path(download_id)
    }

    /// Records a verified contiguous prefix.
    ///
    /// Bytes beyond the verified region are unverified and are truncated. The
    /// prefix counts toward the managed quota from the first allocated byte.
    /// Chunks fully covered by the prefix are also marked in the chunk map, so
    /// a later Stage 17 resume can continue from any verified chunk.
    pub fn checkpoint_partial(&self, download_id: &str, verified_prefix: u64) -> StoreResult<DownloadRecord> {
        let record = self.download(download_id)?;
        if matches!(record.state, DownloadState::Completed | DownloadState::Cancelled) {
            return Err(StoreError::Unsupported(
                "cannot checkpoint a terminal download".to_owned(),
            ));
        }
        if verified_prefix < record.verified_prefix || verified_prefix > record.expected_size {
            return Err(StoreError::Unsupported(
                "verified prefix must be monotonic and no larger than the expected size".to_owned(),
            ));
        }

        let mut map = chunk_map_for(&record);
        for index in 0..map.total() {
            let Some(length) = map.chunk_length(index) else {
                break;
            };
            let end = (u64::from(index) * CHUNK_SIZE_V1).saturating_add(length);
            if end > verified_prefix {
                break;
            }
            map.set(index);
        }

        let verified_end = map.highest_verified_end().max(verified_prefix);
        let file_length = self.managed.partial_length(download_id)?;
        if file_length < verified_end {
            return Err(StoreError::IntegrityMismatch(record.content_id));
        }
        if file_length > verified_end {
            self.managed.truncate_partial(download_id, verified_end)?;
        }

        let verified_bytes = map.verified_bytes().max(verified_prefix).max(record.verified_bytes);
        let contiguous = map.contiguous_prefix().max(verified_prefix);
        let delta = verified_bytes.saturating_sub(record.verified_bytes);
        if delta > 0 {
            self.ensure_capacity(delta)?;
        }

        let download_id_owned = download_id.to_owned();
        let map_bytes = map.bytes().to_vec();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE downloads
                 SET verified_prefix = ?2, verified_bytes = ?3, chunk_map = ?4, state = 'downloading',
                     updated_at = ?5, last_error = NULL
                 WHERE download_id = ?1",
                params![download_id_owned, contiguous, verified_bytes, map_bytes, now],
            )?;
            Ok(())
        })?;
        self.download(download_id)
    }

    /// Marks one authenticated chunk as verified.
    ///
    /// Chunks may arrive in any order. The update is read-modify-write inside
    /// the database writer, so two workers checkpointing different chunks of
    /// the same download cannot lose each other's bits.
    pub fn checkpoint_chunk(&self, download_id: &str, index: u32) -> StoreResult<DownloadRecord> {
        let record = self.download(download_id)?;
        if matches!(record.state, DownloadState::Completed | DownloadState::Cancelled) {
            return Err(StoreError::Unsupported(
                "cannot checkpoint a terminal download".to_owned(),
            ));
        }
        let map = chunk_map_for(&record);
        let Some(length) = map.chunk_length(index) else {
            return Err(StoreError::InvalidId(format!("chunk index {index} is out of range")));
        };
        if map.is_set(index) {
            return Ok(record);
        }
        let end = (u64::from(index) * CHUNK_SIZE_V1).saturating_add(length);
        if self.managed.partial_length(download_id)? < end {
            return Err(StoreError::IntegrityMismatch(record.content_id));
        }
        self.ensure_capacity(length)?;

        let download_id_owned = download_id.to_owned();
        let expected_size = record.expected_size;
        let now = now_secs();
        self.db.write(move |connection| {
            let (stored, prefix, verified_bytes): (Option<Vec<u8>>, i64, i64) = connection.query_row(
                "SELECT chunk_map, verified_prefix, verified_bytes FROM downloads WHERE download_id = ?1",
                [&download_id_owned],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            let prefix = u64::try_from(prefix).unwrap_or(0);
            let verified_bytes = u64::try_from(verified_bytes).unwrap_or(0);
            let mut map = stored
                .as_deref()
                .and_then(|bytes| ChunkMap::from_stored(expected_size, bytes))
                .unwrap_or_else(|| ChunkMap::from_prefix(expected_size, prefix));
            map.set(index);
            let contiguous = map.contiguous_prefix();
            let bytes = map.verified_bytes().max(verified_bytes);
            connection.execute(
                "UPDATE downloads
                 SET verified_prefix = ?2, verified_bytes = ?3, chunk_map = ?4, state = 'downloading',
                     updated_at = ?5, last_error = NULL
                 WHERE download_id = ?1",
                params![download_id_owned, contiguous, bytes, map.bytes().to_vec(), now],
            )?;
            Ok(())
        })?;
        self.download(download_id)
    }

    /// Clears one chunk after a resume re-verification failed.
    pub fn clear_chunk(&self, download_id: &str, index: u32) -> StoreResult<DownloadRecord> {
        let record = self.download(download_id)?;
        if matches!(record.state, DownloadState::Completed | DownloadState::Cancelled) {
            return Err(StoreError::Unsupported(
                "cannot clear a chunk on a terminal download".to_owned(),
            ));
        }
        let download_id_owned = download_id.to_owned();
        let expected_size = record.expected_size;
        let now = now_secs();
        self.db.write(move |connection| {
            let (stored, prefix): (Option<Vec<u8>>, i64) = connection.query_row(
                "SELECT chunk_map, verified_prefix FROM downloads WHERE download_id = ?1",
                [&download_id_owned],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let prefix = u64::try_from(prefix).unwrap_or(0);
            let mut map = stored
                .as_deref()
                .and_then(|bytes| ChunkMap::from_stored(expected_size, bytes))
                .unwrap_or_else(|| ChunkMap::from_prefix(expected_size, prefix));
            map.clear(index);
            connection.execute(
                "UPDATE downloads
                 SET verified_prefix = ?2, verified_bytes = ?3, chunk_map = ?4, state = 'downloading',
                     updated_at = ?5, last_error = NULL
                 WHERE download_id = ?1",
                params![
                    download_id_owned,
                    map.contiguous_prefix(),
                    map.verified_bytes(),
                    map.bytes().to_vec(),
                    now
                ],
            )?;
            Ok(())
        })?;
        self.download(download_id)
    }

    /// Truncates a partial file to the end of its highest verified chunk.
    ///
    /// This removes a tail written just before a crash without deleting
    /// verified chunks that arrived out of order.
    pub fn trim_unverified_tail(&self, download_id: &str) -> StoreResult<DownloadRecord> {
        let record = self.download(download_id)?;
        let map = chunk_map_for(&record);
        let verified_end = map.highest_verified_end().max(record.verified_prefix);
        let file_length = self.managed.partial_length(download_id)?;
        if file_length < verified_end {
            return Err(StoreError::IntegrityMismatch(record.content_id));
        }
        if file_length > verified_end {
            self.managed.truncate_partial(download_id, verified_end)?;
        }
        Ok(record)
    }

    /// Verifies and promotes a complete partial into the managed store.
    pub fn complete_download(&self, download_id: &str) -> StoreResult<ImportedContent> {
        let record = self.download(download_id)?;
        if matches!(record.state, DownloadState::Completed | DownloadState::Cancelled) {
            return Err(StoreError::Unsupported("download is already terminal".to_owned()));
        }
        let map = chunk_map_for(&record);
        if !map.is_complete() || record.verified_prefix != record.expected_size {
            return Err(StoreError::IntegrityMismatch(record.content_id));
        }
        let already_present = self.content_exists(&record.content_id)?;
        let promoted = self
            .managed
            .promote_partial(download_id, &record.content_id, record.expected_size)?;

        let content_id = promoted.content_id;
        let size_bytes = promoted.size_bytes;
        let download_id_owned = download_id.to_owned();
        let owner_id = download_id.to_owned();
        let blob_path = ManagedStore::blob_relative_path(&content_id)?;
        let row_content_id = content_id.clone();
        let ref_content_id = content_id.clone();
        let now = now_secs();
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "INSERT INTO contents (content_id, size_bytes, state, blob_path, verified_at, created_at, updated_at)
                 VALUES (?1, ?2, 'verified', ?3, ?4, ?4, ?4)
                 ON CONFLICT(content_id) DO UPDATE SET
                     size_bytes = excluded.size_bytes,
                     state = 'verified',
                     blob_path = excluded.blob_path,
                     verified_at = excluded.verified_at,
                     updated_at = excluded.updated_at",
                params![row_content_id, size_bytes, blob_path, now],
            )?;
            transaction.execute(
                "INSERT INTO content_refs (content_id, ref_type, owner_id, created_at)
                 VALUES (?1, 'retained_download', ?2, ?3)
                 ON CONFLICT(content_id, ref_type, owner_id) DO NOTHING",
                params![ref_content_id, owner_id, now],
            )?;
            transaction.execute("DELETE FROM downloads WHERE download_id = ?1", [download_id_owned])?;
            transaction.commit()?;
            Ok(())
        })?;

        let _ = self.managed.remove_partial(download_id);
        Ok(ImportedContent {
            content_id,
            size_bytes,
            already_present,
        })
    }

    /// Cancels a download and removes its partial bytes.
    pub fn cancel_download(&self, download_id: &str) -> StoreResult<bool> {
        let removed = self.delete_download_row(download_id)?;
        if removed {
            let _ = self.managed.remove_partial(download_id);
        }
        Ok(removed)
    }

    /// Removes incomplete downloads older than `older_than_secs`.
    ///
    /// This is a user-visible retention action and requires confirmation.
    pub fn cleanup_partials(&self, older_than_secs: u64, confirmed: bool) -> StoreResult<usize> {
        if !confirmed {
            return Err(StoreError::ConfirmationRequired("incomplete downloads cleanup"));
        }
        let cutoff = now_secs().saturating_sub(older_than_secs);
        let ids = self.db.read(move |connection| {
            let mut statement = connection.prepare(
                "SELECT download_id FROM downloads
                 WHERE state IN ('pending', 'downloading', 'paused', 'failed', 'cancelled')
                   AND updated_at <= ?1",
            )?;
            let rows = statement.query_map([cutoff], |row| row.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?;
        let mut removed = 0_usize;
        for download_id in ids {
            if self.cancel_download(&download_id)? {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Adds or replaces a local identity reference.
    pub fn upsert_identity_ref(&self, record: &IdentityReference) -> StoreResult<()> {
        validate_entity_id(&record.identity_id, "identity ID")?;
        if record.public_key.len() != 32 {
            return Err(StoreError::InvalidId("identity public key must be 32 bytes".to_owned()));
        }
        let record = record.clone();
        self.db.write(move |connection| {
            connection.execute(
                "INSERT INTO identity_refs (identity_id, public_key, key_file, label, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(identity_id) DO UPDATE SET
                     public_key = excluded.public_key,
                     key_file = excluded.key_file,
                     label = excluded.label",
                params![
                    record.identity_id,
                    record.public_key,
                    record.key_file,
                    record.label,
                    record.created_at
                ],
            )?;
            Ok(())
        })
    }

    /// Lists local identity references.
    pub fn identity_refs(&self) -> StoreResult<Vec<IdentityReference>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(
                "SELECT identity_id, public_key, key_file, label, created_at
                 FROM identity_refs ORDER BY created_at ASC",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(IdentityReference {
                    identity_id: row.get(0)?,
                    public_key: row.get(1)?,
                    key_file: row.get(2)?,
                    label: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Adds or replaces a verified profile record.
    ///
    /// Core callers use the narrower local and cache methods below; this
    /// method remains the direct storage primitive used by tests and tools.
    pub fn upsert_profile(&self, record: &ProfileRecord) -> StoreResult<()> {
        validate_profile_record(record)?;
        let record = record.clone();
        self.db.write(move |connection| write_profile(connection, &record))
    }

    /// Returns one stored profile row by identity.
    pub fn profile(&self, identity_id: &str) -> StoreResult<Option<ProfileRecord>> {
        validate_entity_id(identity_id, "identity ID")?;
        let identity_id = identity_id.to_owned();
        self.db.read(move |connection| {
            Ok(connection
                .query_row(
                    &format!("SELECT {PROFILE_COLUMNS} FROM profiles WHERE identity_id = ?1"),
                    [&identity_id],
                    profile_from_row,
                )
                .optional()?)
        })
    }

    /// Returns the local profile row for an identity, ignoring remote caches.
    pub fn local_profile(&self, identity_id: &str) -> StoreResult<Option<ProfileRecord>> {
        Ok(self.profile(identity_id)?.filter(|record| record.state.is_local()))
    }

    /// Stores a signed local draft, pending, or published profile.
    ///
    /// `expected_revision` provides optimistic concurrency. When it is absent,
    /// a local row may advance but never roll back to a lower revision.
    pub fn store_local_profile(&self, record: &ProfileRecord, expected_revision: Option<u64>) -> StoreResult<()> {
        validate_profile_record(record)?;
        if !record.state.is_local() {
            return Err(StoreError::InvalidId(
                "local profile state must be draft, pending, or published".to_owned(),
            ));
        }
        let record = record.clone();
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            let existing: Option<i64> = transaction
                .query_row(
                    "SELECT revision FROM profiles WHERE identity_id = ?1",
                    [&record.identity_id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(revision) = existing {
                let revision = u64::try_from(revision).unwrap_or(0);
                if expected_revision.is_some_and(|expected| expected != revision)
                    || (expected_revision.is_none() && record.revision < revision)
                {
                    return Err(StoreError::RecordConflict(record.identity_id.clone()));
                }
            } else if expected_revision.is_some_and(|expected| expected != 0) {
                return Err(StoreError::RecordConflict(record.identity_id.clone()));
            }
            write_profile(&transaction, &record)?;
            transaction.commit()?;
            Ok(())
        })
    }

    /// Caches a verified profile signed by another identity.
    ///
    /// A newer revision replaces the cached row, an identical revision is
    /// idempotent, an older revision is reported as stale, and a same-revision
    /// record with different bytes is retained as a conflict without replacing
    /// the accepted row.
    pub fn cache_remote_profile(&self, record: &ProfileRecord) -> StoreResult<ProfileCacheOutcome> {
        validate_profile_record(record)?;
        if record.state != ProfileState::Remote {
            return Err(StoreError::InvalidId("cached profile state must be remote".to_owned()));
        }
        let record = record.clone();
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            let existing = transaction
                .query_row(
                    "SELECT revision, record_bytes, state FROM profiles WHERE identity_id = ?1",
                    [&record.identity_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?;
            let observed_at = now_ms();
            let Some((revision, record_bytes, state)) = existing else {
                write_profile(&transaction, &record)?;
                transaction.commit()?;
                return Ok(ProfileCacheOutcome::Stored);
            };
            let revision = u64::try_from(revision).unwrap_or(0);
            let state = ProfileState::parse(&state).unwrap_or(ProfileState::Remote);
            let outcome = if state.is_local() {
                if record_bytes == record.record_bytes {
                    ProfileCacheOutcome::Unchanged
                } else {
                    insert_profile_conflict(
                        &transaction,
                        &record.identity_id,
                        record.revision,
                        &record.record_bytes,
                        observed_at,
                    )?;
                    ProfileCacheOutcome::Conflict
                }
            } else if record.revision > revision {
                write_profile(&transaction, &record)?;
                ProfileCacheOutcome::Stored
            } else if record.revision < revision {
                mark_cached_checked(&transaction, &record.identity_id, observed_at)?;
                ProfileCacheOutcome::Stale
            } else if record_bytes == record.record_bytes {
                mark_cached_checked(&transaction, &record.identity_id, observed_at)?;
                ProfileCacheOutcome::Unchanged
            } else {
                insert_profile_conflict(
                    &transaction,
                    &record.identity_id,
                    record.revision,
                    &record.record_bytes,
                    observed_at,
                )?;
                mark_cached_checked(&transaction, &record.identity_id, observed_at)?;
                ProfileCacheOutcome::Conflict
            };
            transaction.commit()?;
            Ok(outcome)
        })
    }

    /// Marks a stored local profile as announced or deferred.
    pub fn mark_profile_published(&self, identity_id: &str, announced: bool, at: u64) -> StoreResult<()> {
        validate_entity_id(identity_id, "identity ID")?;
        let identity_id = identity_id.to_owned();
        let state = if announced { "published" } else { "pending" };
        self.db.write(move |connection| {
            let changed = connection.execute(
                "UPDATE profiles
                 SET state = ?2, published_at = ?3, freshness = 'current', updated_at = ?4
                 WHERE identity_id = ?1",
                params![identity_id, state, i64::try_from(at).unwrap_or(i64::MAX), now_secs()],
            )?;
            if changed == 0 {
                return Err(StoreError::RecordNotFound(identity_id));
            }
            Ok(())
        })
    }

    /// Records the result of a network freshness check for a stored profile.
    pub fn mark_profile_checked(&self, identity_id: &str, freshness: ProfileFreshness, at: u64) -> StoreResult<()> {
        validate_entity_id(identity_id, "identity ID")?;
        let identity_id = identity_id.to_owned();
        let freshness = freshness.as_str();
        self.db.write(move |connection| {
            let changed = connection.execute(
                "UPDATE profiles SET last_checked_at = ?2, freshness = ?3 WHERE identity_id = ?1",
                params![identity_id, i64::try_from(at).unwrap_or(i64::MAX), freshness],
            )?;
            if changed == 0 {
                return Err(StoreError::RecordNotFound(identity_id));
            }
            Ok(())
        })
    }

    /// Lists retained same-revision conflicts for one identity.
    pub fn profile_conflicts(&self, identity_id: &str) -> StoreResult<Vec<ProfileConflict>> {
        validate_entity_id(identity_id, "identity ID")?;
        let identity_id = identity_id.to_owned();
        self.db.read(move |connection| {
            let mut statement = connection.prepare(
                "SELECT identity_id, revision, record_bytes, observed_at
                 FROM profile_conflicts WHERE identity_id = ?1
                 ORDER BY observed_at ASC, conflict_id ASC",
            )?;
            let rows = statement.query_map([&identity_id], |row| {
                Ok(ProfileConflict {
                    identity_id: row.get(0)?,
                    revision: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                    record_bytes: row.get(2)?,
                    observed_at: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Lists every stored profile using a username label, case-insensitively.
    ///
    /// Usernames are labels, not reservations. More than one identity may
    /// match, and callers must show each distinct identity ID.
    pub fn profiles_with_username(&self, username: &str) -> StoreResult<Vec<ProfileRecord>> {
        if username.is_empty() || username.len() > 128 {
            return Err(StoreError::InvalidId("username label is out of bounds".to_owned()));
        }
        let needle = username.to_lowercase();
        self.db.read(move |connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {PROFILE_COLUMNS} FROM profiles
                 WHERE username IS NOT NULL ORDER BY identity_id ASC"
            ))?;
            let rows = statement.query_map([], profile_from_row)?;
            let mut matches = Vec::new();
            for row in rows {
                let record = row?;
                if record
                    .username
                    .as_deref()
                    .is_some_and(|label| label.to_lowercase() == needle)
                {
                    matches.push(record);
                }
            }
            Ok(matches)
        })
    }

    /// Lists stored profiles.
    pub fn profiles(&self) -> StoreResult<Vec<ProfileRecord>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {PROFILE_COLUMNS} FROM profiles ORDER BY identity_id ASC"
            ))?;
            let rows = statement.query_map([], profile_from_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Stores a verified publication record exactly once.
    ///
    /// The signed identity, content ID, revision, and exact record bytes are
    /// immutable. Repeating the same record is an idempotent no-op; any
    /// difference for an existing publication ID is a conflict that preserves
    /// the original row.
    pub fn upsert_publication(&self, record: &PublicationRecord) -> StoreResult<()> {
        validate_entity_id(&record.publication_id, "publication ID")?;
        validate_entity_id(&record.identity_id, "identity ID")?;
        validate_content_id(&record.content_id)?;
        if record.tags.len() > MAX_TAGS {
            return Err(StoreError::InvalidRecord("publication has too many tags".to_owned()));
        }
        for tag in &record.tags {
            if tag.is_empty() || tag.len() > MAX_TAG_BYTES {
                return Err(StoreError::InvalidRecord("publication tag is malformed".to_owned()));
            }
        }
        let record = record.clone();
        let filename = safe_display_name(&record.filename);
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            let existing = transaction
                .query_row(
                    "SELECT identity_id, content_id, revision, record_bytes
                     FROM publications WHERE publication_id = ?1",
                    [&record.publication_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                        ))
                    },
                )
                .optional()?;

            if let Some((identity_id, content_id, revision, record_bytes)) = existing {
                let incoming_revision = i64::try_from(record.revision).unwrap_or(i64::MAX);
                if identity_id != record.identity_id
                    || content_id != record.content_id
                    || revision != incoming_revision
                    || record_bytes != record.record_bytes
                {
                    return Err(StoreError::RecordConflict(record.publication_id));
                }
                transaction.commit()?;
                return Ok(());
            }

            transaction.execute(
                "INSERT INTO publications
                 (publication_id, identity_id, content_id, revision, record_bytes, title, description,
                  filename, published_at, updated_at, tombstoned, category, size_bytes, local, serving,
                  announced, availability, last_checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    record.publication_id,
                    record.identity_id,
                    record.content_id,
                    i64::try_from(record.revision).unwrap_or(i64::MAX),
                    record.record_bytes,
                    record.title,
                    record.description,
                    filename,
                    record.published_at,
                    record.updated_at,
                    if record.tombstoned { 1_i64 } else { 0_i64 },
                    record.category.as_str(),
                    i64::try_from(record.size_bytes).unwrap_or(i64::MAX),
                    if record.local { 1_i64 } else { 0_i64 },
                    if record.serving { 1_i64 } else { 0_i64 },
                    if record.announced { 1_i64 } else { 0_i64 },
                    record.availability.as_str(),
                    record
                        .last_checked_at
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                ],
            )?;
            transaction.execute(
                "UPDATE publications SET tombstoned = 1
                 WHERE publication_id = ?1
                   AND EXISTS (SELECT 1 FROM tombstones WHERE publication_id = ?1)",
                [&record.publication_id],
            )?;
            transaction.execute(
                "DELETE FROM publication_tags WHERE publication_id = ?1",
                [&record.publication_id],
            )?;
            for (position, tag) in record.tags.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO publication_tags (publication_id, tag, position)
                     VALUES (?1, ?2, ?3)",
                    params![record.publication_id, tag, i64::try_from(position).unwrap_or(i64::MAX)],
                )?;
            }
            crate::search_store::refresh_publication_index(&transaction, &record.publication_id)?;
            transaction.commit()?;
            Ok(())
        })
    }

    /// Caches a verified remote publication and reports whether it was new.
    pub fn cache_remote_publication(&self, record: &PublicationRecord) -> StoreResult<PublicationCacheOutcome> {
        let existed = self.publication_exists(&record.publication_id)?;
        if existed {
            let existing = self.publication(&record.publication_id)?;
            if existing.record_bytes == record.record_bytes
                && existing.identity_id == record.identity_id
                && existing.content_id == record.content_id
                && existing.revision == record.revision
            {
                return Ok(PublicationCacheOutcome::Unchanged);
            }
            return Ok(PublicationCacheOutcome::Conflict);
        }
        self.upsert_publication(record)?;
        Ok(PublicationCacheOutcome::Stored)
    }

    /// Returns whether a publication row exists.
    pub fn publication_exists(&self, publication_id: &str) -> StoreResult<bool> {
        validate_entity_id(publication_id, "publication ID")?;
        let publication_id = publication_id.to_owned();
        self.db.read(move |connection| {
            let count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM publications WHERE publication_id = ?1",
                [publication_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Lists publication records.
    pub fn publications(&self) -> StoreResult<Vec<PublicationRecord>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {PUBLICATION_COLUMNS} FROM publications ORDER BY published_at DESC"
            ))?;
            let rows = statement.query_map([], publication_from_row)?;
            let mut records = rows.collect::<Result<Vec<_>, _>>()?;
            for record in &mut records {
                record.tags = publication_tags(connection, &record.publication_id)?;
            }
            Ok(records)
        })
    }

    /// Lists locally owned publications in stable publication-ID order.
    pub fn local_publications(&self, identity_id: Option<&str>) -> StoreResult<Vec<PublicationRecord>> {
        if let Some(identity_id) = identity_id {
            validate_entity_id(identity_id, "identity ID")?;
        }
        let identity = identity_id.map(str::to_owned);
        self.db.read(move |connection| {
            let mut records = match identity {
                Some(identity) => {
                    let mut statement = connection.prepare(&format!(
                        "SELECT {PUBLICATION_COLUMNS} FROM publications
                         WHERE local = 1 AND identity_id = ?1 ORDER BY publication_id ASC"
                    ))?;
                    let rows = statement.query_map([identity], publication_from_row)?;
                    rows.collect::<Result<Vec<_>, _>>()?
                }
                None => {
                    let mut statement = connection.prepare(&format!(
                        "SELECT {PUBLICATION_COLUMNS} FROM publications
                         WHERE local = 1 ORDER BY publication_id ASC"
                    ))?;
                    let rows = statement.query_map([], publication_from_row)?;
                    rows.collect::<Result<Vec<_>, _>>()?
                }
            };
            for record in &mut records {
                record.tags = publication_tags(connection, &record.publication_id)?;
            }
            Ok(records)
        })
    }

    /// Stores a signed tombstone with monotonic revision handling.
    ///
    /// A tombstone is independent of the publication row: it may arrive first.
    /// The uploader identity and content ID are immutable attribution. Lower
    /// revisions are ignored, higher revisions replace the stored record, and
    /// equal revisions must be byte-identical. An equal-revision byte conflict
    /// or an attribution change preserves the original row.
    pub fn upsert_tombstone(&self, record: &TombstoneRecord) -> StoreResult<()> {
        validate_entity_id(&record.publication_id, "publication ID")?;
        validate_entity_id(&record.identity_id, "identity ID")?;
        validate_content_id(&record.content_id)?;
        let record = record.clone();
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            let existing = transaction
                .query_row(
                    "SELECT identity_id, content_id, revision, record_bytes
                     FROM tombstones WHERE publication_id = ?1",
                    [&record.publication_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                        ))
                    },
                )
                .optional()?;

            let incoming_revision = i64::try_from(record.revision).unwrap_or(i64::MAX);
            if let Some((identity_id, content_id, revision, record_bytes)) = existing {
                if identity_id != record.identity_id || content_id != record.content_id {
                    return Err(StoreError::RecordConflict(record.publication_id));
                }
                if incoming_revision < revision {
                    // A stale revision must not downgrade the stored verdict.
                } else if incoming_revision == revision {
                    if record_bytes != record.record_bytes {
                        return Err(StoreError::RecordConflict(record.publication_id));
                    }
                } else {
                    transaction.execute(
                        "UPDATE tombstones SET revision = ?2, record_bytes = ?3 WHERE publication_id = ?1",
                        params![record.publication_id, incoming_revision, record.record_bytes],
                    )?;
                }
            } else {
                transaction.execute(
                    "INSERT INTO tombstones
                     (publication_id, identity_id, content_id, revision, record_bytes, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        record.publication_id,
                        record.identity_id,
                        record.content_id,
                        incoming_revision,
                        record.record_bytes,
                        record.created_at
                    ],
                )?;
            }

            transaction.execute(
                "UPDATE publications SET tombstoned = 1, updated_at = ?2 WHERE publication_id = ?1",
                params![record.publication_id, record.created_at],
            )?;
            crate::search_store::refresh_publication_index(&transaction, &record.publication_id)?;
            transaction.commit()?;
            Ok(())
        })
    }

    /// Lists tombstone records.
    pub fn tombstones(&self) -> StoreResult<Vec<TombstoneRecord>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(
                "SELECT publication_id, identity_id, content_id, revision, record_bytes, created_at
                 FROM tombstones ORDER BY created_at ASC",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(TombstoneRecord {
                    publication_id: row.get(0)?,
                    identity_id: row.get(1)?,
                    content_id: row.get(2)?,
                    revision: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
                    record_bytes: row.get(4)?,
                    created_at: u64::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Stores exact canonical manifest bytes under their trusted root.
    pub fn put_manifest(&self, root: &str, bytes: &[u8]) -> StoreResult<()> {
        self.managed.write_manifest(root, bytes)
    }

    /// Reads and verifies exact manifest bytes by their trusted root.
    pub fn manifest(&self, root: &str) -> StoreResult<Vec<u8>> {
        let bytes = self.managed.read_manifest(root)?;
        ManifestV1::parse_body(&bytes).map_err(|error| StoreError::InvalidRecord(error.to_string()))?;
        Ok(bytes)
    }

    /// Loads one stored publication record by its ID.
    pub fn publication(&self, publication_id: &str) -> StoreResult<PublicationRecord> {
        validate_entity_id(publication_id, "publication ID")?;
        let publication_id = publication_id.to_owned();
        self.db.read(move |connection| {
            let mut record = connection
                .query_row(
                    &format!("SELECT {PUBLICATION_COLUMNS} FROM publications WHERE publication_id = ?1"),
                    [&publication_id],
                    publication_from_row,
                )
                .optional()?
                .ok_or_else(|| StoreError::RecordNotFound(format!("unknown publication {publication_id}")))?;
            record.tags = publication_tags(connection, &publication_id)?;
            Ok(record)
        })
    }

    /// Loads the tombstone for a publication, if one is stored.
    pub fn tombstone(&self, publication_id: &str) -> StoreResult<Option<TombstoneRecord>> {
        validate_entity_id(publication_id, "publication ID")?;
        let publication_id = publication_id.to_owned();
        self.db.read(move |connection| {
            Ok(connection
                .query_row(
                    "SELECT publication_id, identity_id, content_id, revision, record_bytes, created_at
                     FROM tombstones WHERE publication_id = ?1",
                    [&publication_id],
                    |row| {
                        Ok(TombstoneRecord {
                            publication_id: row.get(0)?,
                            identity_id: row.get(1)?,
                            content_id: row.get(2)?,
                            revision: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
                            record_bytes: row.get(4)?,
                            created_at: u64::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
                        })
                    },
                )
                .optional()?)
        })
    }

    /// Records whether a publication was announced to the DHT.
    pub fn mark_publication_announced(&self, publication_id: &str, announced: bool, at: u64) -> StoreResult<()> {
        validate_entity_id(publication_id, "publication ID")?;
        let publication_id = publication_id.to_owned();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE publications SET announced = ?2, updated_at = ?3 WHERE publication_id = ?1",
                params![publication_id, if announced { 1_i64 } else { 0_i64 }, at],
            )?;
            Ok(())
        })
    }

    /// Records whether this device is currently serving a publication.
    pub fn mark_publication_serving(&self, publication_id: &str, serving: bool, at: u64) -> StoreResult<()> {
        validate_entity_id(publication_id, "publication ID")?;
        let publication_id = publication_id.to_owned();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE publications SET serving = ?2, updated_at = ?3 WHERE publication_id = ?1",
                params![publication_id, if serving { 1_i64 } else { 0_i64 }, at],
            )?;
            Ok(())
        })
    }

    /// Stores the last observed availability for a publication.
    pub fn set_publication_availability(
        &self,
        publication_id: &str,
        availability: PublicationAvailability,
        checked_at: Option<u64>,
    ) -> StoreResult<()> {
        validate_entity_id(publication_id, "publication ID")?;
        let publication_id = publication_id.to_owned();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE publications SET availability = ?2, last_checked_at = ?3
                 WHERE publication_id = ?1",
                params![
                    publication_id,
                    availability.as_str(),
                    checked_at.map(|value| i64::try_from(value).unwrap_or(i64::MAX))
                ],
            )?;
            crate::search_store::note_search_change(connection)?;
            Ok(())
        })
    }

    /// Marks a publication as withdrawn while retaining its local bytes.
    pub fn mark_publication_unpublished(&self, publication_id: &str, at: u64) -> StoreResult<()> {
        validate_entity_id(publication_id, "publication ID")?;
        let publication_id = publication_id.to_owned();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE publications
                 SET tombstoned = 1, serving = 0, announced = 0, availability = 'record_only',
                     updated_at = ?2
                 WHERE publication_id = ?1",
                params![&publication_id, at],
            )?;
            crate::search_store::remove_publication(connection, &publication_id)?;
            Ok(())
        })
    }

    /// Returns whether verified content may be served to a remote peer.
    ///
    /// Serving requires a verified blob plus at least one local reason to keep
    /// it: a non-tombstoned serving publication, an active replica, or a
    /// retained download.
    pub fn content_serveable(&self, content_id: &str) -> StoreResult<bool> {
        validate_content_id(content_id)?;
        let content_id = content_id.to_owned();
        self.db.read(move |connection| {
            let count: i64 = connection.query_row(
                "SELECT CASE WHEN
                    EXISTS (SELECT 1 FROM contents WHERE content_id = ?1 AND state = 'verified')
                    AND (
                        EXISTS (SELECT 1 FROM publications
                                WHERE content_id = ?1 AND local = 1 AND serving = 1 AND tombstoned = 0)
                        OR EXISTS (SELECT 1 FROM replicas
                                   WHERE content_id = ?1 AND state = 'active')
                        OR EXISTS (SELECT 1 FROM content_refs
                                   WHERE content_id = ?1 AND ref_type = 'retained_download')
                    )
                 THEN 1 ELSE 0 END",
                [content_id],
                |row| row.get(0),
            )?;
            Ok(count != 0)
        })
    }

    /// Returns whether verified content may still be served for a reason other
    /// than one specific publication.
    pub fn content_serveable_except_publication(&self, content_id: &str, publication_id: &str) -> StoreResult<bool> {
        validate_content_id(content_id)?;
        validate_entity_id(publication_id, "publication ID")?;
        let content_id = content_id.to_owned();
        let publication_id = publication_id.to_owned();
        self.db.read(move |connection| {
            let count: i64 = connection.query_row(
                "SELECT CASE WHEN
                    EXISTS (SELECT 1 FROM contents WHERE content_id = ?1 AND state = 'verified')
                    AND (
                        EXISTS (SELECT 1 FROM publications
                                WHERE content_id = ?1 AND publication_id <> ?2
                                  AND local = 1 AND serving = 1 AND tombstoned = 0)
                        OR EXISTS (SELECT 1 FROM replicas
                                   WHERE content_id = ?1 AND state = 'active')
                        OR EXISTS (SELECT 1 FROM content_refs
                                   WHERE content_id = ?1 AND ref_type = 'retained_download')
                    )
                 THEN 1 ELSE 0 END",
                params![content_id, publication_id],
                |row| row.get(0),
            )?;
            Ok(count != 0)
        })
    }

    /// Converts pending streaming-import references into a publication
    /// original reference.
    ///
    /// The content row must exist. Repeating the adoption is idempotent.
    pub fn adopt_local_import(&self, content_id: &str, publication_id: &str) -> StoreResult<()> {
        validate_content_id(content_id)?;
        validate_entity_id(publication_id, "publication ID")?;
        if !self.content_exists(content_id)? {
            return Err(StoreError::ContentMissing(content_id.to_owned()));
        }
        let content = content_id.to_owned();
        let publication = publication_id.to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "DELETE FROM content_refs WHERE content_id = ?1 AND ref_type = 'local_import'",
                [&content],
            )?;
            transaction.execute(
                "INSERT INTO content_refs (content_id, ref_type, owner_id, created_at)
                 VALUES (?1, 'publication_original', ?2, ?3)
                 ON CONFLICT(content_id, ref_type, owner_id) DO NOTHING",
                params![content, publication, now],
            )?;
            transaction.commit()?;
            Ok(())
        })
    }

    /// Returns signed publication bytes for a content ID when one is stored.
    pub fn publication_for_content(&self, content_id: &str) -> StoreResult<Option<Vec<u8>>> {
        validate_content_id(content_id)?;
        let content_id_owned = content_id.to_owned();
        let records = self.db.read(move |connection| {
            let mut statement = connection.prepare(
                "SELECT record_bytes FROM publications WHERE content_id = ?1
                 ORDER BY revision DESC LIMIT 16",
            )?;
            let rows = statement.query_map([content_id_owned], |row| row.get::<_, Vec<u8>>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?;
        for bytes in records {
            if let Ok(publication) = SignedPublication::parse(&bytes)
                && hex_lower(&publication.publication().content_id) == content_id
            {
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }

    /// Reads one bounded slice of a verified content blob.
    ///
    /// Only verified content is served and the requested range must lie inside
    /// the stored file. The caller verifies the resulting bytes against the
    /// manifest chunk hash.
    /// Removes only verified content and enforces the requested range.
    pub fn read_content_slice(&self, content_id: &str, offset: u64, length: usize) -> StoreResult<Vec<u8>> {
        validate_content_id(content_id)?;
        if length == 0 || length > nexus_protocol::MAX_CHUNK_SLICE_BYTES {
            return Err(StoreError::IntegrityMismatch(content_id.to_owned()));
        }
        let record = self.content(content_id)?;
        if record.state != ContentState::Verified {
            return Err(StoreError::ContentMissing(content_id.to_owned()));
        }
        let length_u64 = u64::try_from(length).unwrap_or(u64::MAX);
        let end = offset
            .checked_add(length_u64)
            .ok_or_else(|| StoreError::IntegrityMismatch(content_id.to_owned()))?;
        if end > record.size_bytes {
            return Err(StoreError::IntegrityMismatch(content_id.to_owned()));
        }
        let path = self.managed.blob_path(content_id)?;
        let metadata = fs::metadata(&path)?;
        if metadata.len() != record.size_bytes {
            return Err(StoreError::IntegrityMismatch(content_id.to_owned()));
        }
        let mut file = fs::File::open(path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0_u8; length];
        file.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    /// Loads a verified manifest for a content ID from a local publication or
    /// from the named manifest directory.
    ///
    /// This is the inbound-serving lookup: it only returns manifest bytes whose
    /// canonical root and shape verify and whose content ID matches.
    pub fn manifest_for_content(&self, content_id: &str) -> StoreResult<Vec<u8>> {
        validate_content_id(content_id)?;
        let content_id_owned = content_id.to_owned();
        let records = self.db.read(move |connection| {
            let mut statement = connection.prepare(
                "SELECT record_bytes FROM publications WHERE content_id = ?1 ORDER BY revision DESC LIMIT 16",
            )?;
            let rows = statement.query_map([content_id_owned], |row| row.get::<_, Vec<u8>>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?;
        for bytes in records {
            let Ok(publication) = SignedPublication::parse(&bytes) else {
                continue;
            };
            let root = hex_lower(&publication.publication().manifest_root);
            if let Ok(manifest) = self.managed.read_manifest(&root)
                && ManifestV1::parse_body(&manifest).is_ok_and(|parsed| {
                    parsed.content_id == publication.publication().content_id
                        && parsed.validate_for_publication(publication.publication()).is_ok()
                })
            {
                return Ok(manifest);
            }
        }
        for entry in self.managed.scan_manifests()? {
            let Some(root) = entry.path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if !is_content_id(root) {
                continue;
            }
            let Ok(manifest) = self.managed.read_manifest(root) else {
                continue;
            };
            if ManifestV1::parse_body(&manifest).is_ok_and(|parsed| hex_lower(&parsed.content_id) == content_id) {
                return Ok(manifest);
            }
        }
        Err(StoreError::ManifestMissing(content_id.to_owned()))
    }

    /// Returns a verified manifest for local content, building and storing one
    /// from the blob when no stored manifest exists.
    pub fn ensure_manifest_for_content(&self, content_id: &str) -> StoreResult<Vec<u8>> {
        validate_content_id(content_id)?;
        if let Ok(manifest) = self.manifest_for_content(content_id) {
            return Ok(manifest);
        }
        let record = self.content(content_id)?;
        if record.state != ContentState::Verified || record.size_bytes == 0 {
            return Err(StoreError::ContentMissing(content_id.to_owned()));
        }
        let content_bytes: [u8; 32] = decode_hex_content_id(content_id)?;
        let path = self.content_path(content_id)?;
        let mut file = fs::File::open(path)?;
        let chunk_size = usize::try_from(CHUNK_SIZE_V1).unwrap_or(usize::MAX);
        let mut remaining = record.size_bytes;
        let mut chunks = Vec::new();
        let mut buffer = vec![0_u8; chunk_size];
        while remaining > 0 {
            let length = usize::try_from(remaining.min(CHUNK_SIZE_V1)).unwrap_or(chunk_size);
            file.read_exact(&mut buffer[..length])?;
            let sha256: [u8; 32] = Sha256::digest(&buffer[..length]).into();
            chunks.push(ChunkEntry::new(u64::try_from(length).unwrap_or(u64::MAX), sha256));
            remaining -= u64::try_from(length).unwrap_or(remaining);
        }
        let manifest = ManifestV1::new(content_bytes, record.size_bytes, chunks);
        let bytes = manifest
            .encode_body()
            .map_err(|error| StoreError::InvalidManifest(error.to_string()))?;
        let root = manifest
            .root()
            .map_err(|error| StoreError::InvalidManifest(error.to_string()))?;
        self.put_manifest(&hex_lower(&root), &bytes)?;
        Ok(bytes)
    }

    /// Loads and verifies the manifest referenced by a stored signed publication.
    pub fn manifest_for_publication(&self, publication_id: &str) -> StoreResult<Vec<u8>> {
        validate_entity_id(publication_id, "publication ID")?;
        let record_bytes = self
            .db
            .read(|connection| {
                Ok(connection
                    .query_row(
                        "SELECT record_bytes FROM publications WHERE publication_id = ?1",
                        [publication_id],
                        |row| row.get::<_, Vec<u8>>(0),
                    )
                    .optional()?)
            })?
            .ok_or_else(|| StoreError::RecordNotFound(publication_id.to_owned()))?;
        let signed =
            SignedPublication::parse(&record_bytes).map_err(|error| StoreError::InvalidRecord(error.to_string()))?;
        let root = hex_lower(&signed.publication().manifest_root);
        let bytes = self.managed.read_manifest(&root)?;
        let manifest = ManifestV1::parse_body(&bytes).map_err(|error| StoreError::InvalidRecord(error.to_string()))?;
        manifest
            .validate_for_publication(signed.publication())
            .map_err(|error| StoreError::InvalidManifest(error.to_string()))?;
        Ok(bytes)
    }

    /// Adds or replaces a hosted replica record.
    pub fn upsert_replica(&self, record: &ReplicaRecord) -> StoreResult<()> {
        validate_entity_id(&record.replica_id, "replica ID")?;
        validate_content_id(&record.content_id)?;
        if let Some(publication_id) = &record.publication_id {
            validate_entity_id(publication_id, "publication ID")?;
        }
        if let Some(source_peer) = &record.source_peer {
            validate_entity_id(source_peer, "source peer ID")?;
        }
        if let Some(error) = &record.last_error
            && error.len() > 512
        {
            return Err(StoreError::InvalidConfig(
                "replica error message exceeds 512 bytes".to_owned(),
            ));
        }
        let record = record.clone();
        self.db.write(move |connection| {
            connection.execute(
                "INSERT INTO replicas
                 (replica_id, content_id, publication_id, state, source_peer, verified_size_bytes,
                  last_verified_at, last_checked_at, last_seen_at, failure_count, last_error, announced,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(replica_id) DO UPDATE SET
                     content_id = excluded.content_id,
                     publication_id = excluded.publication_id,
                     state = excluded.state,
                     source_peer = excluded.source_peer,
                     verified_size_bytes = excluded.verified_size_bytes,
                     last_verified_at = excluded.last_verified_at,
                     last_checked_at = excluded.last_checked_at,
                     last_seen_at = excluded.last_seen_at,
                     failure_count = excluded.failure_count,
                     last_error = excluded.last_error,
                     announced = excluded.announced,
                     updated_at = excluded.updated_at",
                params![
                    record.replica_id,
                    record.content_id,
                    record.publication_id,
                    record.state.as_str(),
                    record.source_peer,
                    record.verified_size_bytes,
                    record.last_verified_at,
                    record.last_checked_at,
                    record.last_seen_at,
                    i64::try_from(record.failure_count).unwrap_or(i64::MAX),
                    record.last_error,
                    record.announced,
                    record.created_at,
                    record.updated_at
                ],
            )?;
            Ok(())
        })
    }

    /// Returns one replica record.
    pub fn replica(&self, replica_id: &str) -> StoreResult<ReplicaRecord> {
        validate_entity_id(replica_id, "replica ID")?;
        let replica_id = replica_id.to_owned();
        self.db.read(move |connection| {
            connection
                .query_row(
                    &format!("SELECT {REPLICA_COLUMNS} FROM replicas WHERE replica_id = ?1"),
                    [&replica_id],
                    replica_from_row,
                )
                .optional()?
                .ok_or(StoreError::RecordNotFound(replica_id))
        })
    }

    /// Lists hosted replicas.
    pub fn replicas(&self) -> StoreResult<Vec<ReplicaRecord>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {REPLICA_COLUMNS} FROM replicas ORDER BY created_at ASC"
            ))?;
            let rows = statement.query_map([], replica_from_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Lists one stable page of replicas ordered by replica ID.
    ///
    /// `cursor` is the last replica ID of the previous page. Removed rows are
    /// hidden unless `include_removed` is true.
    pub fn replicas_page(
        &self,
        cursor: Option<&str>,
        limit: usize,
        include_removed: bool,
    ) -> StoreResult<Vec<ReplicaRecord>> {
        if limit == 0 || limit > 100 {
            return Err(StoreError::InvalidConfig(
                "replica page size must be between 1 and 100".to_owned(),
            ));
        }
        if let Some(cursor) = cursor {
            validate_entity_id(cursor, "replica cursor")?;
        }
        let cursor = cursor.map(str::to_owned);
        let limit = i64::try_from(limit).unwrap_or(100);
        self.db.read(move |connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {REPLICA_COLUMNS} FROM replicas
                 WHERE (?1 IS NULL OR replica_id > ?1)
                   AND (?2 = 1 OR state <> 'removed')
                 ORDER BY replica_id ASC LIMIT ?3"
            ))?;
            let rows = statement.query_map(params![cursor, include_removed, limit], replica_from_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Counts non-removed replicas.
    pub fn replica_count(&self) -> StoreResult<usize> {
        self.db.read(|connection| {
            let count: i64 =
                connection.query_row("SELECT COUNT(*) FROM replicas WHERE state <> 'removed'", [], |row| {
                    row.get(0)
                })?;
            Ok(usize::try_from(count).unwrap_or(usize::MAX))
        })
    }

    /// Aggregate hosted storage and state counts.
    pub fn replica_usage(&self) -> StoreResult<ReplicaUsage> {
        self.db.read(|connection| {
            let (total, active, paused, failed): (i64, i64, i64, i64) = connection.query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN state = 'active' THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN state = 'paused' THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN state = 'verification_failed' THEN 1 ELSE 0 END), 0)
                 FROM replicas WHERE state <> 'removed'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            let storage: i64 = connection.query_row(
                "SELECT COALESCE(SUM(contents.size_bytes), 0)
                 FROM (SELECT DISTINCT content_id FROM replicas WHERE state <> 'removed') hosted
                 JOIN contents ON contents.content_id = hosted.content_id",
                [],
                |row| row.get(0),
            )?;
            Ok(ReplicaUsage {
                total_count: u64::try_from(total).unwrap_or(0),
                active_count: u64::try_from(active).unwrap_or(0),
                paused_count: u64::try_from(paused).unwrap_or(0),
                failed_count: u64::try_from(failed).unwrap_or(0),
                storage_bytes: u64::try_from(storage).unwrap_or(0),
            })
        })
    }

    /// Changes a replica lifecycle state.
    pub fn set_replica_state(&self, replica_id: &str, state: ReplicaState, at: u64) -> StoreResult<()> {
        validate_entity_id(replica_id, "replica ID")?;
        let replica_id = replica_id.to_owned();
        self.db.write(move |connection| {
            let updated = connection.execute(
                "UPDATE replicas SET state = ?2, updated_at = ?3 WHERE replica_id = ?1",
                params![replica_id, state.as_str(), at],
            )?;
            if updated == 0 {
                return Err(StoreError::RecordNotFound(replica_id));
            }
            Ok(())
        })
    }

    /// Records one verification attempt.
    ///
    /// A success updates the verified size and clears the failure count. A
    /// failure increments the count and stores a bounded redacted message.
    pub fn record_replica_check(
        &self,
        replica_id: &str,
        checked_at: u64,
        verified_size: Option<u64>,
        error: Option<&str>,
    ) -> StoreResult<()> {
        validate_entity_id(replica_id, "replica ID")?;
        if let Some(error) = error
            && error.len() > 512
        {
            return Err(StoreError::InvalidConfig(
                "replica error message exceeds 512 bytes".to_owned(),
            ));
        }
        let replica_id = replica_id.to_owned();
        let error = error.map(str::to_owned);
        self.db.write(move |connection| {
            let updated = if let Some(size) = verified_size {
                connection.execute(
                    "UPDATE replicas
                     SET last_checked_at = ?2, last_verified_at = ?2, verified_size_bytes = ?3,
                         failure_count = 0, last_error = NULL, updated_at = ?2
                     WHERE replica_id = ?1",
                    params![replica_id, checked_at, size],
                )?
            } else {
                connection.execute(
                    "UPDATE replicas
                     SET last_checked_at = ?2, failure_count = failure_count + 1,
                         last_error = ?3, updated_at = ?2
                     WHERE replica_id = ?1",
                    params![replica_id, checked_at, error],
                )?
            };
            if updated == 0 {
                return Err(StoreError::RecordNotFound(replica_id));
            }
            Ok(())
        })
    }

    /// Records whether this node advertises the replica as a provider.
    pub fn mark_replica_announced(&self, replica_id: &str, announced: bool, at: u64) -> StoreResult<()> {
        validate_entity_id(replica_id, "replica ID")?;
        let replica_id = replica_id.to_owned();
        self.db.write(move |connection| {
            let updated = if announced {
                connection.execute(
                    "UPDATE replicas SET announced = 1, last_seen_at = ?2, updated_at = ?2 WHERE replica_id = ?1",
                    params![replica_id, at],
                )?
            } else {
                connection.execute(
                    "UPDATE replicas SET announced = 0, updated_at = ?2 WHERE replica_id = ?1",
                    params![replica_id, at],
                )?
            };
            if updated == 0 {
                return Err(StoreError::RecordNotFound(replica_id));
            }
            Ok(())
        })
    }

    /// Records that this node served or advertised one content ID.
    pub fn record_replica_seen(&self, content_id: &str, at: u64) -> StoreResult<()> {
        validate_content_id(content_id)?;
        let content_id = content_id.to_owned();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE replicas SET last_seen_at = ?2, updated_at = ?2
                 WHERE content_id = ?1 AND state = 'active'",
                params![content_id, at],
            )?;
            Ok(())
        })
    }

    /// Reads persisted hosting limits.
    pub fn hosting_limits(&self) -> StoreResult<HostingLimits> {
        let read = |key: &str| {
            self.db.read(|connection| {
                Ok(connection
                    .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                        row.get::<_, String>(0)
                    })
                    .optional()?)
            })
        };
        let upload = read(UPLOAD_LIMIT_SETTING)?
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let allow_metered = read(ALLOW_METERED_SETTING)?.as_deref() == Some("true");
        let battery_min = read(BATTERY_MIN_SETTING)?
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(0);
        let limits = HostingLimits {
            upload_bytes_per_second: upload,
            allow_metered,
            battery_min_percent: battery_min,
        };
        // Invalid persisted values fall back to safe defaults rather than
        // preventing the core from starting.
        if limits.validate().is_err() {
            return Ok(HostingLimits::default());
        }
        Ok(limits)
    }

    /// Persists all hosting limits atomically.
    pub fn set_hosting_limits(&self, limits: &HostingLimits) -> StoreResult<()> {
        limits.validate()?;
        let now = now_secs();
        let upload = limits.upload_bytes_per_second.to_string();
        let allow_metered = if limits.allow_metered { "true" } else { "false" }.to_owned();
        let battery_min = limits.battery_min_percent.to_string();
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            for (key, value) in [
                (UPLOAD_LIMIT_SETTING, upload),
                (ALLOW_METERED_SETTING, allow_metered),
                (BATTERY_MIN_SETTING, battery_min),
            ] {
                transaction.execute(
                    "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                    params![key, value, now],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
    }

    /// Merges a peer observation.
    pub fn record_peer_observation(&self, observation: &PeerObservation) -> StoreResult<()> {
        validate_entity_id(&observation.peer_id, "peer ID")?;
        let mut observation = observation.clone();
        observation.address = bounded_observation_text(observation.address, 512);
        observation.last_error = bounded_observation_text(observation.last_error, 256);
        self.db.write(move |connection| {
            connection.execute(
                "INSERT INTO peers (peer_id, address, last_seen_at, last_error, observation_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(peer_id) DO UPDATE SET
                     address = excluded.address,
                     last_seen_at = excluded.last_seen_at,
                     last_error = excluded.last_error,
                     observation_count = peers.observation_count + 1",
                params![
                    observation.peer_id,
                    observation.address,
                    observation.last_seen_at,
                    observation.last_error,
                    i64::try_from(observation.observation_count).unwrap_or(i64::MAX)
                ],
            )?;
            Ok(())
        })
    }

    /// Lists bounded peer observations.
    pub fn peers(&self) -> StoreResult<Vec<PeerObservation>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(
                "SELECT peer_id, address, last_seen_at, last_error, observation_count
                 FROM peers ORDER BY last_seen_at DESC",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(PeerObservation {
                    peer_id: row.get(0)?,
                    address: row.get(1)?,
                    last_seen_at: u64::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
                    last_error: row.get(3)?,
                    observation_count: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Blocks a peer locally, replacing the reason and note when already blocked.
    ///
    /// The cap is enforced inside the writer transaction so concurrent callers
    /// cannot exceed the configured number of entries.
    pub fn block_peer(&self, peer_id: &str, reason: BlockReason, note: Option<&str>) -> StoreResult<BlockedPeer> {
        validate_entity_id(peer_id, "peer ID")?;
        let peer = peer_id.to_owned();
        let note = bounded_security_note(note);
        let max = self.config.max_blocked_peers;
        let now = now_secs();
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM blocked_peers WHERE peer_id = ?1)",
                [&peer],
                |row| row.get(0),
            )?;
            if exists {
                transaction.execute(
                    "UPDATE blocked_peers SET reason = ?2, note = ?3, updated_at = ?4 WHERE peer_id = ?1",
                    params![peer, reason.as_str(), note, now],
                )?;
            } else {
                let count: i64 = transaction.query_row("SELECT COUNT(*) FROM blocked_peers", [], |row| row.get(0))?;
                if usize::try_from(count).unwrap_or(usize::MAX) >= max {
                    return Err(StoreError::InvalidConfig(format!(
                        "local block list is full ({max} entries)"
                    )));
                }
                transaction.execute(
                    "INSERT INTO blocked_peers (peer_id, reason, note, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![peer, reason.as_str(), note, now],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })?;
        self.blocked_peer(peer_id)?
            .ok_or_else(|| StoreError::RecordNotFound(peer_id.to_owned()))
    }

    /// Removes a local block, returning whether a row was removed.
    pub fn unblock_peer(&self, peer_id: &str) -> StoreResult<bool> {
        validate_entity_id(peer_id, "peer ID")?;
        let peer = peer_id.to_owned();
        self.db.write(move |connection| {
            Ok(connection.execute("DELETE FROM blocked_peers WHERE peer_id = ?1", [peer])? > 0)
        })
    }

    /// Returns one local block, if present.
    pub fn blocked_peer(&self, peer_id: &str) -> StoreResult<Option<BlockedPeer>> {
        validate_entity_id(peer_id, "peer ID")?;
        let peer = peer_id.to_owned();
        self.db.read(move |connection| {
            Ok(connection
                .query_row(
                    "SELECT peer_id, reason, note, created_at, updated_at FROM blocked_peers WHERE peer_id = ?1",
                    [peer],
                    blocked_peer_from_row,
                )
                .optional()?)
        })
    }

    /// Returns true when local policy blocks the peer.
    pub fn is_peer_blocked(&self, peer_id: &str) -> StoreResult<bool> {
        Ok(self.blocked_peer(peer_id)?.is_some())
    }

    /// Lists every local block, newest first.
    pub fn blocked_peers(&self) -> StoreResult<Vec<BlockedPeer>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(
                "SELECT peer_id, reason, note, created_at, updated_at
                 FROM blocked_peers ORDER BY created_at DESC, peer_id ASC",
            )?;
            let rows = statement.query_map([], blocked_peer_from_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Number of locally blocked peers.
    pub fn blocked_peer_count(&self) -> StoreResult<usize> {
        self.db.read(|connection| {
            let count: i64 = connection.query_row("SELECT COUNT(*) FROM blocked_peers", [], |row| row.get(0))?;
            Ok(usize::try_from(count).unwrap_or(usize::MAX))
        })
    }

    /// Records a local abuse report without uploading it.
    ///
    /// The cap is enforced inside the writer transaction so concurrent callers
    /// cannot exceed the configured number of reports.
    pub fn record_abuse_report(
        &self,
        peer_id: &str,
        category: AbuseCategory,
        note: Option<&str>,
    ) -> StoreResult<AbuseReport> {
        validate_entity_id(peer_id, "peer ID")?;
        let peer = peer_id.to_owned();
        let note = bounded_security_note(note);
        let stored_note = note.clone();
        let max = self.config.max_abuse_reports;
        let now = now_secs();
        let report_id = self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            let count: i64 = transaction.query_row("SELECT COUNT(*) FROM abuse_reports", [], |row| row.get(0))?;
            if usize::try_from(count).unwrap_or(usize::MAX) >= max {
                return Err(StoreError::InvalidConfig(format!(
                    "local abuse report list is full ({max} entries)"
                )));
            }
            transaction.execute(
                "INSERT INTO abuse_reports (peer_id, category, note, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![peer, category.as_str(), stored_note, now],
            )?;
            let report_id = u64::try_from(transaction.last_insert_rowid()).unwrap_or(0);
            transaction.commit()?;
            Ok(report_id)
        })?;
        Ok(AbuseReport {
            report_id,
            peer_id: peer_id.to_owned(),
            category,
            note: bounded_security_note(note.as_deref()),
            created_at: now,
        })
    }

    /// Lists local abuse reports, newest first.
    pub fn abuse_reports(&self) -> StoreResult<Vec<AbuseReport>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare(
                "SELECT report_id, peer_id, category, note, created_at
                 FROM abuse_reports ORDER BY created_at DESC, report_id DESC",
            )?;
            let rows = statement.query_map([], abuse_report_from_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Number of local abuse reports.
    pub fn abuse_report_count(&self) -> StoreResult<usize> {
        self.db.read(|connection| {
            let count: i64 = connection.query_row("SELECT COUNT(*) FROM abuse_reports", [], |row| row.get(0))?;
            Ok(usize::try_from(count).unwrap_or(usize::MAX))
        })
    }

    /// Stores a settings value.
    pub fn set_setting(&self, key: &str, value: &str) -> StoreResult<()> {
        validate_entity_id(key, "setting key")?;
        let key = key.to_owned();
        let value = value.to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value, now],
            )?;
            Ok(())
        })
    }

    /// Reads a settings value.
    pub fn get_setting(&self, key: &str) -> StoreResult<Option<String>> {
        validate_entity_id(key, "setting key")?;
        let key = key.to_owned();
        self.db.read(move |connection| {
            Ok(connection
                .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0))
                .optional()?)
        })
    }

    /// Lists settings pairs.
    pub fn settings(&self) -> StoreResult<Vec<(String, String)>> {
        self.db.read(|connection| {
            let mut statement = connection.prepare("SELECT key, value FROM settings ORDER BY key ASC")?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Exports metadata to a host-selected archive outside the managed store.
    pub fn export_metadata(&self, destination: &Path) -> StoreResult<ExportReport> {
        self.validate_external_path(destination, "metadata export destination")?;
        backup::export(&self.db, destination)
    }

    /// Restores metadata from a validated archive.
    ///
    /// The archive is fully staged and integrity-checked before the current
    /// database is replaced. File bytes are not part of the archive.
    pub fn restore_metadata(&mut self, archive: &Path) -> StoreResult<RestoreReport> {
        self.validate_external_path(archive, "metadata archive")?;
        let report = backup::restore(&mut self.db, archive)?;
        self.reconciliation = self.reconcile_internal()?;
        Ok(report)
    }

    /// Reconciles database state and generated directories.
    pub fn reconcile(&mut self) -> StoreResult<&ReconciliationReport> {
        self.reconciliation = self.reconcile_internal()?;
        Ok(&self.reconciliation)
    }

    fn validate_external_path(&self, path: &Path, label: &str) -> StoreResult<()> {
        if !path.is_absolute() {
            return Err(StoreError::UnsafePath(format!("{label} must be absolute")));
        }
        let parent = path
            .parent()
            .ok_or_else(|| StoreError::UnsafePath(format!("{label} has no parent directory")))?;
        let parent = parent.canonicalize()?;
        let store_root = self.managed.root().canonicalize()?;
        if parent.starts_with(&store_root) {
            return Err(StoreError::UnsafePath(format!(
                "{label} must be outside the managed store"
            )));
        }
        Ok(())
    }

    fn content_is_referenced(&self, content_id: &str) -> StoreResult<bool> {
        let content_id = content_id.to_owned();
        self.db.read(move |connection| {
            let count: i64 = connection.query_row(
                "SELECT
                    (SELECT COUNT(*) FROM content_refs WHERE content_id = ?1)
                  + (SELECT COUNT(*) FROM publications WHERE content_id = ?1)
                  + (SELECT COUNT(*) FROM profiles WHERE avatar_content_id = ?1)
                  + (SELECT COUNT(*) FROM replicas WHERE content_id = ?1 AND state != 'removed')",
                [content_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    fn update_content_state(&self, content_id: &str, state: ContentState) -> StoreResult<()> {
        let content_id = content_id.to_owned();
        let state = state.as_str().to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE contents SET state = ?2, updated_at = ?3 WHERE content_id = ?1",
                params![content_id, state, now],
            )?;
            Ok(())
        })
    }

    fn refresh_content(&self, content_id: &str, size_bytes: u64, state: ContentState) -> StoreResult<()> {
        let content_id = content_id.to_owned();
        let state = state.as_str().to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            connection.execute(
                "UPDATE contents SET size_bytes = ?2, state = ?3, verified_at = ?4, updated_at = ?4
                 WHERE content_id = ?1",
                params![content_id, size_bytes, state, now],
            )?;
            Ok(())
        })
    }

    fn delete_download_row(&self, download_id: &str) -> StoreResult<bool> {
        validate_store_component(download_id, "download ID")?;
        let download_id = download_id.to_owned();
        self.db.write(move |connection| {
            let removed = connection.execute("DELETE FROM downloads WHERE download_id = ?1", [download_id])?;
            Ok(removed > 0)
        })
    }

    fn mark_download_failed(&self, download_id: &str, verified_prefix: u64, message: &str) -> StoreResult<()> {
        let download_id = download_id.to_owned();
        let message = message.to_owned();
        let now = now_secs();
        self.db.write(move |connection| {
            let expected_size = connection
                .query_row(
                    "SELECT expected_size FROM downloads WHERE download_id = ?1",
                    [&download_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            let Some(expected_size) = expected_size else {
                return Ok(());
            };
            let map = ChunkMap::from_prefix(u64::try_from(expected_size).unwrap_or(0), verified_prefix);
            let prefix = map.contiguous_prefix();
            connection.execute(
                "UPDATE downloads
                 SET state = 'failed', verified_prefix = ?2, verified_bytes = ?3, chunk_map = ?4,
                     last_error = ?5, updated_at = ?6
                 WHERE download_id = ?1",
                params![
                    download_id,
                    prefix,
                    map.verified_bytes(),
                    map.bytes().to_vec(),
                    message,
                    now
                ],
            )?;
            Ok(())
        })
    }

    fn adopt_download_blob(&self, download: &DownloadRecord) -> StoreResult<()> {
        let content_id = download.content_id.clone();
        let size_bytes = download.expected_size;
        let owner_id = download.download_id.clone();
        let download_id = download.download_id.clone();
        let blob_path = ManagedStore::blob_relative_path(&content_id)?;
        let now = now_secs();
        self.db.write(move |connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "INSERT INTO contents (content_id, size_bytes, state, blob_path, verified_at, created_at, updated_at)
                 VALUES (?1, ?2, 'verified', ?3, ?4, ?4, ?4)
                 ON CONFLICT(content_id) DO UPDATE SET
                     size_bytes = excluded.size_bytes,
                     state = 'verified',
                     blob_path = excluded.blob_path,
                     verified_at = excluded.verified_at,
                     updated_at = excluded.updated_at",
                params![&content_id, size_bytes, blob_path, now],
            )?;
            transaction.execute(
                "INSERT INTO content_refs (content_id, ref_type, owner_id, created_at)
                 VALUES (?1, 'retained_download', ?2, ?3)
                 ON CONFLICT(content_id, ref_type, owner_id) DO NOTHING",
                params![&content_id, owner_id, now],
            )?;
            transaction.execute("DELETE FROM downloads WHERE download_id = ?1", [download_id])?;
            transaction.commit()?;
            Ok(())
        })
    }

    fn reconcile_internal(&self) -> StoreResult<ReconciliationReport> {
        let mut report = ReconciliationReport::default();
        let contents = self.contents()?;
        let downloads = self.downloads()?;
        let content_by_id: HashMap<String, ContentRecord> = contents
            .iter()
            .cloned()
            .map(|record| (record.content_id.clone(), record))
            .collect();
        let mut observed: HashSet<String> = HashSet::new();
        let mut quarantined: HashSet<String> = HashSet::new();
        let blob_entries = self.managed.scan_blobs()?;

        for entry in blob_entries {
            let Some(content_id) = entry.content_id else {
                self.managed.quarantine_file(&entry.path, "unexpected-blob")?;
                report.quarantined_files += 1;
                continue;
            };

            if let Some(record) = content_by_id.get(&content_id).cloned() {
                let actual_size = fs::metadata(&entry.path)?.len();
                match self.managed.verify_blob(&content_id, actual_size) {
                    Ok(()) => {
                        observed.insert(content_id.clone());
                        report.verified_blobs += 1;
                        if record.size_bytes != actual_size || record.state != ContentState::Verified {
                            self.refresh_content(&content_id, actual_size, ContentState::Verified)?;
                        }
                    }
                    Err(StoreError::ContentMissing(_) | StoreError::IntegrityMismatch(_)) => {
                        self.managed.quarantine_file(&entry.path, &content_id)?;
                        self.refresh_content(&content_id, actual_size, ContentState::Quarantined)?;
                        quarantined.insert(content_id.clone());
                        report.quarantined_files += 1;
                    }
                    Err(error) => return Err(error),
                }
                continue;
            }

            if let Some(download) = downloads.iter().find(|download| download.content_id == content_id) {
                match self.managed.verify_blob(&content_id, download.expected_size) {
                    Ok(()) => {
                        self.adopt_download_blob(download)?;
                        let _ = self.managed.remove_partial(&download.download_id);
                        observed.insert(content_id.clone());
                        report.adopted_downloads += 1;
                    }
                    Err(StoreError::ContentMissing(_) | StoreError::IntegrityMismatch(_)) => {
                        self.managed.quarantine_file(&entry.path, &content_id)?;
                        report.quarantined_files += 1;
                    }
                    Err(error) => return Err(error),
                }
                continue;
            }

            let size = fs::metadata(&entry.path)?.len();
            self.managed.remove_file(&entry.path)?;
            report.deleted_orphan_blobs += 1;
            report.bytes_reclaimed = report.bytes_reclaimed.saturating_add(size);
        }

        for record in &contents {
            if observed.contains(&record.content_id) || quarantined.contains(&record.content_id) {
                continue;
            }
            if record.state == ContentState::Quarantined {
                continue;
            }
            self.update_content_state(&record.content_id, ContentState::Missing)?;
            report.missing_contents += 1;
        }

        for record in contents {
            if self.content_is_referenced(&record.content_id)? {
                continue;
            }
            if let Ok(path) = self.managed.blob_path(&record.content_id)
                && let Ok(metadata) = fs::metadata(&path)
            {
                report.bytes_reclaimed = report.bytes_reclaimed.saturating_add(metadata.len());
                let _ = self.managed.remove_file(&path);
            }
            let content_id = record.content_id.clone();
            self.db.write(move |connection| {
                connection.execute("DELETE FROM contents WHERE content_id = ?1", [content_id])?;
                Ok(())
            })?;
            report.removed_unreferenced_contents += 1;
        }

        let download_by_id: HashMap<String, DownloadRecord> = downloads
            .iter()
            .cloned()
            .map(|download| (download.download_id.clone(), download))
            .collect();
        let mut partial_ids = HashSet::new();
        for path in self.managed.scan_partials()? {
            if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                self.managed.quarantine_file(&path, "unexpected-partial")?;
                report.orphan_partials += 1;
                report.quarantined_files += 1;
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                self.managed.quarantine_file(&path, "unexpected-partial")?;
                report.orphan_partials += 1;
                report.quarantined_files += 1;
                continue;
            };
            let Some(download_id) = name.strip_suffix(".part") else {
                self.managed.quarantine_file(&path, "unexpected-partial")?;
                report.orphan_partials += 1;
                report.quarantined_files += 1;
                continue;
            };
            let Some(download) = download_by_id.get(download_id) else {
                self.managed.quarantine_file(&path, "unexpected-partial")?;
                report.orphan_partials += 1;
                report.quarantined_files += 1;
                continue;
            };
            partial_ids.insert(download_id.to_owned());
            let file_length = fs::metadata(&path)?.len();
            let verified_end = chunk_map_for(download)
                .highest_verified_end()
                .max(download.verified_prefix);
            if file_length > verified_end {
                self.managed.truncate_partial(download_id, verified_end)?;
                report.truncated_partials += 1;
            } else if file_length < verified_end {
                self.mark_download_failed(
                    download_id,
                    file_length,
                    "partial file was shorter than its verified chunks",
                )?;
                report.failed_downloads += 1;
            }
        }

        for download in &downloads {
            if !matches!(
                download.state,
                DownloadState::Pending | DownloadState::Downloading | DownloadState::Paused
            ) {
                continue;
            }
            if !partial_ids.contains(&download.download_id) {
                self.mark_download_failed(&download.download_id, 0, "partial file is missing")?;
                report.failed_downloads += 1;
            }
        }

        for entry in self.managed.scan_manifests()? {
            let Some(root) = entry.content_id else {
                self.managed.quarantine_file(&entry.path, "unexpected-manifest")?;
                report.quarantined_files += 1;
                continue;
            };
            match self.managed.verify_manifest_file(&entry.path, &root) {
                Ok(()) => report.verified_manifests += 1,
                Err(StoreError::InvalidManifest(_) | StoreError::ManifestMissing(_)) => {
                    self.managed.quarantine_file(&entry.path, &root)?;
                    report.quarantined_files += 1;
                }
                Err(error) => return Err(error),
            }
        }

        let (removed_temp, temp_bytes) = self.managed.clear_temporary()?;
        report.removed_temp_files += removed_temp;
        report.bytes_reclaimed = report.bytes_reclaimed.saturating_add(temp_bytes);

        Ok(report)
    }
}

fn validate_content_id(content_id: &str) -> StoreResult<()> {
    if is_content_id(content_id) {
        Ok(())
    } else {
        Err(StoreError::InvalidContentId(content_id.to_owned()))
    }
}

fn decode_hex_content_id(content_id: &str) -> StoreResult<[u8; 32]> {
    validate_content_id(content_id)?;
    let mut bytes = [0_u8; 32];
    for (index, pair) in content_id.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let text = std::str::from_utf8(pair).map_err(|_| StoreError::InvalidContentId(content_id.to_owned()))?;
        bytes[index] = u8::from_str_radix(text, 16).map_err(|_| StoreError::InvalidContentId(content_id.to_owned()))?;
    }
    Ok(bytes)
}

fn validate_source_peer(source_peer: Option<&str>) -> StoreResult<Option<String>> {
    let Some(source_peer) = source_peer else {
        return Ok(None);
    };
    if source_peer.is_empty() || source_peer.len() > 128 || !source_peer.is_ascii() {
        return Err(StoreError::InvalidId(
            "source peer must be ASCII and at most 128 bytes".to_owned(),
        ));
    }
    Ok(Some(source_peer.to_owned()))
}

fn validate_entity_id(value: &str, label: &str) -> StoreResult<()> {
    if is_safe_id(value) {
        Ok(())
    } else {
        Err(StoreError::InvalidId(format!("{label} is malformed")))
    }
}

/// Truncates optional observation text on a character boundary.
fn bounded_observation_text(value: Option<String>, max_bytes: usize) -> Option<String> {
    let value = value?;
    if value.len() <= max_bytes {
        return Some(value);
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    Some(value[..end].to_owned())
}

/// Normalizes an optional note, dropping empty notes and capping the length.
fn bounded_security_note(note: Option<&str>) -> Option<String> {
    let note = note?.trim();
    if note.is_empty() {
        return None;
    }
    let mut end = note.len().min(MAX_SECURITY_NOTE_BYTES);
    while end > 0 && !note.is_char_boundary(end) {
        end -= 1;
    }
    Some(note[..end].to_owned())
}

fn blocked_peer_from_row(row: &Row<'_>) -> rusqlite::Result<BlockedPeer> {
    let reason: String = row.get(1)?;
    Ok(BlockedPeer {
        peer_id: row.get(0)?,
        reason: BlockReason::parse(&reason).ok_or_else(|| invalid_enum(1, "block reason", &reason))?,
        note: row.get(2)?,
        created_at: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
        updated_at: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
    })
}

fn abuse_report_from_row(row: &Row<'_>) -> rusqlite::Result<AbuseReport> {
    let category: String = row.get(2)?;
    Ok(AbuseReport {
        report_id: u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
        peer_id: row.get(1)?,
        category: AbuseCategory::parse(&category).ok_or_else(|| invalid_enum(2, "abuse category", &category))?,
        note: row.get(3)?,
        created_at: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
    })
}

fn invalid_enum(index: usize, label: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unknown {label}: {value}"),
        )),
    )
}

fn content_from_row(row: &Row<'_>) -> rusqlite::Result<ContentRecord> {
    let state: String = row.get(2)?;
    Ok(ContentRecord {
        content_id: row.get(0)?,
        size_bytes: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
        state: ContentState::parse(&state).ok_or_else(|| invalid_enum(2, "content state", &state))?,
        blob_path: row.get(3)?,
        verified_at: row
            .get::<_, Option<i64>>(4)?
            .map(|value| u64::try_from(value).unwrap_or(0)),
        created_at: u64::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
        updated_at: u64::try_from(row.get::<_, i64>(6)?).unwrap_or(0),
    })
}

pub(crate) fn publication_from_row(row: &Row<'_>) -> rusqlite::Result<PublicationRecord> {
    let category: String = row.get(11)?;
    let availability: String = row.get(16)?;
    Ok(PublicationRecord {
        publication_id: row.get(0)?,
        identity_id: row.get(1)?,
        content_id: row.get(2)?,
        revision: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
        record_bytes: row.get(4)?,
        title: row.get(5)?,
        description: row.get(6)?,
        filename: row.get(7)?,
        published_at: u64::try_from(row.get::<_, i64>(8)?).unwrap_or(0),
        updated_at: u64::try_from(row.get::<_, i64>(9)?).unwrap_or(0),
        tombstoned: row.get::<_, i64>(10)? != 0,
        category: Category::parse(&category).ok_or_else(|| invalid_enum(11, "publication category", &category))?,
        size_bytes: u64::try_from(row.get::<_, i64>(12)?).unwrap_or(0),
        tags: Vec::new(),
        local: row.get::<_, i64>(13)? != 0,
        serving: row.get::<_, i64>(14)? != 0,
        announced: row.get::<_, i64>(15)? != 0,
        availability: PublicationAvailability::parse(&availability)
            .ok_or_else(|| invalid_enum(16, "publication availability", &availability))?,
        last_checked_at: row
            .get::<_, Option<i64>>(17)?
            .map(|value| u64::try_from(value).unwrap_or(0)),
    })
}

pub(crate) fn publication_tags(
    connection: &rusqlite::Connection,
    publication_id: &str,
) -> rusqlite::Result<Vec<String>> {
    let mut statement =
        connection.prepare("SELECT tag FROM publication_tags WHERE publication_id = ?1 ORDER BY position ASC")?;
    let rows = statement.query_map([publication_id], |row| row.get::<_, String>(0))?;
    rows.collect()
}

fn chunk_map_for(record: &DownloadRecord) -> ChunkMap {
    record
        .chunk_map
        .as_deref()
        .and_then(|bytes| ChunkMap::from_stored(record.expected_size, bytes))
        .unwrap_or_else(|| ChunkMap::from_prefix(record.expected_size, record.verified_prefix))
}

fn download_from_row(row: &Row<'_>) -> rusqlite::Result<DownloadRecord> {
    let state: String = row.get(5)?;
    Ok(DownloadRecord {
        download_id: row.get(0)?,
        content_id: row.get(1)?,
        display_name: row.get(2)?,
        expected_size: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
        verified_prefix: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
        state: DownloadState::parse(&state).ok_or_else(|| invalid_enum(5, "download state", &state))?,
        partial_file: row.get(6)?,
        created_at: u64::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
        updated_at: u64::try_from(row.get::<_, i64>(8)?).unwrap_or(0),
        last_error: row.get(9)?,
        manifest_root: row.get(10)?,
        source_peer: row.get(11)?,
        verified_bytes: u64::try_from(row.get::<_, i64>(12)?).unwrap_or(0),
        chunk_map: row.get(13)?,
    })
}

fn replica_from_row(row: &Row<'_>) -> rusqlite::Result<ReplicaRecord> {
    let state: String = row.get(3)?;
    Ok(ReplicaRecord {
        replica_id: row.get(0)?,
        content_id: row.get(1)?,
        publication_id: row.get(2)?,
        state: ReplicaState::parse(&state).ok_or_else(|| invalid_enum(3, "replica state", &state))?,
        source_peer: row.get(4)?,
        verified_size_bytes: row
            .get::<_, Option<i64>>(5)?
            .map(|value| u64::try_from(value).unwrap_or(0)),
        last_verified_at: row
            .get::<_, Option<i64>>(6)?
            .map(|value| u64::try_from(value).unwrap_or(0)),
        last_checked_at: row
            .get::<_, Option<i64>>(7)?
            .map(|value| u64::try_from(value).unwrap_or(0)),
        last_seen_at: row
            .get::<_, Option<i64>>(8)?
            .map(|value| u64::try_from(value).unwrap_or(0)),
        failure_count: u64::try_from(row.get::<_, i64>(9)?).unwrap_or(0),
        last_error: row.get(10)?,
        announced: row.get::<_, i64>(11)? != 0,
        created_at: u64::try_from(row.get::<_, i64>(12)?).unwrap_or(0),
        updated_at: u64::try_from(row.get::<_, i64>(13)?).unwrap_or(0),
    })
}

fn validate_profile_record(record: &ProfileRecord) -> StoreResult<()> {
    validate_entity_id(&record.identity_id, "identity ID")?;
    if record.revision == 0 {
        return Err(StoreError::InvalidId("profile revision must be at least 1".to_owned()));
    }
    if record.record_bytes.is_empty() || record.record_bytes.len() > 65_536 {
        return Err(StoreError::InvalidRecord(
            "profile record size is out of bounds".to_owned(),
        ));
    }
    if let Some(content_id) = &record.avatar_content_id {
        validate_content_id(content_id)?;
    }
    if let Some(username) = &record.username
        && (username.is_empty() || username.len() > 128)
    {
        return Err(StoreError::InvalidId("username label is out of bounds".to_owned()));
    }
    Ok(())
}

fn write_profile(connection: &rusqlite::Connection, record: &ProfileRecord) -> StoreResult<()> {
    connection.execute(
        "INSERT INTO profiles
         (identity_id, revision, record_bytes, display_name, username, bio,
          avatar_content_id, updated_at, updated_at_ms, state, published_at,
          last_checked_at, freshness)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(identity_id) DO UPDATE SET
             revision = excluded.revision,
             record_bytes = excluded.record_bytes,
             display_name = excluded.display_name,
             username = excluded.username,
             bio = excluded.bio,
             avatar_content_id = excluded.avatar_content_id,
             updated_at = excluded.updated_at,
             updated_at_ms = excluded.updated_at_ms,
             state = excluded.state,
             published_at = excluded.published_at,
             last_checked_at = excluded.last_checked_at,
             freshness = excluded.freshness",
        params![
            record.identity_id,
            i64::try_from(record.revision).unwrap_or(i64::MAX),
            record.record_bytes,
            record.display_name,
            record.username,
            record.bio,
            record.avatar_content_id,
            record.updated_at,
            record.updated_at_ms,
            record.state.as_str(),
            record
                .published_at
                .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
            record
                .last_checked_at
                .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
            record.freshness.as_str(),
        ],
    )?;
    crate::search_store::index_profile(connection, record)?;
    Ok(())
}

fn mark_cached_checked(connection: &rusqlite::Connection, identity_id: &str, at: u64) -> StoreResult<()> {
    connection.execute(
        "UPDATE profiles SET last_checked_at = ?2, freshness = 'current' WHERE identity_id = ?1",
        params![identity_id, i64::try_from(at).unwrap_or(i64::MAX)],
    )?;
    Ok(())
}

fn insert_profile_conflict(
    connection: &rusqlite::Connection,
    identity_id: &str,
    revision: u64,
    record_bytes: &[u8],
    observed_at: u64,
) -> StoreResult<()> {
    connection.execute(
        "INSERT OR IGNORE INTO profile_conflicts
         (identity_id, revision, record_bytes, observed_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            identity_id,
            i64::try_from(revision).unwrap_or(i64::MAX),
            record_bytes,
            i64::try_from(observed_at).unwrap_or(i64::MAX),
        ],
    )?;
    crate::search_store::note_search_change(connection)?;
    Ok(())
}

pub(crate) fn profile_from_row(row: &Row<'_>) -> rusqlite::Result<ProfileRecord> {
    let state: String = row.get(9)?;
    let freshness: String = row.get(12)?;
    Ok(ProfileRecord {
        identity_id: row.get(0)?,
        revision: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
        record_bytes: row.get(2)?,
        display_name: row.get(3)?,
        username: row.get(4)?,
        bio: row.get(5)?,
        avatar_content_id: row.get(6)?,
        updated_at: u64::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
        updated_at_ms: u64::try_from(row.get::<_, i64>(8)?).unwrap_or(0),
        state: ProfileState::parse(&state).ok_or_else(|| invalid_enum(9, "profile state", &state))?,
        published_at: row
            .get::<_, Option<i64>>(10)?
            .map(|value| u64::try_from(value).unwrap_or(0)),
        last_checked_at: row
            .get::<_, Option<i64>>(11)?
            .map(|value| u64::try_from(value).unwrap_or(0)),
        freshness: ProfileFreshness::parse(&freshness)
            .ok_or_else(|| invalid_enum(12, "profile freshness", &freshness))?,
    })
}
