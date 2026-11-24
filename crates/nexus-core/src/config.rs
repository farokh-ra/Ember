//! Validated core configuration parsed from the create-time CBOR map.

use std::path::{Component, PathBuf};

use nexus_crypto::{CborError, CborLimits, Value, decode};
use nexus_net::DhtMode;
use nexus_store::{DEFAULT_QUOTA_BYTES, MAX_QUOTA_BYTES, MIN_QUOTA_BYTES};

use crate::ABI_VERSION;

/// Maximum accepted configuration CBOR size.
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;
const MAX_CONFIG_MAP_ENTRIES: usize = 256;
const MAX_APP_VERSION_BYTES: usize = 64;
const MAX_LOCALE_BYTES: usize = 35;

/// Validated configuration for one core instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreConfig {
    /// Absolute app-private root that owns the database, store, and secrets.
    pub data_root: PathBuf,
    /// Host application version shown in diagnostics.
    pub app_version: String,
    /// BCP-47-style locale tag used for local formatting.
    pub locale: String,
    /// Minimum log level.
    pub log_level: LogLevel,
    /// Whether networking should be started with the core.
    pub network_enabled: bool,
    /// Kademlia role: `client` for ordinary devices, `server` for bootstrap nodes.
    pub dht_mode: DhtMode,
    /// Whether a failed direct dial may fall back to a connected community relay.
    pub relay_fallback: bool,
    /// Whether this node also serves as a community relay.
    pub relay_server: bool,
    /// Whether this node answers bounded `/nexus/index/1` queries with its
    /// verified local index and announces an index-provider record.
    pub search_provider: bool,
    /// Managed storage quota.
    pub quota_bytes: u64,
    /// Whether `test.*` debug methods are enabled.
    pub enable_test_methods: bool,
}

/// Supported log levels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevel {
    /// Trace.
    Trace,
    /// Debug.
    Debug,
    /// Informational.
    Info,
    /// Warnings.
    Warn,
    /// Errors only.
    Error,
}

impl LogLevel {
    /// Parses a config value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => return None,
        })
    }

    /// Wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Configuration validation failure.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The config exceeded the 64 KiB ABI limit.
    #[error("config is {actual} bytes, above the {max}-byte limit")]
    TooLarge {
        /// Observed size.
        actual: usize,
        /// Maximum size.
        max: usize,
    },
    /// Canonical CBOR decoding failed.
    #[error("config CBOR is invalid: {0}")]
    Malformed(#[from] CborError),
    /// The top-level value was not a map.
    #[error("config must be a CBOR map")]
    NotMap,
    /// A map key was not text.
    #[error("config keys must be text")]
    NonTextKey,
    /// Too many map entries.
    #[error("config has {actual} entries, above the {max}-entry limit")]
    TooManyEntries {
        /// Observed entries.
        actual: usize,
        /// Maximum entries.
        max: usize,
    },
    /// A required field is missing.
    #[error("config is missing required field {0}")]
    Missing(&'static str),
    /// A field had the wrong type.
    #[error("config field {0} has the wrong type")]
    WrongType(&'static str),
    /// A field value is invalid.
    #[error("config field {0} is invalid")]
    Invalid(&'static str),
    /// The host and library ABI versions differ.
    #[error("ABI version {found} does not match expected version {expected}")]
    AbiMismatch {
        /// Version required by this library.
        expected: u32,
        /// Version supplied by the host.
        found: u64,
    },
}

impl CoreConfig {
    /// Parses and validates a config map.
    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(ConfigError::TooLarge {
                actual: bytes.len(),
                max: MAX_CONFIG_BYTES,
            });
        }
        let value = decode(bytes, CborLimits::new(16, MAX_CONFIG_MAP_ENTRIES * 4, MAX_CONFIG_BYTES))?;
        let entries = value.as_map().ok_or(ConfigError::NotMap)?;
        if entries.len() > MAX_CONFIG_MAP_ENTRIES {
            return Err(ConfigError::TooManyEntries {
                actual: entries.len(),
                max: MAX_CONFIG_MAP_ENTRIES,
            });
        }
        for (key, _) in entries {
            if key.as_text().is_none() {
                return Err(ConfigError::NonTextKey);
            }
        }

        let abi_version = required_unsigned(&value, "abi_version")?;
        if abi_version != u64::from(ABI_VERSION) {
            return Err(ConfigError::AbiMismatch {
                expected: ABI_VERSION,
                found: abi_version,
            });
        }

        let data_root_text = required_text(&value, "data_root")?;
        if data_root_text.contains('\0') {
            return Err(ConfigError::Invalid("data_root"));
        }
        let data_root = PathBuf::from(data_root_text);
        if !data_root.is_absolute()
            || data_root
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return Err(ConfigError::Invalid("data_root"));
        }

        let app_version = optional_text(&value, "app_version")?.unwrap_or("0.0.0");
        if app_version.is_empty() || app_version.len() > MAX_APP_VERSION_BYTES {
            return Err(ConfigError::Invalid("app_version"));
        }
        let locale = optional_text(&value, "locale")?.unwrap_or("en");
        if locale.is_empty() || locale.len() > MAX_LOCALE_BYTES {
            return Err(ConfigError::Invalid("locale"));
        }
        let log_level = match optional_text(&value, "log_level")?.unwrap_or("info") {
            "trace" => LogLevel::Trace,
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => return Err(ConfigError::Invalid("log_level")),
        };
        let network_enabled = optional_bool(&value, "network_enabled")?.unwrap_or(false);
        let dht_mode = match optional_text(&value, "dht_mode")?.unwrap_or("client") {
            "client" => DhtMode::Client,
            "server" => DhtMode::Server,
            _ => return Err(ConfigError::Invalid("dht_mode")),
        };
        let quota_bytes = optional_unsigned(&value, "quota_bytes")?.unwrap_or(DEFAULT_QUOTA_BYTES);
        if !(MIN_QUOTA_BYTES..=MAX_QUOTA_BYTES).contains(&quota_bytes) {
            return Err(ConfigError::Invalid("quota_bytes"));
        }
        let relay_fallback = optional_bool(&value, "relay_fallback")?.unwrap_or(true);
        let relay_server = optional_bool(&value, "relay_server")?.unwrap_or(false);
        let search_provider = optional_bool(&value, "search_provider")?.unwrap_or(false);
        let enable_test_methods = optional_bool(&value, "enable_test_methods")?.unwrap_or(false);

        Ok(Self {
            data_root,
            app_version: app_version.to_owned(),
            locale: locale.to_owned(),
            log_level,
            network_enabled,
            dht_mode,
            relay_fallback,
            relay_server,
            search_provider,
            quota_bytes,
            enable_test_methods,
        })
    }
}

fn map_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Map(entries) = value else {
        return None;
    };
    entries
        .iter()
        .find_map(|(candidate, value)| (candidate.as_text() == Some(key)).then_some(value))
}

fn required_unsigned(value: &Value, key: &'static str) -> Result<u64, ConfigError> {
    map_get(value, key)
        .ok_or(ConfigError::Missing(key))?
        .as_unsigned()
        .ok_or(ConfigError::WrongType(key))
}

fn optional_unsigned(value: &Value, key: &'static str) -> Result<Option<u64>, ConfigError> {
    match map_get(value, key) {
        None => Ok(None),
        Some(value) => value.as_unsigned().map(Some).ok_or(ConfigError::WrongType(key)),
    }
}

fn required_text<'a>(value: &'a Value, key: &'static str) -> Result<&'a str, ConfigError> {
    map_get(value, key)
        .ok_or(ConfigError::Missing(key))?
        .as_text()
        .ok_or(ConfigError::WrongType(key))
}

fn optional_text<'a>(value: &'a Value, key: &'static str) -> Result<Option<&'a str>, ConfigError> {
    match map_get(value, key) {
        None => Ok(None),
        Some(value) => value.as_text().map(Some).ok_or(ConfigError::WrongType(key)),
    }
}

fn optional_bool(value: &Value, key: &'static str) -> Result<Option<bool>, ConfigError> {
    match map_get(value, key) {
        None => Ok(None),
        Some(value) => value.as_bool().map(Some).ok_or(ConfigError::WrongType(key)),
    }
}

/// Encodes a test configuration with canonical CBOR.
#[cfg(test)]
pub(crate) fn encode_config_for_test(entries: &[(&str, Value)]) -> Vec<u8> {
    let entries = entries
        .iter()
        .map(|(key, value)| (Value::Text((*key).to_owned()), value.clone()))
        .collect();
    Value::Map(entries)
        .to_canonical_bytes_with_limits(CborLimits::new(16, 64, MAX_CONFIG_BYTES))
        .expect("encode config")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use nexus_crypto::Value;
    use nexus_net::DhtMode;

    use super::{CoreConfig, LogLevel, encode_config_for_test};
    use crate::ABI_VERSION;

    fn valid_entries(root: &Path) -> Vec<(&'static str, Value)> {
        vec![
            ("abi_version", Value::Unsigned(u64::from(ABI_VERSION))),
            ("data_root", Value::Text(root.to_string_lossy().into_owned())),
            ("app_version", Value::Text("0.1.0-test".to_owned())),
            ("locale", Value::Text("en-US".to_owned())),
            ("log_level", Value::Text("debug".to_owned())),
            ("network_enabled", Value::Bool(false)),
            ("dht_mode", Value::Text("server".to_owned())),
            ("quota_bytes", Value::Unsigned(nexus_store::MIN_QUOTA_BYTES)),
            ("relay_fallback", Value::Bool(false)),
            ("relay_server", Value::Bool(true)),
            ("enable_test_methods", Value::Bool(true)),
        ]
    }

    #[test]
    fn parses_all_documented_fields() {
        let root = std::env::temp_dir().join("nexus-config-test");
        let config = CoreConfig::parse(&encode_config_for_test(&valid_entries(&root))).expect("config");
        assert_eq!(config.data_root, root);
        assert_eq!(config.app_version, "0.1.0-test");
        assert_eq!(config.locale, "en-US");
        assert_eq!(config.log_level, LogLevel::Debug);
        assert!(!config.network_enabled);
        assert_eq!(config.dht_mode, DhtMode::Server);
        assert!(!config.relay_fallback);
        assert!(config.relay_server);
        assert_eq!(config.quota_bytes, nexus_store::MIN_QUOTA_BYTES);
        assert!(config.enable_test_methods);
    }

    #[test]
    fn applies_documented_defaults() {
        let root = std::env::temp_dir().join("nexus-config-defaults");
        let bytes = encode_config_for_test(&[
            ("abi_version", Value::Unsigned(u64::from(ABI_VERSION))),
            ("data_root", Value::Text(root.to_string_lossy().into_owned())),
        ]);
        let config = CoreConfig::parse(&bytes).expect("config");
        assert_eq!(config.app_version, "0.0.0");
        assert_eq!(config.locale, "en");
        assert_eq!(config.log_level, LogLevel::Info);
        assert!(!config.network_enabled);
        assert_eq!(config.dht_mode, DhtMode::Client);
        assert!(config.relay_fallback);
        assert!(!config.relay_server);
        assert!(!config.enable_test_methods);
    }

    #[test]
    fn rejects_abi_mismatch_and_bad_roots() {
        let root = std::env::temp_dir().join("nexus-config-mismatch");
        let mut entries = valid_entries(&root);
        entries[0].1 = Value::Unsigned(u64::from(ABI_VERSION) + 1);
        let error = CoreConfig::parse(&encode_config_for_test(&entries)).expect_err("ABI mismatch");
        assert!(matches!(
            error,
            super::ConfigError::AbiMismatch {
                expected,
                found
            } if expected == ABI_VERSION && found == u64::from(ABI_VERSION) + 1
        ));

        let mut entries = valid_entries(&root);
        entries[1].1 = Value::Text("relative/path".to_owned());
        assert!(CoreConfig::parse(&encode_config_for_test(&entries)).is_err());

        let mut entries = valid_entries(&root);
        entries[1].1 = Value::Text("../escape".to_owned());
        assert!(CoreConfig::parse(&encode_config_for_test(&entries)).is_err());

        let mut entries = valid_entries(&root);
        entries[6].1 = Value::Text("automatic".to_owned());
        assert!(matches!(
            CoreConfig::parse(&encode_config_for_test(&entries)),
            Err(super::ConfigError::Invalid("dht_mode"))
        ));
    }

    #[test]
    fn rejects_oversized_config_before_parsing() {
        let error = CoreConfig::parse(&vec![0_u8; super::MAX_CONFIG_BYTES + 1]).expect_err("oversized");
        assert!(matches!(error, super::ConfigError::TooLarge { .. }));
    }
}
