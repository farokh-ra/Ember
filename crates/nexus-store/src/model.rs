//! Storage records shared with later stages.

/// The reason a content blob is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceType {
    /// The user's original published copy.
    PublicationOriginal,
    /// A copy the user explicitly hosts.
    ActiveReplica,
    /// A completed download the user chose to retain.
    RetainedDownload,
    /// Content streamed into the store by the local user and not yet
    /// published. This keeps the bytes alive until a publication, replica,
    /// or download reference takes ownership.
    LocalImport,
}

impl ReferenceType {
    /// SQLite and diagnostics representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublicationOriginal => "publication_original",
            Self::ActiveReplica => "active_replica",
            Self::RetainedDownload => "retained_download",
            Self::LocalImport => "local_import",
        }
    }

    /// Parses a stored reference type.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "publication_original" => Some(Self::PublicationOriginal),
            "active_replica" => Some(Self::ActiveReplica),
            "retained_download" => Some(Self::RetainedDownload),
            "local_import" => Some(Self::LocalImport),
            _ => None,
        }
    }

    /// Published originals and active replicas require explicit confirmation
    /// before their last reference can be removed.
    #[must_use]
    pub const fn requires_confirmation(self) -> bool {
        matches!(self, Self::PublicationOriginal | Self::ActiveReplica)
    }

    /// Human-readable label used in confirmation errors.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PublicationOriginal => "a published original",
            Self::ActiveReplica => "an active replica",
            Self::RetainedDownload => "a retained download",
            Self::LocalImport => "a local import",
        }
    }
}

/// Lifecycle state of a stored content blob.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentState {
    /// Metadata exists but verification has not completed.
    Pending,
    /// The blob is present and its hash matches the content ID.
    Verified,
    /// The database knows about the content but its bytes are absent.
    Missing,
    /// The bytes were moved to quarantine after a failed check.
    Quarantined,
}

impl ContentState {
    /// SQLite representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Verified => "verified",
            Self::Missing => "missing",
            Self::Quarantined => "quarantined",
        }
    }

    /// Parses a stored content state.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "verified" => Some(Self::Verified),
            "missing" => Some(Self::Missing),
            "quarantined" => Some(Self::Quarantined),
            _ => None,
        }
    }
}

/// A metadata record for a managed content blob.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRecord {
    /// SHA-256 content ID as lowercase hexadecimal.
    pub content_id: String,
    /// Size of the blob in bytes.
    pub size_bytes: u64,
    /// Current verification state.
    pub state: ContentState,
    /// Generated relative path inside the managed store.
    pub blob_path: String,
    /// Last successful verification time.
    pub verified_at: Option<u64>,
    /// Creation time.
    pub created_at: u64,
    /// Last state update time.
    pub updated_at: u64,
}

/// A reference to a content blob.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentReference {
    /// Content being retained.
    pub content_id: String,
    /// Why it is retained.
    pub ref_type: ReferenceType,
    /// Publication, replica, or download identifier.
    pub owner_id: String,
    /// Creation time.
    pub created_at: u64,
}

/// Outcome of releasing a content reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseOutcome {
    /// No matching reference existed.
    NotFound,
    /// The reference was removed but other references remain.
    Retained {
        /// References still pointing at the content.
        remaining: usize,
    },
    /// The last reference was removed and the blob was deleted.
    BytesDeleted,
    /// The last reference was removed; bytes will be removed by reconciliation.
    BytesPendingCleanup,
}

/// A local identity reference. Private key material lives outside SQLite.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityReference {
    /// Stable identity ID.
    pub identity_id: String,
    /// Ed25519 public key bytes.
    pub public_key: Vec<u8>,
    /// Relative path to the encrypted key file.
    pub key_file: String,
    /// Optional local label.
    pub label: Option<String>,
    /// Creation time.
    pub created_at: u64,
}

/// A verified profile record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileRecord {
    /// Identity that signed the profile.
    pub identity_id: String,
    /// Monotonic profile revision.
    pub revision: u64,
    /// Exact signed record bytes.
    pub record_bytes: Vec<u8>,
    /// Display name.
    pub display_name: String,
    /// Username label; not unique.
    pub username: Option<String>,
    /// Free-form biography.
    pub bio: Option<String>,
    /// Avatar content ID, if any.
    pub avatar_content_id: Option<String>,
    /// Last local update time.
    pub updated_at: u64,
}

/// A verified publication record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationRecord {
    /// Publication identifier.
    pub publication_id: String,
    /// Author identity.
    pub identity_id: String,
    /// Content being published.
    pub content_id: String,
    /// Monotonic publication revision.
    pub revision: u64,
    /// Exact signed record bytes.
    pub record_bytes: Vec<u8>,
    /// User-visible title.
    pub title: String,
    /// Optional description.
    pub description: Option<String>,
    /// Safe display filename; never a path.
    pub filename: String,
    /// Publication time.
    pub published_at: u64,
    /// Last local update time.
    pub updated_at: u64,
    /// Whether a tombstone has been received or created.
    pub tombstoned: bool,
}

/// A signed tombstone record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TombstoneRecord {
    /// Publication being withdrawn.
    pub publication_id: String,
    /// Original uploader identity from the signed record.
    pub identity_id: String,
    /// Content ID of the withdrawn publication from the signed record.
    pub content_id: String,
    /// Tombstone revision.
    pub revision: u64,
    /// Exact signed record bytes.
    pub record_bytes: Vec<u8>,
    /// Creation time.
    pub created_at: u64,
}

/// Download lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownloadState {
    /// Registered but no bytes have been verified.
    Pending,
    /// Transfer is in progress.
    Downloading,
    /// Transfer was paused by the user or restart.
    Paused,
    /// Bytes are verified and promoted.
    Completed,
    /// Transfer failed and can be retried.
    Failed,
    /// The user cancelled the download.
    Cancelled,
}

impl DownloadState {
    /// SQLite representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Downloading => "downloading",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parses a stored download state.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "downloading" => Some(Self::Downloading),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// A resumable download record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadRecord {
    /// Download identifier.
    pub download_id: String,
    /// Expected SHA-256 content ID.
    pub content_id: String,
    /// Safe display filename.
    pub display_name: String,
    /// Manifest size.
    pub expected_size: u64,
    /// Verified contiguous prefix in bytes.
    pub verified_prefix: u64,
    /// Lifecycle state.
    pub state: DownloadState,
    /// Generated partial-file path relative to the managed store.
    pub partial_file: String,
    /// Creation time.
    pub created_at: u64,
    /// Last update time.
    pub updated_at: u64,
    /// Last error message, if any.
    pub last_error: Option<String>,
}

/// Replica lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplicaState {
    /// The user is actively hosting the content.
    Active,
    /// Hosting is suspended.
    Paused,
    /// Re-verification failed.
    VerificationFailed,
    /// The replica has been removed.
    Removed,
}

impl ReplicaState {
    /// SQLite representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::VerificationFailed => "verification_failed",
            Self::Removed => "removed",
        }
    }

    /// Parses a stored replica state.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "verification_failed" => Some(Self::VerificationFailed),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }
}

/// A hosted replica record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplicaRecord {
    /// Replica identifier.
    pub replica_id: String,
    /// Content being hosted.
    pub content_id: String,
    /// Original publication identifier, if known.
    pub publication_id: Option<String>,
    /// Current hosting state.
    pub state: ReplicaState,
    /// Last successful re-verification time.
    pub last_verified_at: Option<u64>,
    /// Creation time.
    pub created_at: u64,
    /// Last update time.
    pub updated_at: u64,
}

/// A bounded peer observation record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerObservation {
    /// Peer identifier.
    pub peer_id: String,
    /// Last known address.
    pub address: Option<String>,
    /// Last time the peer was observed.
    pub last_seen_at: u64,
    /// Last redacted error.
    pub last_error: Option<String>,
    /// Number of observations merged into this row.
    pub observation_count: u64,
}

/// A consumed content import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedContent {
    /// SHA-256 content ID.
    pub content_id: String,
    /// Imported size in bytes.
    pub size_bytes: u64,
    /// Whether identical bytes were already present.
    pub already_present: bool,
}
