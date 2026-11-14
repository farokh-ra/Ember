//! Stage 07 signed-record acceptance tests: manifests, revisions, tombstones.
//!
//! The committed vectors use the Stage 06 identity seed and the independent
//! 2.5 MB transfer fixture. They pin exact canonical bytes, roots, signatures,
//! and envelope hashes for cross-platform reproduction.

use nexus_crypto::{
    CborLimits, DomainTag, IdentityId, KeyPair, Value, decode_signed_record, domain_sha256, sha256, to_hex,
};
use nexus_protocol::{
    CHUNK_SIZE_V1, Category, ChunkEntry, MANIFEST_SCHEMA_VERSION, ManifestV1, ProtocolError, PublicationV1,
    SignedPublication, SignedRecord, SignedTombstone, TombstoneV1,
};

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

const TOMBSTONE_BODY: &str = concat!(
    "a601010278406e78316135716e6b363338686e387464667064763233363335703264757868786566",
    "6a7a346d336d636a7235636176716a397033647661353267366c3864333703582044444444444444",
    "44444444444444444444444444444444444444444444444444045820111111111111111111111111",
    "11111111111111111111111111111111111111110501061b0000018bcfe6eea0"
);
const TOMBSTONE_SIGNATURE: &str = concat!(
    "988cafcacf714ac3dde780c2785af0b66d2c41bc75787e4ca9718a2f55c06e86",
    "a1e2784254381374b9e6e5f9ec66d420b7c82e49672606ff528a1f2a7c548f09"
);
const TOMBSTONE_ENVELOPE_SHA256: &str = "c018998945ee4355c349238a849dafbb59ec44aee9027375ec2ded7d4caa0561";

fn keypair() -> KeyPair {
    KeyPair::from_seed([0_u8; 32])
}

fn identity_id() -> IdentityId {
    IdentityId::from_public_key(&keypair().public_key())
}

fn vector_manifest() -> ManifestV1 {
    let chunks = VECTOR_CHUNK_HASHES
        .iter()
        .zip([1_048_576_u64, 1_048_576, 402_848])
        .map(|(hash, length)| ChunkEntry::new(length, hex32(hash)))
        .collect();
    ManifestV1::new(hex32(VECTOR_CONTENT_ID), 2_500_000, chunks)
}

fn publication() -> PublicationV1 {
    PublicationV1::new(
        identity_id(),
        [0x11_u8; 32],
        4_096,
        "notes.txt",
        "Stage 07 vector",
        Category::Documents,
        [0x22_u8; 32],
        1,
        1_700_000_000_000,
        "0.1.0",
    )
}

fn tombstone() -> TombstoneV1 {
    TombstoneV1::new(identity_id(), [0x44_u8; 32], [0x11_u8; 32], 1, 1_700_000_100_000)
}

fn envelope(body: Vec<u8>, signature: &[u8]) -> Vec<u8> {
    Value::Map(vec![
        (Value::Unsigned(1), Value::Bytes(body)),
        (Value::Unsigned(2), Value::Bytes(signature.to_vec())),
    ])
    .to_canonical_bytes_with_limits(CborLimits::signed_record())
    .expect("canonical envelope")
}

fn resign_body(body: Vec<u8>, tag: DomainTag) -> Vec<u8> {
    SignedRecord::sign(body, tag, &keypair()).expect("sign").raw().to_vec()
}

fn hex32(text: &str) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("hex");
    }
    bytes
}

#[test]
fn committed_manifest_vector_is_exact() {
    let manifest = vector_manifest();
    let body = manifest.encode_body().expect("manifest");
    assert_eq!(to_hex(&body), VECTOR_MANIFEST_BODY);
    assert_eq!(to_hex(&manifest.root().expect("root")), VECTOR_MANIFEST_ROOT);
    assert_eq!(
        manifest.root().expect("root"),
        domain_sha256(DomainTag::Manifest, &body)
    );
    assert_ne!(manifest.root().expect("root"), sha256(&body));

    let parsed = ManifestV1::parse_body(&body).expect("parse");
    assert_eq!(parsed, manifest);
    assert_eq!(parsed.chunk_count(), 3);
    assert_eq!(parsed.chunk(2).expect("last chunk").length, 402_848);
    assert_eq!(parsed.total_size, 2_500_000);
    assert_eq!(parsed.chunk_size, CHUNK_SIZE_V1);
}

#[test]
fn committed_tombstone_vector_is_exact() {
    let signed = SignedTombstone::sign(&tombstone(), &keypair()).expect("sign tombstone");
    assert_eq!(to_hex(signed.record().body_bytes()), TOMBSTONE_BODY);
    assert_eq!(signed.record().signature().to_hex(), TOMBSTONE_SIGNATURE);
    assert_eq!(to_hex(&sha256(signed.raw())), TOMBSTONE_ENVELOPE_SHA256);

    let parsed = SignedTombstone::parse(signed.raw()).expect("verify tombstone");
    assert_eq!(parsed.raw(), signed.raw());
    assert_eq!(parsed.tombstone(), signed.tombstone());
    assert_eq!(parsed.tombstone().revision, 1);
    assert_eq!(parsed.tombstone().unpublished_at_ms, 1_700_000_100_000);
}

#[test]
fn tombstone_changes_bad_signatures_and_replay_are_rejected() {
    let signed = SignedTombstone::sign(&tombstone(), &keypair()).expect("sign");
    let mut body = signed.record().body_bytes().to_vec();
    let last = body.len() - 1;
    body[last] ^= 0x01;
    assert!(matches!(
        SignedTombstone::parse(&envelope(body, signed.record().signature().as_bytes())),
        Err(ProtocolError::VerificationFailed)
    ));

    let mut signature = *signed.record().signature().as_bytes();
    signature[63] ^= 0x01;
    assert!(matches!(
        SignedTombstone::parse(&envelope(signed.record().body_bytes().to_vec(), &signature)),
        Err(ProtocolError::VerificationFailed)
    ));

    signed
        .record()
        .verify(DomainTag::Publication, &keypair().public_key())
        .expect_err("domain replay must fail");
}

#[test]
fn manifest_unknown_minor_fields_are_preserved_and_major_versions_rejected() {
    let body = vector_manifest().encode_body().expect("body");
    let entries = decode_signed_record(&body)
        .expect("decode")
        .as_map()
        .expect("map")
        .to_vec();

    let mut with_minor = entries.clone();
    with_minor.push((Value::Unsigned(100), Value::Text("future".to_owned())));
    let with_minor = Value::Map(with_minor).to_canonical_bytes().expect("canonical");
    let parsed = ManifestV1::parse_body(&with_minor).expect("minor field");
    assert_eq!(parsed.unknown_fields().len(), 1);
    assert_eq!(parsed.unknown_fields()[0].0, 100);
    assert_eq!(parsed.encode_body().expect("re-encode"), with_minor);

    let mut with_major = entries;
    for (key, value) in &mut with_major {
        if key == &Value::Unsigned(1) {
            *value = Value::Unsigned(MANIFEST_SCHEMA_VERSION + 1);
        }
    }
    let with_major = Value::Map(with_major).to_canonical_bytes().expect("canonical");
    assert!(matches!(
        ManifestV1::parse_body(&with_major),
        Err(ProtocolError::UnsupportedVersion(2))
    ));
}

#[test]
fn tombstone_unknown_minor_fields_are_preserved_and_major_versions_rejected() {
    let body = tombstone().encode_body().expect("body");
    let entries = decode_signed_record(&body)
        .expect("decode")
        .as_map()
        .expect("map")
        .to_vec();

    let mut with_minor = entries.clone();
    with_minor.push((Value::Unsigned(100), Value::Text("future".to_owned())));
    let with_minor = Value::Map(with_minor).to_canonical_bytes().expect("canonical");
    let signed = SignedTombstone::parse(&resign_body(with_minor, DomainTag::Tombstone)).expect("minor field");
    assert_eq!(signed.tombstone().unknown_fields().len(), 1);
    assert_eq!(signed.tombstone().unknown_fields()[0].0, 100);

    let mut with_major = entries;
    for (key, value) in &mut with_major {
        if key == &Value::Unsigned(1) {
            *value = Value::Unsigned(2);
        }
    }
    let with_major = Value::Map(with_major).to_canonical_bytes().expect("canonical");
    assert!(matches!(
        SignedTombstone::parse(&resign_body(with_major, DomainTag::Tombstone)),
        Err(ProtocolError::UnsupportedVersion(2))
    ));
}

#[test]
fn manifest_shape_and_publication_cross_checks_are_enforced() {
    let mut bad = vector_manifest();
    bad.total_size = 0;
    assert!(matches!(bad.encode_body(), Err(ProtocolError::InvalidField(3))));

    let mut bad = vector_manifest();
    bad.chunk_size = 2;
    assert!(matches!(bad.encode_body(), Err(ProtocolError::InvalidField(4))));

    let mut bad = vector_manifest();
    bad.chunks.pop();
    assert!(matches!(bad.encode_body(), Err(ProtocolError::InvalidField(5))));

    let mut bad = vector_manifest();
    bad.chunks[0].length -= 1;
    assert!(matches!(bad.encode_body(), Err(ProtocolError::InvalidField(5))));

    let mut bad = vector_manifest();
    bad.chunks[2].length += 1;
    assert!(matches!(bad.encode_body(), Err(ProtocolError::InvalidField(5))));

    let manifest = vector_manifest();
    let matching = PublicationV1::new(
        identity_id(),
        hex32(VECTOR_CONTENT_ID),
        manifest.total_size,
        "vector.bin",
        "",
        Category::Data,
        manifest.root().expect("root"),
        1,
        1_700_000_000_000,
        "0.1.0",
    );
    manifest
        .validate_for_publication(&matching)
        .expect("matching publication");
    matching.validate_manifest(&manifest).expect("publication helper");

    let mut wrong_content = matching.clone();
    wrong_content.content_id = [0x11_u8; 32];
    assert!(matches!(
        manifest.validate_for_publication(&wrong_content),
        Err(ProtocolError::ManifestMismatch)
    ));

    let mut wrong_size = matching.clone();
    wrong_size.size += 1;
    assert!(matches!(
        manifest.validate_for_publication(&wrong_size),
        Err(ProtocolError::ManifestMismatch)
    ));

    let mut wrong_root = matching;
    wrong_root.manifest_root = [0x22_u8; 32];
    assert!(matches!(
        manifest.validate_for_publication(&wrong_root),
        Err(ProtocolError::ManifestMismatch)
    ));
}

#[test]
fn publication_revision_rules_are_enforced() {
    let mut bad = publication();
    bad.supersedes = Some([0x55_u8; 32]);
    assert!(matches!(
        SignedPublication::sign(&bad, &keypair()),
        Err(ProtocolError::InvalidField(13))
    ));

    let mut bad = publication();
    bad.revision = 2;
    assert!(matches!(
        SignedPublication::sign(&bad, &keypair()),
        Err(ProtocolError::InvalidField(13))
    ));

    let first = SignedPublication::sign(&publication(), &keypair()).expect("first");
    let mut second_body = publication();
    second_body.revision = 2;
    second_body.description = "Edited metadata".to_owned();
    second_body.supersedes = Some(first.publication_id());
    let second = SignedPublication::sign(&second_body, &keypair()).expect("second");
    second
        .publication()
        .validate_successor_of(&first)
        .expect("valid successor");
    assert_ne!(first.publication_id(), second.publication_id());

    let mut changed_content = second_body.clone();
    changed_content.content_id = [0x33_u8; 32];
    let changed_content = SignedPublication::sign(&changed_content, &keypair()).expect("sign");
    assert!(matches!(
        changed_content.publication().validate_successor_of(&first),
        Err(ProtocolError::RevisionChainMismatch)
    ));

    let mut changed_size = second_body.clone();
    changed_size.size += 1;
    let changed_size = SignedPublication::sign(&changed_size, &keypair()).expect("sign");
    assert!(matches!(
        changed_size.publication().validate_successor_of(&first),
        Err(ProtocolError::RevisionChainMismatch)
    ));

    let mut changed_root = second_body.clone();
    changed_root.manifest_root = [0x44_u8; 32];
    let changed_root = SignedPublication::sign(&changed_root, &keypair()).expect("sign");
    assert!(matches!(
        changed_root.publication().validate_successor_of(&first),
        Err(ProtocolError::RevisionChainMismatch)
    ));

    let mut skipped = second_body.clone();
    skipped.revision = 3;
    let skipped = SignedPublication::sign(&skipped, &keypair()).expect("sign");
    assert!(matches!(
        skipped.publication().validate_successor_of(&first),
        Err(ProtocolError::RevisionChainMismatch)
    ));

    let mut wrong_predecessor = second_body;
    wrong_predecessor.supersedes = Some([0x55_u8; 32]);
    let wrong_predecessor = SignedPublication::sign(&wrong_predecessor, &keypair()).expect("sign");
    assert!(matches!(
        wrong_predecessor.publication().validate_successor_of(&first),
        Err(ProtocolError::RevisionChainMismatch)
    ));
}

#[test]
fn tombstones_bind_to_the_original_signed_publication_and_revisions() {
    let publication = SignedPublication::sign(&publication(), &keypair()).expect("publication");
    let matching = TombstoneV1::new(
        identity_id(),
        publication.publication_id(),
        publication.publication().content_id,
        1,
        1_700_000_000_000,
    );
    let signed = SignedTombstone::sign(&matching, &keypair()).expect("tombstone");
    signed.validate_for(&publication).expect("matching tombstone");

    let mut wrong_publication = matching.clone();
    wrong_publication.publication_id = [0x55_u8; 32];
    let wrong_publication = SignedTombstone::sign(&wrong_publication, &keypair()).expect("sign");
    assert!(matches!(
        wrong_publication.validate_for(&publication),
        Err(ProtocolError::TombstoneMismatch)
    ));

    let mut wrong_content = matching.clone();
    wrong_content.content_id = [0x66_u8; 32];
    let wrong_content = SignedTombstone::sign(&wrong_content, &keypair()).expect("sign");
    assert!(matches!(
        wrong_content.validate_for(&publication),
        Err(ProtocolError::TombstoneMismatch)
    ));

    let mut newer = matching.clone();
    newer.revision = 3;
    newer.unpublished_at_ms += 1;
    let newer = SignedTombstone::sign(&newer, &keypair()).expect("sign");
    newer
        .validate_successor_of(&signed)
        .expect("higher revision may skip numbers");

    let mut equal_revision = matching.clone();
    equal_revision.unpublished_at_ms += 1;
    let equal_revision = SignedTombstone::sign(&equal_revision, &keypair()).expect("sign");
    assert!(matches!(
        equal_revision.validate_successor_of(&signed),
        Err(ProtocolError::RevisionChainMismatch)
    ));

    let mut changed_content = matching;
    changed_content.revision = 2;
    changed_content.content_id = [0x77_u8; 32];
    let changed_content = SignedTombstone::sign(&changed_content, &keypair()).expect("sign");
    assert!(matches!(
        changed_content.validate_successor_of(&signed),
        Err(ProtocolError::RevisionChainMismatch)
    ));
}
