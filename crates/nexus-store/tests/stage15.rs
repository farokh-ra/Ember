//! Stage 15 storage acceptance tests: replica records, hosting limits, and
//! quota persistence.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use nexus_store::{
    ContentState, HostingLimits, MIN_UPLOAD_BYTES_PER_SECOND, QuotaPolicy, ReplicaRecord, ReplicaState, Store,
    StoreConfig,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-store-stage15-{}-{sequence}-{label}", std::process::id()));
        fs::create_dir_all(&path).expect("create test root");
        Self { path }
    }

    fn source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let directory = self.path.join("external");
        fs::create_dir_all(&directory).expect("create source directory");
        let path = directory.join(name);
        fs::write(&path, bytes).expect("write source");
        path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn open(root: &TempRoot, quota_bytes: u64) -> Store {
    Store::open_with_config(
        &root.path,
        StoreConfig {
            quota: QuotaPolicy::custom(quota_bytes, 0).expect("quota"),
            ..StoreConfig::default()
        },
    )
    .expect("open store")
}

fn replica(id: &str, content_id: &str, state: ReplicaState, at: u64) -> ReplicaRecord {
    ReplicaRecord {
        replica_id: id.to_owned(),
        content_id: content_id.to_owned(),
        publication_id: Some("pub-001".to_owned()),
        state,
        source_peer: Some("peer-001".to_owned()),
        verified_size_bytes: Some(21),
        last_verified_at: Some(at),
        last_checked_at: Some(at),
        last_seen_at: Some(at),
        failure_count: 0,
        last_error: None,
        announced: state == ReplicaState::Active,
        created_at: at,
        updated_at: at,
    }
}

#[test]
fn replica_records_round_trip_page_and_report_usage() {
    let root = TempRoot::new("replica-records");
    let bytes = b"stage 15 hosted bytes";
    let source = root.source("hosted.bin", bytes);
    let store = open(&root, 1024 * 1024 * 1024);
    let imported = store
        .import_publication_original(&source, "pub-001")
        .expect("import original");
    assert_eq!(
        store.content(&imported.content_id).expect("content").state,
        ContentState::Verified
    );

    let active = replica("replica-001", &imported.content_id, ReplicaState::Active, 2_000);
    store.upsert_replica(&active).expect("upsert active");
    store
        .upsert_replica(&ReplicaRecord {
            replica_id: "replica-002".to_owned(),
            content_id: imported.content_id.clone(),
            publication_id: None,
            state: ReplicaState::Paused,
            source_peer: None,
            verified_size_bytes: None,
            last_verified_at: None,
            last_checked_at: None,
            last_seen_at: None,
            failure_count: 0,
            last_error: None,
            announced: false,
            created_at: 2_100,
            updated_at: 2_100,
        })
        .expect("upsert paused");

    assert_eq!(store.replica("replica-001").expect("replica"), active);
    let page = store.replicas_page(None, 10, false).expect("page");
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].replica_id, "replica-001");
    assert_eq!(page[1].replica_id, "replica-002");
    let next = store.replicas_page(Some("replica-001"), 10, false).expect("next page");
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].replica_id, "replica-002");

    let usage = store.replica_usage().expect("usage");
    assert_eq!(usage.total_count, 2);
    assert_eq!(usage.active_count, 1);
    assert_eq!(usage.paused_count, 1);
    assert_eq!(usage.failed_count, 0);
    assert_eq!(usage.storage_bytes, u64::try_from(bytes.len()).expect("byte count"));

    store
        .record_replica_check("replica-001", 2_200, None, Some("hash mismatch"))
        .expect("record failure");
    let failed = store.replica("replica-001").expect("replica");
    assert_eq!(failed.failure_count, 1);
    assert_eq!(failed.last_error.as_deref(), Some("hash mismatch"));
    store
        .record_replica_check("replica-001", 2_300, Some(21), None)
        .expect("record success");
    let recovered = store.replica("replica-001").expect("replica");
    assert_eq!(recovered.failure_count, 0);
    assert_eq!(recovered.last_error, None);
    assert_eq!(recovered.last_verified_at, Some(2_300));
    assert_eq!(recovered.verified_size_bytes, Some(21));

    store
        .mark_replica_announced("replica-001", false, 2_400)
        .expect("unannounce");
    assert!(!store.replica("replica-001").expect("replica").announced);
    store
        .mark_replica_announced("replica-001", true, 2_500)
        .expect("announce");
    let announced = store.replica("replica-001").expect("replica");
    assert!(announced.announced);
    assert_eq!(announced.last_seen_at, Some(2_500));

    store.record_replica_seen(&imported.content_id, 2_600).expect("seen");
    let final_record = store.replica("replica-001").expect("replica");
    assert_eq!(final_record.last_seen_at, Some(2_600));
    assert_eq!(store.replica("replica-002").expect("replica").last_seen_at, None);

    store
        .set_replica_state("replica-002", ReplicaState::Removed, 2_700)
        .expect("remove");
    let page = store.replicas_page(None, 10, false).expect("page");
    assert_eq!(page.len(), 1);
    let all = store.replicas_page(None, 10, true).expect("page with removed");
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].state, ReplicaState::Removed);
    assert_eq!(store.replica_count().expect("count"), 1);

    drop(store);
    let store = open(&root, 1024 * 1024 * 1024);
    let persisted = store.replica("replica-001").expect("persisted replica");
    assert_eq!(persisted, final_record);
    assert_eq!(store.replica_count().expect("count"), 1);
}

#[test]
fn hosting_limits_and_quota_persist_across_reopen() {
    let root = TempRoot::new("hosting-limits");
    let store = open(&root, 2 * 1024 * 1024 * 1024);
    assert_eq!(store.hosting_limits().expect("defaults"), HostingLimits::default());
    assert_eq!(store.config().quota.quota_bytes(), 2 * 1024 * 1024 * 1024);

    let limits = HostingLimits {
        upload_bytes_per_second: MIN_UPLOAD_BYTES_PER_SECOND * 2,
        allow_metered: true,
        battery_min_percent: 25,
    };
    store.set_hosting_limits(&limits).expect("set limits");
    let quota = store.set_quota(3 * 1024 * 1024 * 1024).expect("set quota");
    assert_eq!(quota.quota_bytes(), 3 * 1024 * 1024 * 1024);
    assert_eq!(store.config().quota, quota);

    drop(store);
    let store = open(&root, 2 * 1024 * 1024 * 1024);
    assert_eq!(store.hosting_limits().expect("persisted limits"), limits);
    assert_eq!(
        store.config().quota.quota_bytes(),
        3 * 1024 * 1024 * 1024,
        "the persisted quota overrides the host config"
    );

    let invalid = HostingLimits {
        upload_bytes_per_second: 1,
        ..HostingLimits::default()
    };
    assert!(store.set_hosting_limits(&invalid).is_err());
    assert_eq!(store.hosting_limits().expect("limits unchanged"), limits);
}
