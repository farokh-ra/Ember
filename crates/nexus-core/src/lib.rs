//! NEXUS lifecycle orchestration and command/event bus.

use nexus_observability::LogInitError;

/// Native ABI version exported to Kotlin clients.
pub const ABI_VERSION: u32 = 1;

/// Protocol version implemented by this build.
pub const PROTOCOL_VERSION: &str = "0.1.0-stage-03";

/// Static build information exposed to shells and diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreInfo {
    /// Native ABI version.
    pub abi_version: u32,
    /// Protocol version string.
    pub protocol_version: &'static str,
}

/// Returns static build information.
#[must_use]
pub const fn info() -> CoreInfo {
    CoreInfo {
        abi_version: ABI_VERSION,
        protocol_version: PROTOCOL_VERSION,
    }
}

/// Initializes redacted structured logging for the core process.
pub fn init_logging(debug: bool) -> Result<(), LogInitError> {
    nexus_observability::init(debug)
}

#[cfg(test)]
mod tests {
    use super::{ABI_VERSION, info};

    #[test]
    fn reports_abi_version() {
        assert_eq!(ABI_VERSION, info().abi_version);
        assert_eq!("0.1.0-stage-03", info().protocol_version);
    }
}
