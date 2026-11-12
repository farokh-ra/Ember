//! Cryptographic primitives and canonical encoding.

/// Returns the architecture component name for diagnostics.
#[must_use]
pub const fn component_name() -> &'static str {
    "nexus-crypto"
}
