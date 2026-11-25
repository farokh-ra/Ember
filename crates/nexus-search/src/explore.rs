//! Local Explore modes: categories, recent records, and bounded random
//! discovery without popularity or engagement ranking.

use nexus_protocol::Category;
use sha2::{Digest, Sha256};

use crate::SearchError;
use crate::coverage::Coverage;
use crate::query::SearchQuery;
use crate::result::SearchItem;

/// Maximum Explore page size.
pub const MAX_EXPLORE_LIMIT: u16 = 50;
/// Maximum publications considered by one random Explore query.
pub const MAX_EXPLORE_RANDOM_SAMPLE: usize = 500;
/// Default Explore page size.
pub const DEFAULT_EXPLORE_LIMIT: u16 = 20;

/// Explore mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExploreMode {
    /// Category counts for the local verified index.
    #[default]
    Categories,
    /// Most recently published records.
    Recent,
    /// Deterministic seed-based random sample.
    Random,
}

impl ExploreMode {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Categories => "categories",
            Self::Recent => "recent",
            Self::Random => "random",
        }
    }

    /// Parses a stable mode name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "categories" => Some(Self::Categories),
            "recent" => Some(Self::Recent),
            "random" => Some(Self::Random),
            _ => None,
        }
    }
}

/// One bounded Explore request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreQuery {
    /// Selected mode.
    pub mode: ExploreMode,
    /// Optional category filter.
    pub category: Option<Category>,
    /// Requested page size, 1 through [`MAX_EXPLORE_LIMIT`].
    pub limit: u16,
    /// Caller-supplied random seed. The store generates one when absent.
    pub seed: Option<u64>,
    /// Opaque cursor from a previous page.
    pub cursor: Option<String>,
}

impl Default for ExploreQuery {
    fn default() -> Self {
        Self {
            mode: ExploreMode::Categories,
            category: None,
            limit: DEFAULT_EXPLORE_LIMIT,
            seed: None,
            cursor: None,
        }
    }
}

impl ExploreQuery {
    /// Validates bounds.
    pub fn validate(&self) -> Result<(), SearchError> {
        if self.limit == 0 || self.limit > MAX_EXPLORE_LIMIT {
            return Err(SearchError::InvalidLimit);
        }
        Ok(())
    }

    /// Stable hash binding a cursor to the mode, category, and seed.
    #[must_use]
    pub fn query_hash(&self, seed: u64) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"ember.explore.query.v1");
        hasher.update(self.mode.as_str().as_bytes());
        hasher.update(b"\0");
        hasher.update(self.category.map_or("all", Category::as_str).as_bytes());
        hasher.update(b"\0");
        hasher.update(seed.to_be_bytes());
        let digest = hasher.finalize();
        let mut hash = String::with_capacity(16);
        for byte in &digest[..8] {
            hash.push_str(&format!("{byte:02x}"));
        }
        hash
    }

    /// Converts the Explore request into a bounded search request for provider
    /// or cursor helpers. Categories mode has no text query.
    #[must_use]
    pub fn as_search_query(&self, fallback_text: &str) -> Option<SearchQuery> {
        (self.mode != ExploreMode::Categories).then(|| SearchQuery {
            text: fallback_text.to_owned(),
            kind: None,
            category: self.category,
            tags: Vec::new(),
            sort: if self.mode == ExploreMode::Random {
                crate::query::SortOrder::Relevance
            } else {
                crate::query::SortOrder::Recency
            },
            limit: self.limit.min(crate::query::DEFAULT_PAGE_SIZE),
            cursor: None,
        })
    }
}

/// One category and its verified publication count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoryCount {
    /// Closed category.
    pub category: Category,
    /// Number of visible verified publications.
    pub count: u32,
}

/// One Explore page. Categories and items can both be present only for
/// categories mode, where the first page of recent records accompanies counts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorePage {
    /// Mode that produced the page.
    pub mode: ExploreMode,
    /// Category counts, ordered by the canonical taxonomy.
    pub categories: Vec<CategoryCount>,
    /// Recent or random verified publications.
    pub items: Vec<SearchItem>,
    /// Seed used for random mode and cursor binding.
    pub seed: u64,
    /// Opaque cursor for the next page.
    pub next_cursor: Option<String>,
    /// Whether the page reached the end of local matches.
    pub complete: bool,
    /// Honest coverage; Explore is local-only in v1.
    pub coverage: Coverage,
}

/// Deterministically orders a bounded candidate list from a seed.
///
/// The shuffle is a fixed-xorshift permutation over indices. It is not
/// cryptographic, not a ranking, and never uses popularity.
pub fn seeded_shuffle<T>(items: &mut [T], seed: u64) {
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    for index in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let swap = usize::try_from(state % (index as u64 + 1)).unwrap_or(0);
        items.swap(index, swap);
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_EXPLORE_LIMIT, ExploreMode, ExploreQuery, MAX_EXPLORE_LIMIT, seeded_shuffle};
    use crate::SearchError;
    use nexus_protocol::Category;

    #[test]
    fn explore_modes_parse_and_bound() {
        assert_eq!(ExploreMode::parse("random"), Some(ExploreMode::Random));
        assert_eq!(ExploreMode::parse("popular"), None);
        let mut query = ExploreQuery::default();
        assert!(query.validate().is_ok());
        query.limit = MAX_EXPLORE_LIMIT;
        assert!(query.validate().is_ok());
        query.limit = 0;
        assert_eq!(query.validate().expect_err("zero"), SearchError::InvalidLimit);
        query.limit = MAX_EXPLORE_LIMIT + 1;
        assert_eq!(query.validate().expect_err("too large"), SearchError::InvalidLimit);
        assert_eq!(DEFAULT_EXPLORE_LIMIT, 20);
    }

    #[test]
    fn cursor_hash_binds_mode_category_and_seed() {
        let query = ExploreQuery::default();
        let hash = query.query_hash(7);
        assert_eq!(hash, query.query_hash(7));
        assert_ne!(hash, query.query_hash(8));

        let other = ExploreQuery {
            category: Some(Category::Images),
            ..ExploreQuery::default()
        };
        assert_ne!(hash, other.query_hash(7));

        let recent = ExploreQuery {
            mode: ExploreMode::Recent,
            ..ExploreQuery::default()
        };
        assert_ne!(query.query_hash(7), recent.query_hash(7));
    }

    #[test]
    fn deterministic_shuffle_is_stable_and_seed_sensitive() {
        let original: Vec<u32> = (0..50).collect();
        let mut left = original.clone();
        let mut right = original.clone();
        seeded_shuffle(&mut left, 42);
        seeded_shuffle(&mut right, 42);
        assert_eq!(left, right);
        assert_ne!(left, original);
        seeded_shuffle(&mut right, 43);
        assert_ne!(left, right);
    }
}
