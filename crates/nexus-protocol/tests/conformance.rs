//! Public protocol v1 conformance vectors and rejection cases.
//!
//! These vectors are the same ones published in `docs/protocol/PROTOCOL_SPEC.md`.
//! They are intentionally duplicated here so the public specification has an
//! executable check. A failure is a protocol review event, not a test to update
//! casually.
//!
//! This suite is self-conformance for the reference implementation. It is not
//! evidence that an independently built client interoperates. See
//! `docs/protocol/INTEROPERABILITY.md` for that distinction.

use nexus_crypto::{CborLimits, DomainTag, IdentityId, KeyPair, Value, decode_signed_record, sha256, to_hex};
use nexus_protocol::{
    CHUNK_SIZE_V1, Category, ChunkEntry, ManifestV1, PROFILE_SCHEMA_VERSION, ProfileV1, ProtocolError, PublicationV1,
    SignedProfile, SignedPublication, SignedRecord, SignedTombstone, TombstoneV1, publication_id,
};

const PROFILE_BODY: &str = concat!(
    "a901010278406e78316135716e6b363338686e3874646670647632333633357032647578687865666a",
    "7a346d336d636a7235636176716a397033647661353267366c38643337036c416461204c6f76656c",
    "616365046361646105714c6f63616c2d6669727374206e6f74657306f607800801091b0000018bcf",
    "e56800"
);
const PROFILE_SIGNATURE: &str = concat!(
    "c640003c1849c912705ca5e8ac7e4c6af33befbaa87ce536950dbb23bf23f044a0336e0ce38d667a",
    "b67d17eaf6bb18850023db4f29af995a1f7cf0d237fb8c09"
);
const PROFILE_ENVELOPE_SHA256: &str = "f2f9d085d8873335772fb436aaa171c35e6a74de4f8dae3d15e3a7fe27680193";

const PUBLICATION_BODY: &str = concat!(
    "b001010278406e78316135716e6b363338686e3874646670647632333633357032647578687865666a",
    "7a346d336d636a7235636176716a397033647661353267366c38643337035820",
    "11111111111111111111111111111111111111111111111111111111111111110419100005696e6f7465732e747874066f",
    "537461676520303620766563746f7207000882677374616765303666766563746f72091b0000018bcf",
    "e568000a582022222222222222222222222222222222222222222222222222222222222222220b1a00",
    "1000000c010df60ef60ff61065302e312e30"
);
const PUBLICATION_SIGNATURE: &str = concat!(
    "6e4459e8cc18e2ee0e8c95840a1899a3c7005b882f8a5c44b832c7a0156e9ac5f0173295e3e4a7c5",
    "39d231cbd5f725e3c132eb9401a3d71be4097dba03cf990f"
);
const PUBLICATION_ENVELOPE_SHA256: &str = "24561ecbb4040cd8291d486a84bdc7d3f96cfcc1c8fa0962901afeacfa35f04f";
const PUBLICATION_ID: &str = "40fcc4cc64e3c49ce8e102cfa34ba4c5b225a637b48098f463d428356b028c21";

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

const IDENTITY_ID: &str = "nx1a5qnk638hn8tdfpdv23635p2duxhxefjz4m3mcjr5cavqj9p3dva52g6l8d37";
const IDENTITY_PUBLIC_KEY: &str = "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29";

fn keypair() -> KeyPair {
    KeyPair::from_seed([0_u8; 32])
}

fn identity_id() -> IdentityId {
    IdentityId::from_public_key(&keypair().public_key())
}

fn profile() -> ProfileV1 {
    ProfileV1::new(identity_id(), "Ada Lovelace", "Local-first notes", 1, 1_700_000_000_000).with_username("ada")
}

fn publication() -> PublicationV1 {
    PublicationV1::new(
        identity_id(),
        [0x11_u8; 32],
        4_096,
        "notes.txt",
        "Stage 06 vector",
        Category::Documents,
        [0x22_u8; 32],
        1,
        1_700_000_000_000,
        "0.1.0",
    )
    .with_tags(vec!["stage06".to_owned(), "vector".to_owned()])
}

fn tombstone() -> TombstoneV1 {
    TombstoneV1::new(identity_id(), [0x44_u8; 32], [0x11_u8; 32], 1, 1_700_000_100_000)
}

fn manifest() -> ManifestV1 {
    let chunks = VECTOR_CHUNK_HASHES
        .iter()
        .zip([1_048_576_u64, 1_048_576, 402_848])
        .map(|(hash, length)| ChunkEntry::new(length, hex32(hash)))
        .collect();
    ManifestV1::new(hex32(VECTOR_CONTENT_ID), 2_500_000, chunks)
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
fn v1_identity_vector_is_canonical() {
    assert_eq!(keypair().public_key().to_hex(), IDENTITY_PUBLIC_KEY);
    assert_eq!(identity_id().as_str(), IDENTITY_ID);
    let parsed = IdentityId::parse(IDENTITY_ID).expect("parse canonical identity");
    assert_eq!(parsed.public_key().expect("embedded key"), keypair().public_key());
}

#[test]
fn v1_profile_publication_and_tombstone_vectors_are_exact() {
    let signed_profile = SignedProfile::sign(&profile(), &keypair()).expect("sign profile");
    assert_eq!(to_hex(signed_profile.record().body_bytes()), PROFILE_BODY);
    assert_eq!(signed_profile.record().signature().to_hex(), PROFILE_SIGNATURE);
    assert_eq!(to_hex(&sha256(signed_profile.raw())), PROFILE_ENVELOPE_SHA256);

    let signed_publication = SignedPublication::sign(&publication(), &keypair()).expect("sign publication");
    assert_eq!(to_hex(signed_publication.record().body_bytes()), PUBLICATION_BODY);
    assert_eq!(signed_publication.record().signature().to_hex(), PUBLICATION_SIGNATURE);
    assert_eq!(to_hex(&sha256(signed_publication.raw())), PUBLICATION_ENVELOPE_SHA256);
    assert_eq!(to_hex(&signed_publication.publication_id()), PUBLICATION_ID);
    assert_eq!(
        signed_publication.publication_id(),
        publication_id(signed_publication.record().body_bytes())
    );

    let signed_tombstone = SignedTombstone::sign(&tombstone(), &keypair()).expect("sign tombstone");
    assert_eq!(to_hex(signed_tombstone.record().body_bytes()), TOMBSTONE_BODY);
    assert_eq!(signed_tombstone.record().signature().to_hex(), TOMBSTONE_SIGNATURE);
    assert_eq!(to_hex(&sha256(signed_tombstone.raw())), TOMBSTONE_ENVELOPE_SHA256);
}

#[test]
fn v1_manifest_vector_is_exact() {
    let manifest = manifest();
    let body = manifest.encode_body().expect("manifest");
    assert_eq!(to_hex(&body), VECTOR_MANIFEST_BODY);
    assert_eq!(to_hex(&manifest.root().expect("root")), VECTOR_MANIFEST_ROOT);
    assert_eq!(manifest.total_size, 2_500_000);
    assert_eq!(manifest.chunk_size, CHUNK_SIZE_V1);
    assert_eq!(manifest.chunk_count(), 3);
}

#[test]
fn v1_records_round_trip_without_re_encoding() {
    let signed = SignedProfile::sign(&profile(), &keypair()).expect("sign");
    let parsed = SignedProfile::parse(signed.raw()).expect("verify");
    assert_eq!(parsed.raw(), signed.raw());
    assert_eq!(parsed.record().body_bytes(), signed.record().body_bytes());

    let publication = SignedPublication::sign(&publication(), &keypair()).expect("sign");
    let parsed = SignedPublication::parse(publication.raw()).expect("verify");
    assert_eq!(parsed.raw(), publication.raw());
    assert_eq!(parsed.publication_id(), publication.publication_id());
}

#[test]
fn v1_rejects_tampering_and_domain_replay() {
    let signed = SignedProfile::sign(&profile(), &keypair()).expect("sign");
    let mut body = signed.record().body_bytes().to_vec();
    let position = body
        .windows(b"Ada Lovelace".len())
        .position(|window| window == b"Ada Lovelace")
        .expect("display name");
    body[position] = b'a';
    let tampered = SignedProfile::parse(&envelope(body, signed.record().signature().as_bytes()))
        .expect_err("changed body must fail");
    assert!(matches!(tampered, ProtocolError::VerificationFailed));

    let mut signature = *signed.record().signature().as_bytes();
    signature[0] ^= 0x01;
    let tampered = SignedProfile::parse(&envelope(signed.record().body_bytes().to_vec(), &signature))
        .expect_err("changed signature must fail");
    assert!(matches!(tampered, ProtocolError::VerificationFailed));

    signed
        .record()
        .verify(DomainTag::Publication, &keypair().public_key())
        .expect_err("domain replay must fail");
}

#[test]
fn v1_rejects_unknown_major_and_preserves_unknown_minor_fields() {
    let body = profile().encode_body().expect("body");
    let entries = decode_signed_record(&body)
        .expect("decode")
        .as_map()
        .expect("map")
        .to_vec();

    let mut minor = entries.clone();
    minor.push((Value::Unsigned(100), Value::Text("future".to_owned())));
    let minor = Value::Map(minor).to_canonical_bytes().expect("canonical");
    let parsed = SignedProfile::parse(&resign_body(minor, DomainTag::Profile)).expect("minor field");
    assert_eq!(parsed.profile().unknown_fields().len(), 1);
    assert_eq!(parsed.profile().unknown_fields()[0].0, 100);

    let mut major = entries;
    for entry in &mut major {
        if entry.0 == Value::Unsigned(1) {
            entry.1 = Value::Unsigned(PROFILE_SCHEMA_VERSION + 1);
        }
    }
    let major = Value::Map(major).to_canonical_bytes().expect("canonical");
    assert!(matches!(
        SignedProfile::parse(&resign_body(major, DomainTag::Profile)),
        Err(ProtocolError::UnsupportedVersion(2))
    ));
}

#[test]
fn v1_rejects_non_canonical_cbor_and_oversized_records() {
    let non_canonical = vec![0xa2, 0x02, 0x01, 0x01, 0x01];
    assert!(SignedProfile::parse(&resign_body(non_canonical, DomainTag::Profile)).is_err());

    let duplicate = vec![0xa2, 0x01, 0x01, 0x01, 0x02];
    assert!(SignedProfile::parse(&resign_body(duplicate, DomainTag::Profile)).is_err());

    let non_shortest = vec![0xa1, 0x18, 0x17, 0x00];
    assert!(SignedProfile::parse(&resign_body(non_shortest, DomainTag::Profile)).is_err());

    let mut trailing = profile().encode_body().expect("body");
    trailing.push(0x00);
    assert!(SignedProfile::parse(&resign_body(trailing, DomainTag::Profile)).is_err());

    assert!(SignedRecord::sign(vec![0_u8; 65_537], DomainTag::Profile, &keypair()).is_err());
    assert!(SignedRecord::parse(&vec![0_u8; 65_537]).is_err());
}

#[test]
fn v1_rejects_manifest_shape_errors() {
    let chunks = VECTOR_CHUNK_HASHES[..2]
        .iter()
        .map(|hash| ChunkEntry::new(1_048_576, hex32(hash)))
        .collect();
    let wrong_count = ManifestV1::new(hex32(VECTOR_CONTENT_ID), 2_500_000, chunks);
    assert!(wrong_count.encode_body().is_err());

    let wrong_size = ManifestV1::new(
        hex32(VECTOR_CONTENT_ID),
        1,
        vec![ChunkEntry::new(1_048_576, hex32(VECTOR_CHUNK_HASHES[0]))],
    );
    assert!(wrong_size.encode_body().is_err());
}
