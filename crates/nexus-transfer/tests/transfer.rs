//! Stage 07 acceptance tests for file identity, manifests, and verification.
//!
//! The 2.5 MB vector below is generated independently of the Rust code and is
//! committed as a cross-platform fixture. It exercises three chunks, including
//! a non-full final chunk, and a one-byte change.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use nexus_crypto::{IdentityId, KeyPair, sha256, to_hex};
use nexus_protocol::{Category, ProtocolError, PublicationV1, SignedPublication};
use nexus_transfer::{
    ContentHash, ManifestVerifier, TransferError, build_manifest, build_manifest_for_file, hash_file, hash_reader,
    verify_chunk, verify_file, verify_file_path, verify_manifest_bytes, verify_publication_manifest,
};

const VECTOR_SIZE: usize = 2_500_000;
const VECTOR_CONTENT_ID: &str = "910b21fbdf3343e088e1ac6c2aa9aaffad0e893a43ff3b7d282e43f2f4d1e0fa";
const VECTOR_MANIFEST_BODY: &str = concat!(
    "a50101025820910b21fbdf3343e088e1ac6c2aa9aaffad0e893a43ff3b7d282e43f2f4d1e0fa031a002625a0",
    "041a001000000583821a0010000058201c59b8670027384143781a8a8bff2f3b44bd8818d0f53b13b064c2375",
    "a1afe38821a001000005820d50d98efacff4cc59dedce2c1b0add120d2c6264418cc699883fa1fd389ac7a982",
    "1a000625a058203ff806e9f7fdd1004fa33358186c040d7cf5704684cc360d53c7864b83ede395"
);
const VECTOR_MANIFEST_ROOT: &str = "d99a72330148d149138852e4afebcf486e84ac1853b1da13f937e5485a913a30";
const VECTOR_CHUNK_HASHES: [&str; 3] = [
    "1c59b8670027384143781a8a8bff2f3b44bd8818d0f53b13b064c2375a1afe38",
    "d50d98efacff4cc59dedce2c1b0add120d2c6264418cc699883fa1fd389ac7a9",
    "3ff806e9f7fdd1004fa33358186c040d7cf5704684cc360d53c7864b83ede395",
];
const VECTOR_ONE_BYTE_CONTENT_ID: &str = "3441cd7ebe54222fcc3f01bffc3194f41a3a7a237b09fedc8d8910d983e920b8";

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(label: &str, bytes: &[u8]) -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-transfer-it-{}-{sequence}-{label}", std::process::id()));
        fs::write(&path, bytes).expect("write fixture");
        Self { path }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn vector_bytes() -> Vec<u8> {
    (0..VECTOR_SIZE)
        .map(|index| u8::try_from((index * 31 + 7) % 251).expect("byte"))
        .collect()
}

#[test]
fn deterministic_vector_matches_committed_identity_and_manifest() {
    let data = vector_bytes();
    let built = build_manifest(&mut Cursor::new(data.clone())).expect("build manifest");

    assert_eq!(built.content_hash().to_hex(), VECTOR_CONTENT_ID);
    assert_eq!(built.content_hash(), ContentHash::from_bytes(sha256(&data)));
    assert_eq!(built.size(), u64::try_from(VECTOR_SIZE).expect("size"));
    assert_eq!(built.manifest().chunk_count(), 3);
    assert_eq!(
        built
            .manifest()
            .chunks
            .iter()
            .map(|chunk| chunk.length)
            .collect::<Vec<_>>(),
        [1_048_576, 1_048_576, 402_848]
    );
    for (entry, expected) in built.manifest().chunks.iter().zip(VECTOR_CHUNK_HASHES) {
        assert_eq!(to_hex(&entry.sha256), expected);
    }

    let body = built.manifest_bytes().expect("manifest bytes");
    assert_eq!(to_hex(&body), VECTOR_MANIFEST_BODY);
    assert_eq!(to_hex(&built.manifest_root().expect("root")), VECTOR_MANIFEST_ROOT);

    let parsed = verify_manifest_bytes(&body, built.manifest_root().expect("root")).expect("verify manifest");
    assert_eq!(&parsed, built.manifest());
    assert_eq!(
        verify_file(&mut Cursor::new(data.clone()), &parsed).expect("verify file"),
        built.content_hash()
    );

    let mut hashed = Cursor::new(data);
    assert_eq!(hash_reader(&mut hashed).expect("hash"), built.content_hash());
}

#[test]
fn one_byte_change_produces_a_different_content_id_and_root() {
    let mut changed = vector_bytes();
    changed[1_234_567] ^= 0x01;
    let built = build_manifest(&mut Cursor::new(changed)).expect("build manifest");
    assert_ne!(built.content_hash().to_hex(), VECTOR_CONTENT_ID);
    assert_eq!(built.content_hash().to_hex(), VECTOR_ONE_BYTE_CONTENT_ID);
    assert_ne!(built.manifest_root().expect("root"), hex(VECTOR_MANIFEST_ROOT));
}

#[test]
fn corrupted_chunks_are_rejected_by_chunk_and_stream_verification() {
    let data = vector_bytes();
    let built = build_manifest(&mut Cursor::new(data.clone())).expect("build manifest");
    let manifest = built.manifest();

    let mut corrupted = data.clone();
    corrupted[0] ^= 0xff;
    assert!(matches!(
        verify_chunk(manifest, 0, &corrupted[..1_048_576]),
        Err(TransferError::ChunkHashMismatch { index: 0 })
    ));
    assert!(matches!(
        verify_file(&mut Cursor::new(corrupted), manifest),
        Err(TransferError::ChunkHashMismatch { index: 0 })
    ));

    let mut corrupted_last = data.clone();
    let last_index = data.len() - 1;
    corrupted_last[last_index] ^= 0x80;
    assert!(matches!(
        verify_chunk(manifest, 2, &corrupted_last[2 * 1_048_576..]),
        Err(TransferError::ChunkHashMismatch { index: 2 })
    ));
}

#[test]
fn reordered_and_truncated_chunks_are_rejected() {
    let data = vector_bytes();
    let built = build_manifest(&mut Cursor::new(data.clone())).expect("build manifest");
    let manifest = built.manifest();

    let mut verifier = ManifestVerifier::new(manifest.clone());
    assert!(matches!(
        verifier.verify_next(&data[1_048_576..2 * 1_048_576]),
        Err(TransferError::ChunkHashMismatch { index: 0 })
    ));

    let mut reordered = data.clone();
    let (first, rest) = reordered.split_at_mut(1_048_576);
    first.copy_from_slice(&rest[..1_048_576]);
    rest[..1_048_576].copy_from_slice(&data[..1_048_576]);
    assert!(matches!(
        verify_file(&mut Cursor::new(reordered), manifest),
        Err(TransferError::ChunkHashMismatch { index: 0 })
    ));

    let truncated = &data[..data.len() - 1];
    assert!(matches!(
        verify_file(&mut Cursor::new(truncated.to_vec()), manifest),
        Err(TransferError::SizeMismatch { .. })
    ));

    let mut extra = data.clone();
    extra.push(0);
    assert!(matches!(
        verify_file(&mut Cursor::new(extra), manifest),
        Err(TransferError::SizeMismatch { .. })
    ));
}

#[test]
fn manifest_bytes_must_match_the_signed_root_and_publication() {
    let data = vector_bytes();
    let built = build_manifest(&mut Cursor::new(data)).expect("build manifest");
    let body = built.manifest_bytes().expect("manifest bytes");
    let root = built.manifest_root().expect("root");

    let mut tampered = body.clone();
    tampered[10] ^= 0x01;
    assert!(matches!(
        verify_manifest_bytes(&tampered, root),
        Err(TransferError::ManifestRootMismatch { .. })
    ));
    assert!(matches!(
        verify_manifest_bytes(&body[..body.len() - 1], root),
        Err(TransferError::Protocol(ProtocolError::Cbor(_)))
    ));

    let keypair = KeyPair::from_seed([7_u8; 32]);
    let identity = IdentityId::from_public_key(&keypair.public_key());
    let publication = PublicationV1::new(
        identity,
        *built.content_hash().as_bytes(),
        built.size(),
        "stage07-vector.bin",
        "deterministic Stage 07 vector",
        Category::Data,
        root,
        1,
        1_700_000_000_000,
        "0.1.0",
    );
    let signed = SignedPublication::sign(&publication, &keypair).expect("sign publication");
    let verified = verify_publication_manifest(&signed, &body).expect("publication manifest");
    assert_eq!(&verified, built.manifest());

    let mut other = publication.clone();
    other.content_id = [0x99_u8; 32];
    let other = SignedPublication::sign(&other, &keypair).expect("sign other");
    assert!(matches!(
        verify_publication_manifest(&other, &body),
        Err(TransferError::Protocol(ProtocolError::ManifestMismatch))
    ));
}

#[test]
fn file_path_helpers_round_trip_the_committed_vector() {
    let data = vector_bytes();
    let temp = TempFile::new("vector.bin", &data);

    assert_eq!(
        hash_file(&temp.path).expect("hash file"),
        ContentHash::from_bytes(sha256(&data))
    );
    let built = build_manifest_for_file(&temp.path).expect("build from file");
    assert_eq!(built.content_hash().to_hex(), VECTOR_CONTENT_ID);
    assert_eq!(
        verify_file_path(&temp.path, built.manifest()).expect("verify path"),
        built.content_hash()
    );
}

#[test]
fn empty_files_are_rejected() {
    let error = build_manifest(&mut Cursor::new(Vec::new())).expect_err("empty file");
    assert!(matches!(error, TransferError::EmptyFile));
}

fn hex(text: &str) -> [u8; 32] {
    *ContentHash::parse_hex(text).expect("hex").as_bytes()
}
