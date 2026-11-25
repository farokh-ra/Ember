# Stage 17 - Multi-Peer Downloads and Advanced Resilience

**Status:** Complete on Windows loopback; review and merge pending
**Completed:** 2026-11-25
**Branch:** `feat/17-multi-peer`, based on `feat/16-search` at `4e42574`
**Progress guide:** 81-85%
**Next stage:** 18 - security, abuse resistance, and privacy hardening (requires
explicit project-owner approval)

## 1. Scope

Stage 17 removes the single-source transfer limit. One download may now take
different chunks from several connected peers at the same time, verify each
chunk against the signed manifest, and keep going when one source becomes
slow, corrupt, or disappears. There is still no central download service, no
erasure coding, and no trust in a peer's claim of possession.

The stage covers:

- Bounded parallel chunk transfer from up to four connected sources.
- Per-chunk SHA-256 verification against the authenticated manifest before any
  byte reaches the partial file.
- A pure scheduler that owns pending chunks, one in-flight lease per chunk, and
  one lease per peer.
- Source scoring from observed transfer outcomes only: authenticated chunks,
  transport failures, corrupt responses, and measured latency.
- Retry of a failed or corrupt chunk from another source.
- Quarantine of a source that returns two corrupt responses or three
  consecutive transport failures.
- Per-chunk persistence in schema version 10 so out-of-order chunks survive
  pause, failure, and restart.
- Cross-peer resume: verify recorded chunks locally, clear anything corrupt or
  truncated, and continue from the missing chunks, possibly through a different
  peer.
- Adversarial transfer tests, including a malicious peer that serves corrupt
  chunks and a source that stops answering.

## 2. Architecture

Stage 17 keeps transfer policy separate from I/O and storage:

| Crate | Responsibility |
|---|---|
| `nexus-transfer` | `MultiSourceScheduler`: pending and in-flight chunk state, peer ranking, leases, retry, and quarantine policy. No I/O. |
| `nexus-store` | Per-chunk bitmap (`ChunkMap`), schema migration, checkpointing, clearing a chunk, truncating the unverified tail, quota accounting, and reconciliation. |
| `nexus-core` | `0x0801`-`0x0806` download dispatch, metadata verification, bounded worker threads, scheduler progress, retry backoff, resume re-verification, and events. |

`nexus-protocol` and `nexus-net` are unchanged. Stage 17 reuses the existing
`/nexus/file/1` metadata and 64 KiB chunk-slice protocol. No new frame type,
network protocol, or FFI method was added.

## 3. Multi-source scheduler

`crates/nexus-transfer/src/scheduler.rs` is a deterministic, I/O-free policy
engine. It tracks:

- The pending chunk set and the verified chunk set.
- At most one in-flight lease per chunk.
- At most one busy lease per peer.
- Per-peer `SourceStats`: attempts, authenticated successes, transport
  failures, corrupt responses, verified bytes, exponentially weighted latency,
  `proven`, and `quarantined`.

A peer is `proven` only after it serves a chunk whose hash matches the signed
manifest. A peer that only answered metadata is not trusted with file content.
The score is:

```text
reliability = (successes + 1) / (attempts + 2)
integrity   = 1 / (1 + corruptions)
speed       = 1 / (1 + latency_ms / 1000)
trust       = proven ? 1.0 : 0.25
score       = reliability * integrity * speed * trust
```

Identity, username, publication popularity, provider claims, and peer count
never enter the score. A quarantined peer scores zero and is skipped.

`next_lease()` returns one of:

| Decision | Meaning |
|---|---|
| `Lease { index, peer, attempt }` | Fetch this chunk from this peer now. |
| `Wait` | Every usable source is busy or a chunk is still in flight. |
| `Done` | Every chunk is verified. |
| `Failed(NoUsableSource)` | Chunks are pending but no source can be used. |
| `Failed(ChunkExhausted(index))` | One chunk used every attempt without a verified response. |

Scheduler bounds:

| Constant | Value |
|---|---|
| Parallel sources | 4 |
| Attempts per chunk across every source | 4 |
| Corrupt responses before quarantine | 2 |
| Consecutive transport failures before quarantine | 3 |

With one source the scheduler reproduces the Stage 12 sequential behavior:
one lease at a time in ascending chunk order. With several sources it fills
each free peer with the lowest pending chunk.

## 4. Per-chunk persistence

Schema version 10 adds two columns to `downloads`:

- `verified_bytes` is the quota charge and counts every verified chunk,
  including chunks beyond a gap.
- `chunk_map` is a little-endian bitmap with one bit per 1 MiB chunk.

`verified_prefix` remains the longest contiguous verified prefix for existing
callers. The migration backfills `verified_bytes` from `verified_prefix`;
pre-Stage-17 rows have a `NULL` chunk map and are treated as prefix-only until
their next checkpoint.

`ChunkMap` provides construction from size or prefix, stored-byte parsing,
`set`/`clear`, `is_set`, `verified_bytes`, `contiguous_prefix`,
`highest_verified_end`, `is_complete`, `completed`, and `completed_count`. Its
size is capped by the existing `MAX_CHUNKS` protocol bound, so a bitmap can
never grow beyond the file model.

Store methods added or rewritten:

| Method | Behaviour |
|---|---|
| `checkpoint_chunk` | Records one verified chunk, requires the partial file to already extend to the chunk end, charges only newly verified bytes, and refreshes `verified_prefix`, `verified_bytes`, and `chunk_map` in one transaction. |
| `clear_chunk` | Removes one chunk from the map when resume re-verification fails. |
| `trim_unverified_tail` | Truncates the partial file to the end of the highest verified chunk. |
| `checkpoint_partial` | Legacy prefix checkpoint; now maintains the bitmap from the prefix. |

Quota usage sums `verified_bytes`, so out-of-order verified chunks count
immediately. Completion still requires `chunk_map.is_complete()` and
`verified_prefix == expected_size`.

## 5. Transfer lifecycle

`download_chunks` now starts one transfer worker per usable source, capped at
four, and one job still runs on the bounded download pool. Shared state holds
the scheduler, progress counters, stop flag, and the current error. A worker
loops:

1. Ask the scheduler for a lease.
2. Fetch the chunk as 64 KiB slices.
3. Assemble the chunk in memory and compare its SHA-256 with the manifest entry.
4. On success, write it at the chunk offset, flush, checkpoint the chunk, and
   record the observed latency and bytes.
5. On a corrupt response, discard the bytes, report a corruption, and release
   the chunk for another peer.
6. On a transport error, report a failure and release the chunk.
7. On pause, cancel, or stop, exit without writing the in-flight chunk.

A retry waits 150 ms, and the wait is interruptible by pause or cancel. Chunks
may arrive out of order. The final whole-file SHA-256 check and promotion path
from Stage 12 is unchanged.

Metadata still uses two rounds over the ordered source list. The first valid
metadata wins and its peer becomes the first proven source, but it does not
have to serve every chunk. Every source must already be connected; the manager
does not dial peers.

## 6. Corruption, failure, and disconnect handling

- A chunk is written only after its hash matches the signed manifest.
- A corrupt response is retried from another source and quarantines its sender
  after two corrupt responses.
- A transport failure is retried from another source and quarantines a source
  after three consecutive failures.
- A disconnected source is skipped for the rest of the download; its in-flight
  lease returns to the pending set.
- If no usable source remains while chunks are pending, the download fails with
  a typed error and its verified chunks stay resumable.
- If one chunk exhausts four attempts, the download fails with
  `verification_failed` / `chunk_hash_mismatch`.
- One malicious peer cannot silently corrupt the completed file, because a
  corrupt chunk never reaches the partial file or the chunk map.

## 7. Cross-peer resume

`prepare_resume` uses the chunk map instead of a single prefix:

1. Re-verify the stored manifest root, or fetch metadata again and check it
   against the stored content ID and expected size.
2. Require a chunk map with one bit per manifest chunk.
3. Re-hash every chunk the database records as verified from the partial file.
4. Clear a chunk that fails re-verification; clear a chunk that is truncated or
   unreadable.
5. Truncate the unverified tail to the end of the highest verified chunk.
6. Reset the state to `downloading` and schedule only the missing chunks.

A resume without an explicit source uses the stored `source_peer`. An explicit
`peer_id` or `sources` override replaces it, and later chunks may come from any
listed source, not only the peer that supplied earlier chunks. A legacy
mid-chunk `verified_prefix` is still rejected because only complete chunks were
ever checkpointed.

## 8. Events and interface

No method was added. The three download events gained transfer detail:

| Event | Added fields |
|---|---|
| `download.started` | `total_chunks`, `sources` (ordered peer IDs) |
| `download.progress` | `verified_chunks`, `total_chunks`, `sources` (observed stats) |
| `download.resumed` | `verified_chunks`, `total_chunks`, `sources` (ordered peer IDs) |

`download.progress` source stats are `{peer_id, attempts, successes, failures,
corruptions, bytes, latency_ms, proven, quarantined}` for the current download
only. They are not persisted as a reputation record and never outlive the job.

The protocol version is now `0.1.0-stage-17`. The ABI remains 1, and the FFI
interface document is at version 1.10.0 with forty-five callable methods.

## 9. Protections and bounds

- At most four peers serve one download at once, with one chunk lease per peer.
- At most 8 sources may be listed, matching the existing `download.start`
  bound.
- A chunk has four total attempts across all sources.
- A peer is quarantined after two corrupt responses or three consecutive
  transport failures.
- No bytes from a corrupt response are written or checkpointed.
- Every chunk is verified against the signed manifest; the final file is still
  verified against its content ID before promotion.
- Resume re-hashes local chunks before trusting them, so tampering with the
  partial file cannot be mistaken for progress.
- Quota is charged per newly verified byte; out-of-order completion cannot
  bypass the quota or free-space reserve.
- Reconciliation aligns a short partial's failure state down to the last
  complete chunk, so a truncated file stays resumable instead of keeping a
  mid-chunk prefix.
- Scheduler state is bounded by `MAX_CHUNKS`, source count, and the in-flight
  lease map.
- Source statistics are per download and observation only; no identity,
  popularity, or provider claim affects scheduling.

## 10. Verification evidence

### 10.1 Stage 17 unit and integration tests

23 new Rust tests, 0 failures:

| Area | Tests | Covers |
|---|---:|---|
| `nexus-transfer` scheduler | 9 | Lowest-index leasing, one lease per chunk and peer, proven-peer preference, corrupt retry to another source, persistent-corruption quarantine, chunk attempt exhaustion, transport-failure quarantine, a simulated two-source makespan improvement, and a score that ignores non-observed fields. |
| `nexus-store` chunk map | 5 | Prefix construction, sparse set/clear, contiguous prefix versus verified bytes, highest verified end, and zero-byte/complete edge cases. |
| `nexus-core` download unit tests | 5 | Two peers combining chunks, a malicious peer unable to corrupt the file, a source that stops answering, cross-peer pause/resume, and resume re-verifying and refetching a locally corrupted chunk. |
| `nexus-store` storage tests | 2 | Out-of-order chunks are tracked and completion requires every chunk; a short partial fails back to a chunk boundary and stays resumable. |
| `nexus-core/tests/stage17.rs` | 2 | Two real engines serving one download, and a listed peer that cannot serve the file without blocking the other. |

The real-engine test imports identical bytes on two independent engines, so
both share a content ID and manifest root, connects a third engine to both, and
downloads with a two-entry `sources` array. It asserts:

- `download.completed` arrives and both peers delivered authenticated chunks.
- The reported `verified_chunks` equals the sum of per-source successes and
  `total_chunks`.
- The exported bytes are exactly the imported bytes.

The second test lists a peer that does not have the content first. The download
still completes from the serving peer and the final chunk counters agree.

### 10.2 Workspace totals

`cargo test --workspace` passed with 353 tests and 0 failures:

| Suite | Tests |
|---|---:|
| `nexus-core` unit tests | 65 |
| `nexus-core` Stage 09 tests | 4 |
| `nexus-core` Stage 10 tests | 2 |
| `nexus-core` Stage 11 tests | 4 |
| `nexus-core` Stage 12 tests | 1 |
| `nexus-core` Stage 13 tests | 1 |
| `nexus-core` Stage 14 tests | 1 |
| `nexus-core` Stage 15 tests | 3 |
| `nexus-core` Stage 16 tests | 4 |
| `nexus-core` Stage 17 tests | 2 |
| `nexus-crypto` unit tests | 24 |
| `nexus-ffi` unit tests | 4 |
| `nexus-ffi` C ABI tests | 6 |
| `nexus-identity` unit tests | 9 |
| `nexus-identity` integration tests | 12 |
| `nexus-net` unit tests | 22 |
| `nexus-net` Stage 09 tests | 8 |
| `nexus-net` Stage 10 tests | 4 |
| `nexus-net` Stage 11 tests | 5 |
| `nexus-net` Stage 12 tests | 2 |
| `nexus-observability` unit tests | 3 |
| `nexus-protocol` unit tests | 38 |
| `nexus-protocol` signed-record tests | 9 |
| `nexus-protocol` Stage 07 tests | 8 |
| `nexus-search` unit tests | 27 |
| `nexus-store` unit tests | 23 |
| `nexus-store` local-import tests | 4 |
| `nexus-store` profile tests | 5 |
| `nexus-store` search tests | 4 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-store` Stage 14 tests | 5 |
| `nexus-store` Stage 15 tests | 2 |
| `nexus-store` storage tests | 14 |
| `nexus-transfer` unit tests | 16 |
| `nexus-transfer` integration tests | 7 |
| `nexus-types` unit tests | 1 |
| **Total** | **353** |

Stage 16 had 330 tests. Stage 17 adds 23: 9 transfer unit tests, 5 store unit
tests, 5 core unit tests, 2 storage tests, and 2 real-engine integration tests.
The Stage 12 two-engine test is unchanged and still passes, so the
single-source path retains its earlier behavior.

### 10.3 Formatting, linting, and dependencies

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `python scripts/check_repo_hygiene.py` passes.
- `python scripts/validate_stage01_docs.py` and
  `python scripts/validate_stage02_docs.py` both pass.
- `cargo deny check` passes: advisories, bans, licenses, and sources are all
  allowed.
- `cargo audit` reports only the documented allowed `paste` unmaintained
  warning.
- Stage 17 adds no direct dependency; the workspace lockfile remains at 411
  packages.
- The FFI interface document is at version 1.10.0 with forty-five callable
  methods.

## 11. Exit criteria

| Criterion | Evidence |
|---|---|
| A download can combine valid chunks from multiple peers. | The real two-engine test downloads one file from two sources, both deliver authenticated chunks, and the exported bytes match. |
| Every chunk is verified against the authenticated chunk structure. | `fetch_chunk` compares each assembled chunk with the manifest SHA-256 before `write_chunk`; corrupt bytes are never written or checkpointed. |
| Chunks are scheduled by responsiveness and availability. | The scheduler ranks proven peers by observed reliability, integrity, and latency, leases only pending chunks, and quarantines unusable sources. |
| A failed chunk is retried from another source. | Scheduler corrupt-retry and failure-quarantine tests, plus the malicious-peer and source-loss core tests. |
| A peer's possession claim is not trusted until it serves valid chunks. | `proven` becomes true only after an authenticated chunk; untrusted peers score lower than proven peers. |
| Source scoring uses observed reliability, not popularity. | `score_uses_only_observed_outcomes` proves that identity and popularity fields do not exist in the stats or the score. |
| One malicious peer cannot corrupt the completed file. | The malicious-peer core test serves corrupt chunks and the final file still matches the imported bytes. |
| Cross-peer resume works. | Pause from one peer, resume through another; resume re-hashes recorded chunks and refetches a locally corrupted chunk. |
| A source that disappears does not stop the download. | The source-loss core test and the scheduler `NoUsableSource` handling complete while another source remains. |

Multi-peer speed is measured only as a deterministic simulated makespan in the
scheduler test. No realistic-network benchmark is claimed.

## 12. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries cross-build with the same
  scheduler, chunk map, resume, and transfer code.
- The Stage 17 proofs run on Windows loopback through the same Rust code that
  Android will load.
- Android runtime multi-peer transfer, on-device resume after process death, and
  Kotlin wiring remain deferred.

This is not claimed as Android runtime evidence.

## 13. Failure evidence and fixes

1. The first multi-worker implementation used one shared progress counter but
   no per-chunk lease, which allowed two workers to fetch the same chunk. The
   scheduler now owns leases; the `one_chunk_is_in_flight_from_at_most_one_peer`
   test guards it.
2. The pause/resume test assumed the paused record would always show a
   one-chunk contiguous prefix. With several workers a later chunk can finish
   first, so the test now asserts the chunk map directly and checks that some,
   but not all, chunks are pending.
3. The scheduler transport-quarantine test originally expected a second peer to
   take over after the first was quarantined. The quota is per chunk, not per
   peer, so the test now asserts the clean `NoUsableSource` outcome instead of
   an unspecified fallback.
4. Clippy rejected a `u32`-to-`u64` fallible conversion, a manually rounded
   chunk count, and a boolean comparison in tests. They were corrected without
   weakening the checks.
5. A `cargo fmt` pass initially failed on two over-long call sites and an
   unformatted stats map. Both were formatted; `--check` now passes.

## 14. Decisions and unresolved questions

- Stage 17 is the roadmap's multi-peer resilience stage, not a UI stage. The
  full UI, settings, foreground service, and tray work remains future work.
- Scheduling is pure policy. `nexus-core` owns threads, I/O, and persistence so
  the scheduler stays deterministic and testable.
- One chunk lease per peer keeps memory bounded and avoids multiplying retry
  state; it also means a slow peer cannot starve the others.
- Quarantine is per download, not a global reputation store. A peer that
  misbehaves in one download starts clean in the next, because observed
  outcomes do not persist as a network-wide score.
- Chunks are verified before writing, so the partial file never contains
  unverified data and full-file promotion stays a simple content-ID check.
- Resume re-hashes every recorded chunk. This costs local I/O but prevents a
  tampered or truncated partial file from being trusted.
- If every source becomes unusable, the download fails honestly with verified
  chunks intact rather than blocking or inventing progress.
- Realistic-network speedup, Android runtime evidence, and Kotlin wiring remain
  outstanding.

## 15. Next stage

Stage 18 is security, abuse resistance, and privacy hardening. It requires
explicit project-owner approval. Stage 17 remains on `feat/17-multi-peer`,
stacked on `feat/16-search`, until Stage 16 is reviewed and merged.
