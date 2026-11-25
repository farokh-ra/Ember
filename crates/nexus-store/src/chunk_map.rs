//! Persisted per-chunk completion for multi-source downloads.
//!
//! A download is split into fixed 1 MiB chunks. Stage 17 records a bit per
//! chunk so chunks can arrive out of order, from different peers, and survive
//! a pause or restart. The bitmap is small: a 4 GiB file needs 4,096 bits.

use nexus_protocol::{CHUNK_SIZE_V1, MAX_CHUNKS};

/// Fixed chunk bitmap for one download.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkMap {
    expected_size: u64,
    bits: Vec<u8>,
}

impl ChunkMap {
    /// Builds an empty map for `expected_size` bytes.
    #[must_use]
    pub fn new(expected_size: u64) -> Self {
        let total = chunk_count(expected_size);
        let bytes = usize::try_from(total.div_ceil(8)).unwrap_or(0);
        Self {
            expected_size,
            bits: vec![0; bytes],
        }
    }

    /// Restores a map from stored bytes, rejecting a wrong-sized bitmap.
    #[must_use]
    pub fn from_stored(expected_size: u64, stored: &[u8]) -> Option<Self> {
        let total = chunk_count(expected_size);
        let expected = usize::try_from(total.div_ceil(8)).ok()?;
        if stored.len() != expected {
            return None;
        }
        Some(Self {
            expected_size,
            bits: stored.to_vec(),
        })
    }

    /// Derives a map from a legacy contiguous verified prefix.
    #[must_use]
    pub fn from_prefix(expected_size: u64, prefix: u64) -> Self {
        let mut map = Self::new(expected_size);
        let prefix = prefix.min(expected_size);
        for index in 0..map.total() {
            let Some(length) = map.chunk_length(index) else {
                break;
            };
            let end = chunk_start(index).saturating_add(length);
            if end > prefix {
                break;
            }
            map.set(index);
        }
        map
    }

    /// Exact stored bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bits
    }

    /// Total number of chunks.
    #[must_use]
    pub const fn total(&self) -> u32 {
        chunk_count(self.expected_size)
    }

    /// Length of one chunk, or `None` when the index is out of range.
    #[must_use]
    pub fn chunk_length(&self, index: u32) -> Option<u64> {
        if index >= self.total() {
            return None;
        }
        let start = chunk_start(index);
        Some((self.expected_size - start).min(CHUNK_SIZE_V1))
    }

    /// True when the chunk is recorded as verified.
    #[must_use]
    pub fn is_set(&self, index: u32) -> bool {
        let Some(byte) = usize::try_from(index / 8).ok().and_then(|slot| self.bits.get(slot)) else {
            return false;
        };
        let bit = u8::try_from(index % 8).unwrap_or(0);
        byte & (1_u8 << bit) != 0
    }

    /// Marks one chunk verified.
    pub fn set(&mut self, index: u32) {
        if index >= self.total() {
            return;
        }
        if let Some(byte) = usize::try_from(index / 8).ok().and_then(|slot| self.bits.get_mut(slot)) {
            *byte |= 1_u8 << u8::try_from(index % 8).unwrap_or(0);
        }
    }

    /// Clears one chunk, for example after a failed resume re-verification.
    pub fn clear(&mut self, index: u32) {
        if index >= self.total() {
            return;
        }
        if let Some(byte) = usize::try_from(index / 8).ok().and_then(|slot| self.bits.get_mut(slot)) {
            *byte &= !(1_u8 << u8::try_from(index % 8).unwrap_or(0));
        }
    }

    /// Number of verified bytes, independent of arrival order.
    #[must_use]
    pub fn verified_bytes(&self) -> u64 {
        (0..self.total())
            .filter(|index| self.is_set(*index))
            .filter_map(|index| self.chunk_length(index))
            .fold(0_u64, u64::saturating_add)
    }

    /// Longest verified prefix that starts at byte zero.
    #[must_use]
    pub fn contiguous_prefix(&self) -> u64 {
        let mut prefix = 0_u64;
        for index in 0..self.total() {
            if !self.is_set(index) {
                break;
            }
            let Some(length) = self.chunk_length(index) else {
                break;
            };
            prefix = prefix.saturating_add(length);
        }
        prefix
    }

    /// End of the highest verified chunk, used to trim an unverified tail.
    #[must_use]
    pub fn highest_verified_end(&self) -> u64 {
        (0..self.total())
            .filter(|index| self.is_set(*index))
            .filter_map(|index| {
                let length = self.chunk_length(index)?;
                Some(chunk_start(index).saturating_add(length))
            })
            .max()
            .unwrap_or(0)
    }

    /// True when every chunk is verified.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        (0..self.total()).all(|index| self.is_set(index))
    }

    /// Per-chunk completion flags, for the transfer scheduler.
    #[must_use]
    pub fn completed(&self) -> Vec<bool> {
        (0..self.total()).map(|index| self.is_set(index)).collect()
    }

    /// Number of verified chunks.
    #[must_use]
    pub fn completed_count(&self) -> u32 {
        (0..self.total()).filter(|index| self.is_set(*index)).count() as u32
    }
}

const fn chunk_count(expected_size: u64) -> u32 {
    let total = expected_size.div_ceil(CHUNK_SIZE_V1);
    if total > MAX_CHUNKS as u64 {
        MAX_CHUNKS as u32
    } else {
        total as u32
    }
}

const fn chunk_start(index: u32) -> u64 {
    (index as u64) * CHUNK_SIZE_V1
}

#[cfg(test)]
mod tests {
    use nexus_protocol::CHUNK_SIZE_V1;

    use super::ChunkMap;

    #[test]
    fn empty_map_is_complete_for_an_empty_file() {
        let map = ChunkMap::new(0);
        assert_eq!(map.total(), 0);
        assert!(map.bytes().is_empty());
        assert!(map.is_complete());
        assert_eq!(map.verified_bytes(), 0);
        assert_eq!(map.contiguous_prefix(), 0);
    }

    #[test]
    fn out_of_order_chunks_preserve_a_contiguous_prefix() {
        let size = CHUNK_SIZE_V1 * 3 + 7;
        let mut map = ChunkMap::new(size);
        assert_eq!(map.total(), 4);
        map.set(2);
        assert_eq!(map.verified_bytes(), CHUNK_SIZE_V1);
        assert_eq!(map.contiguous_prefix(), 0);
        assert_eq!(map.highest_verified_end(), CHUNK_SIZE_V1 * 3);
        map.set(0);
        map.set(1);
        assert_eq!(map.contiguous_prefix(), CHUNK_SIZE_V1 * 3);
        map.set(3);
        assert!(map.is_complete());
        assert_eq!(map.contiguous_prefix(), size);
        assert_eq!(map.verified_bytes(), size);
    }

    #[test]
    fn a_legacy_prefix_maps_to_whole_chunks() {
        let size = CHUNK_SIZE_V1 * 2 + 11;
        let map = ChunkMap::from_prefix(size, CHUNK_SIZE_V1 + 5);
        assert!(map.is_set(0));
        assert!(!map.is_set(1));
        assert_eq!(map.contiguous_prefix(), CHUNK_SIZE_V1);
        assert_eq!(map.verified_bytes(), CHUNK_SIZE_V1);
    }

    #[test]
    fn stored_bytes_round_trip_and_reject_the_wrong_size() {
        let size = CHUNK_SIZE_V1 * 9;
        let mut map = ChunkMap::new(size);
        map.set(0);
        map.set(8);
        let restored = ChunkMap::from_stored(size, map.bytes()).expect("valid map");
        assert_eq!(restored, map);
        assert!(ChunkMap::from_stored(size, &[0_u8; 1]).is_none());
    }

    #[test]
    fn clearing_a_chunk_restores_the_prefix() {
        let size = CHUNK_SIZE_V1 * 2;
        let mut map = ChunkMap::new(size);
        map.set(0);
        map.set(1);
        assert_eq!(map.contiguous_prefix(), size);
        map.clear(1);
        assert_eq!(map.contiguous_prefix(), CHUNK_SIZE_V1);
        assert_eq!(map.verified_bytes(), CHUNK_SIZE_V1);
        assert_eq!(map.completed(), vec![true, false]);
    }
}
