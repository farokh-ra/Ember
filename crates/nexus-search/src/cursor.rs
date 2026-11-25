//! Stable pagination cursors tied to one query and one index generation.

use crate::SearchError;

/// Maximum opaque cursor bytes accepted from a caller.
pub const MAX_CURSOR_BYTES: usize = 128;
const CURSOR_VERSION: &str = "v1";

/// One parsed search cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchCursor {
    /// Local index generation the cursor was issued against.
    pub generation: u64,
    /// Zero-based result offset inside the filtered, sorted result set.
    pub offset: u32,
    /// Query hash that produced the cursor.
    pub query_hash: String,
}

impl SearchCursor {
    /// Encodes a cursor for the next page.
    #[must_use]
    pub fn encode(generation: u64, offset: u32, query_hash: &str) -> String {
        format!("{CURSOR_VERSION}:{generation}:{offset}:{query_hash}")
    }

    /// Parses a cursor and checks that it belongs to `expected_hash`.
    ///
    /// Generation freshness is checked by the caller because only the store
    /// knows the current index generation.
    pub fn parse(cursor: &str, expected_hash: &str) -> Result<Self, SearchError> {
        if cursor.is_empty() || cursor.len() > MAX_CURSOR_BYTES {
            return Err(SearchError::InvalidCursor);
        }
        let mut parts = cursor.split(':');
        let version = parts.next().ok_or(SearchError::InvalidCursor)?;
        let generation = parts.next().ok_or(SearchError::InvalidCursor)?;
        let offset = parts.next().ok_or(SearchError::InvalidCursor)?;
        let query_hash = parts.next().ok_or(SearchError::InvalidCursor)?;
        if parts.next().is_some() || version != CURSOR_VERSION {
            return Err(SearchError::InvalidCursor);
        }
        let generation = generation.parse::<u64>().map_err(|_| SearchError::InvalidCursor)?;
        let offset = offset.parse::<u32>().map_err(|_| SearchError::InvalidCursor)?;
        if offset > crate::MAX_CURSOR_OFFSET {
            return Err(SearchError::InvalidCursor);
        }
        if !is_hash(query_hash) {
            return Err(SearchError::InvalidCursor);
        }
        if query_hash != expected_hash {
            return Err(SearchError::CursorMismatch);
        }
        Ok(Self {
            generation,
            offset,
            query_hash: query_hash.to_owned(),
        })
    }

    /// Checks that the cursor still matches the current index generation.
    pub fn check_generation(self, generation: u64) -> Result<Self, SearchError> {
        if self.generation != generation {
            return Err(SearchError::StaleCursor);
        }
        Ok(self)
    }
}

fn is_hash(text: &str) -> bool {
    text.len() == 16 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{MAX_CURSOR_BYTES, SearchCursor};
    use crate::SearchError;

    const HASH: &str = "0123456789abcdef";

    #[test]
    fn cursor_round_trips() {
        let encoded = SearchCursor::encode(7, 40, HASH);
        let cursor = SearchCursor::parse(&encoded, HASH).expect("parse");
        assert_eq!(cursor.generation, 7);
        assert_eq!(cursor.offset, 40);
        assert_eq!(cursor.query_hash, HASH);
        assert_eq!(cursor.clone().check_generation(7).expect("fresh").offset, 40);
        assert_eq!(cursor.check_generation(8).expect_err("stale"), SearchError::StaleCursor);
    }

    #[test]
    fn malformed_and_mismatched_cursors_are_rejected() {
        for cursor in [
            "",
            "v1:1:2",
            "v2:1:2:0123456789abcdef",
            "v1:x:2:0123456789abcdef",
            "v1:1:x:0123456789abcdef",
            "v1:1:2:xyz",
            "v1:1:2:0123456789abcdef:extra",
        ] {
            assert!(SearchCursor::parse(cursor, HASH).is_err(), "{cursor}");
        }
        assert_eq!(
            SearchCursor::parse("v1:1:2:fedcba9876543210", HASH).expect_err("mismatch"),
            SearchError::CursorMismatch
        );
        let oversized = format!("v1:1:2:{}", "a".repeat(MAX_CURSOR_BYTES));
        assert_eq!(
            SearchCursor::parse(&oversized, HASH).expect_err("oversized"),
            SearchError::InvalidCursor
        );
    }
}
