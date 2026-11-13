//! Typed signed-record errors.

use nexus_crypto::{CborError, CryptoError};
use thiserror::Error;

/// Errors returned while parsing or verifying signed records.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// Canonical CBOR parsing failed.
    #[error(transparent)]
    Cbor(#[from] CborError),
    /// A cryptographic primitive failed.
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    /// The complete signed envelope exceeded 64 KiB.
    #[error("signed record exceeds 64 KiB")]
    RecordTooLarge,
    /// The signed envelope did not have exactly the `{1: body, 2: sig}` shape.
    #[error("signed record envelope is malformed")]
    InvalidEnvelope,
    /// The record schema major version is unknown.
    #[error("unsupported record version {0}")]
    UnsupportedVersion(u64),
    /// A required field was absent.
    #[error("missing required field {0}")]
    MissingField(u64),
    /// A field had an invalid type or value.
    #[error("invalid field {0}")]
    InvalidField(u64),
    /// A map key was not a non-negative integer.
    #[error("record field keys must be unsigned integers")]
    InvalidFieldKey,
    /// The identity ID did not match the signing key.
    #[error("identity ID does not match the signing key")]
    IdentityKeyMismatch,
    /// The signature did not verify over the exact signed body bytes.
    #[error("signature verification failed")]
    VerificationFailed,
}
