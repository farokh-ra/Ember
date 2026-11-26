//! Safe-file classification and user-facing security notices.
//!
//! Ember never executes downloaded content and never opens a preview in
//! process. Classification only warns the person about what a file could do
//! when they choose to open it with their own platform tools. Previews are
//! delegated to the operating system or an application sandbox.

use nexus_crypto::Value;

/// Extensions that are programs or installers on common platforms.
const EXECUTABLE_EXTENSIONS: &[&str] = &[
    "exe", "msi", "msix", "appx", "com", "scr", "pif", "bat", "cmd", "ps1", "psm1", "vbs", "vbe", "wsf", "wsh", "hta",
    "app", "dmg", "pkg", "deb", "rpm", "apk", "jar", "run", "bin", "elf",
];

/// Extensions that run code inside another program.
const SCRIPT_EXTENSIONS: &[&str] = &[
    "js", "mjs", "cjs", "py", "pyw", "rb", "pl", "php", "lua", "sh", "bash", "zsh", "fish", "ahk", "reg", "lnk",
    "scpt", "swf", "svg", "html", "htm", "xhtml",
];

/// Archive and container formats that can hide executables.
const ARCHIVE_EXTENSIONS: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "iso", "cab", "img",
];

/// Document formats with delegated viewers.
const DOCUMENT_EXTENSIONS: &[&str] = &[
    "pdf", "txt", "md", "rtf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "epub", "csv",
];

/// Image, audio, and video formats with delegated viewers.
const MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "mp3", "wav", "flac", "ogg", "m4a", "mp4", "mkv", "mov", "avi",
    "webm",
];

/// Classification of a downloaded file's name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileCategory {
    /// Portable document, text, or office file.
    Document,
    /// Image, audio, or video file.
    Media,
    /// Archive that may contain other files.
    Archive,
    /// Program, installer, or package.
    Executable,
    /// Script executed by another program.
    Script,
    /// The extension is missing or not recognized.
    Unknown,
}

impl FileCategory {
    /// Stable category name used in events and the FFI.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Media => "media",
            Self::Archive => "archive",
            Self::Executable => "executable",
            Self::Script => "script",
            Self::Unknown => "unknown",
        }
    }

    /// Whether opening the file deserves an explicit warning.
    #[must_use]
    pub(crate) const fn warn(self) -> bool {
        matches!(self, Self::Executable | Self::Script | Self::Archive | Self::Unknown)
    }

    /// User-facing warning. Every message repeats that Ember does not run it.
    #[must_use]
    pub(crate) const fn message(self) -> &'static str {
        match self {
            Self::Executable => {
                "This file is a program or installer. Ember never runs downloaded files; open it only if you trust the publisher."
            }
            Self::Script => {
                "This file is active content that another program can run. Ember never runs downloaded files; inspect it with a text editor first."
            }
            Self::Archive => {
                "This archive can contain programs or scripts. Ember never unpacks or runs it; inspect the contents with your platform tools."
            }
            Self::Unknown => {
                "Ember cannot tell what this file type does. It is stored as data and never run; open it only if you trust the publisher."
            }
            Self::Document | Self::Media => {
                "This file is stored as data. Ember never runs it; previews are delegated to your platform's viewer."
            }
        }
    }
}

/// Returns the lower-case extension of a sanitized display name.
fn extension(name: &str) -> Option<&str> {
    let (_, extension) = name.rsplit_once('.')?;
    if extension.is_empty() || extension.len() > 16 {
        return None;
    }
    Some(extension)
}

/// Classifies a downloaded file by its display name.
#[must_use]
pub(crate) fn classify(name: &str) -> FileCategory {
    let Some(extension) = extension(name) else {
        return FileCategory::Unknown;
    };
    let lowered = extension.to_ascii_lowercase();
    if EXECUTABLE_EXTENSIONS.contains(&lowered.as_str()) {
        FileCategory::Executable
    } else if SCRIPT_EXTENSIONS.contains(&lowered.as_str()) {
        FileCategory::Script
    } else if ARCHIVE_EXTENSIONS.contains(&lowered.as_str()) {
        FileCategory::Archive
    } else if DOCUMENT_EXTENSIONS.contains(&lowered.as_str()) {
        FileCategory::Document
    } else if MEDIA_EXTENSIONS.contains(&lowered.as_str()) {
        FileCategory::Media
    } else {
        FileCategory::Unknown
    }
}

/// Canonical safety value attached to download events and listings.
#[must_use]
pub(crate) fn safety_value(name: &str) -> Value {
    let category = classify(name);
    Value::Map(vec![
        (
            Value::Text("category".to_owned()),
            Value::Text(category.as_str().to_owned()),
        ),
        (Value::Text("warn".to_owned()), Value::Bool(category.warn())),
        (
            Value::Text("message".to_owned()),
            Value::Text(category.message().to_owned()),
        ),
        (Value::Text("auto_execute".to_owned()), Value::Bool(false)),
        (Value::Text("preview".to_owned()), Value::Text("delegated".to_owned())),
    ])
}

/// Bounded list of standing safety notices for the interface.
#[must_use]
pub(crate) fn notices() -> Value {
    let notice = |id: &str, severity: &str, title: &str, body: &str| {
        Value::Map(vec![
            (Value::Text("id".to_owned()), Value::Text(id.to_owned())),
            (Value::Text("severity".to_owned()), Value::Text(severity.to_owned())),
            (Value::Text("title".to_owned()), Value::Text(title.to_owned())),
            (Value::Text("body".to_owned()), Value::Text(body.to_owned())),
        ])
    };
    Value::Array(vec![
        notice(
            "no_auto_execution",
            "info",
            "Downloaded files are never run",
            "Ember stores downloads as data. It does not launch programs, installers, or scripts, and it never opens a preview in its own process.",
        ),
        notice(
            "delegated_previews",
            "info",
            "Previews use platform tools",
            "Opening a file hands it to your operating system or an application sandbox. Review the file type warning before you open it.",
        ),
        notice(
            "key_backup",
            "warning",
            "Back up your identity and keys",
            "A lost passphrase or device cannot be recovered by Ember. Keep an encrypted identity backup and its passphrase in separate safe places.",
        ),
        notice(
            "public_visibility",
            "warning",
            "Published records are public",
            "Profiles, publications, and hosted files you publish can be copied and stored by anyone who receives them. Do not publish private information.",
        ),
        notice(
            "local_blocking",
            "info",
            "Blocking stays on this device",
            "Blocks and abuse reports are local records. Ember does not upload them and does not require a central moderation service.",
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::{FileCategory, classify, notices, safety_value};
    use nexus_crypto::Value;

    #[test]
    fn classifies_risky_and_ordinary_extensions() {
        assert_eq!(classify("installer.EXE"), FileCategory::Executable);
        assert_eq!(classify("update.msi"), FileCategory::Executable);
        assert_eq!(classify("tool.py"), FileCategory::Script);
        assert_eq!(classify("page.html"), FileCategory::Script);
        assert_eq!(classify("diagram.svg"), FileCategory::Script);
        assert_eq!(classify("bundle.zip"), FileCategory::Archive);
        assert_eq!(classify("paper.pdf"), FileCategory::Document);
        assert_eq!(classify("photo.webp"), FileCategory::Media);
        assert_eq!(classify("no-extension"), FileCategory::Unknown);
        assert_eq!(classify("trailing."), FileCategory::Unknown);
        assert_eq!(classify(".hidden"), FileCategory::Unknown);
    }

    #[test]
    fn warnings_never_claim_automatic_execution() {
        let value = safety_value("setup.exe");
        let Value::Map(entries) = &value else {
            panic!("safety value must be a map");
        };
        assert!(
            entries
                .iter()
                .any(|(key, value)| { key.as_text() == Some("auto_execute") && matches!(value, Value::Bool(false)) })
        );
        assert!(
            entries
                .iter()
                .any(|(key, value)| { key.as_text() == Some("preview") && value.as_text() == Some("delegated") })
        );
        assert!(matches!(notices(), Value::Array(items) if items.len() == 5));
    }
}
