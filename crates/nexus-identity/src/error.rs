//! Typed identity and encrypted-storage errors.

use std::io;

use nexus_crypto::CryptoError;
use thiserror::Error;

/// Errors returned by identity creation, storage, backup, and restore.
#[derive(Debug, Error)]
pub enum IdentityError {
    /// A cryptographic primitive failed.
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    /// A filesystem operation failed.
    #[error("identity storage I/O failed: {0}")]
    Io(#[from] io::Error),
    /// An identity already exists at the requested root.
    #[error("an identity already exists")]
    AlreadyExists,
    /// No identity exists at the requested root.
    #[error("no identity exists")]
    NotFound,
    /// The identity file is truncated or has an invalid header.
    #[error("identity file is invalid")]
    InvalidIdentityFile,
    /// The identity file uses an unknown format version or KDF.
    #[error("identity file format version is not supported")]
    UnsupportedFormat,
    /// The identity file could not be decrypted with the supplied LEK.
    #[error("identity decryption failed")]
    DecryptionFailed,
    /// A root, secrets path, or storage path is not a safe absolute directory.
    #[error("identity path is not a safe absolute directory path")]
    UnsafePath,
    /// A symlink or reparse point was found on the identity path.
    #[error("symlinked identity path is not allowed")]
    SymlinkRejected,
    /// The backup uses an unknown format, KDF, or Argon2 version.
    #[error("backup format version is not supported")]
    UnsupportedBackupVersion,
    /// The backup is truncated or structurally invalid.
    #[error("backup is malformed or truncated")]
    MalformedBackup,
    /// The backup could not be authenticated or decrypted.
    ///
    /// This is deliberately one generic error for a wrong passphrase,
    /// tampered ciphertext, or an inconsistent authenticated header.
    #[error("backup could not be opened; the passphrase or file is invalid")]
    BackupUnreadable,
    /// A new backup passphrase was shorter than the product minimum.
    #[error("passphrase must be at least {min} characters")]
    WeakPassphrase {
        /// Minimum number of Unicode scalar values.
        min: usize,
    },
    /// The supplied passphrase was empty.
    #[error("passphrase must not be empty")]
    EmptyPassphrase,
    /// The supplied passphrase exceeded the protocol limit.
    #[error("passphrase exceeds 1024 UTF-8 bytes")]
    PassphraseTooLong,
    /// The decrypted identity payload was inconsistent.
    #[error("identity metadata is invalid")]
    InvalidMetadata,
}
