//! Stage 06 signed-record acceptance tests: ID-002, ID-007, and format rules.

use nexus_crypto::{CborLimits, DomainTag, IdentityId, KeyPair, Value, decode_signed_record, sha256, to_hex};
use nexus_protocol::{
    Category, MAX_FILE_SIZE, MAX_TAG_BYTES, MAX_TAGS, PROFILE_SCHEMA_VERSION, ProtocolError, PublicationV1,
    SignedProfile, SignedPublication, SignedRecord, publication_id,
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

fn keypair() -> KeyPair {
    KeyPair::from_seed([0_u8; 32])
}

fn identity_id() -> IdentityId {
    IdentityId::from_public_key(&keypair().public_key())
}

fn profile() -> nexus_protocol::ProfileV1 {
    nexus_protocol::ProfileV1::new(identity_id(), "Ada Lovelace", "Local-first notes", 1, 1_700_000_000_000)
        .with_username("ada")
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

fn cbor_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::new();
    if bytes.len() < 24 {
        encoded.push(0x40 | u8::try_from(bytes.len()).expect("short"));
    } else if bytes.len() <= 0xff {
        encoded.push(0x58);
        encoded.push(u8::try_from(bytes.len()).expect("byte"));
    } else {
        encoded.push(0x59);
        encoded.extend_from_slice(&u16::try_from(bytes.len()).expect("u16").to_be_bytes());
    }
    encoded.extend_from_slice(bytes);
    encoded
}

#[test]
fn committed_profile_and_publication_vectors_are_exact() {
    let signed_profile = SignedProfile::sign(&profile(), &keypair()).expect("sign profile");
    assert_eq!(to_hex(signed_profile.record().body_bytes()), PROFILE_BODY);
    assert_eq!(signed_profile.record().signature().to_hex(), PROFILE_SIGNATURE);
    assert_eq!(to_hex(&sha256(signed_profile.raw())), PROFILE_ENVELOPE_SHA256);
    let parsed_profile = SignedProfile::parse(signed_profile.raw()).expect("verify profile");
    assert_eq!(parsed_profile.raw(), signed_profile.raw());
    assert_eq!(parsed_profile.profile(), signed_profile.profile());

    let signed_publication = SignedPublication::sign(&publication(), &keypair()).expect("sign publication");
    assert_eq!(to_hex(signed_publication.record().body_bytes()), PUBLICATION_BODY);
    assert_eq!(signed_publication.record().signature().to_hex(), PUBLICATION_SIGNATURE);
    assert_eq!(to_hex(&sha256(signed_publication.raw())), PUBLICATION_ENVELOPE_SHA256);
    assert_eq!(to_hex(&signed_publication.publication_id()), PUBLICATION_ID);
    assert_eq!(
        signed_publication.publication_id(),
        publication_id(signed_publication.record().body_bytes())
    );
    let parsed_publication = SignedPublication::parse(signed_publication.raw()).expect("verify publication");
    assert_eq!(parsed_publication.raw(), signed_publication.raw());
    assert_eq!(parsed_publication.publication_id(), signed_publication.publication_id());

    let canonical = Value::Map(vec![
        (Value::Unsigned(1), Value::Unsigned(1)),
        (Value::Unsigned(2), Value::Text("nx".to_owned())),
    ])
    .to_canonical_bytes()
    .expect("canonical");
    assert_eq!(to_hex(&canonical), "a2010102626e78");
    assert_eq!(
        decode_signed_record(&canonical).expect("decode"),
        Value::Map(vec![
            (Value::Unsigned(1), Value::Unsigned(1)),
            (Value::Unsigned(2), Value::Text("nx".to_owned())),
        ])
    );
}

#[test]
fn id_002_any_changed_signed_field_or_signature_fails_verification() {
    let signed = SignedProfile::sign(&profile(), &keypair()).expect("sign");
    let body = signed.record().body_bytes().to_vec();
    let position = body
        .windows(b"Ada Lovelace".len())
        .position(|window| window == b"Ada Lovelace")
        .expect("display name");
    let mut changed = body;
    changed[position] = b'a';
    let tampered = SignedProfile::parse(&envelope(changed, signed.record().signature().as_bytes()))
        .expect_err("changed body must fail");
    assert!(matches!(tampered, ProtocolError::VerificationFailed));

    let mut signature = *signed.record().signature().as_bytes();
    signature[63] ^= 0x01;
    let tampered = SignedProfile::parse(&envelope(signed.record().body_bytes().to_vec(), &signature))
        .expect_err("changed signature must fail");
    assert!(matches!(tampered, ProtocolError::VerificationFailed));

    signed
        .record()
        .verify(DomainTag::Publication, &keypair().public_key())
        .expect_err("domain replay must fail");
}

#[test]
fn identity_mismatch_and_cross_identity_verification_are_rejected() {
    let other = KeyPair::from_seed([1_u8; 32]);
    assert!(matches!(
        SignedProfile::sign(&profile(), &other),
        Err(ProtocolError::IdentityKeyMismatch)
    ));
    assert!(matches!(
        SignedPublication::sign(&publication(), &other),
        Err(ProtocolError::IdentityKeyMismatch)
    ));

    let signed = SignedProfile::sign(&profile(), &keypair()).expect("sign");
    let other_public = IdentityId::from_public_key(&other.public_key());
    signed
        .record()
        .verify(DomainTag::Profile, &other_public.public_key().expect("key"))
        .expect_err("different key must fail");
}

#[test]
fn unknown_minor_fields_are_preserved_and_unknown_major_versions_are_rejected() {
    let body = profile().encode_body().expect("body");
    let mut entries = decode_signed_record(&body)
        .expect("decode")
        .as_map()
        .expect("map")
        .to_vec();
    entries.push((Value::Unsigned(100), Value::Text("future".to_owned())));
    let with_minor = Value::Map(entries).to_canonical_bytes().expect("canonical");
    let signed = SignedProfile::parse(&resign_body(with_minor, DomainTag::Profile)).expect("minor field is valid");
    assert_eq!(signed.profile().unknown_fields().len(), 1);
    assert_eq!(signed.profile().unknown_fields()[0].0, 100);

    let mut entries = decode_signed_record(&body)
        .expect("decode")
        .as_map()
        .expect("map")
        .to_vec();
    for entry in &mut entries {
        if entry.0 == Value::Unsigned(1) {
            entry.1 = Value::Unsigned(PROFILE_SCHEMA_VERSION + 1);
        }
    }
    let with_major = Value::Map(entries).to_canonical_bytes().expect("canonical");
    assert!(matches!(
        SignedProfile::parse(&resign_body(with_major, DomainTag::Profile)),
        Err(ProtocolError::UnsupportedVersion(2))
    ));
}

#[test]
fn non_canonical_bodies_duplicate_keys_and_trailing_bytes_are_rejected() {
    // Map keys out of canonical order: 2 before 1.
    let non_canonical = vec![0xa2, 0x02, 0x01, 0x01, 0x01];
    assert!(SignedProfile::parse(&resign_body(non_canonical, DomainTag::Profile)).is_err());

    // Duplicate key 1.
    let duplicate = vec![0xa2, 0x01, 0x01, 0x01, 0x02];
    assert!(SignedProfile::parse(&resign_body(duplicate, DomainTag::Profile)).is_err());

    // Non-shortest integer encoding of 23.
    let non_shortest = vec![0xa1, 0x18, 0x17, 0x00];
    assert!(SignedProfile::parse(&resign_body(non_shortest, DomainTag::Profile)).is_err());

    // Trailing bytes after a complete body.
    let mut trailing = profile().encode_body().expect("body");
    trailing.push(0x00);
    assert!(SignedProfile::parse(&resign_body(trailing, DomainTag::Profile)).is_err());

    // The envelope itself must also be canonical: put key 2 before key 1.
    let signed = SignedProfile::sign(&profile(), &keypair()).expect("sign");
    let mut reordered = vec![0xa2, 0x02];
    reordered.extend_from_slice(&cbor_bytes(signed.record().signature().as_bytes()));
    reordered.push(0x01);
    reordered.extend_from_slice(&cbor_bytes(signed.record().body_bytes()));
    assert!(SignedRecord::parse(&reordered).is_err());
}

#[test]
fn cbor_depth_items_and_input_bounds_are_enforced() {
    let mut nested = vec![0x81; 17];
    nested.push(0x00);
    assert!(decode_signed_record(&nested).is_err());

    assert!(SignedRecord::sign(vec![0u8; 65_537], DomainTag::Profile, &keypair()).is_err());
    assert!(SignedRecord::parse(&vec![0u8; 65_537]).is_err());
}

#[test]
fn user_text_is_nfc_on_sign_and_must_be_nfc_on_parse() {
    let mut normalized = profile();
    normalized.display_name = "Cafe\u{301}".to_owned();
    let signed = SignedProfile::sign(&normalized, &keypair()).expect("sign");
    assert_eq!(signed.profile().display_name, "Caf\u{e9}");
    SignedProfile::parse(signed.raw()).expect("verify normalized body");
    assert!(!contains(signed.record().body_bytes(), "Cafe\u{301}".as_bytes()));

    let mut body = profile().encode_body().expect("body");
    let entries = decode_signed_record(&body)
        .expect("decode")
        .as_map()
        .expect("map")
        .to_vec();
    let mut rewritten = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        if key == Value::Unsigned(3) {
            rewritten.push((key, Value::Text("Cafe\u{301}".to_owned())));
        } else {
            rewritten.push((key, value));
        }
    }
    body = Value::Map(rewritten).to_canonical_bytes().expect("canonical");
    assert!(matches!(
        SignedProfile::parse(&resign_body(body, DomainTag::Profile)),
        Err(ProtocolError::InvalidField(3))
    ));

    let mut changed_publication = publication();
    changed_publication.display_filename = "re\u{301}sume\u{301}.txt".to_owned();
    let signed = SignedPublication::sign(&changed_publication, &keypair()).expect("sign");
    assert_eq!(signed.publication().display_filename, "r\u{e9}sum\u{e9}.txt");
}

#[test]
fn publication_and_profile_bounds_are_enforced() {
    let mut bad = profile();
    bad.display_name = "x".repeat(65);
    assert!(SignedProfile::sign(&bad, &keypair()).is_err());
    let mut bad = profile();
    bad.username = Some("ab".to_owned());
    assert!(SignedProfile::sign(&bad, &keypair()).is_err());

    let mut bad = publication();
    bad.size = 0;
    assert!(SignedPublication::sign(&bad, &keypair()).is_err());
    let mut bad = publication();
    bad.size = MAX_FILE_SIZE + 1;
    assert!(SignedPublication::sign(&bad, &keypair()).is_err());
    let mut bad = publication();
    bad.chunk_size = 1;
    assert!(SignedPublication::sign(&bad, &keypair()).is_err());
    let mut bad = publication();
    bad.revision = 0;
    assert!(SignedPublication::sign(&bad, &keypair()).is_err());
    let mut bad = publication();
    bad.tags = vec!["x".repeat(MAX_TAG_BYTES + 1)];
    assert!(SignedPublication::sign(&bad, &keypair()).is_err());
    let mut bad = publication();
    bad.tags = vec!["tag".to_owned(); MAX_TAGS + 1];
    assert!(SignedPublication::sign(&bad, &keypair()).is_err());
    let mut bad = publication();
    bad.display_filename = String::new();
    assert!(SignedPublication::sign(&bad, &keypair()).is_err());
}

#[test]
fn id_007_equal_revision_conflicts_remain_distinct_and_verifiable() {
    let first = SignedProfile::sign(&profile(), &keypair()).expect("first");
    let mut conflicting_profile = profile();
    conflicting_profile.bio = "A different local view".to_owned();
    let second = SignedProfile::sign(&conflicting_profile, &keypair()).expect("second");

    assert_eq!(first.profile().revision, second.profile().revision);
    assert_ne!(first.raw(), second.raw());
    assert_ne!(first.record().body_bytes(), second.record().body_bytes());
    SignedProfile::parse(first.raw()).expect("first still verifies");
    SignedProfile::parse(second.raw()).expect("second still verifies");

    let first_publication = SignedPublication::sign(&publication(), &keypair()).expect("first");
    let mut changed = publication();
    changed.description = "A different description".to_owned();
    let second_publication = SignedPublication::sign(&changed, &keypair()).expect("second");
    assert_ne!(first_publication.publication_id(), second_publication.publication_id());
    SignedPublication::parse(first_publication.raw()).expect("first publication verifies");
    SignedPublication::parse(second_publication.raw()).expect("second publication verifies");
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}
