//! Stable domain errors and their ABI-level classification.

use nexus_store::StoreError;
use nexus_transfer::TransferError;

use crate::config::ConfigError;
use crate::envelope::EnvelopeError;
use crate::streaming::ImportError;

/// Stable domain error codes carried in CBOR error envelopes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainCode {
    /// Malformed field, bad length, or unsupported category.
    InvalidArgument,
    /// Missing record, content, download, or peer.
    NotFound,
    /// Conflicting revisions, duplicate operation, or invalid transition.
    Conflict,
    /// Managed quota or free-space reserve would be exceeded.
    QuotaExceeded,
    /// Signature, manifest, chunk, or content mismatch.
    VerificationFailed,
    /// Unknown protocol, record, or manifest major version.
    UnsupportedVersion,
    /// No path, bootstrap, or source is available.
    NetworkUnavailable,
    /// A bounded deadline expired.
    Timeout,
    /// User or lifecycle cancellation.
    Cancelled,
    /// Local file, database, or export failure.
    IoError,
    /// Missing platform permission or inaccessible picker result.
    PermissionDenied,
    /// Identity is locked because the LEK or identity is unavailable.
    Locked,
    /// Guarded core fault.
    Internal,
}

impl DomainCode {
    /// Stable wire code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArgument => "invalid_argument",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::QuotaExceeded => "quota_exceeded",
            Self::VerificationFailed => "verification_failed",
            Self::UnsupportedVersion => "unsupported_version",
            Self::NetworkUnavailable => "network_unavailable",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::IoError => "io_error",
            Self::PermissionDenied => "permission_denied",
            Self::Locked => "locked",
            Self::Internal => "internal",
        }
    }

    /// Parses a wire code.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "invalid_argument" => Self::InvalidArgument,
            "not_found" => Self::NotFound,
            "conflict" => Self::Conflict,
            "quota_exceeded" => Self::QuotaExceeded,
            "verification_failed" => Self::VerificationFailed,
            "unsupported_version" => Self::UnsupportedVersion,
            "network_unavailable" => Self::NetworkUnavailable,
            "timeout" => Self::Timeout,
            "cancelled" => Self::Cancelled,
            "io_error" => Self::IoError,
            "permission_denied" => Self::PermissionDenied,
            "locked" => Self::Locked,
            "internal" => Self::Internal,
            _ => return None,
        })
    }

    /// Whether an automatic retry can reasonably succeed.
    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::NetworkUnavailable | Self::Timeout | Self::Cancelled)
    }
}

/// A user-facing domain failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainError {
    /// Stable code.
    pub code: DomainCode,
    /// Redacted, user-readable message.
    pub message: String,
    /// Whether an automatic retry can reasonably succeed.
    pub retryable: bool,
    /// Optional bounded structured details.
    pub details: Option<nexus_crypto::Value>,
}

impl DomainError {
    /// Creates an error using the code's retry policy.
    #[must_use]
    pub fn new(code: DomainCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: code.retryable(),
            details: None,
        }
    }

    /// Adds bounded structured details.
    #[must_use]
    pub fn with_details(mut self, details: nexus_crypto::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Creates an invalid-argument error.
    #[must_use]
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(DomainCode::InvalidArgument, message)
    }

    /// Creates an internal error with a redacted message.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(DomainCode::Internal, message)
    }
}

/// Errors that prevent a normal method response.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// The config CBOR is invalid.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// A caller supplied an invalid argument outside a CBOR envelope.
    #[error("invalid argument: {0}")]
    InvalidArgument(&'static str),
    /// A local I/O operation failed outside a storage method.
    #[error("local I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// A request or response envelope is invalid.
    #[error(transparent)]
    Envelope(#[from] EnvelopeError),
    /// A storage operation failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A streaming operation failed.
    #[error(transparent)]
    Import(#[from] ImportError),
    /// An identity operation failed.
    #[error("identity operation failed: {0}")]
    Identity(#[from] nexus_identity::IdentityError),
    /// The method ID is not known to this build.
    #[error("method 0x{0:04X} is not available in this build")]
    Unsupported(u32),
    /// The core is not in a state that accepts the operation.
    #[error("the core is not running")]
    NotRunning,
    /// A guarded internal failure with a redacted message.
    #[error("internal core failure: {0}")]
    Internal(String),
    /// An event queue operation failed.
    #[error(transparent)]
    Event(#[from] crate::event::EventError),
}

impl From<CoreError> for DomainError {
    fn from(error: CoreError) -> Self {
        Self::from_core(&error)
    }
}

impl DomainError {
    /// Maps a core or storage failure to a redacted domain error.
    #[must_use]
    pub fn from_core(error: &CoreError) -> Self {
        match error {
            CoreError::Config(config) => Self::from_config(config),
            CoreError::InvalidArgument(_) => Self::invalid("A required value is invalid."),
            CoreError::Io(_) => Self::new(DomainCode::IoError, "A local file operation failed."),
            CoreError::Envelope(envelope) => Self::invalid(envelope.to_string()),
            CoreError::Store(store) => Self::from_store(store),
            CoreError::Import(import) => Self::from_import(import),
            CoreError::Identity(identity) => match identity {
                nexus_identity::IdentityError::NotFound => {
                    Self::new(DomainCode::Locked, "No local identity is available.")
                }
                nexus_identity::IdentityError::DecryptionFailed => {
                    Self::new(DomainCode::Locked, "The local identity could not be unlocked.")
                }
                nexus_identity::IdentityError::UnsupportedFormat
                | nexus_identity::IdentityError::UnsupportedBackupVersion => Self::new(
                    DomainCode::UnsupportedVersion,
                    "The identity format is not supported by this build.",
                ),
                _ => Self::new(DomainCode::IoError, "The local identity could not be read."),
            },
            CoreError::Unsupported(_) => Self::invalid("The operation is not available in this build."),
            CoreError::NotRunning => Self::new(DomainCode::Conflict, "The core is not running."),
            CoreError::Internal(_) => Self::internal("The core could not complete the operation."),
            CoreError::Event(_) => Self::internal("The event queue is unavailable."),
        }
    }

    fn from_config(error: &ConfigError) -> Self {
        match error {
            ConfigError::AbiMismatch { .. } => {
                Self::new(DomainCode::UnsupportedVersion, "The core ABI version does not match.")
            }
            ConfigError::TooLarge { .. } => Self::invalid("The core configuration exceeds the 64 KiB limit."),
            _ => Self::invalid("The core configuration is invalid."),
        }
    }

    fn from_import(error: &ImportError) -> Self {
        match error {
            ImportError::SizeMismatch { .. } | ImportError::SizeExceeded { .. } => {
                Self::invalid("The imported file size does not match the declared size.")
            }
            ImportError::Empty => Self::invalid("Empty files are not supported in this version."),
            ImportError::ChunkTooLarge => Self::invalid("Import chunks must not exceed 1 MiB."),
            ImportError::Manifest(_) => {
                Self::new(DomainCode::VerificationFailed, "The file manifest could not be built.")
            }
            ImportError::Io(_) => Self::new(DomainCode::IoError, "The local file operation failed."),
        }
    }

    fn from_store(error: &StoreError) -> Self {
        match error {
            StoreError::Io(_) | StoreError::Database(_) | StoreError::DatabaseWorkerStopped => {
                Self::new(DomainCode::IoError, "A local storage operation failed.")
            }
            StoreError::Migration { .. } | StoreError::CorruptArchive(_) | StoreError::Serialization(_) => {
                Self::internal("Local data could not be opened safely.")
            }
            StoreError::UnsupportedSchema(_) | StoreError::IncompatibleArchive(_) => Self::new(
                DomainCode::UnsupportedVersion,
                "The local data is newer than this build supports.",
            ),
            StoreError::UnsafePath(_) | StoreError::InvalidConfig(_) => {
                Self::invalid("The supplied path or configuration is not allowed.")
            }
            StoreError::InvalidContentId(_) => Self::invalid("The content identifier is invalid."),
            StoreError::InvalidId(_) => Self::invalid("A record identifier is invalid."),
            StoreError::InvalidFilename(_) => Self::invalid("The display name is not valid."),
            StoreError::FileTooLarge { .. } => Self::invalid("The file exceeds the 4 GiB limit."),
            StoreError::QuotaExceeded { .. } => {
                Self::new(DomainCode::QuotaExceeded, "Not enough managed storage for this file.")
            }
            StoreError::FreeSpaceReserve { .. } => Self::new(
                DomainCode::QuotaExceeded,
                "The device does not have enough free space for this file.",
            ),
            StoreError::ContentMissing(_) => {
                Self::new(DomainCode::NotFound, "The requested content is not available locally.")
            }
            StoreError::IntegrityMismatch(_) => Self::new(
                DomainCode::VerificationFailed,
                "The content failed integrity verification.",
            ),
            StoreError::RecordConflict(_) => Self::new(DomainCode::Conflict, "The record conflicts with stored data."),
            StoreError::RecordNotFound(_) => Self::new(DomainCode::NotFound, "The requested record was not found."),
            StoreError::ManifestMissing(_) => Self::new(DomainCode::NotFound, "The manifest is not available locally."),
            StoreError::InvalidManifest(_) => Self::new(
                DomainCode::VerificationFailed,
                "The manifest failed integrity verification.",
            ),
            StoreError::InvalidRecord(_) => Self::new(DomainCode::VerificationFailed, "The signed record is invalid."),
            StoreError::ConfirmationRequired(_) => {
                Self::new(DomainCode::Conflict, "This action requires explicit confirmation.")
            }
            StoreError::DestinationExists(_) => Self::new(DomainCode::Conflict, "The destination already exists."),
            StoreError::StoreLocked => Self::new(DomainCode::Conflict, "The local store is already in use."),
            StoreError::Unsupported(_) => Self::invalid("The operation is not available in this build."),
        }
    }
}

impl From<TransferError> for DomainError {
    fn from(error: TransferError) -> Self {
        match error {
            TransferError::EmptyFile => Self::invalid("Empty files are not supported in this version."),
            TransferError::FileTooLarge => Self::invalid("The file exceeds the 4 GiB limit."),
            TransferError::Io(_) => Self::new(DomainCode::IoError, "The local file operation failed."),
            _ => Self::new(
                DomainCode::VerificationFailed,
                "The content failed integrity verification.",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DomainCode, DomainError};
    use nexus_store::StoreError;

    #[test]
    fn codes_and_retry_policy_are_stable() {
        assert_eq!(DomainCode::QuotaExceeded.as_str(), "quota_exceeded");
        assert_eq!(DomainCode::parse("cancelled"), Some(DomainCode::Cancelled));
        assert!(DomainCode::Timeout.retryable());
        assert!(!DomainCode::NotFound.retryable());
        assert!(!DomainCode::Internal.retryable());
    }

    #[test]
    fn storage_errors_are_redacted() {
        let error = DomainError::from_store(&StoreError::ContentMissing("a".repeat(64)));
        assert_eq!(error.code, DomainCode::NotFound);
        assert!(!error.message.contains(&"a".repeat(64)));

        let error = DomainError::from_store(&StoreError::QuotaExceeded {
            used: 10,
            incoming: 20,
            quota: 25,
        });
        assert_eq!(error.code, DomainCode::QuotaExceeded);
    }
}
