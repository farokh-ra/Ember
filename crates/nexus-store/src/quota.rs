//! Managed quota, maximum file size, and free-space reserve policy.

use crate::error::{StoreError, StoreResult};

/// One binary gibibyte.
pub const GIB: u64 = 1024 * 1024 * 1024;
/// Maximum supported file size (4 GiB).
pub const MAX_FILE_SIZE: u64 = 4 * GIB;
/// Default managed quota (10 GiB).
pub const DEFAULT_QUOTA_BYTES: u64 = 10 * GIB;
/// Minimum quota exposed by product settings (1 GiB).
pub const MIN_QUOTA_BYTES: u64 = GIB;
/// Maximum quota exposed by product settings (500 GiB).
pub const MAX_QUOTA_BYTES: u64 = 500 * GIB;
/// Free-space reserve that must remain available (1 GiB).
pub const FREE_SPACE_RESERVE: u64 = GIB;

/// Configured quota and reserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuotaPolicy {
    quota_bytes: u64,
    reserve_bytes: u64,
}

impl Default for QuotaPolicy {
    fn default() -> Self {
        Self {
            quota_bytes: DEFAULT_QUOTA_BYTES,
            reserve_bytes: FREE_SPACE_RESERVE,
        }
    }
}

impl QuotaPolicy {
    /// Creates a policy using the product quota range and default reserve.
    pub fn new(quota_bytes: u64) -> StoreResult<Self> {
        Self::with_reserve(quota_bytes, FREE_SPACE_RESERVE)
    }

    /// Creates a policy using the product quota range and an explicit reserve.
    pub fn with_reserve(quota_bytes: u64, reserve_bytes: u64) -> StoreResult<Self> {
        if !(MIN_QUOTA_BYTES..=MAX_QUOTA_BYTES).contains(&quota_bytes) {
            return Err(StoreError::InvalidConfig(format!(
                "managed quota must be between {MIN_QUOTA_BYTES} and {MAX_QUOTA_BYTES} bytes"
            )));
        }
        Self::custom(quota_bytes, reserve_bytes)
    }

    /// Creates a policy with arbitrary limits. This exists for embedded tests
    /// and deterministic harnesses; the product settings surface enforces the
    /// documented 1–500 GiB range through [`QuotaPolicy::new`].
    pub fn custom(quota_bytes: u64, reserve_bytes: u64) -> StoreResult<Self> {
        if quota_bytes == 0 {
            return Err(StoreError::InvalidConfig(
                "managed quota must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            quota_bytes,
            reserve_bytes,
        })
    }

    /// Configured managed quota.
    #[must_use]
    pub const fn quota_bytes(self) -> u64 {
        self.quota_bytes
    }

    /// Configured free-space reserve.
    #[must_use]
    pub const fn reserve_bytes(self) -> u64 {
        self.reserve_bytes
    }

    /// Checks whether `incoming` additional bytes fit within the quota and
    /// leave the free-space reserve intact.
    pub fn check(self, usage: QuotaUsage, incoming: u64, available_disk: u64) -> StoreResult<()> {
        let used = usage.total_bytes();
        let projected = used.saturating_add(incoming);
        if projected > self.quota_bytes {
            return Err(StoreError::QuotaExceeded {
                used,
                incoming,
                quota: self.quota_bytes,
            });
        }
        if available_disk.saturating_sub(incoming) < self.reserve_bytes {
            return Err(StoreError::FreeSpaceReserve {
                available: available_disk,
                incoming,
                reserve: self.reserve_bytes,
            });
        }
        Ok(())
    }

    /// Checks that a single file is within the maximum supported size.
    pub fn check_file_size(self, incoming: u64, max_file_size: u64) -> StoreResult<()> {
        if incoming > max_file_size {
            return Err(StoreError::FileTooLarge {
                actual: incoming,
                limit: max_file_size,
            });
        }
        Ok(())
    }
}

/// Bytes currently counted against the managed quota.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QuotaUsage {
    /// Verified or pending content blobs, counted once per content ID.
    pub managed_bytes: u64,
    /// Verified prefixes of incomplete downloads, counted from the first
    /// allocated byte.
    pub partial_bytes: u64,
}

impl QuotaUsage {
    /// Total bytes counted against the quota.
    #[must_use]
    pub const fn total_bytes(self) -> u64 {
        self.managed_bytes.saturating_add(self.partial_bytes)
    }
}
