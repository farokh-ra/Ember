//! SQLite metadata and managed blob storage.
//!
//! Rust is the only reader and writer of durable NEXUS state. This crate owns
//! the embedded schema and migrations, the content-addressed managed store,
//! quota accounting, reference-counted deletion, startup reconciliation, and
//! metadata export/restore. Kotlin shells receive app-private roots but never
//! open these files themselves.

mod backup;
mod db;
mod error;
mod managed_store;
mod migrations;
mod model;
mod paths;
mod quota;
mod store;
mod util;

pub use backup::{ExportReport, RestoreReport};
pub use error::{StoreError, StoreResult};
pub use managed_store::ManagedStore;
pub use model::{
    ContentRecord, ContentReference, ContentState, DownloadRecord, DownloadState, IdentityReference, ImportedContent,
    PeerObservation, ProfileRecord, PublicationRecord, ReferenceType, ReleaseOutcome, ReplicaRecord, ReplicaState,
    TombstoneRecord,
};
pub use paths::{
    FALLBACK_FILENAME, MAX_FILENAME_BYTES, safe_display_name, unique_export_path, validate_store_component,
};
pub use quota::{
    DEFAULT_QUOTA_BYTES, FREE_SPACE_RESERVE, GIB, MAX_FILE_SIZE, MAX_QUOTA_BYTES, MIN_QUOTA_BYTES, QuotaPolicy,
    QuotaUsage,
};
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
