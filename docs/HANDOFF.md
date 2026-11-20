# Ember Project Handoff

**Current stage:** 12 - direct file transfer
**Stage status:** Complete
**Last updated:** 2026-11-20
**Branch:** `feat/12-streaming-transfer`
**Next stage:** 13 - profiles and signed public metadata (waiting for user permission)

## What was implemented

Stage 12 adds direct peer-to-peer file transfer on top of the Stage 09
transports and the Stage 11 relay fallback:

- A bounded `/nexus/file/1` request/response protocol for metadata and chunk
  slices, with canonical CBOR, exact field sets, and a 512 KiB message cap.
- A `FileProvider` contract in `nexus-net` and a storage-backed
  `CoreFileProvider` in the core that serves only verified content.
- Direct transfer from one connected peer at a time, with up to 8 ordered
  fallback sources and 4 attempts per chunk.
- Per-chunk SHA-256 verification before any bytes are written.
- Manifest, signed-publication, content-ID, and whole-file verification before
  promotion into the managed library.
- Pause, resume, retry, cancellation with optional partial retention, and
  source failover.
- Durable verified-prefix checkpoints and process-death recovery: interrupted
  downloads become `paused` on start and resume from the last complete chunk.
  A graceful stop also pauses active downloads before networking shuts down.
- Quota and free-space checks at registration and after every verified chunk.
- Bounded memory: 64 KiB wire slices and one 1 MiB chunk assembled at a time.
- Safe display names and generated partial paths that cannot traverse or
  overwrite.
- Six new core methods: `download.start` (0x0801), `download.list` (0x0802),
  `download.pause` (0x0803), `download.resume` (0x0804),
  `download.cancel` (0x0805), and `download.remove` (0x0806).
- Eight transfer events: `download.started`, `download.progress`,
  `download.resumed`, `download.paused`, `download.completed`,
  `download.failed`, `download.cancelled`, and `download.removed`.
- Schema version 5 with `manifest_root` and `source_peer` on downloads.
- A demo Downloads card that shows speed and the serving peer alongside
  progress, status, source count, and verification state. It remains visibly
  simulated.
- 22 new Rust tests, for 251 total.

## Files changed

Protocol crate:

- `crates/nexus-protocol/src/file.rs` (new)
- `crates/nexus-protocol/src/lib.rs`

Network crate:

- `crates/nexus-net/src/behaviour.rs`
- `crates/nexus-net/src/codec.rs`
- `crates/nexus-net/src/driver.rs`
- `crates/nexus-net/src/error.rs`
- `crates/nexus-net/src/file.rs` (new)
- `crates/nexus-net/src/lib.rs`
- `crates/nexus-net/tests/stage12.rs` (new)

Core crate:

- `crates/nexus-core/src/downloads.rs` (new)
- `crates/nexus-core/src/engine.rs`
- `crates/nexus-core/src/error.rs`
- `crates/nexus-core/src/lib.rs`
- `crates/nexus-core/src/network.rs`
- `crates/nexus-core/tests/stage12.rs` (new)

Store crate:

- `crates/nexus-store/migrations/0005_download_transfer_metadata.sql` (new)
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/model.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/tests/local_import.rs`
- `crates/nexus-store/tests/storage.rs`

FFI and workspace:

- `crates/nexus-ffi/tests/c_abi.rs`
- `.gitignore`
- `build.gradle.kts`

Shared Kotlin:

- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoModels.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoCatalog.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/components/Cards.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/resources/NexusStrings.kt`

Documentation:

- `docs/architecture/FILE_TRANSFER.md` (new)
- `docs/progress/STAGE_12.md` (new)
- `docs/architecture/FFI_INTERFACE.md` (version 1.5.0)
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

## Commands and results

### Rust tests

```text
cargo test --workspace -- --test-threads=1
```

Result: PASS, 251 tests, 0 failures.

Stage 12-specific suites:

```text
cargo test -p nexus-net --test stage12 -- --test-threads=1
cargo test -p nexus-core --test stage12 -- --test-threads=1
```

Result: PASS, 2 and 1 tests respectively.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 41 | 0 |
| `nexus-core` Stage 09 tests | 4 | 0 |
| `nexus-core` Stage 10 tests | 2 | 0 |
| `nexus-core` Stage 11 tests | 4 | 0 |
| `nexus-core` Stage 12 tests | 1 | 0 |
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
| `nexus-protocol` unit tests | 31 | 0 |
| `nexus-protocol` signed-record tests | 9 | 0 |
| `nexus-protocol` Stage 07 tests | 8 | 0 |
| `nexus-store` unit tests | 17 | 0 |
| `nexus-store` local-import tests | 4 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` storage tests | 12 | 0 |
| `nexus-transfer` unit tests | 7 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **251** | **0** |

Stage 11 had 229 tests. Stage 12 adds 13 core download-manager tests, 6 protocol
wire tests, 2 network Stage 12 tests, and 1 core Stage 12 test.

### Kotlin checks

```text
gradle spotlessCheck detekt :shared:presentation:jvmTest :shared:ui:jvmTest
```

Result: PASS.

### Formatting, linting, and dependency policy

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
cargo audit
```

- Formatting and Clippy with warnings denied: PASS.
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`. One
  unmaintained, non-vulnerable Linux-only `paste` advisory
  (`RUSTSEC-2024-0436`) remains documented in `deny.toml`.
- `cargo audit`: 411 dependencies scanned; no vulnerabilities and one allowed
  warning.
- `Cargo.lock` remains at 411 packages. Stage 12 adds no dependency.
- The native artifact build was not re-run because the ABI and exported surface
  did not change. The six C ABI tests still pass in the workspace suite.

## Errors and failed attempts

1. Migration tests still asserted schema version 4 after migration 5 was added.
   They now assert `LATEST_VERSION`, and the checkpoint test asserts the
   current version.
2. `mixed_or_unknown_shapes_are_rejected` changed a valid content-ID byte
   instead of testing truncation and trailing bytes. It now truncates and
   appends bytes.
3. Clippy rejected `i64::from(LATEST_VERSION)` as a useless conversion. The
   assertion now compares the integer directly.
4. Spotless walked the local Gradle cache inside `NeededToolsHere` and tried to
   format generated Kotlin DSL accessors. The directory is excluded from
   Spotless and ignored by Git.
5. `download.started` needed more fields for the UI than the reserved contract,
   so it also carries `total_bytes`, `source_peer`, `publication_id`, and
   `display_name`.
6. A pause can race a chunk failure. The worker re-reads the control before
   recording a failure, so the persisted state and terminal event match the
   user's action.

## Decisions and unresolved questions

- Stage 12 was explicitly authorized by the project owner after Stage 11 was
  approved.
- One source is used at a time. Parallel chunk swarms and erasure coding are
  deferred.
- A source must already be connected; the download manager does not dial.
- Chunk slices are 64 KiB on the wire even though chunks are 1 MiB. This keeps
  each frame small enough for a relayed circuit.
- Only complete chunks are checkpointed. A mid-chunk prefix is rejected on
  resume rather than re-verified in place.
- The manifest root and source peer are persisted so a restart can resume
  without trusting a fresh metadata response.
- Completed downloads leave the download queue and become verified content with
  a `retained_download` reference.
- The ABI remains 1; the protocol version is `0.1.0-stage-12`, and the identify
  protocol is `/nexus/0.1.0-stage-12`.
- The Kotlin Downloads UI remains labeled demo data; core wiring is a later UI
  stage.
- Android runtime transfer evidence is deferred because no emulator or device
  is available.
- Remote GitHub Actions execution has not run for Stage 12.

## Exact next action

1. Review `docs/progress/STAGE_12.md`,
   `docs/architecture/FILE_TRANSFER.md`, the updated
   `docs/architecture/FFI_INTERFACE.md`, and the `nexus-protocol`,
   `nexus-net`, `nexus-store`, and `nexus-core` Stage 12 changes.
2. Give explicit approval to start Stage 13.
3. Stage 13 adds profiles and signed public metadata: the public profile schema,
   username collision behavior without a central registry, signed profile
   publication and retrieval, stale-record handling, and profile pages. Do not
   begin it before that approval.
