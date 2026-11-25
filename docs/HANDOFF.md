# Ember Project Handoff

**Current stage:** 17 - multi-peer downloads and advanced resilience
**Stage status:** Complete on Windows loopback; review and merge pending
**Last updated:** 2026-11-25
**Branch:** `feat/16-search` at `72521c9`, committed and pushed to `origin`;
PR #24 targets `main`; merge pending
**Next stage:** 18 - security, abuse resistance, and privacy hardening
(requires explicit project-owner approval)

## What was implemented

Stage 17 lets one download take verified chunks from several connected peers
at once, isolates failed or malicious sources, and keeps going when a source
disappears:

- A pure multi-source scheduler in `nexus-transfer` that owns pending chunks,
  one in-flight lease per chunk, one lease per peer, and observed source
  statistics.
- Source ranking from observed outcomes only: authenticated chunks, transport
  failures, corrupt responses, and exponentially weighted chunk latency. A peer
  becomes `proven` only after serving a chunk whose hash matches the signed
  manifest. Identity, popularity, and provider claims never affect the score.
- Bounded parallel transfer in `nexus-core`: one worker per usable source,
  capped at four, with a shared scheduler and interruptible 150 ms retry
  backoff.
- Per-chunk SHA-256 verification before any byte is written. Corrupt bytes are
  discarded, the chunk returns to the scheduler, and another source is tried.
- Quarantine after two corrupt responses or three consecutive transport
  failures. The download fails only when no usable source remains or one chunk
  exhausts four attempts.
- Schema version 10 with a per-chunk bitmap (`chunk_map`) and `verified_bytes`.
  `verified_prefix` remains the contiguous prefix for existing callers. Quota
  usage sums sparse verified bytes.
- Cross-peer resume: re-hash every recorded chunk from the partial file, clear
  corrupt or truncated chunks, truncate the unverified tail to the highest
  verified chunk, and continue from missing chunks through any listed source.
- `download.started`, `download.progress`, and `download.resumed` now report
  chunk counters and per-source observed statistics.
- 23 new Rust tests, for 353 total. The Stage 12 single-source test remains
  unchanged and passing.
- No new FFI method, network protocol, or frame type. Protocol version is
  `0.1.0-stage-17`; ABI remains 1; FFI interface document is 1.10.0.

## Files changed

Transfer crate:

- `crates/nexus-transfer/src/scheduler.rs` (new)
- `crates/nexus-transfer/src/lib.rs`

Store crate:

- `crates/nexus-store/migrations/0010_multi_source_downloads.sql` (new)
- `crates/nexus-store/src/chunk_map.rs` (new)
- `crates/nexus-store/src/lib.rs`
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/model.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/tests/storage.rs`
- `crates/nexus-store/tests/local_import.rs` (schema assertion only)
- `crates/nexus-store/tests/profiles.rs` (schema assertion only)

Core crate:

- `crates/nexus-core/src/downloads.rs`
- `crates/nexus-core/src/lib.rs` (protocol version)
- `crates/nexus-core/tests/stage17.rs` (new)

Documentation:

- `docs/progress/STAGE_17.md` (new)
- `docs/architecture/FFI_INTERFACE.md` (version 1.10.0)
- `docs/architecture/FILE_TRANSFER.md` (version 1.2.0)
- `docs/ARCHITECTURE.md`
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

No Kotlin, FFI adapter, protocol, or network source changed in Stage 17.

Dependency lock:

- No direct dependency was added; `Cargo.lock` remains at 411 packages.

## Commands and results

### Rust tests

```text
cargo test --workspace
```

Result: PASS, 353 tests, 0 failures.

Stage 17-specific suites:

```text
cargo test -p nexus-core --test stage17
cargo test -p nexus-core --lib downloads::tests
cargo test -p nexus-transfer --lib scheduler
cargo test -p nexus-store --lib chunk_map
```

Result: PASS, 2, 18, 9, and 5 tests respectively (the scheduler count is the 9
new tests; the store count is the 5 new chunk-map tests).

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 65 | 0 |
| `nexus-core` Stage 09 tests | 4 | 0 |
| `nexus-core` Stage 10 tests | 2 | 0 |
| `nexus-core` Stage 11 tests | 4 | 0 |
| `nexus-core` Stage 12 tests | 1 | 0 |
| `nexus-core` Stage 13 tests | 1 | 0 |
| `nexus-core` Stage 14 tests | 1 | 0 |
| `nexus-core` Stage 15 tests | 3 | 0 |
| `nexus-core` Stage 16 tests | 4 | 0 |
| `nexus-core` Stage 17 tests | 2 | 0 |
| `nexus-crypto` unit tests | 24 | 0 |
| `nexus-ffi` unit tests | 4 | 0 |
| `nexus-ffi` C ABI tests | 6 | 0 |
| `nexus-identity` unit tests | 9 | 0 |
| `nexus-identity` integration tests | 12 | 0 |
| `nexus-net` unit tests | 22 | 0 |
| `nexus-net` Stage 09 tests | 8 | 0 |
| `nexus-net` Stage 10 tests | 4 | 0 |
| `nexus-net` Stage 11 tests | 5 | 0 |
| `nexus-net` Stage 12 tests | 2 | 0 |
| `nexus-observability` unit tests | 3 | 0 |
| `nexus-protocol` unit tests | 38 | 0 |
| `nexus-protocol` signed-record tests | 9 | 0 |
| `nexus-protocol` Stage 07 tests | 8 | 0 |
| `nexus-search` unit tests | 27 | 0 |
| `nexus-store` unit tests | 23 | 0 |
| `nexus-store` local-import tests | 4 | 0 |
| `nexus-store` profile tests | 5 | 0 |
| `nexus-store` search tests | 4 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` Stage 14 tests | 5 | 0 |
| `nexus-store` Stage 15 tests | 2 | 0 |
| `nexus-store` storage tests | 14 | 0 |
| `nexus-transfer` unit tests | 16 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **353** | **0** |

Stage 16 had 330 tests. Stage 17 adds 9 transfer unit tests, 5 store unit tests,
5 core unit tests, 2 storage tests, and 2 real-engine integration tests.

### Kotlin and native builds

No Kotlin or FFI/native surface changed in Stage 17, so the Stage 16 Kotlin and
native build gates were not re-run for this stage. The Stage 16 results still
describe the current Kotlin and native code.

### Formatting, linting, and dependency policy

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
python scripts/check_repo_hygiene.py
python scripts/validate_stage01_docs.py
python scripts/validate_stage02_docs.py
cargo deny check
cargo audit
```

- Formatting, Clippy with warnings denied, repository hygiene, and both
  documentation validators: PASS.
- `cargo deny check`: PASS (advisories, bans, licenses, and sources).
- `cargo audit`: PASS with the documented allowed `paste` unmaintained warning.
- No direct dependency added; `Cargo.lock` remains at 411 packages.
- The FFI interface document is at version 1.10.0 with forty-five callable
  methods.
- The ABI remains 1; the core protocol version is `0.1.0-stage-17`. The
  identify protocol remains `/nexus/0.1.0-stage-12`; Stage 17 adds no new
  protocol or frame type.

## Errors and failed attempts

1. The first multi-worker version shared a progress counter without per-chunk
   leases, so two workers could fetch the same chunk. The scheduler now issues
   one lease per chunk and one per peer; the `one_chunk_is_in_flight_from_at_most_one_peer`
   test guards it.
2. The pause/resume test assumed a one-chunk contiguous prefix. With parallel
   workers a later chunk can finish first, so the test now asserts the chunk map
   directly and requires some, but not all, chunks to be pending.
3. The scheduler transport-quarantine test expected a second source to take
   over. Attempts are per chunk, not per peer, so the test now asserts the
   clean `NoUsableSource` outcome.
4. Clippy rejected a fallible `u32`-to-`u64` conversion, a manually rounded
   chunk count, and a boolean comparison in tests. They were corrected without
   relaxing the gate.
5. `cargo fmt --check` initially failed on two long call sites and an
   unformatted stats map. They were formatted.
6. `git` was not on the default PATH in the automation shell; the Git cmd
   directory was added for repository commands.

## Decisions and unresolved questions

- Stage 17 is the roadmap's multi-peer resilience stage, not a UI stage. The
  full UI, settings, foreground service, and tray work remains future work.
- The scheduler is pure policy in `nexus-transfer`; `nexus-core` owns threads,
  I/O, and persistence, keeping the policy deterministic and testable.
- One chunk lease per peer bounds memory and retry state and stops a slow peer
  from starving the others.
- Quarantine is per download, not a persisted global reputation score. A peer
  that misbehaves in one download starts clean in the next.
- A corrupt chunk is never written, so the partial file only contains verified
  bytes and final promotion remains a content-ID check.
- Resume re-hashes every recorded chunk. This costs local I/O but prevents a
  tampered or truncated partial file from being trusted.
- If every source becomes unusable, the download fails honestly with verified
  chunks intact instead of blocking or inventing progress.
- Multi-peer speedup is measured only as a deterministic simulated makespan.
  No realistic-network benchmark is claimed.
- Android runtime multi-peer transfer, on-device resume after process death, and
  Kotlin-to-core wiring remain deferred because no emulator or device is
  available.

## Exact next action

1. Review PR #24 from `feat/16-search` into `main`; it carries the Stage 17
   commit after Stage 16 merged through PR #22.
2. Merge only after review, then delete the merged Stage 16 and Stage 17
   branches.
3. Do not start Stage 18 without explicit project-owner approval.
4. Stage 18 covers the threat model, abuse resistance, and privacy hardening,
   including connection, request, storage, and bandwidth limits.
