//! Honest coverage labels for local and optional provider results.

use crate::SEARCH_DEADLINE_MS;
use crate::sanitize::sanitize_text;

const MAX_NOTES: usize = 4;
const MAX_NOTE_BYTES: usize = 160;
const MAX_PROVIDER_ID_BYTES: usize = 128;

/// Terminal state of one optional index provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceState {
    /// The provider answered with a valid, bounded page.
    Ok,
    /// The provider answered with malformed or invalid content.
    Failed,
    /// The provider did not answer before the deadline.
    Timeout,
    /// The provider was discovered but could not be reached.
    Unavailable,
    /// The provider ignored the request or the protocol version is unknown.
    Unsupported,
    /// The response page was valid but the provider reported more matches.
    Truncated,
}

impl SourceState {
    /// Stable diagnostics name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Timeout => "timeout",
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
            Self::Truncated => "truncated",
        }
    }
}

/// Per-provider coverage detail kept for diagnostics and UI labelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCoverage {
    /// Stable peer identifier, never an identity or authority.
    pub provider_id: String,
    /// Terminal state.
    pub state: SourceState,
    /// Number of verified records accepted from this provider.
    pub records: u32,
    /// Optional redacted detail such as `invalid_signature` or `rate_limited`.
    pub detail: Option<String>,
}

impl ProviderCoverage {
    /// Builds bounded coverage for one provider.
    #[must_use]
    pub fn new(provider_id: &str, state: SourceState) -> Self {
        Self {
            provider_id: sanitize_text(provider_id, MAX_PROVIDER_ID_BYTES),
            state,
            records: 0,
            detail: None,
        }
    }

    /// Adds a bounded redacted detail.
    #[must_use]
    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(sanitize_text(detail, MAX_NOTE_BYTES));
        self
    }

    /// Adds the verified record count.
    #[must_use]
    pub fn with_records(mut self, records: u32) -> Self {
        self.records = records;
        self
    }
}

/// Aggregate coverage for one search response or terminal event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Coverage {
    /// Whether the local verified index answered.
    pub local: bool,
    /// Whether any displayed result came from the remote cache.
    pub cached: bool,
    /// Whether any optional provider participated.
    pub distributed: bool,
    /// True only when every contacted provider returned a complete page.
    pub complete: bool,
    /// Providers discovered and attempted.
    pub providers_queried: u32,
    /// Providers that returned at least one valid page.
    pub providers_succeeded: u32,
    /// Providers that failed validation or transport.
    pub providers_failed: u32,
    /// Providers that hit the total deadline.
    pub providers_timed_out: u32,
    /// Providers using an unknown protocol version or refusing the request.
    pub providers_unsupported: u32,
    /// Last local index refresh in Unix milliseconds.
    pub indexed_at_ms: Option<u64>,
    /// Total deadline applied to this search.
    pub deadline_ms: u64,
    /// Bounded honest notes such as `local_index_only`.
    pub notes: Vec<String>,
    /// Per-provider detail.
    pub providers: Vec<ProviderCoverage>,
}

impl Coverage {
    /// Local-only coverage before any provider is attempted.
    #[must_use]
    pub fn local_only(indexed_at_ms: Option<u64>) -> Self {
        Self {
            local: true,
            cached: false,
            distributed: false,
            complete: false,
            providers_queried: 0,
            providers_succeeded: 0,
            providers_failed: 0,
            providers_timed_out: 0,
            providers_unsupported: 0,
            indexed_at_ms,
            deadline_ms: SEARCH_DEADLINE_MS,
            notes: vec!["local_index_only".to_owned()],
            providers: Vec::new(),
        }
    }

    /// Replaces the deadline shown to callers.
    #[must_use]
    pub fn with_deadline(mut self, deadline_ms: u64) -> Self {
        self.deadline_ms = deadline_ms;
        self
    }

    /// Mark that provider results are still pending.
    #[must_use]
    pub fn pending(mut self) -> Self {
        self.complete = false;
        self.push_note("distributed_results_pending");
        self
    }

    /// Adds a bounded honest note.
    #[must_use]
    pub fn note(mut self, note: &str) -> Self {
        self.push_note(note);
        self
    }

    /// Adds one provider result and updates counters and the label.
    pub fn record_provider(&mut self, provider: ProviderCoverage) {
        self.distributed = true;
        self.providers_queried = self.providers_queried.saturating_add(1);
        match provider.state {
            SourceState::Ok => {
                self.providers_succeeded = self.providers_succeeded.saturating_add(1);
            }
            SourceState::Truncated => {
                self.providers_succeeded = self.providers_succeeded.saturating_add(1);
                self.push_note("provider_page_truncated");
            }
            SourceState::Timeout => {
                self.providers_timed_out = self.providers_timed_out.saturating_add(1);
            }
            SourceState::Unsupported => {
                self.providers_unsupported = self.providers_unsupported.saturating_add(1);
                self.push_note("provider_protocol_unsupported");
            }
            SourceState::Failed | SourceState::Unavailable => {
                self.providers_failed = self.providers_failed.saturating_add(1);
            }
        }
        if self.providers.len() < crate::MAX_PROVIDERS {
            self.providers.push(provider);
        }
    }

    /// Marks the distributed phase terminal and computes completeness.
    pub fn finish(mut self) -> Self {
        if !self.distributed {
            self.complete = false;
            if !self.notes.iter().any(|note| note == "local_index_only") {
                self.push_note("local_index_only");
            }
            return self;
        }
        self.complete = self.providers_queried > 0
            && self.providers_succeeded == self.providers_queried
            && self.providers_failed == 0
            && self.providers_timed_out == 0
            && self.providers_unsupported == 0
            && !self.notes.iter().any(|note| note == "provider_page_truncated");
        if !self.complete {
            self.push_note("may_be_incomplete");
        }
        self
    }

    /// Stable label for the UI.
    ///
    /// `local_only` is never presented as network coverage, and `partial`
    /// always means at least one requested source did not complete.
    #[must_use]
    pub fn label(&self) -> &'static str {
        if !self.distributed {
            if self.cached { "cached_local" } else { "local_only" }
        } else if self.complete {
            "complete"
        } else {
            "partial"
        }
    }

    /// True when the caller should show a "may be incomplete" indicator.
    #[must_use]
    pub const fn may_be_incomplete(&self) -> bool {
        self.distributed && !self.complete
    }

    fn push_note(&mut self, note: &str) {
        if self.notes.iter().any(|existing| existing == note) {
            return;
        }
        if self.notes.len() < MAX_NOTES {
            self.notes.push(note.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Coverage, ProviderCoverage, SourceState};

    #[test]
    fn local_only_never_claims_network_coverage() {
        let coverage = Coverage::local_only(Some(42)).finish();
        assert_eq!(coverage.label(), "local_only");
        assert!(!coverage.complete);
        assert!(!coverage.distributed);
        assert!(!coverage.may_be_incomplete());
        assert!(coverage.notes.iter().any(|note| note == "local_index_only"));
    }

    #[test]
    fn one_failed_provider_marks_partial_and_incomplete() {
        let mut coverage = Coverage::local_only(Some(42)).with_deadline(2_000).pending();
        coverage.record_provider(ProviderCoverage::new("peer-a", SourceState::Ok).with_records(3));
        coverage.record_provider(ProviderCoverage::new("peer-b", SourceState::Timeout).with_detail("deadline"));
        let coverage = coverage.finish();
        assert_eq!(coverage.label(), "partial");
        assert!(coverage.may_be_incomplete());
        assert_eq!(coverage.providers_queried, 2);
        assert_eq!(coverage.providers_succeeded, 1);
        assert_eq!(coverage.providers_timed_out, 1);
        assert!(!coverage.complete);
        assert_eq!(coverage.deadline_ms, 2_000);
    }

    #[test]
    fn all_complete_providers_label_complete() {
        let mut coverage = Coverage::local_only(Some(42)).pending();
        coverage.record_provider(ProviderCoverage::new("peer-a", SourceState::Ok));
        coverage.record_provider(ProviderCoverage::new("peer-b", SourceState::Ok));
        let coverage = coverage.finish();
        assert_eq!(coverage.label(), "complete");
        assert!(coverage.complete);
        assert!(!coverage.may_be_incomplete());
    }

    #[test]
    fn unsupported_versions_are_reported_without_failing_the_search() {
        let mut coverage = Coverage::local_only(None);
        coverage.record_provider(ProviderCoverage::new("peer-a", SourceState::Unsupported));
        let coverage = coverage.finish();
        assert_eq!(coverage.label(), "partial");
        assert_eq!(coverage.providers_unsupported, 1);
        assert!(
            coverage
                .notes
                .iter()
                .any(|note| note == "provider_protocol_unsupported")
        );
    }

    #[test]
    fn notes_and_provider_ids_are_bounded() {
        let mut coverage = Coverage::local_only(None);
        for index in 0..20 {
            coverage.record_provider(ProviderCoverage::new(&"p".repeat(500), SourceState::Failed));
            let _ = index;
        }
        assert_eq!(coverage.providers.len(), crate::MAX_PROVIDERS);
        for provider in &coverage.providers {
            assert!(provider.provider_id.len() <= 160);
        }
    }
}
