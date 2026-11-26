//! SQLite metadata and managed blob storage.
//!
//! Rust is the only reader and writer of durable Ember state. This crate owns
//! the embedded schema and migrations, the content-addressed managed store,
//! quota accounting, reference-counted deletion, startup reconciliation, and
//! metadata export/restore. Kotlin shells receive app-private roots but never
//! open these files themselves.

mod backup;
mod chunk_map;
mod db;
mod error;
mod managed_store;
mod migrations;
mod model;
mod paths;
mod quota;
mod search_store;
mod store;
mod util;

pub use backup::{ExportReport, RestoreReport};
pub use chunk_map::ChunkMap;
pub use error::{StoreError, StoreResult};
pub use managed_store::ManagedStore;
pub use model::{
    AbuseCategory, AbuseReport, BlockReason, BlockedPeer, ContentRecord, ContentReference, ContentState,
    DownloadRecord, DownloadState, HostingLimits, IdentityReference, ImportedContent, MAX_ABUSE_REPORTS,
    MAX_BLOCKED_PEERS, MAX_SECURITY_NOTE_BYTES, MAX_UPLOAD_BYTES_PER_SECOND, MIN_UPLOAD_BYTES_PER_SECOND,
    PeerObservation, ProfileCacheOutcome, ProfileConflict, ProfileFreshness, ProfileRecord, ProfileState,
    PublicationAvailability, PublicationCacheOutcome, PublicationRecord, ReferenceType, ReleaseOutcome, ReplicaRecord,
    ReplicaState, ReplicaUsage, TombstoneRecord,
};
pub use paths::{
    FALLBACK_FILENAME, MAX_FILENAME_BYTES, safe_display_name, unique_export_path, validate_store_component,
};
pub use quota::{
    DEFAULT_QUOTA_BYTES, FREE_SPACE_RESERVE, GIB, MAX_FILE_SIZE, MAX_QUOTA_BYTES, MIN_QUOTA_BYTES, QuotaPolicy,
    QuotaUsage,
};
pub use search_store::{LocalSearchPage, ProviderPage, SearchIndexStats};
pub use store::{ExportedContent, ReconciliationReport, Store, StoreConfig};

/// Returns the architecture component name for diagnostics.
#[must_use]
pub const fn component_name() -> &'static str {
    "nexus-store"
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_component_name() {
        assert_eq!(super::component_name(), "nexus-store");
    }
}
