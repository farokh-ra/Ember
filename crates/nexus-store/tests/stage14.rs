//! Stage 14 storage acceptance tests: signed publication metadata, explicit
//! serving state, remote cache conflicts, the serving gate, and withdrawal
//! that keeps local bytes.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use nexus_crypto::{IdentityId, KeyPair, sha256, to_hex};
use nexus_protocol::{
    Category, MAX_TAG_BYTES, MAX_TAGS, ManifestV1, PublicationV1, SignedPublication, SignedTombstone, TombstoneV1,
};
use nexus_store::{
    ContentState, IdentityReference, PublicationAvailability, PublicationCacheOutcome, PublicationRecord,
    ReferenceType, ReplicaRecord, ReplicaState, Store, StoreError, TombstoneRecord, safe_display_name,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-store-stage14-{}-{sequence}-{label}", std::process::id()));
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

fn keypair() -> KeyPair {
    KeyPair::from_seed([14_u8; 32])
}

fn identity() -> IdentityId {
    IdentityId::from_public_key(&keypair().public_key())
}

fn identity_ref() -> IdentityReference {
    IdentityReference {
        identity_id: identity().as_str().to_owned(),
        public_key: vec![14_u8; 32],
        key_file: "secrets/identity.enc".to_owned(),
        label: Some("Stage 14".to_owned()),
        created_at: 1_000,
    }
}

fn open(root: &TempRoot) -> Store {
    Store::open(&root.path).expect("open store")
}

fn content_id(bytes: &[u8]) -> String {
    to_hex(&sha256(bytes))
}

fn signed_publication(content_id: &str, size: u64, tags: Vec<&str>) -> SignedPublication {
    let publication = PublicationV1::new(
        identity(),
        hex32(content_id),
        size,
        "stage14-notes.txt",
        "Stage 14 store vector",
        Category::Documents,
        [0x22_u8; 32],
        1,
        1_700_000_000_000,
        "0.1.0",
    )
    .with_tags(tags.into_iter().map(str::to_owned).collect());
    SignedPublication::sign(&publication, &keypair()).expect("sign publication")
}

fn publication_record(signed: &SignedPublication, content_id: &str, local: bool) -> PublicationRecord {
    let body = signed.publication();
    PublicationRecord {
        publication_id: to_hex(&signed.publication_id()),
        identity_id: body.identity_id.as_str().to_owned(),
        content_id: content_id.to_owned(),
        revision: body.revision,
        record_bytes: signed.raw().to_vec(),
        title: body.display_filename.clone(),
        description: Some(body.description.clone()),
        filename: safe_display_name(&body.display_filename),
        published_at: 1_700_000_000,
        updated_at: 2_000,
        tombstoned: false,
        category: body.category,
        size_bytes: body.size,
        tags: body.tags.clone(),
        local,
        serving: local,
        announced: false,
        availability: if local {
            PublicationAvailability::Local
        } else {
            PublicationAvailability::NoKnownSource
        },
        last_checked_at: None,
    }
}

fn tombstone_record(signed: &SignedTombstone, created_at: u64) -> TombstoneRecord {
    let body = signed.tombstone();
    TombstoneRecord {
        publication_id: to_hex(&body.publication_id),
        identity_id: body.identity_id.as_str().to_owned(),
        content_id: to_hex(&body.content_id),
        revision: body.revision,
        record_bytes: signed.raw().to_vec(),
        created_at,
    }
}

fn hex32(text: &str) -> [u8; 32] {
    assert_eq!(text.len(), 64, "content ID must be 32-byte hex");
    let mut bytes = [0_u8; 32];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("hex byte");
    }
    bytes
}

#[test]
fn publications_round_trip_tags_and_filter_local_rows() {
    let root = TempRoot::new("metadata");
    let bytes = b"stage 14 publication metadata".to_vec();
    let source = root.source("notes.txt", &bytes);
    let content = content_id(&bytes);
    let signed = signed_publication(
        &content,
        u64::try_from(bytes.len()).expect("size"),
        vec!["notes", "local-first"],
    );
    let stored = publication_record(&signed, &content, true);

    let store = open(&root);
    store.upsert_identity_ref(&identity_ref()).expect("identity");
    store
        .import_publication_original(&source, &stored.publication_id)
        .expect("import original");
    store.upsert_publication(&stored).expect("insert publication");
    assert!(store.publication_exists(&stored.publication_id).expect("exists"));

    let record = store.publication(&stored.publication_id).expect("read publication");
    assert_eq!(record.tags, vec!["notes".to_owned(), "local-first".to_owned()]);
    assert_eq!(record.category, Category::Documents);
    assert_eq!(record.size_bytes, u64::try_from(bytes.len()).expect("size"));
    assert!(record.local);
    assert!(record.serving);
    assert!(!record.announced);
    assert_eq!(record.availability, PublicationAvailability::Local);

    let local = store
        .local_publications(Some(identity().as_str()))
        .expect("local publications");
    assert_eq!(local.len(), 1);
    assert_eq!(local[0].publication_id, stored.publication_id);
    assert!(
        store
            .local_publications(Some("nx1someoneelse"))
            .expect("other identity")
            .is_empty()
    );
    assert_eq!(store.local_publications(None).expect("all local").len(), 1);

    // A cached remote row is visible by ID but never appears in local lists.
    let remote_bytes = b"remote stage 14 publication".to_vec();
    let remote_content = content_id(&remote_bytes);
    let remote_signed = signed_publication(
        &remote_content,
        u64::try_from(remote_bytes.len()).expect("size"),
        vec![],
    );
    let remote = publication_record(&remote_signed, &remote_content, false);
    assert_eq!(
        store.cache_remote_publication(&remote).expect("cache remote"),
        PublicationCacheOutcome::Stored
    );
    assert_eq!(store.local_publications(None).expect("all local").len(), 1);
    assert_eq!(store.publications().expect("all publications").len(), 2);

    let mut too_many_tags = stored.clone();
    too_many_tags.tags = (0..=MAX_TAGS).map(|index| format!("tag{index}")).collect();
    assert!(matches!(
        store.upsert_publication(&too_many_tags),
        Err(StoreError::InvalidRecord(_))
    ));

    let mut long_tag = stored.clone();
    long_tag.tags = vec!["x".repeat(MAX_TAG_BYTES + 1)];
    assert!(matches!(
        store.upsert_publication(&long_tag),
        Err(StoreError::InvalidRecord(_))
    ));

    let mut empty_tag = stored.clone();
    empty_tag.tags = vec![String::new()];
    assert!(matches!(
        store.upsert_publication(&empty_tag),
        Err(StoreError::InvalidRecord(_))
    ));
}

#[test]
fn remote_publication_cache_is_exact_and_conflict_safe() {
    let root = TempRoot::new("cache");
    let bytes = b"stage 14 remote cache".to_vec();
    let content = content_id(&bytes);
    let signed = signed_publication(&content, u64::try_from(bytes.len()).expect("size"), vec!["cache"]);
    let remote = publication_record(&signed, &content, false);

    let store = open(&root);
    assert_eq!(
        store.cache_remote_publication(&remote).expect("first cache"),
        PublicationCacheOutcome::Stored
    );
    assert_eq!(
        store.cache_remote_publication(&remote).expect("idempotent cache"),
        PublicationCacheOutcome::Unchanged
    );

    let mut changed_bytes = remote.clone();
    changed_bytes.record_bytes.push(0);
    assert_eq!(
        store.cache_remote_publication(&changed_bytes).expect("changed bytes"),
        PublicationCacheOutcome::Conflict
    );

    let mut changed_identity = remote.clone();
    changed_identity.identity_id = "nx1differentowner".to_owned();
    assert_eq!(
        store
            .cache_remote_publication(&changed_identity)
            .expect("changed identity"),
        PublicationCacheOutcome::Conflict
    );

    let mut changed_content = remote.clone();
    changed_content.content_id = "b".repeat(64);
    assert_eq!(
        store
            .cache_remote_publication(&changed_content)
            .expect("changed content"),
        PublicationCacheOutcome::Conflict
    );

    let mut changed_revision = remote.clone();
    changed_revision.revision = 2;
    assert_eq!(
        store
            .cache_remote_publication(&changed_revision)
            .expect("changed revision"),
        PublicationCacheOutcome::Conflict
    );

    let stored = store.publication(&remote.publication_id).expect("stored publication");
    assert_eq!(stored.record_bytes, signed.raw());
    assert_eq!(stored.revision, 1);
    assert_eq!(stored.identity_id, identity().as_str());
    assert!(!stored.local);
}

#[test]
fn serving_gate_requires_verified_bytes_and_a_retained_reason() {
    let root = TempRoot::new("serving");
    let bytes = b"stage 14 serveable content".to_vec();
    let content = content_id(&bytes);

    let store = open(&root);
    let staged = store.temporary_directory().join("staged.part");
    fs::write(&staged, &bytes).expect("stage bytes");
    let imported = store
        .import_local_staged(&staged, &content, u64::try_from(bytes.len()).expect("size"), "import-1")
        .expect("import staged");
    assert!(!imported.already_present);
    assert_eq!(store.content(&content).expect("content").state, ContentState::Verified);

    // A bare local import is not a serving reason.
    assert!(!store.content_serveable(&content).expect("serveable"));

    let signed = signed_publication(&content, u64::try_from(bytes.len()).expect("size"), vec!["serving"]);
    let stored = publication_record(&signed, &content, true);
    store.upsert_publication(&stored).expect("publication");
    store
        .adopt_local_import(&content, &stored.publication_id)
        .expect("adopt local import");
    let references = store.references().expect("references");
    assert!(
        references
            .iter()
            .any(|reference| reference.ref_type == ReferenceType::PublicationOriginal)
    );
    assert!(
        !references
            .iter()
            .any(|reference| reference.ref_type == ReferenceType::LocalImport)
    );
    store
        .adopt_local_import(&content, &stored.publication_id)
        .expect("adoption is idempotent");

    assert!(store.content_serveable(&content).expect("serveable"));
    assert!(
        !store
            .content_serveable_except_publication(&content, &stored.publication_id)
            .expect("serveable except")
    );

    store
        .mark_publication_serving(&stored.publication_id, false, 2_100)
        .expect("stop serving");
    assert!(!store.content_serveable(&content).expect("serveable"));

    let replica = ReplicaRecord {
        replica_id: "replica-1".to_owned(),
        content_id: content.clone(),
        publication_id: Some(stored.publication_id.clone()),
        state: ReplicaState::Active,
        last_verified_at: Some(2_200),
        created_at: 2_200,
        updated_at: 2_200,
    };
    store.upsert_replica(&replica).expect("active replica");
    assert!(store.content_serveable(&content).expect("replica serves"));
    assert!(
        store
            .content_serveable_except_publication(&content, &stored.publication_id)
            .expect("replica serves")
    );

    store
        .upsert_replica(&ReplicaRecord {
            state: ReplicaState::Removed,
            updated_at: 2_300,
            ..replica
        })
        .expect("remove replica");
    assert!(!store.content_serveable(&content).expect("serveable"));

    // A retained download is an independent serving reason.
    let download_bytes = b"stage 14 retained download".to_vec();
    let download_content = content_id(&download_bytes);
    let download_source = root.source("retained.bin", &download_bytes);
    store
        .import_retained_download(&download_source, "download-1")
        .expect("retained download");
    assert!(store.content_serveable(&download_content).expect("retained serves"));
    assert!(
        store
            .content_serveable_except_publication(&download_content, &stored.publication_id)
            .expect("retained serves")
    );
}

#[test]
fn unpublish_marks_record_only_and_retains_local_bytes() {
    let root = TempRoot::new("unpublish");
    let bytes = b"stage 14 unpublish bytes".to_vec();
    let source = root.source("withdraw.txt", &bytes);
    let content = content_id(&bytes);
    let signed = signed_publication(&content, u64::try_from(bytes.len()).expect("size"), vec!["withdraw"]);
    let stored = publication_record(&signed, &content, true);
    let publication_id = stored.publication_id.clone();

    let store = open(&root);
    store.upsert_identity_ref(&identity_ref()).expect("identity");
    store
        .import_publication_original(&source, &publication_id)
        .expect("import original");
    store.upsert_publication(&stored).expect("publication");
    store
        .mark_publication_announced(&publication_id, true, 2_500)
        .expect("announced");
    store
        .set_publication_availability(&publication_id, PublicationAvailability::AvailableNow, Some(2_501))
        .expect("availability");
    let record = store.publication(&publication_id).expect("read publication");
    assert!(record.announced);
    assert_eq!(record.availability, PublicationAvailability::AvailableNow);
    assert_eq!(record.last_checked_at, Some(2_501));

    let tombstone = TombstoneV1::new(
        identity(),
        signed.publication_id(),
        hex32(&content),
        1,
        1_700_000_100_000,
    );
    let tombstone = SignedTombstone::sign(&tombstone, &keypair()).expect("sign tombstone");
    store
        .upsert_tombstone(&tombstone_record(&tombstone, 2_600))
        .expect("tombstone");
    store
        .mark_publication_unpublished(&publication_id, 2_601)
        .expect("mark unpublished");

    let record = store.publication(&publication_id).expect("read withdrawn publication");
    assert!(record.tombstoned);
    assert!(!record.serving);
    assert!(!record.announced);
    assert_eq!(record.availability, PublicationAvailability::RecordOnly);
    assert_eq!(record.record_bytes, signed.raw());
    assert_eq!(
        store
            .tombstone(&publication_id)
            .expect("tombstone")
            .expect("stored")
            .record_bytes,
        tombstone.raw()
    );
    assert!(!store.content_serveable(&content).expect("serveable"));
    assert!(
        store
            .content_path(&content)
            .is_ok_and(|path| fs::read(path).expect("retained bytes") == bytes)
    );
}

#[test]
fn ensure_manifest_builds_and_persists_a_verified_manifest() {
    let root = TempRoot::new("manifest");
    let bytes = b"stage 14 manifest payload".repeat(2_000);
    let source = root.source("manifest.bin", &bytes);
    let content = content_id(&bytes);
    let signed = signed_publication(&content, u64::try_from(bytes.len()).expect("size"), vec![]);
    let stored = publication_record(&signed, &content, true);

    let store = open(&root);
    store.upsert_identity_ref(&identity_ref()).expect("identity");
    store
        .import_publication_original(&source, &stored.publication_id)
        .expect("import original");
    store.upsert_publication(&stored).expect("publication");

    let manifest_bytes = store.ensure_manifest_for_content(&content).expect("ensure manifest");
    let manifest = ManifestV1::parse_body(&manifest_bytes).expect("parse manifest");
    assert_eq!(manifest.content_id, hex32(&content));
    assert_eq!(manifest.total_size, u64::try_from(bytes.len()).expect("size"));
    assert_eq!(
        store.manifest_for_content(&content).expect("stored manifest"),
        manifest_bytes
    );
    assert_eq!(
        store.ensure_manifest_for_content(&content).expect("idempotent ensure"),
        manifest_bytes
    );
}
