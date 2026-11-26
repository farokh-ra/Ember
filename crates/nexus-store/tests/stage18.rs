//! Stage 18 storage: local block list and abuse reports.
//!
//! Blocking and reporting are local policy. These tests cover persistence,
//! idempotent updates, atomic caps, bounded notes, and input validation.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use nexus_store::{AbuseCategory, BlockReason, MAX_SECURITY_NOTE_BYTES, Store, StoreConfig, StoreError};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-store-stage18-{label}-{}-{sequence}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test root");
        Self { path }
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn open(root: &TempRoot, max_blocked: usize, max_reports: usize) -> Store {
    Store::open_with_config(
        &root.path,
        StoreConfig {
            max_blocked_peers: max_blocked,
            max_abuse_reports: max_reports,
            ..StoreConfig::default()
        },
    )
    .expect("open store")
}

#[test]
fn blocks_are_idempotent_persistent_and_bounded() {
    let root = TempRoot::new("blocks");
    let long_note = "x".repeat(MAX_SECURITY_NOTE_BYTES + 100);
    {
        let store = open(&root, 16, 16);
        assert!(!store.is_peer_blocked("12D3KooWpeer").expect("check"));

        let first = store
            .block_peer("12D3KooWpeer", BlockReason::Spam, Some(&long_note))
            .expect("block");
        assert_eq!(first.reason, BlockReason::Spam);
        assert_eq!(
            first.note.as_deref().map(str::len),
            Some(MAX_SECURITY_NOTE_BYTES),
            "notes are truncated at the storage bound"
        );
        assert_eq!(store.blocked_peer_count().expect("count"), 1);

        let updated = store
            .block_peer("12D3KooWpeer", BlockReason::Harassment, None)
            .expect("re-block");
        assert_eq!(updated.reason, BlockReason::Harassment);
        assert_eq!(updated.note, None, "an empty note clears the previous note");
        assert_eq!(updated.created_at, first.created_at);
        assert!(updated.updated_at >= updated.created_at);
        assert_eq!(store.blocked_peer_count().expect("count"), 1);
        assert!(store.is_peer_blocked("12D3KooWpeer").expect("blocked"));
    }

    let store = open(&root, 16, 16);
    assert!(store.is_peer_blocked("12D3KooWpeer").expect("persisted block"));
    assert_eq!(store.blocked_peers().expect("list").len(), 1);
    assert!(store.unblock_peer("12D3KooWpeer").expect("unblock"));
    assert!(!store.unblock_peer("12D3KooWpeer").expect("second unblock"));
    assert_eq!(store.blocked_peer_count().expect("count"), 0);
}

#[test]
fn block_cap_is_enforced_atomically() {
    let root = TempRoot::new("block-cap");
    let store = open(&root, 2, 2);
    store.block_peer("peer-one", BlockReason::Manual, None).expect("first");
    store
        .block_peer("peer-two", BlockReason::Other, Some("note"))
        .expect("second");
    let error = store
        .block_peer("peer-three", BlockReason::Manual, None)
        .expect_err("cap must reject a third block");
    assert!(matches!(error, StoreError::InvalidConfig(_)));
    assert_eq!(store.blocked_peer_count().expect("count"), 2);

    assert!(store.unblock_peer("peer-one").expect("unblock"));
    store
        .block_peer("peer-three", BlockReason::Scam, None)
        .expect("room after unblock");
    assert_eq!(store.blocked_peer_count().expect("count"), 2);
}

#[test]
fn abuse_reports_are_local_capped_and_newest_first() {
    let root = TempRoot::new("reports");
    let store = open(&root, 8, 2);
    let first = store
        .record_abuse_report("peer-one", AbuseCategory::Spam, Some("repeat messages"))
        .expect("report");
    let second = store
        .record_abuse_report("peer-two", AbuseCategory::Malware, None)
        .expect("report");
    assert!(second.report_id > first.report_id);

    let error = store
        .record_abuse_report("peer-three", AbuseCategory::Other, None)
        .expect_err("cap must reject a third report");
    assert!(matches!(error, StoreError::InvalidConfig(_)));

    let reports = store.abuse_reports().expect("list");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].report_id, second.report_id);
    assert_eq!(reports[0].category, AbuseCategory::Malware);
    assert_eq!(reports[1].note.as_deref(), Some("repeat messages"));
    assert_eq!(store.abuse_report_count().expect("count"), 2);
}

#[test]
fn multibyte_notes_truncate_on_a_character_boundary() {
    let root = TempRoot::new("utf8");
    let store = open(&root, 4, 4);
    let note = "é".repeat(MAX_SECURITY_NOTE_BYTES);
    let blocked = store
        .block_peer("peer-utf8", BlockReason::Manual, Some(&note))
        .expect("block");
    let stored = blocked.note.expect("note");
    assert!(stored.len() <= MAX_SECURITY_NOTE_BYTES);
    assert!(stored.is_char_boundary(stored.len()));
    assert_eq!(stored, "é".repeat(MAX_SECURITY_NOTE_BYTES / 2));
}

#[test]
fn malformed_identifiers_and_config_are_rejected() {
    let root = TempRoot::new("invalid");
    let store = open(&root, 4, 4);
    assert!(matches!(
        store.block_peer("peer with spaces", BlockReason::Manual, None),
        Err(StoreError::InvalidId(_))
    ));
    assert!(matches!(
        store.record_abuse_report("", AbuseCategory::Spam, None),
        Err(StoreError::InvalidId(_))
    ));

    let zero = Store::open_with_config(
        root.path.join("nested"),
        StoreConfig {
            max_blocked_peers: 0,
            ..StoreConfig::default()
        },
    );
    assert!(matches!(zero, Err(StoreError::InvalidConfig(_))));
}
