//! Stage 13 profile storage: local lifecycle, cache outcomes, conflicts, and
//! username-label collision queries.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use nexus_store::{
    ProfileCacheOutcome, ProfileFreshness, ProfileRecord, ProfileState, QuotaPolicy, Store, StoreConfig, StoreError,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nexus-store-profiles-{}-{}-{sequence}-{label}",
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

fn open(root: &TempRoot) -> Store {
    Store::open_with_config(
        &root.path,
        StoreConfig {
            quota: QuotaPolicy::custom(16 * 1024 * 1024, 0).expect("test quota"),
            max_file_size: 8 * 1024 * 1024,
            busy_timeout: Duration::from_secs(5),
            cache_size_kib: 1024,
        },
    )
    .expect("open store")
}

fn record(
    identity_id: &str,
    revision: u64,
    username: Option<&str>,
    state: ProfileState,
    bytes: &[u8],
) -> ProfileRecord {
    ProfileRecord {
        identity_id: identity_id.to_owned(),
        revision,
        record_bytes: bytes.to_vec(),
        display_name: format!("Profile {identity_id}"),
        username: username.map(str::to_owned),
        bio: Some("test profile".to_owned()),
        avatar_content_id: None,
        updated_at: revision,
        updated_at_ms: revision * 1_000,
        state,
        published_at: None,
        last_checked_at: None,
        freshness: ProfileFreshness::Current,
    }
}

#[test]
fn local_profile_moves_from_draft_to_pending_to_published() {
    let root = TempRoot::new("local-lifecycle");
    let store = open(&root);
    assert_eq!(store.schema_version().expect("schema"), 9);

    let draft = record("nx1alice", 1, Some("ada"), ProfileState::Draft, b"draft-1");
    store.store_local_profile(&draft, None).expect("store draft");
    let stored = store.local_profile("nx1alice").expect("local").expect("row");
    assert_eq!(stored.state, ProfileState::Draft);
    assert_eq!(stored.freshness, ProfileFreshness::Current);
    assert_eq!(stored.updated_at_ms, 1_000);

    store.mark_profile_published("nx1alice", false, 5_000).expect("pending");
    assert_eq!(
        store.profile("nx1alice").expect("profile").expect("row").state,
        ProfileState::Pending
    );

    store
        .mark_profile_published("nx1alice", true, 6_000)
        .expect("published");
    let published = store.profile("nx1alice").expect("profile").expect("row");
    assert_eq!(published.state, ProfileState::Published);
    assert_eq!(published.published_at, Some(6_000));
}

#[test]
fn local_writes_honor_expected_revision_and_never_roll_back() {
    let root = TempRoot::new("expected-revision");
    let store = open(&root);

    store
        .store_local_profile(&record("nx1alice", 1, None, ProfileState::Draft, b"rev-1"), None)
        .expect("first");
    store
        .store_local_profile(&record("nx1alice", 2, None, ProfileState::Draft, b"rev-2"), Some(1))
        .expect("second");

    assert!(matches!(
        store.store_local_profile(&record("nx1alice", 3, None, ProfileState::Draft, b"rev-3"), Some(1)),
        Err(StoreError::RecordConflict(_))
    ));
    assert!(matches!(
        store.store_local_profile(&record("nx1alice", 1, None, ProfileState::Draft, b"rev-1"), None),
        Err(StoreError::RecordConflict(_))
    ));
    assert_eq!(store.profile("nx1alice").expect("profile").expect("row").revision, 2);
}

#[test]
fn remote_cache_replaces_newer_and_retains_conflicts() {
    let root = TempRoot::new("remote-cache");
    let store = open(&root);

    let first = record("nx1bob", 1, Some("bob"), ProfileState::Remote, b"rev-1");
    assert_eq!(
        store.cache_remote_profile(&first).expect("first"),
        ProfileCacheOutcome::Stored
    );
    assert_eq!(
        store.cache_remote_profile(&first).expect("same"),
        ProfileCacheOutcome::Unchanged
    );

    let newer = record("nx1bob", 2, Some("bob"), ProfileState::Remote, b"rev-2");
    assert_eq!(
        store.cache_remote_profile(&newer).expect("newer"),
        ProfileCacheOutcome::Stored
    );
    assert_eq!(
        store.cache_remote_profile(&first).expect("older"),
        ProfileCacheOutcome::Stale
    );
    let cached = store.profile("nx1bob").expect("profile").expect("row");
    assert_eq!(cached.revision, 2);
    assert_eq!(cached.record_bytes, b"rev-2");
    assert_eq!(cached.freshness, ProfileFreshness::Current);

    let conflict = record("nx1bob", 2, Some("bob"), ProfileState::Remote, b"other-2");
    assert_eq!(
        store.cache_remote_profile(&conflict).expect("conflict"),
        ProfileCacheOutcome::Conflict
    );
    assert_eq!(
        store.profile("nx1bob").expect("profile").expect("row").record_bytes,
        b"rev-2"
    );
    let conflicts = store.profile_conflicts("nx1bob").expect("conflicts");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].revision, 2);
    assert_eq!(conflicts[0].record_bytes, b"other-2");
}

#[test]
fn remote_cache_never_overwrites_a_local_profile() {
    let root = TempRoot::new("local-wins");
    let store = open(&root);
    store
        .store_local_profile(
            &record("nx1alice", 2, Some("ada"), ProfileState::Published, b"local-2"),
            None,
        )
        .expect("local");

    let same = record("nx1alice", 2, Some("ada"), ProfileState::Remote, b"local-2");
    assert_eq!(
        store.cache_remote_profile(&same).expect("same"),
        ProfileCacheOutcome::Unchanged
    );

    let different = record("nx1alice", 2, Some("ada"), ProfileState::Remote, b"remote-2");
    assert_eq!(
        store.cache_remote_profile(&different).expect("different"),
        ProfileCacheOutcome::Conflict
    );
    let local = store.local_profile("nx1alice").expect("local").expect("row");
    assert_eq!(local.state, ProfileState::Published);
    assert_eq!(local.record_bytes, b"local-2");
    assert_eq!(store.profile_conflicts("nx1alice").expect("conflicts").len(), 1);
}

#[test]
fn username_collisions_are_case_insensitive_and_keep_distinct_identities() {
    let root = TempRoot::new("username-collisions");
    let store = open(&root);
    store
        .store_local_profile(
            &record("nx1alice", 1, Some("Ada"), ProfileState::Published, b"alice"),
            None,
        )
        .expect("alice");
    store
        .cache_remote_profile(&record("nx1bob", 1, Some("ada"), ProfileState::Remote, b"bob"))
        .expect("bob");
    store
        .cache_remote_profile(&record("nx1carol", 1, Some("Äda"), ProfileState::Remote, b"carol"))
        .expect("carol");

    let matches = store.profiles_with_username("ADA").expect("matches");
    assert_eq!(matches.len(), 2);
    let mut ids: Vec<_> = matches.iter().map(|profile| profile.identity_id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["nx1alice", "nx1bob"]);

    let unicode = store.profiles_with_username("äda").expect("unicode matches");
    assert_eq!(unicode.len(), 1);
    assert_eq!(unicode[0].identity_id, "nx1carol");
}
