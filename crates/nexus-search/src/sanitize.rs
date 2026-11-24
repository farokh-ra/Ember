//! Plain-text sanitization for untrusted metadata.
//!
//! Ember renders metadata as plain text. This function removes control
//! characters, bidi overrides, zero-width characters, and collapses whitespace
//! so a signed record cannot inject terminal controls, break layout, or spoof
//! direction. Markup is left literal; the UI never interprets it as HTML.

/// Removes unsafe characters, collapses whitespace, and truncates on a UTF-8
/// character boundary. Truncated text ends with `...`.
///
/// `max_bytes` must be at least 4. Values below that are treated as 4.
#[must_use]
pub fn sanitize_text(text: &str, max_bytes: usize) -> String {
    let max_bytes = max_bytes.max(4);
    let mut cleaned = String::with_capacity(text.len().min(max_bytes));
    for character in text.chars() {
        if is_unsafe(character) {
            continue;
        }
        if character.is_whitespace() {
            if !cleaned.is_empty() && !cleaned.ends_with(' ') {
                cleaned.push(' ');
            }
            continue;
        }
        cleaned.push(character);
    }
    while cleaned.ends_with(' ') {
        cleaned.pop();
    }
    if cleaned.len() <= max_bytes {
        return cleaned;
    }
    let mut end = max_bytes - 3;
    while !cleaned.is_char_boundary(end) {
        end -= 1;
    }
    cleaned.truncate(end);
    while cleaned.ends_with(' ') {
        cleaned.pop();
    }
    cleaned.push_str("...");
    cleaned
}

fn is_unsafe(character: char) -> bool {
    if character.is_control() {
        return true;
    }
    matches!(
        character,
        '\u{200B}'
            | '\u{200C}'
            | '\u{200D}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::sanitize_text;

    #[test]
    fn control_characters_and_bidi_overrides_are_removed() {
        let text = "Hello\u{0000}\u{0007} world\u{202E} \u{200B}!";
        assert_eq!(sanitize_text(text, 64), "Hello world !");
    }

    #[test]
    fn whitespace_is_collapsed_and_trimmed() {
        assert_eq!(sanitize_text("  a \n\t b  ", 64), "a b");
    }

    #[test]
    fn truncation_respects_character_boundaries() {
        let text = "é".repeat(20);
        let cleaned = sanitize_text(&text, 10);
        assert!(cleaned.ends_with("..."));
        assert!(cleaned.len() <= 10);
        assert!(cleaned.is_char_boundary(cleaned.len()));
    }

    #[test]
    fn markup_stays_literal_and_bounded() {
        let cleaned = sanitize_text("<script>alert(1)</script>", 12);
        assert_eq!(cleaned, "<script>a...");
    }

    #[test]
    fn tiny_limits_still_produce_bounded_text() {
        assert_eq!(sanitize_text("abcdef", 4), "a...");
        assert_eq!(sanitize_text("ab", 0), "ab");
    }
}
