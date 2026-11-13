//! Stage 06 identity acceptance tests: ID-001, ID-003 through ID-008.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nexus_crypto::{DomainTag, IdentityId, PublicKey, sha256, to_hex};
use nexus_identity::{
    AT_REST_HEADER_LENGTH, AT_REST_MAGIC, BACKUP_MAGIC, CRYPTOGRAPHIC_IDENTITY_NOTICE, Identity, IdentityVault,
    KEY_LIMITS_NOTICE, KEY_LOSS_WARNING, LocalEncryptionKey, SIGNATURE_STATUS_LABEL, decrypt_identity,
    encrypt_identity_with_material, export_backup_with_params, restore_backup, v1_params,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nexus-identity-test-{}-{sequence}", std::process::id()));
        fs::create_dir_all(&path).expect("temp root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fast_params() -> nexus_crypto::Argon2idParams {
    nexus_crypto::Argon2idParams::new(8, 1, 1)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn id_001_local_identity_has_a_stable_bech32m_id() {
    let identity = Identity::from_seed([0_u8; 32], 1_700_000_000_000);
    assert_eq!(
        identity.public_key().to_hex(),
        "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29"
    );
    let expected = "nx1a5qnk638hn8tdfpdv23635p2duxhxefjz4m3mcjr5cavqj9p3dva52g6l8d37";
    assert_eq!(identity.identity_id().as_str(), expected);

    let parsed = IdentityId::parse(expected).expect("parse committed vector");
    assert_eq!(parsed.as_str(), expected);
    assert_eq!(parsed.public_key().expect("embedded key"), identity.public_key());
    assert!(expected.starts_with("nx1"));
}

#[test]
fn id_001_identity_survives_a_restart_and_never_needs_a_server() {
    let root = TempRoot::new();
    let lek = LocalEncryptionKey::new([21_u8; 32]);
    let vault = IdentityVault::new(root.path()).expect("vault");
    let created = vault.create(&lek).expect("create");

    let restarted = IdentityVault::new(root.path()).expect("vault");
    let loaded = restarted.load(&lek).expect("load");
    assert_eq!(loaded.identity_id(), created.identity_id());
    assert_eq!(loaded.public_key(), created.public_key());
    assert!(restarted.create(&lek).is_err());
}

#[test]
fn id_003_plaintext_key_material_is_absent_from_storage_and_debug_output() {
    let root = TempRoot::new();
    let lek = LocalEncryptionKey::new([22_u8; 32]);
    let seed = [0x42_u8; 32];
    let identity = Identity::from_seed(seed, 1_234);
    let vault = IdentityVault::new(root.path()).expect("vault");
    vault.save(&lek, &identity).expect("save");

    let path = vault.identity_path();
    let file = fs::read(&path).expect("read identity file");
    assert_eq!(&file[..8], &AT_REST_MAGIC);
    assert!(file.len() > AT_REST_HEADER_LENGTH);
    assert!(!contains(&file, &seed));
    assert!(!contains(&file, identity.public_key().as_bytes()));

    let debug = format!("{identity:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("4242424242424242"));

    let leftover: Vec<_> = fs::read_dir(root.path().join("secrets"))
        .expect("secrets directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(leftover.is_empty(), "temporary key files must not remain");
}

#[test]
fn id_004_encrypted_backup_restores_the_same_identity_and_signatures() {
    let identity = Identity::from_seed([0x11_u8; 32], 1_700_000_000_000);
    let backup =
        export_backup_with_params(&identity, "correct horse battery staple", fast_params(), None).expect("export");

    assert_eq!(&backup[..8], &BACKUP_MAGIC);
    assert!(!contains(&backup, &[0x11_u8; 32]));
    assert!(!contains(&backup, b"correct horse battery staple"));

    let restored = restore_backup(&backup, "correct horse battery staple").expect("restore");
    assert_eq!(restored.identity_id(), identity.identity_id());
    assert_eq!(restored.created_at_ms(), identity.created_at_ms());

    let signature = restored.sign_domain(DomainTag::Profile, b"after restore");
    identity
        .public_key()
        .verify_domain(DomainTag::Profile, b"after restore", &signature)
        .expect("cross-device signature verifies");

    let error = restore_backup(&backup, "wrong horse battery staple").expect_err("wrong key");
    assert_eq!(
        error.to_string(),
        "backup could not be opened; the passphrase or file is invalid"
    );
}

#[test]
fn id_004_vault_backup_round_trips_with_v1_defaults() {
    let source_root = TempRoot::new();
    let source_vault = IdentityVault::new(source_root.path()).expect("vault");
    let source_lek = LocalEncryptionKey::new([23_u8; 32]);
    let identity = source_vault.create(&source_lek).expect("create");
    let backup = source_vault
        .export_backup(&identity, "correct horse battery staple")
        .expect("backup");

    let target_root = TempRoot::new();
    let target_vault = IdentityVault::new(target_root.path()).expect("vault");
    let target_lek = LocalEncryptionKey::new([24_u8; 32]);
    let restored = target_vault
        .restore_backup(&target_lek, "correct horse battery staple", &backup)
        .expect("restore");
    assert_eq!(restored.identity_id(), identity.identity_id());
    assert_eq!(
        target_vault.load(&target_lek).expect("load").identity_id(),
        identity.identity_id()
    );
}

#[test]
fn id_005_key_loss_and_v1_limits_are_stated_without_fake_recovery() {
    assert!(KEY_LOSS_WARNING.contains("Losing this identity key loses the identity"));
    assert!(KEY_LOSS_WARNING.contains("no central account, password reset, or recovery service"));
    assert!(KEY_LIMITS_NOTICE.contains("no automatic key rotation"));
    assert!(KEY_LIMITS_NOTICE.contains("central revocation"));
    assert!(KEY_LIMITS_NOTICE.contains("create and republish under a new identity"));
    assert!(CRYPTOGRAPHIC_IDENTITY_NOTICE.contains("does not prove a real name"));
    assert!(CRYPTOGRAPHIC_IDENTITY_NOTICE.contains("legal authorship"));
}

#[test]
fn id_006_unsupported_truncated_and_tampered_backups_do_not_replace_identity() {
    let root = TempRoot::new();
    let vault = IdentityVault::new(root.path()).expect("vault");
    let lek = LocalEncryptionKey::new([25_u8; 32]);
    let identity = vault.create(&lek).expect("create");
    let backup =
        export_backup_with_params(&identity, "correct horse battery staple", fast_params(), None).expect("export");

    let mut unsupported = backup.clone();
    unsupported[8..10].copy_from_slice(&2_u16.to_le_bytes());
    let mut truncated = backup.clone();
    truncated.truncate(backup.len() - 1);
    let mut tampered = backup.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;

    for candidate in [&unsupported, &truncated, &tampered] {
        assert!(
            vault
                .restore_backup(&lek, "correct horse battery staple", candidate)
                .is_err()
        );
        let loaded = vault.load(&lek).expect("existing identity survives");
        assert_eq!(loaded.identity_id(), identity.identity_id());
    }
}

#[test]
fn id_006_committed_v1_backup_vector_is_exact() {
    let identity = Identity::from_seed([0_u8; 32], 1_700_000_000_000);
    let backup = export_backup_with_params(
        &identity,
        "correct horse battery staple",
        v1_params(),
        Some(([0x33_u8; 16], [0x44_u8; 24])),
    )
    .expect("backup");

    assert_eq!(backup.len(), 233);
    assert_eq!(
        to_hex(&backup[..44]),
        concat!(
            "4e4558424b5550310100010013000000",
            "000001000300000001000000",
            "33333333333333333333333333333333"
        )
    );
    assert_eq!(
        to_hex(&sha256(&backup)),
        "d3cf4d6ba0e22e5e8c26276629c8c3f77899d05d1b1656eb78a3cb493c3febee"
    );
    let restored = restore_backup(&backup, "correct horse battery staple").expect("restore");
    assert_eq!(restored.identity_id(), identity.identity_id());
}

#[test]
fn deterministic_at_rest_vector_is_exact_and_round_trips() {
    let identity = Identity::from_seed([0_u8; 32], 1_700_000_000_000);
    let lek = LocalEncryptionKey::new([0x55_u8; 32]);
    let file = encrypt_identity_with_material(&identity, &lek, &[0x66_u8; 32], &[0x77_u8; 24]).expect("encrypt");

    assert_eq!(file.len(), 168);
    assert_eq!(
        to_hex(&file[..AT_REST_HEADER_LENGTH]),
        concat!(
            "4e455849444e543101000100",
            "6666666666666666666666666666666666666666666666666666666666666666",
            "777777777777777777777777777777777777777777777777"
        )
    );
    assert_eq!(
        to_hex(&sha256(&file)),
        "fcad93c2d40b8ec483c1c7c164e33f9228e40e0c21f5f147f09e4ff3f5cde84b"
    );
    let restored = decrypt_identity(&file, &lek).expect("decrypt");
    assert_eq!(restored.identity_id(), identity.identity_id());
    assert!(decrypt_identity(&file, &LocalEncryptionKey::new([0x56_u8; 32])).is_err());
}

#[test]
fn id_007_the_same_backup_on_two_devices_signs_verifiable_records() {
    let identity = Identity::from_seed([0x77_u8; 32], 2_000);
    let backup =
        export_backup_with_params(&identity, "correct horse battery staple", fast_params(), None).expect("backup");

    let device_a = restore_backup(&backup, "correct horse battery staple").expect("device a");
    let device_b = restore_backup(&backup, "correct horse battery staple").expect("device b");
    assert_eq!(device_a.identity_id(), device_b.identity_id());

    let record_a = device_a.sign_domain(DomainTag::Publication, b"from device a");
    let record_b = device_b.sign_domain(DomainTag::Publication, b"from device b");
    let public_key: PublicKey = IdentityId::parse(device_a.identity_id().as_str())
        .expect("id")
        .public_key()
        .expect("key");
    public_key
        .verify_domain(DomainTag::Publication, b"from device a", &record_a)
        .expect("device b verifies a");
    public_key
        .verify_domain(DomainTag::Publication, b"from device b", &record_b)
        .expect("device a verifies b");
    assert_ne!(record_a, record_b);
}

#[test]
fn id_008_status_label_does_not_claim_real_world_identity() {
    assert_eq!(SIGNATURE_STATUS_LABEL, "Signed record verified");
    assert!(!SIGNATURE_STATUS_LABEL.contains("identity verified"));
    assert!(!SIGNATURE_STATUS_LABEL.contains("authentic"));
    assert!(CRYPTOGRAPHIC_IDENTITY_NOTICE.contains("control of this identity key"));
    assert!(CRYPTOGRAPHIC_IDENTITY_NOTICE.contains("does not prove"));
}
