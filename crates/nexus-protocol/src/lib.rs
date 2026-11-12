//! Signed record schemas and wire validation.

/// Returns the architecture component name for diagnostics.
#[must_use]
pub const fn component_name() -> &'static str {
    "nexus-protocol"
}
