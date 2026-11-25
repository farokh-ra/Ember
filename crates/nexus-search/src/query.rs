//! Bounded query validation and canonical FTS5 expression construction.

use nexus_protocol::Category;
use sha2::{Digest, Sha256};

use crate::SearchError;

/// Maximum query text in UTF-8 bytes.
pub const MAX_QUERY_BYTES: usize = 128;
/// Maximum distinct searchable tokens in one query.
pub const MAX_QUERY_TOKENS: usize = 12;
/// Maximum tag filters in one query.
pub const MAX_TAG_FILTERS: usize = 5;
/// Maximum bytes in one tag filter.
pub const MAX_TAG_BYTES: usize = 32;
/// Default page size.
pub const DEFAULT_PAGE_SIZE: u16 = 20;
/// Maximum page size for local and provider results.
pub const MAX_PAGE_SIZE: usize = 20;
/// Maximum bytes in one token. A token cannot exceed the query bound, so this
/// only guards against pathological inputs after normalization.
const MAX_TOKEN_BYTES: usize = 64;

/// A signed record kind that can be searched.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RecordKind {
    /// A signed public profile.
    Profile,
    /// A signed file publication.
    Publication,
}

impl RecordKind {
    /// Stable wire, database, and diagnostics name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Publication => "publication",
        }
    }

    /// Parses a stable kind name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "profile" => Some(Self::Profile),
            "publication" => Some(Self::Publication),
            _ => None,
        }
    }
}

/// Explainable result order. No popularity or engagement signal is available.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortOrder {
    /// FTS5 relevance, then recency.
    #[default]
    Relevance,
    /// Publication or record recency.
    Recency,
    /// Locally observed availability, then recency.
    Availability,
}

impl SortOrder {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Recency => "recency",
            Self::Availability => "availability",
        }
    }

    /// Parses a stable sort name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "relevance" => Some(Self::Relevance),
            "recency" => Some(Self::Recency),
            "availability" => Some(Self::Availability),
            _ => None,
        }
    }
}

/// One validated local or provider search request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchQuery {
    /// Raw query text as supplied by the user.
    pub text: String,
    /// Optional restriction to one record kind.
    pub kind: Option<RecordKind>,
    /// Optional exact category filter for publications.
    pub category: Option<Category>,
    /// Exact tag filters; every listed tag must match.
    pub tags: Vec<String>,
    /// Result ordering.
    pub sort: SortOrder,
    /// Requested page size, 1 through [`MAX_PAGE_SIZE`].
    pub limit: u16,
    /// Opaque cursor from a previous page.
    pub cursor: Option<String>,
}

impl SearchQuery {
    /// Creates a query with default filters and page size.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: None,
            category: None,
            tags: Vec::new(),
            sort: SortOrder::Relevance,
            limit: DEFAULT_PAGE_SIZE,
            cursor: None,
        }
    }

    /// Validates bounds and returns the tokenized query used by the store.
    pub fn validate(&self) -> Result<NormalizedQuery, SearchError> {
        validate_limit(self.limit)?;
        if self.text.len() > MAX_QUERY_BYTES {
            return Err(SearchError::QueryTooLong);
        }
        if self.text.chars().any(char::is_control) {
            return Err(SearchError::ControlCharacter);
        }
        if self.tags.len() > MAX_TAG_FILTERS {
            return Err(SearchError::TooManyTags);
        }
        for tag in &self.tags {
            if tag.is_empty()
                || tag.len() > MAX_TAG_BYTES
                || tag.chars().any(char::is_control)
                || tag.chars().any(char::is_whitespace)
            {
                return Err(SearchError::InvalidTag);
            }
        }
        let text = self.text.trim();
        if text.is_empty() {
            return Err(SearchError::EmptyQuery);
        }
        let tokens = tokenize(text)?;
        if tokens.is_empty() {
            return Err(SearchError::NoSearchableTokens);
        }
        let expression = fts_expression(&tokens);
        let query_hash = query_hash(self, &tokens);
        Ok(NormalizedQuery {
            text: text.to_owned(),
            tokens,
            expression,
            query_hash,
        })
    }
}

/// Query text after tokenization, plus its bounded FTS5 expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedQuery {
    /// Trimmed original text.
    pub text: String,
    /// Lowercase alphanumeric tokens in input order.
    pub tokens: Vec<String>,
    /// Safe FTS5 `MATCH` expression. Every token is quoted and prefix-matched.
    pub expression: String,
    /// Stable hash across text and filters, excluding the page size and cursor.
    pub query_hash: String,
}

/// Validates a page size for local or provider results.
pub fn validate_limit(limit: u16) -> Result<(), SearchError> {
    if limit == 0 || usize::from(limit) > MAX_PAGE_SIZE {
        return Err(SearchError::InvalidLimit);
    }
    Ok(())
}

fn tokenize(text: &str) -> Result<Vec<String>, SearchError> {
    let mut tokens = Vec::new();
    for raw in text.split(|character: char| !character.is_alphanumeric()) {
        if raw.is_empty() {
            continue;
        }
        if raw.len() > MAX_TOKEN_BYTES {
            return Err(SearchError::QueryTooLong);
        }
        let token = raw.to_lowercase();
        if !tokens.iter().any(|existing| existing == &token) {
            tokens.push(token);
        }
        if tokens.len() > MAX_QUERY_TOKENS {
            return Err(SearchError::TooManyTokens);
        }
    }
    Ok(tokens)
}

fn fts_expression(tokens: &[String]) -> String {
    tokens
        .iter()
        .map(|token| format!("\"{token}\"*"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn query_hash(query: &SearchQuery, tokens: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"ember.search.query.v1");
    hash_part(&mut hasher, query.kind.map_or("all", RecordKind::as_str));
    hash_part(&mut hasher, query.category.map_or("all", Category::as_str));
    hash_part(&mut hasher, query.sort.as_str());
    for token in tokens {
        hash_part(&mut hasher, token);
    }
    for tag in &query.tags {
        hash_part(&mut hasher, tag);
    }
    let digest = hasher.finalize();
    let mut hash = String::with_capacity(16);
    for byte in &digest[..8] {
        hash.push_str(&format!("{byte:02x}"));
    }
    hash
}

fn hash_part(hasher: &mut Sha256, part: &str) {
    hasher.update(u32::try_from(part.len()).unwrap_or(u32::MAX).to_be_bytes());
    hasher.update(part.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::{MAX_QUERY_BYTES, MAX_QUERY_TOKENS, RecordKind, SearchQuery, SortOrder, fts_expression, tokenize};
    use crate::SearchError;
    use nexus_protocol::Category;

    #[test]
    fn tokenizes_and_quotes_safely() {
        let query = SearchQuery::new("  Field-notes: \"Rust\" & CBOR  ");
        let normalized = query.validate().expect("valid query");
        assert_eq!(normalized.tokens, ["field", "notes", "rust", "cbor"]);
        assert_eq!(
            normalized.expression,
            "\"field\"* AND \"notes\"* AND \"rust\"* AND \"cbor\"*"
        );
    }

    #[test]
    fn rejects_abusive_queries_with_distinct_codes() {
        assert_eq!(
            SearchQuery::new("   ").validate().expect_err("empty"),
            SearchError::EmptyQuery
        );
        assert_eq!(
            SearchQuery::new("*** ---").validate().expect_err("no tokens"),
            SearchError::NoSearchableTokens
        );
        assert_eq!(
            SearchQuery::new("x".repeat(MAX_QUERY_BYTES + 1))
                .validate()
                .expect_err("too long"),
            SearchError::QueryTooLong
        );
        assert_eq!(
            SearchQuery::new("safe\nquery").validate().expect_err("control"),
            SearchError::ControlCharacter
        );
        let words = (0..=MAX_QUERY_TOKENS)
            .map(|index| format!("word{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            SearchQuery::new(words).validate().expect_err("too many"),
            SearchError::TooManyTokens
        );
        let mut query = SearchQuery::new("rust");
        query.limit = 0;
        assert_eq!(query.validate().expect_err("limit"), SearchError::InvalidLimit);
        query.limit = 21;
        assert_eq!(query.validate().expect_err("limit"), SearchError::InvalidLimit);
    }

    #[test]
    fn hash_is_stable_and_filter_sensitive() {
        let base = SearchQuery::new("field notes").validate().expect("valid");
        let same = SearchQuery::new("field notes").validate().expect("valid");
        assert_eq!(base.query_hash, same.query_hash);

        let mut recency = SearchQuery::new("field notes");
        recency.sort = SortOrder::Recency;
        assert_ne!(base.query_hash, recency.validate().expect("valid").query_hash);

        let mut category = SearchQuery::new("field notes");
        category.category = Some(Category::Documents);
        assert_ne!(base.query_hash, category.validate().expect("valid").query_hash);

        let mut publication = SearchQuery::new("field notes");
        publication.kind = Some(RecordKind::Publication);
        assert_ne!(base.query_hash, publication.validate().expect("valid").query_hash);
    }

    #[test]
    fn tokens_do_not_leak_fts_syntax() {
        let tokens = tokenize("NEAR(a b) OR \"x\"").expect("tokens");
        let expression = fts_expression(&tokens);
        assert!(!expression.contains("NEAR"));
        assert!(!expression.contains(" OR "));
        assert!(!expression.contains('"') || expression.matches('"').count() == tokens.len() * 2);
    }

    #[test]
    fn duplicate_tokens_are_collapsed() {
        let normalized = SearchQuery::new("rust Rust RUST").validate().expect("valid");
        assert_eq!(normalized.tokens, ["rust"]);
    }
}
