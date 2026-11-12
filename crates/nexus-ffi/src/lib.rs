//! C ABI surface for the NEXUS core.
//!
//! Stage 03 exports only the ABI version. The full operation and event
//! envelope surface is implemented in a later stage.

#![allow(unsafe_code)]

/// Returns the native ABI version expected by the Kotlin client.
///
/// # Safety
///
/// This function has no preconditions and does not dereference pointers.
#[unsafe(no_mangle)]
pub extern "C" fn nexus_abi_version() -> u32 {
    nexus_core::ABI_VERSION
}

#[cfg(test)]
mod tests {
    use super::nexus_abi_version;

    #[test]
    fn exports_abi_version() {
        assert_eq!(1, nexus_abi_version());
    }
}
