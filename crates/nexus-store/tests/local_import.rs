//! Streaming-import storage conventions added in Stage 08.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use nexus_store::{ContentState, QuotaPolicy, ReferenceType, ReleaseOutcome, Store, StoreConfig, StoreError};
use sha2::{Digest, Sha256};

static COUNTER: AtomicU64 = AtomicU64::new(1);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "nexus-store-local-import-{}-{sequence}-{label}",
            std::process::id()
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

fn store(root: &TempRoot) -> Store {
    Store::open_with_config(
        &root.path,
        StoreConfig {
            quota: QuotaPolicy::custom(64 * 1024 * 1024, 0).expect("quota"),
            max_file_size: 16 * 1024 * 1024,
            ..StoreConfig::default()
        },
    )
    .expect("open store")
}

fn content_id(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    nexus_crypto::to_hex(&digest)
}

fn stage(store: &Store, name: &str, bytes: &[u8]) -> PathBuf {
    let path = store.temporary_directory().join(name);
    fs::write(&path, bytes).expect("stage bytes");
    path
}

#[test]
fn local_import_promotes_staged_bytes_without_copying() {
    let root = TempRoot::new("promote");
    let store = store(&root);
    let bytes = b"streamed import bytes";
    let id = content_id(bytes);
    let staged = stage(&store, "import-1.part", bytes);

    let imported = store
        .import_local_staged(&staged, &id, bytes.len() as u64, "import-1")
        .expect("import staged");
    assert!(!imported.already_present);
    assert!(!staged.exists(), "staged file is renamed into the blob tree");

    let blob = store.content_path(&id).expect("verified content path");
    assert_eq!(fs::read(&blob).expect("blob bytes"), bytes);
    assert_eq!(store.content(&id).expect("record").state, ContentState::Verified);
    assert!(
        store
            .references()
            .expect("references")
            .iter()
            .any(|reference| reference.ref_type == ReferenceType::LocalImport)
    );

    // Re-importing the same bytes is idempotent and discards the new temp file.
    let staged_again = stage(&store, "import-2.part", bytes);
    let usage_before = store.usage().expect("usage").managed_bytes;
    let imported = store
        .import_local_staged(&staged_again, &id, bytes.len() as u64, "import-2")
        .expect("duplicate import");
    assert!(imported.already_present);
    assert!(!staged_again.exists());
    assert_eq!(store.usage().expect("usage").managed_bytes, usage_before);

    let outcome = store
        .release_reference(&id, ReferenceType::LocalImport, "import-1", false)
        .expect("release first import");
    assert!(matches!(outcome, ReleaseOutcome::Retained { .. }));
    let outcome = store
        .release_reference(&id, ReferenceType::LocalImport, "import-2", false)
        .expect("release second import");
    assert_eq!(outcome, ReleaseOutcome::BytesDeleted);
    assert!(matches!(store.content_path(&id), Err(StoreError::ContentMissing(_))));
}

#[test]
fn local_import_rejects_a_staged_file_outside_the_temporary_directory() {
    let root = TempRoot::new("outside");
    let store = store(&root);
    let bytes = b"outside bytes";
    let id = content_id(bytes);
    let outside = root.path.join("outside.part");
    fs::write(&outside, bytes).expect("outside file");

    let error = store
        .import_local_staged(&outside, &id, bytes.len() as u64, "import-1")
        .expect_err("outside path");
    assert!(matches!(error, StoreError::UnsafePath(_)));
}

#[test]
fn local_import_verifies_exact_bytes_before_promotion() {
    let root = TempRoot::new("corrupt");
    let store = store(&root);
    let bytes = b"expected bytes";
    let id = content_id(bytes);
    let staged = stage(&store, "corrupt.part", b"different bytes");

    let error = store
        .import_local_staged(&staged, &id, bytes.len() as u64, "import-1")
        .expect_err("hash mismatch");
    assert!(matches!(error, StoreError::IntegrityMismatch(_)));
    assert!(staged.exists(), "the failed staging file is left for cleanup");
    assert!(matches!(store.content_path(&id), Err(StoreError::ContentMissing(_))));
}

#[test]
fn checkpoint_flushes_and_schema_is_current() {
    let root = TempRoot::new("checkpoint");
    let store = store(&root);
    assert_eq!(store.schema_version().expect("schema"), 5);
    store.checkpoint().expect("checkpoint");
    assert_eq!(store.schema_version().expect("schema after checkpoint"), 5);
}
