//! Opaque handle registry for live core instances.
//!
//! Handles are opaque `u64` tokens, never pointers. Removing a handle is the
//! first step of `destroy`, so no new call can resolve it afterwards.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use nexus_core::Engine;

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<Engine>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<u64, Arc<Engine>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<u64, Arc<Engine>>> {
    registry().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Inserts a started core and returns its opaque handle.
pub(crate) fn insert(engine: Engine) -> u64 {
    let handle = loop {
        let candidate = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        if candidate != 0 {
            break candidate;
        }
    };
    lock().insert(handle, Arc::new(engine));
    handle
}

/// Resolves a handle for one call.
pub(crate) fn get(handle: u64) -> Option<Arc<Engine>> {
    lock().get(&handle).cloned()
}

/// Removes a handle so no further call can resolve it.
pub(crate) fn remove(handle: u64) -> Option<Arc<Engine>> {
    lock().remove(&handle)
}

/// Number of live handles, used by leak checks in native tests.
#[cfg(test)]
pub(crate) fn len() -> usize {
    lock().len()
}

#[cfg(test)]
mod tests {
    use super::{get, insert, len, remove};
    use nexus_core::Engine;

    fn test_engine(label: &str) -> Engine {
        let root = std::env::temp_dir().join(format!(
            "nexus-ffi-registry-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let config = nexus_core::CoreConfig {
            data_root: root,
            app_version: "test".to_owned(),
            locale: "en".to_owned(),
            log_level: nexus_core::LogLevel::Error,
            network_enabled: false,
            dht_mode: nexus_core::DhtMode::Client,
            relay_fallback: true,
            relay_server: false,
            quota_bytes: nexus_store::MIN_QUOTA_BYTES,
            enable_test_methods: true,
        };
        Engine::create(config, &[7_u8; 32]).expect("engine")
    }

    #[test]
    fn handles_round_trip_and_remove_once() {
        let before = len();
        let handle = insert(test_engine("round-trip"));
        assert!(handle != 0);
        assert!(get(handle).is_some());
        assert!(remove(handle).is_some());
        assert!(get(handle).is_none());
        assert!(remove(handle).is_none());
        assert_eq!(len(), before);
    }
}
