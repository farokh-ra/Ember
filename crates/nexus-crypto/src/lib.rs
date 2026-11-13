//! Cryptographic primitives and canonical encoding.
//!
//! This crate owns the byte-level formats that both platforms must reproduce
//! exactly: canonical CBOR, Ed25519 signing and strict verification, Bech32m
//! `nx1...` identity IDs, domain-separated hashing, XChaCha20-Poly1305, and
//! HKDF/Argon2id key derivation. It never touches the network, SQLite, or UI.

mod aead;
mod cbor;
mod domain;
mod error;
mod hash;
mod identity_id;
mod kdf;
mod key;
mod random;
mod text;

pub use aead::{AEAD_KEY_LENGTH, AEAD_NONCE_LENGTH, AEAD_TAG_LENGTH, decrypt, encrypt};
pub use cbor::{CborLimits, Value, decode, decode_signed_record};
pub use domain::{
    BACKUP, DHT_PROFILE, DHT_PUBLICATION, DHT_TOMBSTONE, DomainTag, IDENTITY_KEY, MANIFEST, PROFILE, PUBLICATION,
    PUBLICATION_ID, TOMBSTONE, signing_message,
};
pub use error::{CborError, CryptoError};
pub use hash::{domain_sha256, sha256, to_hex};
pub use identity_id::{ED25519_PUBLIC_KEY_MULTICODEC, IDENTITY_ID_HRP, IDENTITY_ID_PAYLOAD_LENGTH, IdentityId};
pub use kdf::{
    ARGON2_SALT_LENGTH, Argon2idParams, DERIVED_KEY_LENGTH, MAX_ARGON2_ITERATIONS, MAX_ARGON2_MEMORY_KIB,
    MAX_ARGON2_PARALLELISM, argon2id_key, hkdf_sha256, hkdf_sha256_32,
};
pub use key::{KeyPair, PUBLIC_KEY_LENGTH, PublicKey, SECRET_KEY_LENGTH, SIGNATURE_LENGTH, SignatureBytes};
pub use random::{fill_random, random_array};
pub use text::{normalize_nfc, normalize_text_bytes, normalize_text_chars};

/// Returns the architecture component name for diagnostics.
#[must_use]
pub const fn component_name() -> &'static str {
    "nexus-crypto"
}
