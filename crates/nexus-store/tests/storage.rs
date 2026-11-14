//! Stage 05 acceptance tests for DATA-001 through DATA-008.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use nexus_store::{
    ContentState, IdentityReference, PeerObservation, ProfileRecord, PublicationRecord, QuotaPolicy, ReferenceType,
    ReplicaRecord, ReplicaState, Store, StoreConfig, StoreError, safe_display_name,
};
use sha2::{Digest, Sha256};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nexus-store-it-{}-{}-{sequence}-{label}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        ));
        fs::create_dir_all(&path).expect("create test root");
        Self { path }
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn config(quota_bytes: u64, max_file_size: u64) -> StoreConfig {
    StoreConfig {
        quota: QuotaPolicy::custom(quota_bytes, 0).expect("test quota"),
        max_file_size,
        busy_timeout: Duration::from_secs(5),
        cache_size_kib: 1024,
    }
}

fn open(root: &TempRoot, quota_bytes: u64, max_file_size: u64) -> Store {
    Store::open_with_config(&root.path, config(quota_bytes, max_file_size)).expect("open store")
}

fn write_bytes(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, bytes).expect("write fixture");
}

fn content_id(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut output = String::with_capacity(64);
    for byte in digest.as_slice() {
        output.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        output.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
    }
    output
}

fn blob_path(root: &TempRoot, content_id: &str) -> PathBuf {
    root.path
        .join("store")
        .join("blobs")
        .join(&content_id[0..2])
        .join(&content_id[2..4])
        .join(content_id)
}

fn identity() -> IdentityReference {
    IdentityReference {
        identity_id: "nx1stage05identity".to_owned(),
        public_key: vec![7_u8; 32],
        key_file: "secrets/identity.enc".to_owned(),
        label: Some("Stage 05".to_owned()),
        created_at: 1_000,
    }
}

#[test]
fn data_001_schema_and_restart_persistence() {
    let root = TempRoot::new("persistence");
    let source = root.path.join("external/source.bin");
    let bytes = b"persistent content".to_vec();
    write_bytes(&source, &bytes);

    let imported = {
        let store = open(&root, 8 * 1024 * 1024, 8 * 1024 * 1024);
        assert_eq!(store.schema_version().expect("schema version"), 3);
        assert!(store.database_path().is_file());

        let imported = store
            .import_publication_original(&source, "pub-001")
            .expect("import original");
        assert_eq!(imported.content_id, content_id(&bytes));

        store.upsert_identity_ref(&identity()).expect("identity");
        store
            .upsert_profile(&ProfileRecord {
                identity_id: identity().identity_id,
                revision: 1,
                record_bytes: b"profile-record".to_vec(),
                display_name: "Stage Five".to_owned(),
                username: Some("stage05".to_owned()),
                bio: Some("storage test".to_owned()),
                avatar_content_id: None,
                updated_at: 1_001,
            })
            .expect("profile");
        store
            .upsert_publication(&PublicationRecord {
                publication_id: "pub-001".to_owned(),
                identity_id: identity().identity_id,
                content_id: imported.content_id.clone(),
                revision: 1,
                record_bytes: b"publication-record".to_vec(),
                title: "Persistent file".to_owned(),
                description: Some("kept across restart".to_owned()),
                filename: "source.bin".to_owned(),
                published_at: 1_002,
                updated_at: 1_002,
                tombstoned: false,
            })
            .expect("publication");
        store
            .upsert_replica(&ReplicaRecord {
                replica_id: "replica-001".to_owned(),
                content_id: imported.content_id.clone(),
                publication_id: Some("pub-001".to_owned()),
                state: ReplicaState::Active,
                last_verified_at: Some(1_002),
                created_at: 1_002,
                updated_at: 1_002,
            })
            .expect("replica");
        store
            .record_peer_observation(&PeerObservation {
                peer_id: "peer-001".to_owned(),
                address: Some("/ip4/127.0.0.1/tcp/9000".to_owned()),
                last_seen_at: 1_003,
                last_error: None,
                observation_count: 1,
            })
            .expect("peer");
        store.set_setting("theme", "dark").expect("setting");
        imported
    };

    let store = open(&root, 8 * 1024 * 1024, 8 * 1024 * 1024);
    assert_eq!(store.schema_version().expect("schema version"), 3);
    assert_eq!(store.identity_refs().expect("identities").len(), 1);
    assert_eq!(store.profiles().expect("profiles").len(), 1);
    assert_eq!(store.publications().expect("publications").len(), 1);
    assert_eq!(store.replicas().expect("replicas").len(), 1);
    assert_eq!(store.peers().expect("peers").len(), 1);
    assert_eq!(store.get_setting("theme").expect("setting"), Some("dark".to_owned()));
    assert_eq!(
        store.content(&imported.content_id).expect("content").size_bytes,
        u64::try_from(bytes.len()).expect("byte count")
    );
    store
        .verify_content(&imported.content_id)
        .expect("content survives restart");
    assert_eq!(
        fs::read(blob_path(&root, &imported.content_id)).expect("blob bytes"),
        b"persistent content"
    );
}

#[test]
fn data_003_file_bytes_stay_outside_sqlite_and_import_copies() {
    let root = TempRoot::new("managed-copy");
    let bytes = vec![0x5a_u8; 2 * 1024 * 1024];
    let source = root.path.join("external/large.bin");
    write_bytes(&source, &bytes);
    let original = fs::read(&source).expect("read source");

    let store = open(&root, 8 * 1024 * 1024, 8 * 1024 * 1024);
    let imported = store
        .import_publication_original(&source, "pub-large")
        .expect("import large file");
    assert_eq!(imported.content_id, content_id(&bytes));
    assert_eq!(fs::read(&source).expect("source still readable"), original);
    assert!(source.is_file());

    let blob = blob_path(&root, &imported.content_id);
    assert!(blob.is_file());
    assert_eq!(fs::metadata(&blob).expect("blob metadata").len(), 2 * 1024 * 1024);
    assert!(
        store.database_size_bytes().expect("database size") < 2 * 1024 * 1024,
        "database must remain bounded and must not hold the file bytes"
    );

    let export_directory = root.path.join("exports");
    fs::create_dir_all(&export_directory).expect("export directory");
    let exported = store
        .export_content(&imported.content_id, &export_directory, "large.bin")
        .expect("export content");
    assert_eq!(fs::read(&exported.path).expect("exported bytes"), original);

    drop(store);
    let store = open(&root, 8 * 1024 * 1024, 8 * 1024 * 1024);
    store
        .verify_content(&imported.content_id)
        .expect("blob survives restart");
}

#[test]
fn data_004_safe_export_names_and_no_silent_overwrite() {
    let root = TempRoot::new("path-safety");
    let source = root.path.join("external/report.bin");
    write_bytes(&source, b"report bytes");
    let store = open(&root, 1024 * 1024, 1024 * 1024);
    let imported = store.import_publication_original(&source, "pub-path").expect("import");
    let export_directory = root.path.join("exports");
    fs::create_dir_all(&export_directory).expect("export directory");

    let first = store
        .export_content(&imported.content_id, &export_directory, "../../report.bin")
        .expect("first export");
    assert_eq!(
        first.path.file_name().and_then(|name| name.to_str()),
        Some(".._.._report.bin")
    );
    let canonical_export_directory = export_directory.canonicalize().expect("canonical export directory");
    assert!(first.path.starts_with(&canonical_export_directory));

    let second = store
        .export_content(&imported.content_id, &export_directory, "../../report.bin")
        .expect("second export");
    assert_ne!(first.path, second.path);
    assert_eq!(fs::read(&first.path).expect("first bytes"), b"report bytes");
    assert_eq!(fs::read(&second.path).expect("second bytes"), b"report bytes");
    assert_eq!(
        second.path.file_name().and_then(|name| name.to_str()),
        Some(".._.._report (1).bin")
    );

    let error = store
        .export_content(&imported.content_id, store.store_path(), "inside.bin")
        .expect_err("exports must stay outside the managed store");
    assert!(matches!(error, StoreError::UnsafePath(_)));

    assert_eq!(safe_display_name("CON"), "_CON");
    assert_eq!(safe_display_name(""), "untitled");
}

#[test]
fn data_005_quota_blocks_new_work_without_damaging_existing_data() {
    let root = TempRoot::new("quota");
    let first_source = root.path.join("external/first.bin");
    let second_source = root.path.join("external/second.bin");
    write_bytes(&first_source, b"12345678");
    write_bytes(&second_source, b"abcdefgh");
    let store = open(&root, 8, 64);

    let imported = store
        .import_publication_original(&first_source, "pub-first")
        .expect("first import fits exactly");
    assert_eq!(store.usage().expect("usage").total_bytes(), 8);

    let error = store
        .import_publication_original(&second_source, "pub-second")
        .expect_err("second import must not fit");
    assert!(matches!(error, StoreError::QuotaExceeded { .. }));
    assert_eq!(store.contents().expect("contents").len(), 1);
    assert_eq!(store.usage().expect("usage").total_bytes(), 8);
    store
        .verify_content(&imported.content_id)
        .expect("existing data intact");

    drop(store);
    fs::remove_file(&first_source).expect("remove source");
}

#[test]
fn data_005_partial_downloads_count_from_the_first_allocated_byte() {
    let root = TempRoot::new("partial-quota");
    let bytes = b"partial!";
    let content = content_id(bytes);
    let store = open(&root, 8, 64);

    let download = store
        .begin_download("download-001", &content, 8, "partial.bin")
        .expect("begin download");
    assert_eq!(download.verified_prefix, 0);
    assert_eq!(store.usage().expect("usage").partial_bytes, 0);

    let partial = store.partial_path("download-001").expect("partial path");
    fs::write(&partial, &bytes[..4]).expect("write first prefix");
    let download = store
        .checkpoint_partial("download-001", 4)
        .expect("checkpoint first prefix");
    assert_eq!(download.verified_prefix, 4);
    assert_eq!(store.usage().expect("usage").partial_bytes, 4);

    fs::write(&partial, bytes).expect("write full partial");
    let download = store
        .checkpoint_partial("download-001", 8)
        .expect("checkpoint full prefix");
    assert_eq!(download.verified_prefix, 8);
    assert_eq!(store.usage().expect("usage").partial_bytes, 8);

    let error = store
        .begin_download("download-002", &content, 1, "second.bin")
        .expect_err("quota is full");
    assert!(matches!(error, StoreError::QuotaExceeded { .. }));

    let completed = store.complete_download("download-001").expect("complete download");
    assert_eq!(completed.content_id, content);
    assert_eq!(store.usage().expect("usage").total_bytes(), 8);
    assert_eq!(store.reference_count(&content).expect("references"), 1);
}

#[test]
fn duplicate_bytes_are_counted_and_charged_once() {
    let root = TempRoot::new("dedupe-quota");
    let source = root.path.join("external/duplicate.bin");
    write_bytes(&source, b"12345678");
    let store = open(&root, 8, 64);

    let first = store
        .import_publication_original(&source, "pub-duplicate-a")
        .expect("first import");
    let second = store
        .import_publication_original(&source, "pub-duplicate-b")
        .expect("identical bytes re-import without a second quota charge");
    assert_eq!(first.content_id, second.content_id);
    assert!(second.already_present);
    assert_eq!(store.usage().expect("usage").total_bytes(), 8);
    assert_eq!(store.reference_count(&first.content_id).expect("references"), 2);
}

#[test]
fn data_006_reference_counting_and_confirmation_rules() {
    let root = TempRoot::new("references");
    let source = root.path.join("external/shared.bin");
    write_bytes(&source, b"shared bytes");
    let store = open(&root, 1024 * 1024, 1024 * 1024);
    let imported = store
        .import_publication_original(&source, "pub-shared")
        .expect("import shared content");
    let content = imported.content_id;
    store
        .add_reference(&content, ReferenceType::RetainedDownload, "download-shared")
        .expect("add retained reference");
    assert_eq!(store.reference_count(&content).expect("references"), 2);

    let outcome = store
        .release_reference(&content, ReferenceType::RetainedDownload, "download-shared", false)
        .expect("release retained reference");
    assert_eq!(outcome, nexus_store::ReleaseOutcome::Retained { remaining: 1 });
    assert!(blob_path(&root, &content).is_file());
    assert_eq!(store.reference_count(&content).expect("references"), 1);

    let error = store
        .release_reference(&content, ReferenceType::PublicationOriginal, "pub-shared", false)
        .expect_err("confirmation required");
    assert!(matches!(error, StoreError::ConfirmationRequired(_)));
    assert!(blob_path(&root, &content).is_file());
    assert_eq!(store.reference_count(&content).expect("references"), 1);

    let outcome = store
        .release_reference(&content, ReferenceType::PublicationOriginal, "pub-shared", true)
        .expect("release with confirmation");
    assert_eq!(outcome, nexus_store::ReleaseOutcome::BytesDeleted);
    assert!(!blob_path(&root, &content).exists());
    assert!(!store.content_exists(&content).expect("content existence"));

    let partial_content = content_id(b"partial cleanup");
    store
        .begin_download("download-cleanup", &partial_content, 8, "cleanup.bin")
        .expect("begin cleanup download");
    let partial = store.partial_path("download-cleanup").expect("partial path");
    fs::write(&partial, b"partial!").expect("write partial");
    store
        .checkpoint_partial("download-cleanup", 8)
        .expect("checkpoint partial");
    assert_eq!(store.usage().expect("usage").partial_bytes, 8);

    assert_eq!(
        store.cleanup_partials(3_600, true).expect("recent partial is retained"),
        0
    );
    let error = store
        .cleanup_partials(0, false)
        .expect_err("partial cleanup requires confirmation");
    assert!(matches!(error, StoreError::ConfirmationRequired(_)));
    assert!(store.download("download-cleanup").is_ok());
    assert_eq!(store.cleanup_partials(0, true).expect("remove old partial"), 1);
    assert!(store.download("download-cleanup").is_err());
    assert!(!partial.exists());
    assert_eq!(store.usage().expect("usage").partial_bytes, 0);
}

#[test]
fn data_007_metadata_export_and_restore_round_trip() {
    let source_root = TempRoot::new("backup-source");
    let source = source_root.path.join("external/file.bin");
    write_bytes(&source, b"backup file bytes");
    let archive_directory = source_root.path.join("export");
    fs::create_dir_all(&archive_directory).expect("archive directory");
    let archive = archive_directory.join("metadata.nexus");

    {
        let store = open(&source_root, 1024 * 1024, 1024 * 1024);
        let imported = store
            .import_publication_original(&source, "pub-backup")
            .expect("import");
        store.upsert_identity_ref(&identity()).expect("identity");
        store
            .upsert_publication(&PublicationRecord {
                publication_id: "pub-backup".to_owned(),
                identity_id: identity().identity_id,
                content_id: imported.content_id,
                revision: 1,
                record_bytes: b"backup-publication".to_vec(),
                title: "Backed up file".to_owned(),
                description: None,
                filename: "file.bin".to_owned(),
                published_at: 2_000,
                updated_at: 2_000,
                tombstoned: false,
            })
            .expect("publication");
        store.set_setting("theme", "light").expect("setting");
        let report = store.export_metadata(&archive).expect("export metadata");
        assert_eq!(report.schema_version, 3);
        assert!(archive.is_file());

        let archive_bytes = fs::read(&archive).expect("archive bytes");
        assert_eq!(&archive_bytes[..15], b"NEXUS-METADATA\0");
        let manifest_length =
            u32::from_le_bytes(archive_bytes[17..21].try_into().expect("manifest length field")) as usize;
        let payload_offset = 21 + manifest_length;
        let payload = &archive_bytes[payload_offset..];
        assert_eq!(
            u64::try_from(payload.len()).expect("payload length"),
            report.database_size
        );
        assert_eq!(content_id(payload), report.database_sha256);
    }

    let target_root = TempRoot::new("backup-target");
    let mut target = open(&target_root, 1024 * 1024, 1024 * 1024);
    let restore = target.restore_metadata(&archive).expect("restore metadata");
    assert_eq!(restore.archive_schema_version, 3);
    assert_eq!(restore.restored_schema_version, 3);
    assert_eq!(target.identity_refs().expect("identities").len(), 1);
    assert_eq!(target.get_setting("theme").expect("setting"), Some("light".to_owned()));
    assert_eq!(target.publications().expect("publications").len(), 1);

    let sentinel_root = TempRoot::new("backup-sentinel");
    let mut sentinel = open(&sentinel_root, 1024 * 1024, 1024 * 1024);
    sentinel.set_setting("sentinel", "keep me").expect("set sentinel");
    let corrupt = sentinel_root.path.join("corrupt.nexus");
    let mut bytes = fs::read(&archive).expect("archive bytes");
    let payload_index = bytes.len() - 1;
    bytes[payload_index] ^= 0xff;
    fs::write(&corrupt, bytes).expect("write corrupt archive");

    let error = sentinel
        .restore_metadata(&corrupt)
        .expect_err("corrupt archive must be rejected");
    assert!(matches!(
        error,
        StoreError::CorruptArchive(_) | StoreError::IncompatibleArchive(_) | StoreError::Database(_)
    ));
    assert_eq!(
        sentinel.get_setting("sentinel").expect("setting"),
        Some("keep me".to_owned())
    );

    let incompatible = sentinel_root.path.join("incompatible.nexus");
    let mut bytes = fs::read(&archive).expect("archive bytes");
    bytes[15..17].copy_from_slice(&2_u16.to_le_bytes());
    fs::write(&incompatible, bytes).expect("write incompatible archive");
    let error = sentinel
        .restore_metadata(&incompatible)
        .expect_err("incompatible archive must be rejected");
    assert!(matches!(error, StoreError::IncompatibleArchive(_)));
    assert_eq!(
        sentinel.get_setting("sentinel").expect("setting"),
        Some("keep me".to_owned())
    );
}

#[test]
fn reconciliation_removes_orphans_quarantines_unexpected_files_and_truncates_partials() {
    let root = TempRoot::new("reconcile");
    let bytes = b"partial bytes";
    let content_id = content_id(bytes);
    {
        let store = open(&root, 1024 * 1024, 1024 * 1024);
        store
            .begin_download("download-001", &content_id, 13, "partial.bin")
            .expect("begin download");
        let partial = store.partial_path("download-001").expect("partial path");
        fs::write(&partial, &bytes[..4]).expect("write verified prefix");
        store.checkpoint_partial("download-001", 4).expect("checkpoint");
        drop(store);
    }

    write_bytes(&root.path.join("store/partials/download-001.part"), bytes);
    write_bytes(&root.path.join("store/partials/orphan.part"), b"orphan");
    let orphan_blob = root.path.join("store/blobs/ab/ab").join("ab".repeat(32));
    write_bytes(&orphan_blob, b"orphan blob");
    write_bytes(&root.path.join("store/blobs/unexpected.txt"), b"unexpected");

    let store = open(&root, 1024 * 1024, 1024 * 1024);
    let report = store.last_reconciliation();
    assert!(report.truncated_partials >= 1);
    assert!(report.orphan_partials >= 1);
    assert!(report.quarantined_files >= 1);
    assert!(report.deleted_orphan_blobs >= 1);
    assert_eq!(
        fs::metadata(root.path.join("store/partials/download-001.part"))
            .expect("partial metadata")
            .len(),
        4
    );
    assert!(!root.path.join("store/partials/orphan.part").exists());
    assert!(!orphan_blob.exists());
    assert!(
        fs::read_dir(root.path.join("store/quarantine"))
            .expect("quarantine directory")
            .filter_map(Result::ok)
            .any(|entry| fs::read(entry.path()).ok().as_deref() == Some(b"orphan"))
    );
    assert_eq!(store.download("download-001").expect("download").verified_prefix, 4);
}

#[test]
fn reconciliation_adopts_a_download_promoted_before_the_database_commit() {
    let root = TempRoot::new("adopt");
    let bytes = b"crash window";
    let content_id = content_id(bytes);
    {
        let store = open(&root, 1024 * 1024, 1024 * 1024);
        store
            .begin_download("download-crash", &content_id, 12, "crash.bin")
            .expect("begin download");
        let partial = store.partial_path("download-crash").expect("partial path");
        fs::write(&partial, bytes).expect("write complete partial");
        store
            .checkpoint_partial("download-crash", 12)
            .expect("checkpoint complete");
        drop(store);
    }

    let partial = root.path.join("store/partials/download-crash.part");
    let blob = blob_path(&root, &content_id);
    fs::create_dir_all(blob.parent().expect("blob parent")).expect("blob parent");
    fs::rename(partial, &blob).expect("simulate promote before commit");

    let store = open(&root, 1024 * 1024, 1024 * 1024);
    assert!(store.last_reconciliation().adopted_downloads >= 1);
    assert!(store.downloads().expect("downloads").is_empty());
    assert_eq!(
        store.content(&content_id).expect("adopted content").state,
        ContentState::Verified
    );
    assert_eq!(store.reference_count(&content_id).expect("references"), 1);
    store.verify_content(&content_id).expect("adopted blob verifies");
}

#[test]
fn missing_blob_is_preserved_as_missing_metadata() {
    let root = TempRoot::new("missing");
    let source = root.path.join("external/missing.bin");
    write_bytes(&source, b"will disappear");
    let content;
    {
        let store = open(&root, 1024 * 1024, 1024 * 1024);
        content = store
            .import_publication_original(&source, "pub-missing")
            .expect("import")
            .content_id;
        drop(store);
    }
    fs::remove_file(blob_path(&root, &content)).expect("remove blob");

    let store = open(&root, 1024 * 1024, 1024 * 1024);
    assert_eq!(
        store.content(&content).expect("content row").state,
        ContentState::Missing
    );
    assert!(store.last_reconciliation().missing_contents >= 1);
    assert_eq!(store.reference_count(&content).expect("reference"), 1);
}

#[test]
fn reconciliation_quarantines_corrupt_blob_and_keeps_that_state() {
    let root = TempRoot::new("quarantine-state");
    let source = root.path.join("external/corrupt.bin");
    let bytes = vec![0x11_u8; 32];
    write_bytes(&source, &bytes);
    let content;
    {
        let store = open(&root, 1024 * 1024, 1024 * 1024);
        content = store
            .import_publication_original(&source, "pub-corrupt")
            .expect("import")
            .content_id;
        drop(store);
    }

    let blob = blob_path(&root, &content);
    write_bytes(&blob, &[0x22_u8; 32]);

    let store = open(&root, 1024 * 1024, 1024 * 1024);
    assert_eq!(
        store.content(&content).expect("content row").state,
        ContentState::Quarantined,
        "a quarantined blob must not be downgraded to missing"
    );
    assert!(store.last_reconciliation().quarantined_files >= 1);
    assert!(!blob.exists());
    assert_eq!(store.reference_count(&content).expect("reference"), 1);
}
