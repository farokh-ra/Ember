//! Multi-source chunk scheduling and observed peer reliability.
//!
//! Stage 17 lets one download take different chunks from different peers. The
//! scheduler in this module is pure policy: it owns the pending chunk set, the
//! in-flight leases, and per-peer transfer statistics. It never performs I/O
//! and never reads identity or popularity data. A peer becomes "proven" only
//! after it serves a chunk whose SHA-256 matches the signed manifest, so a
//! peer that merely answers metadata is not trusted with the file.
//!
//! The caller serialises access (one mutex) and performs the network calls. A
//! lease guarantees that one chunk is in flight from at most one peer.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

/// Maximum peers that may serve one download at the same time.
pub const MAX_PARALLEL_SOURCES: usize = 4;

/// Total attempts for one chunk across every source before the download fails.
pub const MAX_CHUNK_ATTEMPTS: u32 = 4;

/// Corrupt responses before a peer is removed from the source pool.
pub const CORRUPTION_QUARANTINE: u64 = 2;

/// Consecutive transport failures before a peer is removed from the source pool.
pub const FAILURE_QUARANTINE: u64 = 3;

/// Why a source did not deliver a usable chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFailure {
    /// The peer was unreachable, refused the request, or lacked the content.
    Unavailable,
    /// The peer returned bytes that did not match the authenticated chunk hash.
    Corrupt,
}

/// One chunk assigned to one peer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkLease {
    /// Zero-based chunk index.
    pub index: u32,
    /// Peer selected for this attempt.
    pub peer: String,
    /// One-based attempt number for this chunk.
    pub attempt: u32,
}

/// What the caller should do next.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaseDecision {
    /// Fetch the leased chunk from the named peer.
    Lease(ChunkLease),
    /// Every usable source is busy or a chunk is still in flight; wait.
    Wait,
    /// Every chunk is verified.
    Done,
    /// The download cannot complete from the remaining sources.
    Failed(ScheduleFailure),
}

/// A terminal scheduling failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleFailure {
    /// No usable source remains while chunks are still pending.
    NoUsableSource,
    /// One chunk consumed every attempt without a verified response.
    ChunkExhausted(u32),
}

/// Observed transfer statistics for one peer.
#[derive(Clone, Debug)]
pub struct SourceStats {
    /// Peer identifier exactly as supplied by the caller.
    pub peer: String,
    /// Chunk requests handed to this peer.
    pub attempts: u64,
    /// Requests that produced an authenticated chunk.
    pub successes: u64,
    /// Requests that failed in transport or availability.
    pub failures: u64,
    /// Requests that returned bytes failing the chunk hash.
    pub corruptions: u64,
    /// Verified bytes delivered by this peer.
    pub bytes: u64,
    /// Exponentially weighted mean chunk latency in milliseconds.
    pub latency_ms: u64,
    /// True after the peer served at least one authenticated chunk.
    pub proven: bool,
    /// True once the peer is excluded for the rest of this download.
    pub quarantined: bool,
    failure_streak: u64,
}

impl SourceStats {
    fn new(peer: String) -> Self {
        Self {
            peer,
            attempts: 0,
            successes: 0,
            failures: 0,
            corruptions: 0,
            bytes: 0,
            latency_ms: 0,
            proven: false,
            quarantined: false,
            failure_streak: 0,
        }
    }

    /// Reliability score derived only from observed transfer outcomes.
    ///
    /// Proven peers are preferred, fast peers outrank slow peers, and corrupt
    /// responses are penalised twice: once as an attempt and once directly.
    #[must_use]
    pub fn score(&self) -> f64 {
        if self.quarantined {
            return 0.0;
        }
        let attempts = self.attempts as f64;
        let reliability = (self.successes as f64 + 1.0) / (attempts + 2.0);
        let integrity = 1.0 / (1.0 + self.corruptions as f64);
        let speed = 1.0 / (1.0 + self.latency_ms as f64 / 1_000.0);
        let trust = if self.proven { 1.0 } else { 0.25 };
        reliability * integrity * speed * trust
    }
}

/// Pending, in-flight, and completed chunk state for one download.
#[derive(Debug)]
pub struct MultiSourceScheduler {
    total_chunks: u32,
    completed: Vec<bool>,
    pending: BTreeSet<u32>,
    in_flight: BTreeMap<u32, ChunkLease>,
    busy: BTreeSet<String>,
    attempts: Vec<u32>,
    sources: Vec<SourceStats>,
    failure: Option<ScheduleFailure>,
    saw_corruption: bool,
}

impl MultiSourceScheduler {
    /// Builds a schedule over `completed`, one entry per chunk.
    ///
    /// Duplicate source IDs are ignored. An empty completion slice means the
    /// file has no chunks; the schedule is immediately `Done`.
    #[must_use]
    pub fn new(sources: Vec<String>, completed: &[bool]) -> Self {
        let mut seen = BTreeSet::new();
        let sources = sources
            .into_iter()
            .filter(|peer| !peer.is_empty() && seen.insert(peer.clone()))
            .map(SourceStats::new)
            .collect();
        let total_chunks = u32::try_from(completed.len()).unwrap_or(u32::MAX);
        let pending = completed
            .iter()
            .enumerate()
            .filter_map(|(index, done)| if *done { None } else { u32::try_from(index).ok() })
            .collect();
        Self {
            total_chunks,
            completed: completed.to_vec(),
            pending,
            in_flight: BTreeMap::new(),
            busy: BTreeSet::new(),
            attempts: vec![0; completed.len()],
            sources,
            failure: None,
            saw_corruption: false,
        }
    }

    /// Total chunk count.
    #[must_use]
    pub const fn total_chunks(&self) -> u32 {
        self.total_chunks
    }

    /// Chunks still waiting for a lease.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Chunks verified so far.
    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.completed.iter().filter(|done| **done).count()
    }

    /// True when every chunk is verified.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.completed.iter().all(|done| *done)
    }

    /// True when at least one peer returned an unauthenticated chunk.
    #[must_use]
    pub const fn saw_corruption(&self) -> bool {
        self.saw_corruption
    }

    /// Current per-peer statistics, in the order the peers were supplied.
    #[must_use]
    pub fn stats(&self) -> &[SourceStats] {
        &self.sources
    }

    /// Terminal scheduling failure, if one was reached.
    #[must_use]
    pub const fn failure(&self) -> Option<ScheduleFailure> {
        self.failure
    }

    /// Returns the next action. Callers must not lease again until the previous
    /// lease is reported through [`Self::record_success`] or
    /// [`Self::record_failure`].
    pub fn next_lease(&mut self) -> LeaseDecision {
        if let Some(failure) = self.failure {
            return LeaseDecision::Failed(failure);
        }
        if self.is_complete() {
            return LeaseDecision::Done;
        }
        let Some(index) = self.pending.iter().next().copied() else {
            return if self.in_flight.is_empty() {
                LeaseDecision::Done
            } else {
                LeaseDecision::Wait
            };
        };
        let Some(peer) = self.best_source() else {
            if self.in_flight.is_empty() {
                self.failure = Some(ScheduleFailure::NoUsableSource);
                return LeaseDecision::Failed(ScheduleFailure::NoUsableSource);
            }
            return LeaseDecision::Wait;
        };
        let attempt = self.attempts[usize::try_from(index).unwrap_or(0)].saturating_add(1);
        self.attempts[usize::try_from(index).unwrap_or(0)] = attempt;
        self.pending.remove(&index);
        self.busy.insert(peer.clone());
        if let Some(stats) = self.sources.iter_mut().find(|stats| stats.peer == peer) {
            stats.attempts = stats.attempts.saturating_add(1);
        }
        let lease = ChunkLease { index, peer, attempt };
        self.in_flight.insert(index, lease.clone());
        LeaseDecision::Lease(lease)
    }

    /// Records an authenticated chunk.
    pub fn record_success(&mut self, lease: &ChunkLease, bytes: u64, elapsed: Duration) {
        if !self.release(lease) {
            return;
        }
        if let Some(slot) = self
            .completed
            .get_mut(usize::try_from(lease.index).unwrap_or(usize::MAX))
        {
            *slot = true;
        }
        let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        if let Some(stats) = self.sources.iter_mut().find(|stats| stats.peer == lease.peer) {
            stats.successes = stats.successes.saturating_add(1);
            stats.bytes = stats.bytes.saturating_add(bytes);
            stats.latency_ms = if stats.proven {
                stats.latency_ms.saturating_mul(3).saturating_add(elapsed_ms) / 4
            } else {
                elapsed_ms
            };
            stats.proven = true;
            stats.failure_streak = 0;
        }
    }

    /// Records a failed or corrupt chunk and returns it to the pending set.
    pub fn record_failure(&mut self, lease: &ChunkLease, kind: SourceFailure) {
        if !self.release(lease) {
            return;
        }
        if let Some(stats) = self.sources.iter_mut().find(|stats| stats.peer == lease.peer) {
            match kind {
                SourceFailure::Unavailable => {
                    stats.failures = stats.failures.saturating_add(1);
                    stats.failure_streak = stats.failure_streak.saturating_add(1);
                    if stats.failure_streak >= FAILURE_QUARANTINE {
                        stats.quarantined = true;
                    }
                }
                SourceFailure::Corrupt => {
                    stats.corruptions = stats.corruptions.saturating_add(1);
                    stats.failure_streak = stats.failure_streak.saturating_add(1);
                    self.saw_corruption = true;
                    if stats.corruptions >= CORRUPTION_QUARANTINE {
                        stats.quarantined = true;
                    }
                }
            }
        }
        let slot = usize::try_from(lease.index).unwrap_or(usize::MAX);
        if self.attempts.get(slot).copied().unwrap_or(0) >= MAX_CHUNK_ATTEMPTS {
            self.failure = Some(ScheduleFailure::ChunkExhausted(lease.index));
        } else {
            self.pending.insert(lease.index);
        }
    }

    fn release(&mut self, lease: &ChunkLease) -> bool {
        let Some(current) = self.in_flight.get(&lease.index) else {
            return false;
        };
        if current != lease {
            return false;
        }
        self.in_flight.remove(&lease.index);
        self.busy.remove(&lease.peer);
        true
    }

    fn best_source(&self) -> Option<String> {
        let mut best: Option<&SourceStats> = None;
        for source in &self.sources {
            if source.quarantined || self.busy.contains(&source.peer) {
                continue;
            }
            let replace = match best {
                None => true,
                Some(current) => {
                    let ordering = source.score().total_cmp(&current.score());
                    ordering.is_gt() || (ordering.is_eq() && source.peer < current.peer)
                }
            };
            if replace {
                best = Some(source);
            }
        }
        best.map(|stats| stats.peer.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{ChunkLease, LeaseDecision, MAX_CHUNK_ATTEMPTS, MultiSourceScheduler, ScheduleFailure, SourceFailure};

    fn scheduler(sources: &[&str], chunks: usize) -> MultiSourceScheduler {
        MultiSourceScheduler::new(
            sources.iter().map(|peer| (*peer).to_owned()).collect(),
            &vec![false; chunks],
        )
    }

    fn lease(decision: LeaseDecision) -> ChunkLease {
        match decision {
            LeaseDecision::Lease(lease) => lease,
            other => panic!("expected a lease, got {other:?}"),
        }
    }

    #[test]
    fn skips_completed_chunks_and_leases_the_lowest_index() {
        let mut schedule = MultiSourceScheduler::new(vec!["peer-a".to_owned()], &[true, false, false]);
        let first = lease(schedule.next_lease());
        assert_eq!(first.index, 1);
        assert_eq!(first.attempt, 1);
        schedule.record_success(&first, 4, Duration::from_millis(5));
        let second = lease(schedule.next_lease());
        assert_eq!(second.index, 2);
        schedule.record_success(&second, 4, Duration::from_millis(5));
        assert_eq!(schedule.next_lease(), LeaseDecision::Done);
        assert!(schedule.is_complete());
    }

    #[test]
    fn one_chunk_is_in_flight_from_at_most_one_peer() {
        let mut schedule = scheduler(&["peer-a", "peer-b"], 4);
        let first = lease(schedule.next_lease());
        let second = lease(schedule.next_lease());
        assert_ne!(first.peer, second.peer);
        assert_ne!(first.index, second.index);
        assert_eq!(schedule.next_lease(), LeaseDecision::Wait);
    }

    #[test]
    fn a_proven_source_is_preferred_over_an_unproven_one() {
        let mut schedule = scheduler(&["peer-a", "peer-b"], 4);
        let first = lease(schedule.next_lease());
        assert_eq!(first.peer, "peer-a");
        schedule.record_success(&first, 8, Duration::from_millis(10));
        let next = lease(schedule.next_lease());
        assert_eq!(next.peer, "peer-a", "the proven source should keep the work");
        assert!(!schedule.stats()[1].proven);
    }

    #[test]
    fn a_corrupt_chunk_moves_to_another_source() {
        let mut schedule = scheduler(&["peer-a", "peer-b"], 2);
        let first = lease(schedule.next_lease());
        schedule.record_failure(&first, SourceFailure::Corrupt);
        let retry = lease(schedule.next_lease());
        assert_eq!(retry.index, first.index);
        assert_eq!(retry.peer, "peer-b");
        assert!(schedule.saw_corruption());
        assert_eq!(schedule.stats()[0].corruptions, 1);
        assert!(!schedule.stats()[0].quarantined);
    }

    #[test]
    fn a_persistently_corrupt_source_is_quarantined() {
        let mut schedule = scheduler(&["peer-a"], 1);
        let first = lease(schedule.next_lease());
        schedule.record_failure(&first, SourceFailure::Corrupt);
        let second = lease(schedule.next_lease());
        schedule.record_failure(&second, SourceFailure::Corrupt);
        assert!(schedule.stats()[0].quarantined);
        assert_eq!(
            schedule.next_lease(),
            LeaseDecision::Failed(ScheduleFailure::NoUsableSource)
        );
    }

    #[test]
    fn one_chunk_exhausts_every_attempt_across_two_sources() {
        let mut schedule = scheduler(&["peer-a", "peer-b"], 1);
        for _ in 0..MAX_CHUNK_ATTEMPTS {
            let current = lease(schedule.next_lease());
            schedule.record_failure(&current, SourceFailure::Unavailable);
        }
        assert_eq!(
            schedule.next_lease(),
            LeaseDecision::Failed(ScheduleFailure::ChunkExhausted(0))
        );
        assert!(!schedule.stats()[0].quarantined);
        assert!(!schedule.stats()[1].quarantined);
    }

    #[test]
    fn transport_failures_quarantine_a_source_after_a_bounded_streak() {
        let mut schedule = scheduler(&["peer-a"], 1);
        for _ in 0..super::FAILURE_QUARANTINE {
            let current = lease(schedule.next_lease());
            assert_eq!(current.peer, "peer-a");
            schedule.record_failure(&current, SourceFailure::Unavailable);
        }
        assert!(schedule.stats()[0].quarantined);
        assert_eq!(
            schedule.next_lease(),
            LeaseDecision::Failed(ScheduleFailure::NoUsableSource)
        );
    }

    #[test]
    fn two_sources_halve_the_simulated_makespan() {
        fn rounds(sources: &[&str], chunks: usize) -> usize {
            let mut schedule = scheduler(sources, chunks);
            let mut elapsed = 0_usize;
            while !schedule.is_complete() {
                let mut leased = Vec::new();
                loop {
                    match schedule.next_lease() {
                        LeaseDecision::Lease(current) => leased.push(current),
                        LeaseDecision::Wait => break,
                        LeaseDecision::Done => break,
                        LeaseDecision::Failed(failure) => panic!("unexpected failure {failure:?}"),
                    }
                }
                assert!(!leased.is_empty());
                elapsed += 10;
                for current in leased {
                    schedule.record_success(&current, 8, Duration::from_millis(10));
                }
            }
            elapsed
        }

        let sequential = rounds(&["peer-a"], 8);
        let parallel = rounds(&["peer-a", "peer-b"], 8);
        assert_eq!(sequential, 80);
        assert_eq!(parallel, 40);
    }

    #[test]
    fn score_uses_only_observed_outcomes() {
        let mut schedule = scheduler(&["peer-a"], 2);
        let before = schedule.stats()[0].score();
        let first = lease(schedule.next_lease());
        schedule.record_success(&first, 8, Duration::from_millis(100));
        let after_success = schedule.stats()[0].score();
        assert!(after_success > before);
        let second = lease(schedule.next_lease());
        schedule.record_failure(&second, SourceFailure::Corrupt);
        let after_corruption = schedule.stats()[0].score();
        assert!(after_corruption < after_success);
    }
}
