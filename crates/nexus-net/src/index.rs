//! Inbound optional index-provider contract.
//!
//! `nexus-net` owns framing, decoding, rate limiting, and response bounds. The
//! core owns the verified local index and decides which signed records it can
//! return. Records are never trusted because a peer returned them: the
//! requester verifies every signature before display or caching.

use nexus_protocol::{IndexRequest, IndexResponse};

/// Source for inbound `/nexus/index/1` requests.
///
/// Implementations must answer only from verified signed records. Returning an
/// error response is always preferable to returning partial or unverified
/// metadata.
pub trait IndexProvider: Send + Sync {
    /// Answers one already-decoded, bounded request.
    fn respond(&self, request: &IndexRequest) -> IndexResponse;
}

/// Provider used when the host has no local index attached.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EmptyIndexProvider;

impl IndexProvider for EmptyIndexProvider {
    fn respond(&self, _request: &IndexRequest) -> IndexResponse {
        IndexResponse {
            error: Some("unsupported".to_owned()),
            ..IndexResponse::default()
        }
    }
}
