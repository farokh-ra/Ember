//! Local full-text search and coverage metadata.
//!
//! This crate is pure domain logic shared by the store, core, and networking
//! layers. It owns query validation, the bounded FTS5 expression, stable
//! pagination cursors, honest coverage labels, result sanitization, and the
//! deduplication/conflict rules used when local and provider results merge.
//!
//! Nothing here touches SQLite, the network, or the filesystem. That keeps the
//! verification rules testable without an engine and prevents a provider from
//! reaching storage directly.

mod coverage;
mod cursor;
mod explore;
mod query;
mod result;
mod sanitize;

pub use coverage::{Coverage, ProviderCoverage, SourceState};
pub use cursor::SearchCursor;
pub use explore::{
    CategoryCount, DEFAULT_EXPLORE_LIMIT, ExploreMode, ExplorePage, ExploreQuery, MAX_EXPLORE_LIMIT,
    MAX_EXPLORE_RANDOM_SAMPLE, seeded_shuffle,
};
pub use query::validate_limit;
pub use query::{
    DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, MAX_QUERY_BYTES, MAX_QUERY_TOKENS, MAX_TAG_BYTES, MAX_TAG_FILTERS,
    NormalizedQuery, RecordKind, SearchQuery, SortOrder,
};
pub use result::{
    Availability, Conflict, MAX_CONFLICTS_PER_RESULT, MAX_RESULT_SUMMARY_BYTES, MAX_RESULT_TITLE_BYTES,
    ResultAccumulator, ResultSource, SearchItem, item_matches,
};
pub use sanitize::sanitize_text;

/// Maximum number of optional index providers contacted for one search.
pub const MAX_PROVIDERS: usize = 8;
/// Total wall-clock deadline for distributed search, in milliseconds.
pub const SEARCH_DEADLINE_MS: u64 = 10_000;
/// Maximum provider pages consumed for one search.
pub const MAX_PROVIDER_PAGES: usize = 1;
/// Maximum local candidates examined for one query.
pub const MAX_LOCAL_CANDIDATES: usize = 2_000;
/// Maximum cursor offset accepted for page walking.
pub const MAX_CURSOR_OFFSET: u32 = 1_000_000;

/// A bounded search-domain failure with a stable diagnostic code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchError {
    /// The query text was empty after trimming.
    EmptyQuery,
    /// The query exceeded [`MAX_QUERY_BYTES`] UTF-8 bytes.
    QueryTooLong,
    /// The query contained C0/C1 controls or bidi overrides.
    ControlCharacter,
    /// The query produced no searchable tokens.
    NoSearchableTokens,
    /// The query produced more than [`MAX_QUERY_TOKENS`] tokens.
    TooManyTokens,
    /// A tag filter was empty, oversized, or contained controls.
    InvalidTag,
    /// Too many tag filters were supplied.
    TooManyTags,
    /// A page size was zero or above [`MAX_PAGE_SIZE`].
    InvalidLimit,
    /// A cursor was malformed, oversized, or used an unknown version.
    InvalidCursor,
    /// A cursor belongs to a different query.
    CursorMismatch,
    /// The index changed after the cursor was issued.
    StaleCursor,
}

impl SearchError {
    /// Stable lowercase code used in diagnostics and domain errors.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyQuery => "empty_query",
            Self::QueryTooLong => "query_too_long",
            Self::ControlCharacter => "control_character",
            Self::NoSearchableTokens => "no_searchable_tokens",
            Self::TooManyTokens => "too_many_tokens",
            Self::InvalidTag => "invalid_tag",
            Self::TooManyTags => "too_many_tags",
            Self::InvalidLimit => "invalid_limit",
            Self::InvalidCursor => "invalid_cursor",
            Self::CursorMismatch => "cursor_mismatch",
            Self::StaleCursor => "stale_cursor",
        }
    }

    /// True when the caller must restart the search instead of retrying the
    /// same cursor.
    #[must_use]
    pub const fn is_conflict(self) -> bool {
        matches!(self, Self::CursorMismatch | Self::StaleCursor)
    }
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::EmptyQuery => "search query is empty",
            Self::QueryTooLong => "search query is longer than 128 bytes",
            Self::ControlCharacter => "search query contains control characters",
            Self::NoSearchableTokens => "search query has no searchable words",
            Self::TooManyTokens => "search query has too many words",
            Self::InvalidTag => "a tag filter is invalid",
            Self::TooManyTags => "too many tag filters",
            Self::InvalidLimit => "page size must be between 1 and 20",
            Self::InvalidCursor => "search cursor is malformed",
            Self::CursorMismatch => "search cursor belongs to a different query",
            Self::StaleCursor => "the search index changed; restart the search",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SearchError {}

/// Returns the architecture component name for diagnostics.
#[must_use]
pub const fn component_name() -> &'static str {
    "nexus-search"
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_component_name() {
        assert_eq!(super::component_name(), "nexus-search");
    }
}
