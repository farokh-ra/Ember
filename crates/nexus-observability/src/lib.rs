//! Redacted structured logging and diagnostics.
//!
//! This crate is the only place that should initialize a global tracing
//! subscriber. Callers pass already-redacted fields through [`field`] and
//! free-form messages through [`text`].

use std::error::Error;

use tracing_subscriber::EnvFilter;

/// Replacement text used for any sensitive value.
pub const REDACTED: &str = "[REDACTED]";

/// Error returned when the global logger has already been initialized.
pub type LogInitError = Box<dyn Error + Send + Sync>;

const SENSITIVE_TOKENS: &[&str] = &[
    "password",
    "passphrase",
    "secret",
    "token",
    "apikey",
    "privatekey",
    "signingkey",
    "seed",
    "mnemonic",
    "authorization",
    "cookie",
    "credential",
    "pin",
    "keystore",
];

/// Initializes the process-wide logger.
///
/// Debug output is opt-in. Environment filters use `RUST_LOG` when set.
pub fn init(debug: bool) -> Result<(), LogInitError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(if debug { "debug" } else { "info" }));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init()?;

    Ok(())
}

/// Returns true when a field name conventionally carries secret material.
#[must_use]
pub fn is_sensitive_field(name: &str) -> bool {
    let mut normalized = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_uppercase() {
            normalized.push(' ');
            normalized.push(character.to_ascii_lowercase());
        } else if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else {
            normalized.push(' ');
        }
    }

    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    tokens.iter().any(|token| SENSITIVE_TOKENS.contains(token))
        || tokens
            .windows(2)
            .any(|pair| SENSITIVE_TOKENS.contains(&format!("{}{}", pair[0], pair[1]).as_str()))
}

/// Formats a field, replacing its value when the field is sensitive.
#[must_use]
pub fn field(name: &str, value: &str) -> String {
    if is_sensitive_field(name) {
        format!("{name}={REDACTED}")
    } else {
        format!("{name}={value}")
    }
}

/// Redacts common credential shapes embedded in free-form debug text.
#[must_use]
pub fn text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = find_assignment(remaining) {
        output.push_str(&remaining[..index]);
        output.push_str(REDACTED);
        remaining = &remaining[index..];
        if let Some(end) = remaining.find(char::is_whitespace) {
            remaining = &remaining[end..];
        } else {
            remaining = "";
        }
    }
    output.push_str(remaining);

    redact_bearer(&output)
}

fn find_assignment(input: &str) -> Option<usize> {
    let lowercase = input.to_ascii_lowercase();
    let markers = [
        "password=",
        "password:",
        "passphrase=",
        "secret=",
        "token=",
        "api_key=",
        "apikey=",
        "private_key=",
        "seed=",
        "mnemonic=",
    ];

    markers
        .iter()
        .filter_map(|marker| lowercase.find(marker))
        .min()
        .map(|index| index + input[index..].find(['=', ':']).unwrap_or(0) + 1)
}

fn redact_bearer(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.to_ascii_lowercase().find("bearer ") {
        output.push_str(&remaining[..index]);
        output.push_str("Bearer ");
        output.push_str(REDACTED);
        remaining = &remaining[index + 7..];
        if let Some(end) = remaining.find(char::is_whitespace) {
            remaining = &remaining[end..];
        } else {
            remaining = "";
        }
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::{REDACTED, field, is_sensitive_field, text};

    #[test]
    fn detects_sensitive_field_names() {
        assert!(is_sensitive_field("apiKey"));
        assert!(is_sensitive_field("access_token"));
        assert!(is_sensitive_field("privateKeyPath"));
        assert!(is_sensitive_field("mnemonic"));
        assert!(!is_sensitive_field("publicKey"));
        assert!(!is_sensitive_field("recordId"));
    }

    #[test]
    fn redacts_fields() {
        assert_eq!(format!("token={REDACTED}"), field("token", "abc123"));
        assert_eq!("peerId=peer-1", field("peerId", "peer-1"));
    }

    #[test]
    fn redacts_free_form_text() {
        let redacted = text("password=hunter2 Authorization: Bearer abc.def.ghi api_key=secret-value");

        assert!(!redacted.contains("hunter2"));
        assert!(!redacted.contains("abc.def.ghi"));
        assert!(!redacted.contains("secret-value"));
    }
}
