//! Typed errors for storage and managed-store operations.

use std::path::PathBuf;

/// Result type used throughout `nexus-store`.
pub type StoreResult<T> = Result<T, StoreError>;

/// A storage, database, or managed-store failure.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A filesystem operation failed.
    #[error("storage I/O failed: {0}")]
    Io(#[from] std::io::Error),

    /// SQLite reported an error.
    #[error("database operation failed: {0}")]
    Database(#[from] rusqlite::Error),

    /// The dedicated writer thread ended before answering a request.
    #[error("database writer stopped unexpectedly")]
    DatabaseWorkerStopped,

    /// A migration failed and its transaction was rolled back.
    #[error("migration {version} ({name}) failed and was rolled back: {message}")]
    Migration {
        /// Migration version that failed.
        version: i64,
        /// Human-readable migration name.
        name: &'static str,
        /// Underlying failure message.
        message: String,
    },

    /// The database schema is newer or internally inconsistent.
    #[error("unsupported database schema: {0}")]
    UnsupportedSchema(String),

    /// The store root, database path, or export path is not safe.
    #[error("unsafe storage path: {0}")]
    UnsafePath(String),

    /// A content ID is not exactly 64 lowercase hexadecimal characters.
    #[error("invalid content ID: {0}")]
    InvalidContentId(String),

    /// A download, publication, replica, or peer identifier is malformed.
    #[error("invalid identifier: {0}")]
    InvalidId(String),

    /// A filename cannot be represented safely.
    #[error("invalid filename: {0}")]
    InvalidFilename(String),

    /// A file exceeds the configured maximum file size.
    #[error("file is {actual} bytes, above the {limit}-byte limit")]
    FileTooLarge {
        /// Actual or expected file size.
        actual: u64,
        /// Configured maximum.
        limit: u64,
    },

    /// The managed quota would be exceeded.
    #[error("managed quota would be exceeded: {used} bytes used, {incoming} bytes requested, {quota} byte quota")]
    QuotaExceeded {
        /// Bytes already counted against the quota.
        used: u64,
        /// Additional bytes requested.
        incoming: u64,
        /// Configured managed quota.
        quota: u64,
    },

    /// The free-space reserve would be consumed.
    #[error(
        "free-space reserve would be consumed: {available} bytes available, {incoming} bytes requested, {reserve} byte reserve"
    )]
    FreeSpaceReserve {
        /// Bytes currently available on the volume.
        available: u64,
        /// Additional bytes requested.
        incoming: u64,
        /// Configured reserve.
        reserve: u64,
    },

    /// A referenced content blob is absent or unreadable.
    #[error("content {0} is missing from the managed store")]
    ContentMissing(String),

    /// Stored bytes did not match their content ID or expected size.
    #[error("content {0} failed integrity verification")]
    IntegrityMismatch(String),

    /// A destructive operation requires explicit user confirmation.
    #[error("explicit confirmation is required to remove {0}")]
    ConfirmationRequired(&'static str),

    /// An export or restore destination already exists.
    #[error("destination already exists: {0}")]
    DestinationExists(PathBuf),

    /// Another core instance holds the managed-store lock.
    #[error("managed store is already in use by another process")]
    StoreLocked,

    /// A metadata archive is truncated, malformed, or fails integrity checks.
    #[error("metadata archive is corrupt: {0}")]
    CorruptArchive(String),

    /// A metadata archive has a newer or unsupported version.
    #[error("metadata archive is incompatible: {0}")]
    IncompatibleArchive(String),

    /// CBOR encoding or decoding failed.
    #[error("metadata archive encoding failed: {0}")]
    Serialization(String),

    /// The store configuration is invalid.
    #[error("invalid store configuration: {0}")]
    InvalidConfig(String),

    /// A setting or command is not valid in this build.
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}
