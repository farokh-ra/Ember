//! Ember lifecycle orchestration, method dispatch, event queue, and streaming.
//!
//! This crate owns the real core behind the C ABI: configuration validation,
//! handle lifecycle, bounded asynchronous operations, the coalescing event
//! queue, and the streaming import/export cursors. Platform shells reach it
//! only through `nexus-ffi`.

mod config;
mod downloads;
mod engine;
mod envelope;
mod error;
mod event;
mod hosting;
mod network;
mod ops;
mod profiles;
mod publications;
mod replicas;
mod search;
mod streaming;

pub use config::{ConfigError, CoreConfig, LogLevel, MAX_CONFIG_BYTES};
pub use engine::{Engine, IdentityStatus, LifecycleState};
pub use envelope::{
    ENVELOPE_VERSION, EnvelopeError, MAX_CBOR_DEPTH, MAX_CBOR_MAP_ENTRIES, MAX_EVENT_BYTES, MAX_REQUEST_BYTES,
    MAX_RESPONSE_BYTES, Request, Response, decode_response, encode_failure, encode_success, error_value,
};
pub use error::{CoreError, DomainCode, DomainError};
pub use event::{EVENT_QUEUE_CAPACITY, Event, EventClass, EventError, EventQueue, PendingEvent};
pub use nexus_net::DhtMode;
pub use nexus_store::StoreError;
pub use ops::{CancelOutcome, CancelToken, OPERATION_WORKERS, OperationRegistry, WorkerPool};
pub use profiles::PROFILE_CACHE_STALE_MS;
pub use publications::{PUBLICATION_AVAILABILITY_STALE_MS, UNPUBLISH_LIMITS};
pub use replicas::{REPLICA_LIMITS, REPLICA_REVERIFY_INTERVAL_MS};
pub use streaming::{ExportCursor, FinishedImport, ImportError, ImportSession, MAX_STREAM_CHUNK};

use nexus_observability::LogInitError;

/// Native ABI version exported to Kotlin clients.
pub const ABI_VERSION: u32 = 1;

/// Protocol version implemented by this build.
pub const PROTOCOL_VERSION: &str = "0.1.0-stage-16";

/// Static build information exposed to shells and diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreInfo {
    /// Native ABI version.
    pub abi_version: u32,
    /// Protocol version string.
    pub protocol_version: &'static str,
}

/// Returns static build information.
#[must_use]
pub const fn info() -> CoreInfo {
    CoreInfo {
        abi_version: ABI_VERSION,
        protocol_version: PROTOCOL_VERSION,
    }
}

/// Stable platform name reported by `system.info`.
#[must_use]
pub const fn platform() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}

/// Initializes redacted structured logging for the core process.
pub fn init_logging(debug: bool) -> Result<(), LogInitError> {
    nexus_observability::init(debug)
}

#[cfg(test)]
mod tests {
    use super::{ABI_VERSION, info};

    #[test]
    fn reports_abi_version() {
        assert_eq!(ABI_VERSION, info().abi_version);
        assert_eq!("0.1.0-stage-16", info().protocol_version);
    }

    #[test]
    fn reports_a_platform_name() {
        assert_ne!(super::platform(), "unknown");
    }
}
