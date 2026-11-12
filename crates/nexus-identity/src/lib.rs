//! Identity lifecycle and encrypted key storage.

/// Returns the architecture component name for diagnostics.
#[must_use]
pub const fn component_name() -> &'static str {
    "nexus-identity"
}
