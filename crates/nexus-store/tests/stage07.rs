//! Stage 07 acceptance tests for immutable publications, signed tombstones,
//! manifest persistence, and reconciliation.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use nexus_crypto::{IdentityId, KeyPair, to_hex};
use nexus_protocol::{Category, PublicationV1, SignedPublication, SignedTombstone, TombstoneV1};
use nexus_store::{IdentityReference, PublicationRecord, Store, StoreError, TombstoneRecord, safe_display_name};
use nexus_transfer::{ContentHash, build_manifest};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-store-stage07-{}-{sequence}-{label}", std::process::id()));
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
    KeyPair::from_seed([0_u8; 32])
}

fn identity() -> IdentityId {
    IdentityId::from_public_key(&keypair().public_key())
}

fn identity_ref() -> IdentityReference {
    IdentityReference {
        identity_id: identity().as_str().to_owned(),
        public_key: vec![7_u8; 32],
        key_file: "secrets/identity.enc".to_owned(),
        label: Some("Stage 07".to_owned()),
        created_at: 1_000,
    }
}

fn open(root: &TempRoot) -> Store {
    Store::open(&root.path).expect("open store")
}

fn publication(content_id: &str, size: u64, manifest_root: [u8; 32]) -> PublicationV1 {
    PublicationV1::new(
        identity(),
        hex32(content_id),
        size,
        "stage07.bin",
        "Stage 07 store vector",
        Category::Data,
        manifest_root,
        1,
        1_700_000_000_000,
        "0.1.0",
    )
}

fn publication_record(signed: &SignedPublication, content_id: &str, updated_at: u64) -> PublicationRecord {
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
        updated_at,
        tombstoned: false,
        category: body.category,
        size_bytes: body.size,
        tags: body.tags.clone(),
        local: true,
        serving: true,
        announced: false,
        availability: nexus_store::PublicationAvailability::Local,
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

fn signed_publication(content_id: &str, size: u64, manifest_root: [u8; 32]) -> SignedPublication {
    SignedPublication::sign(&publication(content_id, size, manifest_root), &keypair()).expect("sign publication")
}

#[test]
fn publications_are_immutable_idempotent_and_preserve_original_attribution() {
    let root = TempRoot::new("publications");
    let bytes = b"stage 07 publication bytes".to_vec();
    let source = root.source("publication.bin", &bytes);
    let content_id = to_hex(&nexus_crypto::sha256(&bytes));
    let signed = signed_publication(&content_id, u64::try_from(bytes.len()).expect("size"), [0x22_u8; 32]);
    let stored = publication_record(&signed, &content_id, 2_000);

    let store = open(&root);
    store.upsert_identity_ref(&identity_ref()).expect("identity");
    store
        .import_publication_original(&source, &stored.publication_id)
        .expect("import original");
    store.upsert_publication(&stored).expect("insert publication");

    let records = store.publications().expect("publications");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].record_bytes, signed.raw());
    assert_eq!(records[0].identity_id, identity().as_str());
    assert_eq!(records[0].updated_at, 2_000);

    store.upsert_publication(&stored).expect("idempotent repeat");
    let records = store.publications().expect("publications");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].record_bytes, signed.raw());
    assert_eq!(records[0].updated_at, 2_000);

    let mut conflicting_bytes = stored.clone();
    conflicting_bytes.record_bytes = signed.raw().iter().copied().chain([0_u8]).collect();
    assert!(matches!(
        store.upsert_publication(&conflicting_bytes),
        Err(StoreError::RecordConflict(_))
    ));

    let mut conflicting_identity = stored.clone();
    conflicting_identity.identity_id = "nx1otherowner".to_owned();
    assert!(matches!(
        store.upsert_publication(&conflicting_identity),
        Err(StoreError::RecordConflict(_))
    ));

    let mut conflicting_content = stored.clone();
    conflicting_content.content_id = "b".repeat(64);
    assert!(matches!(
        store.upsert_publication(&conflicting_content),
        Err(StoreError::RecordConflict(_))
    ));

    let mut conflicting_revision = stored;
    conflicting_revision.revision = 2;
    assert!(matches!(
        store.upsert_publication(&conflicting_revision),
        Err(StoreError::RecordConflict(_))
    ));

    let records = store.publications().expect("publications");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].record_bytes, signed.raw());
    assert_eq!(records[0].revision, 1);
}

#[test]
fn tombstones_are_independent_monotonic_and_keep_signed_attribution() {
    let root = TempRoot::new("tombstones");
    let bytes = b"stage 07 tombstone bytes".to_vec();
    let source = root.source("tombstone.bin", &bytes);
    let content_id = to_hex(&nexus_crypto::sha256(&bytes));
    let signed_publication = signed_publication(&content_id, u64::try_from(bytes.len()).expect("size"), [0x22_u8; 32]);
    let publication_id = signed_publication.publication_id();

    let first = TombstoneV1::new(identity(), publication_id, hex32(&content_id), 1, 1_700_000_000_000);
    let first = SignedTombstone::sign(&first, &keypair()).expect("sign first");
    let first_record = tombstone_record(&first, 3_000);

    let store = open(&root);
    // The tombstone arrives before any publication, identity, or content row.
    store.upsert_tombstone(&first_record).expect("orphan tombstone");
    assert_eq!(store.tombstones().expect("tombstones").len(), 1);

    // A late publication row inherits the tombstone state without losing the
    // original signed publication bytes.
    let stored = publication_record(&signed_publication, &content_id, 3_100);
    store.upsert_identity_ref(&identity_ref()).expect("identity");
    store
        .import_publication_original(&source, &stored.publication_id)
        .expect("import original");
    store.upsert_publication(&stored).expect("late publication");
    let publications = store.publications().expect("publications");
    assert_eq!(publications.len(), 1);
    assert!(publications[0].tombstoned);
    assert_eq!(publications[0].record_bytes, signed_publication.raw());

    let mut second = TombstoneV1::new(identity(), publication_id, hex32(&content_id), 2, 1_700_000_000_100);
    second.unpublished_at_ms += 1;
    let second = SignedTombstone::sign(&second, &keypair()).expect("sign second");
    let second_record = tombstone_record(&second, 3_200);
    store.upsert_tombstone(&second_record).expect("higher revision");
    let records = store.tombstones().expect("tombstones");
    assert_eq!(records[0].revision, 2);
    assert_eq!(records[0].record_bytes, second.raw());
    assert_eq!(records[0].identity_id, identity().as_str());

    store.upsert_tombstone(&first_record).expect("stale revision ignored");
    let records = store.tombstones().expect("tombstones");
    assert_eq!(records[0].revision, 2);
    assert_eq!(records[0].record_bytes, second.raw());

    store
        .upsert_tombstone(&second_record)
        .expect("identical equal revision");
    assert_eq!(store.tombstones().expect("tombstones")[0].record_bytes, second.raw());

    let mut conflicting = TombstoneV1::new(identity(), publication_id, hex32(&content_id), 2, 1_700_000_000_200);
    conflicting.unpublished_at_ms += 2;
    let conflicting = SignedTombstone::sign(&conflicting, &keypair()).expect("sign conflicting");
    assert!(matches!(
        store.upsert_tombstone(&tombstone_record(&conflicting, 3_300)),
        Err(StoreError::RecordConflict(_))
    ));
    assert_eq!(store.tombstones().expect("tombstones")[0].record_bytes, second.raw());

    let other = KeyPair::from_seed([8_u8; 32]);
    let other_identity = IdentityId::from_public_key(&other.public_key());
    let other_tombstone = TombstoneV1::new(other_identity, publication_id, hex32(&content_id), 5, 1_700_000_000_300);
    let other_tombstone = SignedTombstone::sign(&other_tombstone, &other).expect("sign other");
    assert!(matches!(
        store.upsert_tombstone(&tombstone_record(&other_tombstone, 3_400)),
        Err(StoreError::RecordConflict(_))
    ));
    assert_eq!(store.tombstones().expect("tombstones")[0].record_bytes, second.raw());

    let changed_content = TombstoneV1::new(identity(), publication_id, [0x99_u8; 32], 5, 1_700_000_000_400);
    let changed_content = SignedTombstone::sign(&changed_content, &keypair()).expect("sign changed");
    assert!(matches!(
        store.upsert_tombstone(&tombstone_record(&changed_content, 3_500)),
        Err(StoreError::RecordConflict(_))
    ));
    let records = store.tombstones().expect("tombstones");
    assert_eq!(records[0].revision, 2);
    assert_eq!(records[0].record_bytes, second.raw());
    assert_eq!(records[0].content_id, content_id);
}

#[test]
fn manifests_round_trip_by_trusted_root_and_bind_to_signed_publications() {
    let root = TempRoot::new("manifests");
    let bytes = b"stage 07 manifest payload".repeat(20_000);
    let source = root.source("manifest.bin", &bytes);
    let built = build_manifest(&mut Cursor::new(bytes.clone())).expect("build manifest");
    let content_id = built.content_hash().to_hex();
    let manifest_root = to_hex(&built.manifest_root().expect("root"));
    let manifest_bytes = built.manifest_bytes().expect("manifest bytes");
    let signed = signed_publication(
        &content_id,
        u64::try_from(bytes.len()).expect("size"),
        built.manifest_root().expect("root"),
    );
    let stored = publication_record(&signed, &content_id, 5_000);

    let store = open(&root);
    store.upsert_identity_ref(&identity_ref()).expect("identity");
    store
        .import_publication_original(&source, &stored.publication_id)
        .expect("import original");
    store.upsert_publication(&stored).expect("publication");

    store
        .put_manifest(&manifest_root, &manifest_bytes)
        .expect("store manifest");
    store
        .put_manifest(&manifest_root, &manifest_bytes)
        .expect("idempotent manifest");
    assert_eq!(store.manifest(&manifest_root).expect("read manifest"), manifest_bytes);
    assert_eq!(
        store
            .manifest_for_publication(&stored.publication_id)
            .expect("publication manifest"),
        manifest_bytes
    );

    let wrong_root = "0".repeat(64);
    assert!(matches!(
        store.put_manifest(&wrong_root, &manifest_bytes),
        Err(StoreError::InvalidManifest(_))
    ));
    let oversized = vec![0_u8; 262_145];
    assert!(matches!(
        store.put_manifest(&manifest_root, &oversized),
        Err(StoreError::InvalidManifest(_))
    ));
    assert!(matches!(
        store.manifest(&wrong_root),
        Err(StoreError::ManifestMissing(_))
    ));
    assert!(matches!(
        store.manifest_for_publication("unknown-publication"),
        Err(StoreError::RecordNotFound(_))
    ));

    let path = root.path.join("store/manifests").join(format!("{manifest_root}.cbor"));
    fs::write(&path, b"corrupt manifest").expect("corrupt manifest");
    assert!(matches!(
        store.manifest(&manifest_root),
        Err(StoreError::InvalidManifest(_))
    ));
    assert!(matches!(
        store.manifest_for_publication(&stored.publication_id),
        Err(StoreError::InvalidManifest(_))
    ));
    store
        .put_manifest(&manifest_root, &manifest_bytes)
        .expect("repair manifest");
    assert_eq!(
        store.manifest(&manifest_root).expect("read repaired manifest"),
        manifest_bytes
    );
}

#[test]
fn reconciliation_verifies_named_manifests_and_quarantines_corrupt_files() {
    let root = TempRoot::new("manifest-reconcile");
    let built = build_manifest(&mut Cursor::new(b"reconciliation manifest".repeat(1_000))).expect("build manifest");
    let manifest_root = to_hex(&built.manifest_root().expect("root"));
    let manifest_bytes = built.manifest_bytes().expect("manifest bytes");

    {
        let store = open(&root);
        store
            .put_manifest(&manifest_root, &manifest_bytes)
            .expect("store manifest");
    }

    let manifests = root.path.join("store/manifests");
    let corrupt_root = "ab".repeat(32);
    fs::write(manifests.join(format!("{corrupt_root}.cbor")), b"not a manifest").expect("corrupt manifest");
    fs::write(manifests.join("unexpected.cbor"), b"bad name").expect("unexpected manifest");

    let store = open(&root);
    let report = store.last_reconciliation();
    assert_eq!(report.verified_manifests, 1);
    assert!(report.quarantined_files >= 2);
    assert!(manifests.join(format!("{manifest_root}.cbor")).is_file());
    assert!(!manifests.join(format!("{corrupt_root}.cbor")).exists());
    assert!(!manifests.join("unexpected.cbor").exists());
    assert_eq!(
        store.manifest(&manifest_root).expect("verified manifest"),
        manifest_bytes
    );
}

fn hex32(text: &str) -> [u8; 32] {
    *ContentHash::parse_hex(text).expect("content ID").as_bytes()
}
