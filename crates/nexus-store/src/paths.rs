//! Safe display filenames and collision-free export paths.

use std::path::{Path, PathBuf};

use crate::error::{StoreError, StoreResult};
use crate::util::is_store_component;

/// Maximum UTF-8 byte length of a stored display name.
pub const MAX_FILENAME_BYTES: usize = 255;
/// Fallback used when a user provides an empty or unusable name.
pub const FALLBACK_FILENAME: &str = "untitled";

const FORBIDDEN_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];
const MAX_EXTENSION_BYTES: usize = 32;

/// Returns a safe display name that never acts as a path.
///
/// Control characters, NUL, path separators, Windows-forbidden characters,
/// trailing dots and spaces, and reserved device names are neutralized. Long
/// names are truncated on a UTF-8 boundary while preserving a short extension.
#[must_use]
pub fn safe_display_name(input: &str) -> String {
    let mut cleaned = String::with_capacity(input.len().min(MAX_FILENAME_BYTES));
    let mut last_was_space = false;
    for character in input.chars() {
        let replacement = if character.is_control() || FORBIDDEN_CHARS.contains(&character) {
            Some('_')
        } else {
            None
        };
        let character = replacement.unwrap_or(character);
        if character == ' ' {
            if last_was_space {
                continue;
            }
            last_was_space = true;
        } else {
            last_was_space = false;
        }
        cleaned.push(character);
    }

    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_owned();
    let cleaned = if cleaned.is_empty() {
        FALLBACK_FILENAME.to_owned()
    } else {
        cleaned
    };

    let reserved = is_reserved_name(&cleaned);
    let mut cleaned = if reserved { format!("_{cleaned}") } else { cleaned };

    cleaned = truncate_name(&cleaned, MAX_FILENAME_BYTES);
    if cleaned.is_empty() {
        FALLBACK_FILENAME.to_owned()
    } else {
        cleaned
    }
}

/// Validates a generated store component such as a download identifier.
pub fn validate_store_component(value: &str, label: &str) -> StoreResult<()> {
    if is_store_component(value) {
        Ok(())
    } else {
        Err(StoreError::InvalidId(format!(
            "{label} must contain only letters, digits, '-', or '_' and be at most 128 bytes"
        )))
    }
}

/// Returns a collision-free export path in `directory`.
///
/// Existing files are never overwritten. The suffix is inserted before the
/// extension: `report.pdf`, `report (1).pdf`, and so on.
pub fn unique_export_path(directory: &Path, display_name: &str) -> StoreResult<PathBuf> {
    let safe_name = safe_display_name(display_name);
    let candidate = directory.join(&safe_name);
    if !candidate.exists() {
        return Ok(candidate);
    }

    let (stem, extension) = split_extension(&safe_name);
    for index in 1..=10_000_u32 {
        let name = if extension.is_empty() {
            format!("{stem} ({index})")
        } else {
            format!("{stem} ({index}).{extension}")
        };
        let candidate = directory.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(StoreError::InvalidFilename(
        "could not find a collision-free export name".to_owned(),
    ))
}

fn split_extension(name: &str) -> (&str, &str) {
    name.rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .unwrap_or((name, ""))
}

fn is_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    RESERVED_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
}

fn truncate_name(name: &str, max_bytes: usize) -> String {
    if name.len() <= max_bytes {
        return name.to_owned();
    }

    let (stem, extension) = split_extension(name);
    if !extension.is_empty() && extension.len() <= MAX_EXTENSION_BYTES {
        let suffix = format!(".{extension}");
        let budget = max_bytes.saturating_sub(suffix.len());
        let mut stem = truncate_utf8(stem, budget);
        if stem.is_empty() {
            stem = FALLBACK_FILENAME.to_owned();
        }
        let candidate = format!("{stem}{suffix}");
        return truncate_utf8(&candidate, max_bytes);
    }

    truncate_utf8(name, max_bytes)
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value[..boundary].trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::{MAX_FILENAME_BYTES, safe_display_name, unique_export_path};
    use std::path::PathBuf;

    fn temporary_directory(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("nexus-path-test-{}-{label}", crate::util::unique_suffix()));
        std::fs::create_dir_all(&path).expect("create temporary directory");
        path
    }

    #[test]
    fn strips_traversal_and_forbidden_characters() {
        assert_eq!(safe_display_name("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(safe_display_name(r"C:\Windows\system.ini"), "C__Windows_system.ini");
        assert_eq!(safe_display_name("a\u{0}b\u{1}c"), "a_b_c");
    }

    #[test]
    fn handles_reserved_and_empty_names() {
        assert_eq!(safe_display_name("CON"), "_CON");
        assert_eq!(safe_display_name("lpt1.txt"), "_lpt1.txt");
        assert_eq!(safe_display_name("   "), "untitled");
        assert_eq!(safe_display_name("name."), "name");
    }

    #[test]
    fn enforces_255_utf8_bytes() {
        let exact = "a".repeat(MAX_FILENAME_BYTES);
        assert_eq!(safe_display_name(&exact).len(), MAX_FILENAME_BYTES);

        let long = format!("{}.pdf", "a".repeat(300));
        let safe = safe_display_name(&long);
        assert!(safe.len() <= MAX_FILENAME_BYTES);
        assert!(safe.ends_with(".pdf"));

        let unicode = "é".repeat(200);
        let safe = safe_display_name(&unicode);
        assert!(safe.len() <= MAX_FILENAME_BYTES);
        assert!(safe.is_char_boundary(safe.len()));
    }

    #[test]
    fn never_overwrites_existing_export_names() {
        let directory = temporary_directory("collision");
        std::fs::write(directory.join("report.pdf"), b"existing").expect("write fixture");

        let first = unique_export_path(&directory, "report.pdf").expect("first path");
        assert_eq!(first.file_name().and_then(|name| name.to_str()), Some("report (1).pdf"));

        std::fs::write(&first, b"second").expect("write second fixture");
        let second = unique_export_path(&directory, "report.pdf").expect("second path");
        assert_eq!(
            second.file_name().and_then(|name| name.to_str()),
            Some("report (2).pdf")
        );

        std::fs::remove_dir_all(directory).expect("remove fixture");
    }
}
