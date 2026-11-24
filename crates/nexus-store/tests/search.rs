//! Stage 16 storage acceptance tests: the verified derived search index,
//! lifecycle-driven index maintenance, generation-bound cursors, provider
//! pages, conflict visibility, and a bounded local-search corpus.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use nexus_crypto::{IdentityId, KeyPair, sha256, to_hex};
use nexus_protocol::{Category, PublicationV1, SignedPublication};
use nexus_search::{ExploreMode, ExploreQuery, RecordKind, SearchQuery};
use nexus_store::{
    ProfileCacheOutcome, ProfileFreshness, ProfileRecord, ProfileState, PublicationAvailability, PublicationRecord,
    Store, StoreError, safe_display_name,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-store-search-{}-{sequence}-{label}", std::process::id()));
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
    Store::open(&root.path).expect("open store")
}

fn keypair(seed: u8) -> KeyPair {
    KeyPair::from_seed([seed; 32])
}

fn identity(seed: u8) -> IdentityId {
    IdentityId::from_public_key(&keypair(seed).public_key())
}

fn profile_record(
    identity_id: &IdentityId,
    display_name: &str,
    username: &str,
    state: ProfileState,
    revision: u64,
    record_bytes: Vec<u8>,
) -> ProfileRecord {
    ProfileRecord {
        identity_id: identity_id.as_str().to_owned(),
        revision,
        record_bytes,
        display_name: display_name.to_owned(),
        username: Some(username.to_owned()),
        bio: Some("A Stage 16 verified index fixture".to_owned()),
        avatar_content_id: None,
        updated_at: 2_000,
        updated_at_ms: 2_000_000,
        state,
        published_at: (state == ProfileState::Published).then_some(2_000_000),
        last_checked_at: None,
        freshness: ProfileFreshness::Current,
    }
}

fn signed_publication(
    seed: u8,
    filename: &str,
    description: &str,
    category: Category,
    tags: &[&str],
    revision: u64,
    published_at_ms: u64,
) -> SignedPublication {
    let content_id = sha256(format!("{filename}-{revision}").as_bytes());
    let identity = identity(seed);
    let publication = PublicationV1::new(
        identity,
        content_id,
        128,
        filename,
        description,
        category,
        [seed; 32],
        revision,
        published_at_ms,
        "0.1.0-stage-16",
    )
    .with_tags(tags.iter().map(|tag| (*tag).to_owned()).collect());
    SignedPublication::sign(&publication, &keypair(seed)).expect("sign publication")
}

fn publication_record(signed: &SignedPublication, local: bool, tombstoned: bool) -> PublicationRecord {
    let body = signed.publication();
    PublicationRecord {
        publication_id: to_hex(&signed.publication_id()),
        identity_id: body.identity_id.as_str().to_owned(),
        content_id: to_hex(&body.content_id),
        revision: body.revision,
        record_bytes: signed.raw().to_vec(),
        title: body.display_filename.clone(),
        description: Some(body.description.clone()),
        filename: safe_display_name(&body.display_filename),
        published_at: body.published_at_ms / 1_000,
        updated_at: 2_000,
        tombstoned,
        category: body.category,
        size_bytes: body.size,
        tags: body.tags.clone(),
        local,
        serving: local,
        announced: local,
        availability: if local {
            PublicationAvailability::Local
        } else {
            PublicationAvailability::RecordOnly
        },
        last_checked_at: None,
    }
}

fn search(store: &Store, text: &str) -> nexus_store::LocalSearchPage {
    store.search_local(&SearchQuery::new(text)).expect("local search")
}

#[test]
fn lifecycle_changes_update_the_derived_index_and_conflicts_stay_visible() {
    let root = TempRoot::new("lifecycle");
    let store = open(&root);
    let author = identity(16);

    let draft = profile_record(
        &author,
        "Index Draft",
        "indexdraft",
        ProfileState::Draft,
        1,
        vec![0x11; 64],
    );
    store.store_local_profile(&draft, None).expect("store draft profile");
    assert!(search(&store, "indexdraft").items.is_empty());

    let published = profile_record(
        &author,
        "Index Published",
        "indexpublished",
        ProfileState::Published,
        2,
        vec![0x22; 64],
    );
    store.store_local_profile(&published, Some(1)).expect("publish profile");
    let profile_page = search(&store, "indexpublished");
    assert_eq!(profile_page.items.len(), 1);
    assert_eq!(profile_page.items[0].kind, RecordKind::Profile);

    // A same-revision remote profile with different bytes is retained as a
    // visible conflict, never silently overwriting the accepted row.
    let conflicting = profile_record(
        &author,
        "Index Conflicting",
        "indexconflict",
        ProfileState::Remote,
        2,
        vec![0x33; 64],
    );
    assert_eq!(
        store.cache_remote_profile(&conflicting).expect("cache conflict"),
        ProfileCacheOutcome::Conflict
    );
    let accepted = search(&store, "indexpublished");
    assert_eq!(accepted.items.len(), 1);
    assert_eq!(accepted.items[0].conflicts.len(), 1);

    let signed = signed_publication(
        17,
        "stage16-index-note.txt",
        "A verified publication in the derived index",
        Category::Documents,
        &["stage16", "index"],
        1,
        1_700_000_000_000,
    );
    let record = publication_record(&signed, true, false);
    store.upsert_publication(&record).expect("store publication");
    let publication_page = search(&store, "stage16-index-note");
    assert_eq!(publication_page.items.len(), 1);
    assert_eq!(publication_page.items[0].kind, RecordKind::Publication);
    assert_eq!(
        publication_page.items[0].availability,
        nexus_search::Availability::Local
    );

    // Unpublishing removes the row from the derived index. A rebuild from the
    // canonical store must not resurrect a tombstoned record.
    store
        .mark_publication_unpublished(&record.publication_id, 3_000)
        .expect("unpublish publication");
    assert!(search(&store, "stage16-index-note").items.is_empty());
    let rebuilt = store.rebuild_search_index().expect("rebuild index");
    assert_eq!(rebuilt.publications, 0);
    assert!(search(&store, "stage16-index-note").items.is_empty());

    let stats = store.search_index_stats().expect("index stats");
    assert_eq!(stats.profiles, 1);
    assert_eq!(stats.publications, 0);
    assert!(stats.generation > 0);
}

#[test]
fn pages_are_generation_bound_and_stale_cursors_conflict() {
    let root = TempRoot::new("cursor");
    let store = open(&root);
    for index in 0..5_u8 {
        let signed = signed_publication(
            20 + index,
            &format!("stage16-page-{index}.txt"),
            "A paged Stage 16 record",
            Category::Documents,
            &["stage16", "paged"],
            1,
            1_700_000_000_000 + u64::from(index),
        );
        store
            .upsert_publication(&publication_record(&signed, true, false))
            .expect("store publication");
    }

    let mut query = SearchQuery::new("paged");
    query.limit = 2;
    let first = store.search_local(&query).expect("first page");
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.total_matches, 5);
    assert!(!first.complete);
    let cursor = first.next_cursor.clone().expect("next cursor");

    query.cursor = Some(cursor.clone());
    let second = store.search_local(&query).expect("second page");
    assert_eq!(second.items.len(), 2);
    assert!(
        first
            .items
            .iter()
            .all(|item| second.items.iter().all(|other| other.record_id != item.record_id))
    );

    let signed = signed_publication(
        30,
        "stage16-page-late.txt",
        "A later Stage 16 record",
        Category::Documents,
        &["stage16", "paged"],
        1,
        1_700_000_000_100,
    );
    store
        .upsert_publication(&publication_record(&signed, true, false))
        .expect("store late publication");

    query.cursor = Some(cursor);
    match store.search_local(&query) {
        Err(StoreError::InvalidSearch(nexus_search::SearchError::StaleCursor)) => {}
        other => panic!("expected a stale-cursor conflict, got {other:?}"),
    }

    query.cursor = None;
    let recent = store
        .search_local(&SearchQuery {
            sort: nexus_search::SortOrder::Recency,
            ..SearchQuery::new("paged")
        })
        .expect("recent search");
    let mut previous = u64::MAX;
    for item in &recent.items {
        assert!(item.published_at_ms <= previous);
        previous = item.published_at_ms;
    }
}

#[test]
fn explore_and_provider_pages_use_only_verified_indexed_records() {
    let root = TempRoot::new("explore-provider");
    let store = open(&root);
    for (index, category) in [Category::Documents, Category::Images, Category::Documents]
        .into_iter()
        .enumerate()
    {
        let signed = signed_publication(
            40 + u8::try_from(index).expect("index fits"),
            &format!("stage16-explore-{index}.txt"),
            "An Explore Stage 16 record",
            category,
            &["stage16", "explore"],
            1,
            1_700_000_000_000 + u64::try_from(index).expect("index fits"),
        );
        store
            .upsert_publication(&publication_record(&signed, true, false))
            .expect("store publication");
    }

    let categories = store
        .explore_local(&ExploreQuery {
            mode: ExploreMode::Categories,
            ..ExploreQuery::default()
        })
        .expect("category Explore");
    assert_eq!(categories.items.len(), 0);
    let documents = categories
        .categories
        .iter()
        .find(|entry| entry.category == Category::Documents)
        .expect("documents category");
    assert_eq!(documents.count, 2);
    let images = categories
        .categories
        .iter()
        .find(|entry| entry.category == Category::Images)
        .expect("images category");
    assert_eq!(images.count, 1);

    let random = store
        .explore_local(&ExploreQuery {
            mode: ExploreMode::Random,
            limit: 10,
            seed: Some(7),
            ..ExploreQuery::default()
        })
        .expect("random Explore");
    let random_again = store
        .explore_local(&ExploreQuery {
            mode: ExploreMode::Random,
            limit: 10,
            seed: Some(7),
            ..ExploreQuery::default()
        })
        .expect("repeat random Explore");
    let ids: Vec<&str> = random.items.iter().map(|item| item.record_id.as_str()).collect();
    let repeat: Vec<&str> = random_again.items.iter().map(|item| item.record_id.as_str()).collect();
    assert_eq!(ids, repeat);
    assert_eq!(ids.len(), 3);

    let provider = store
        .provider_search(&SearchQuery::new("stage16-explore"))
        .expect("provider page");
    assert_eq!(provider.records.len(), 3);
    assert!(!provider.complete || provider.next_cursor.is_none());
    for (kind, bytes) in &provider.records {
        assert_eq!(*kind, RecordKind::Publication);
        let signed = SignedPublication::parse(bytes).expect("provider record verifies");
        let record = store
            .publication(&to_hex(&signed.publication_id()))
            .expect("provider publication");
        assert_eq!(record.record_bytes, *bytes);
    }
}

#[test]
fn local_search_stays_bounded_with_ten_thousand_records() {
    let root = TempRoot::new("bulk");
    let store = open(&root);
    let author = identity(51);
    let identity_id = author.as_str().to_owned();
    for index in 0..10_000_u64 {
        let content_id = format!("{index:064x}");
        let filename = format!("bulk-corpus-{index}.txt");
        let record = PublicationRecord {
            publication_id: content_id.clone(),
            identity_id: identity_id.clone(),
            content_id,
            revision: 1,
            record_bytes: vec![0x55; 64],
            title: filename.clone(),
            description: Some("A bounded Stage 16 bulk corpus record".to_owned()),
            filename,
            published_at: 1_700_000_000 + index,
            updated_at: 2_000,
            tombstoned: false,
            category: Category::Documents,
            size_bytes: 128,
            tags: vec!["bulk".to_owned(), "stage16".to_owned()],
            local: true,
            serving: true,
            announced: true,
            availability: PublicationAvailability::Local,
            last_checked_at: None,
        };
        store.upsert_publication(&record).expect("store bulk record");
    }

    let started = Instant::now();
    let page = search(&store, "bulk");
    let elapsed = started.elapsed();
    assert_eq!(page.items.len(), usize::from(nexus_search::DEFAULT_PAGE_SIZE));
    assert!(page.candidate_limit_reached);
    assert_eq!(page.coverage.label(), "local_only");
    assert!(elapsed < Duration::from_secs(1), "local search took {elapsed:?}");

    let started = Instant::now();
    let rebuild = store.rebuild_search_index().expect("rebuild index");
    let elapsed = started.elapsed();
    assert_eq!(rebuild.publications, 10_000);
    assert!(elapsed < Duration::from_secs(30), "index rebuild took {elapsed:?}");
}
