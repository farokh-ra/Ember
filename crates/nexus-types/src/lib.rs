//! Shared identifiers and value types used by every NEXUS crate.

use std::fmt;

/// Identifier for a locally indexed record.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RecordId(String);

impl RecordId {
    /// Creates an identifier, returning `None` when the input is blank.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            None
        } else {
            Some(Self(value))
        }
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::RecordId;

    #[test]
    fn rejects_blank_identifiers() {
        assert!(RecordId::new("   ").is_none());
        assert_eq!(
            RecordId::new("rec-1").map(|id| id.to_string()),
            Some("rec-1".to_owned())
        );
    }
}
