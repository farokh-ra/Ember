//! Identity lifecycle and encrypted key storage.
//!
//! The identity keypair is generated locally, stored only in
//! `secrets/identity.enc` under the app-private root, and restored from a
//! passphrase-encrypted backup. No account service, central recovery, or
//! automatic key rotation exists in v1.

mod at_rest;
mod backup;
mod error;
mod identity;
mod local_key;
mod payload;
mod vault;

pub use at_rest::{
    AT_REST_FORMAT_VERSION, AT_REST_HEADER_LENGTH, AT_REST_KDF_HKDF_SHA256, AT_REST_MAGIC, AT_REST_MAX_FILE_LENGTH,
    AT_REST_SALT_LENGTH, decrypt_identity, encrypt_identity, encrypt_identity_with_material,
};
pub use backup::{
    BACKUP_ARGON2_VERSION, BACKUP_DEFAULT_ITERATIONS, BACKUP_DEFAULT_MEMORY_KIB, BACKUP_DEFAULT_PARALLELISM,
    BACKUP_FORMAT_VERSION, BACKUP_KDF_ARGON2ID, BACKUP_MAGIC, BACKUP_MAX_FILE_LENGTH, PASSPHRASE_MAX_BYTES,
    PASSPHRASE_MIN_CHARS, export_backup, export_backup_with_params, restore_backup, v1_params,
};
pub use error::IdentityError;
pub use identity::{
    CRYPTOGRAPHIC_IDENTITY_NOTICE, Identity, KEY_LIMITS_NOTICE, KEY_LOSS_WARNING, SIGNATURE_STATUS_LABEL,
};
pub use local_key::{LOCAL_ENCRYPTION_KEY_LENGTH, LocalEncryptionKey};
pub use payload::{PAYLOAD_KEY_TYPE_ED25519, PAYLOAD_LENGTH, PAYLOAD_MAGIC, PAYLOAD_VERSION};
pub use vault::{IDENTITY_FILE_NAME, IdentityVault, SECRETS_DIRECTORY};

/// Returns the architecture component name for diagnostics.
#[must_use]
pub const fn component_name() -> &'static str {
    "nexus-identity"
}
