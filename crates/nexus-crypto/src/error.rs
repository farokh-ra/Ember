//! Typed cryptographic and encoding errors.

use thiserror::Error;

/// Errors returned by the cryptographic primitives.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CryptoError {
    /// The identity ID is not a canonical `nx1...` Bech32m identity ID.
    #[error("invalid identity ID")]
    InvalidIdentityId,
    /// The public key is not a valid Ed25519 verifying key.
    #[error("invalid public key")]
    InvalidPublicKey,
    /// The signature is not exactly 64 bytes.
    #[error("invalid signature")]
    InvalidSignature,
    /// The signature did not verify for the supplied key, domain, and message.
    #[error("signature verification failed")]
    SignatureVerification,
    /// The supplied secret key material is invalid.
    #[error("invalid key material")]
    InvalidKeyMaterial,
    /// The operating-system random number generator failed.
    #[error("secure randomness is unavailable")]
    Randomness,
    /// Authenticated encryption failed.
    #[error("encryption failed")]
    Encryption,
    /// Authenticated decryption failed.
    #[error("decryption failed")]
    Decryption,
    /// Key derivation failed.
    #[error("key derivation failed")]
    KeyDerivation,
    /// Text exceeded the character limit for the field.
    #[error("text exceeds {max} characters")]
    TextTooManyChars {
        /// Maximum permitted number of Unicode scalar values.
        max: usize,
    },
    /// Text exceeded the UTF-8 byte limit for the field.
    #[error("text exceeds {max} bytes")]
    TextTooLong {
        /// Maximum permitted UTF-8 byte length.
        max: usize,
    },
    /// A canonical CBOR operation failed.
    #[error(transparent)]
    Cbor(#[from] CborError),
}

/// Errors returned by the canonical CBOR profile.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CborError {
    /// The input ended in the middle of a CBOR item.
    #[error("unexpected end of CBOR input")]
    UnexpectedEnd,
    /// Bytes remained after the top-level CBOR value.
    #[error("trailing bytes after CBOR value")]
    TrailingBytes,
    /// The CBOR type is outside the signed-record profile.
    #[error("unsupported CBOR type: {0}")]
    UnsupportedType(&'static str),
    /// An indefinite-length item was encountered.
    #[error("indefinite-length CBOR is not allowed")]
    IndefiniteLength,
    /// An integer used a longer encoding than necessary.
    #[error("non-shortest CBOR integer")]
    NonShortestInteger,
    /// A CBOR integer was outside the supported range.
    #[error("CBOR integer is out of range")]
    IntegerOutOfRange,
    /// The same map key appeared more than once.
    #[error("duplicate CBOR map key")]
    DuplicateKey,
    /// The input was valid CBOR but not in the canonical profile.
    #[error("CBOR value is not canonical")]
    NonCanonical,
    /// A text string was not valid UTF-8.
    #[error("invalid UTF-8 text")]
    InvalidUtf8,
    /// A map key was an array or map.
    #[error("CBOR map key must be a scalar value")]
    UnsupportedMapKey,
    /// Nesting exceeded the configured bound.
    #[error("CBOR nesting depth exceeds {max}")]
    DepthExceeded {
        /// Maximum permitted nesting depth.
        max: usize,
    },
    /// The number of parsed items exceeded the configured bound.
    #[error("CBOR item count exceeds {max}")]
    ItemsExceeded {
        /// Maximum permitted item count.
        max: usize,
    },
    /// The input exceeded the configured bound.
    #[error("CBOR input exceeds {max} bytes")]
    InputTooLarge {
        /// Maximum permitted input length.
        max: usize,
    },
}
