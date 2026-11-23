//! User-controlled hosting limits and host-reported power/network signals.
//!
//! The state is deliberately small and synchronous: inbound file serving reads
//! it on every request, so a paused or blocked host stops advertising and
//! serving immediately. Persistence and upload-rate application live with the
//! engine that owns both the store and the network handle.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use nexus_store::HostingLimits;

/// Sentinel meaning the host has not reported a battery percentage.
pub(crate) const BATTERY_UNKNOWN: u8 = 101;

/// Shared hosting policy and current device signals.
pub(crate) struct HostingState {
    limits: Mutex<HostingLimits>,
    metered: AtomicBool,
    battery_percent: AtomicU8,
}

impl HostingState {
    pub(crate) fn new(limits: HostingLimits) -> Arc<Self> {
        Arc::new(Self {
            limits: Mutex::new(limits),
            metered: AtomicBool::new(false),
            battery_percent: AtomicU8::new(BATTERY_UNKNOWN),
        })
    }

    pub(crate) fn limits(&self) -> HostingLimits {
        *self.limits.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn replace_limits(&self, limits: HostingLimits) {
        *self.limits.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = limits;
    }

    pub(crate) fn upload_bytes_per_second(&self) -> u64 {
        self.limits().upload_bytes_per_second
    }

    pub(crate) fn set_metered(&self, metered: bool) {
        self.metered.store(metered, Ordering::Release);
    }

    pub(crate) fn metered(&self) -> bool {
        self.metered.load(Ordering::Acquire)
    }

    pub(crate) fn set_battery_percent(&self, percent: Option<u8>) {
        let value = percent.filter(|value| *value <= 100).unwrap_or(BATTERY_UNKNOWN);
        self.battery_percent.store(value, Ordering::Release);
    }

    pub(crate) fn battery_percent(&self) -> Option<u8> {
        let value = self.battery_percent.load(Ordering::Acquire);
        (value <= 100).then_some(value)
    }

    /// Why this host may not serve content right now, if it may not.
    ///
    /// Metered networks are opt-in. A battery threshold of zero disables the
    /// battery check; an unknown battery level never blocks hosting.
    pub(crate) fn blocked_reason(&self) -> Option<&'static str> {
        let limits = self.limits();
        if self.metered() && !limits.allow_metered {
            return Some("metered_network");
        }
        if limits.battery_min_percent > 0
            && self
                .battery_percent()
                .is_some_and(|percent| percent < limits.battery_min_percent)
        {
            return Some("battery_low");
        }
        None
    }

    pub(crate) fn serving_allowed(&self) -> bool {
        self.blocked_reason().is_none()
    }
}
