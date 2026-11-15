//! NFC text normalization and bounded user-visible text.

use unicode_normalization::UnicodeNormalization;

use crate::error::CryptoError;

/// Normalizes text to Unicode NFC.
#[must_use]
pub fn normalize_nfc(value: &str) -> String {
    value.nfc().collect()
}

/// Normalizes text to NFC and enforces a Unicode scalar-value limit.
pub fn normalize_text_chars(value: &str, max: usize) -> Result<String, CryptoError> {
    let normalized = normalize_nfc(value);
    if normalized.chars().count() > max {
        return Err(CryptoError::TextTooManyChars { max });
    }
    Ok(normalized)
}

/// Normalizes text to NFC and enforces a UTF-8 byte limit.
pub fn normalize_text_bytes(value: &str, max: usize) -> Result<String, CryptoError> {
    let normalized = normalize_nfc(value);
    if normalized.len() > max {
        return Err(CryptoError::TextTooLong { max });
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{normalize_nfc, normalize_text_bytes, normalize_text_chars};

    #[test]
    fn nfc_composes_decomposed_text() {
        assert_eq!(normalize_nfc("e\u{301}"), "\u{e9}");
    }

    #[test]
    fn bounds_apply_after_normalization() {
        assert!(normalize_text_bytes("abcd", 4).is_ok());
        assert!(normalize_text_bytes("abcde", 4).is_err());
        assert!(normalize_text_chars("e\u{301}", 1).is_ok());
        assert!(normalize_text_chars("e\u{301}", 1).is_ok());
    }
}
