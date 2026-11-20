# Stage 12 - Direct File Transfer

**Status:** Complete
**Completed:** 2026-11-20
**Progress guide:** 54-60%
**Next stage:** 13 - profiles and signed public metadata (requires user permission)

## 1. Scope

Stage 12 implements the direct-transfer layer required by
[ADR-0005](../decisions/ADR-0005-libp2p-transports-discovery.md) and
[ADR-0007](../decisions/ADR-0007-protocol-boundaries-versioning.md) on top of
the Stage 09 transports and the Stage 11 relay fallback. It adds no central
content service, proxy, or account authority.

The stage covers:

- A bounded `/nexus/file/1` request/response protocol for metadata and chunk
  slices.
- Direct peer-to-peer transfer from verified sources.
- Per-chunk SHA-256 verification before any bytes are written.
- Pause, resume, retry, cancellation, and source failover.
- Process-death recovery from a durable verified prefix.
- Quota and free-space checks before and during transfer.
- Bounded memory: at most one 1 MiB chunk is assembled at a time.
- Safe display names and generated store paths that cannot traverse or
  overwrite.
- Speed, progress, source peer, and typed errors in the transfer events and the
  demo Downloads screen.
- A documented design in
  [FILE_TRANSFER.md](../architecture/FILE_TRANSFER.md).

## 2. Architecture

| Component | Behaviour |
|---|---|
| `/nexus/file/1` | Canonical CBOR metadata and chunk messages over a length-prefixed request/response stream. |
| `FileCodec` | Enforces the 512 KiB frame cap and rejects empty or oversized frames. |
| `FileProvider` | Trait used by `nexus-net` to ask storage for metadata or a chunk slice. |
| `CoreFileProvider` | Serves only verified content whose manifest and publication agree. |
| `DownloadTransport` | Core-side transport abstraction; production forwards to `Network`, tests inject corruption and disconnects. |
| Download pool | Two dedicated worker threads; one cooperative control per job. |
| Store | Durable download row, partial file, verified prefix, manifest root, and source peer. |

The protocol version is `0.1.0-stage-12`; the identify protocol is
`/nexus/0.1.0-stage-12`. The ABI remains 1.

## 3. Protocol and bounds

Metadata requests select by content ID or signed publication ID. Chunk
requests name a content ID, chunk index, offset, and length. Responses carry a
manifest and optional signed publication, one chunk slice, or a typed error.

| Item | Value |
|---|---|
| Wire version | 1 |
| Maximum encoded message | 512 KiB |
| Maximum chunk slice | 64 KiB |
| Maximum manifest | 256 KiB |
| Maximum signed publication | 64 KiB |
| Maximum typed error | 128 bytes |
| Chunk size | 1 MiB |
| Maximum file size | 4 GiB |
| Inbound file requests per peer | 12,000 per minute |
| Rate-window table | 256 peers |
| Download workers | 2 |
| Ordered sources | 8 |
| Chunk attempts | 4 |
| Metadata rounds | 2 |
| Retry backoff | 150 ms |
| Progress interval | 250 ms |

A 1 MiB chunk is fetched as sixteen 64 KiB slices. This keeps each message
small enough for a relayed circuit and keeps memory bounded.

## 4. Verification chain

A source is never trusted:

1. The manifest must be canonical and parse within bounds.
2. When a signed publication is present, its ID, manifest root, content ID,
   size, chunk hashes, and display filename must agree with the manifest.
3. A content-ID request must match the manifest content ID.
4. Each assembled chunk must match the manifest chunk hash before it is
   written.
5. The completed partial must match the whole-file content ID before promotion.

A failure at any step produces `verification_failed` with a stable `reason`
detail such as `manifest_mismatch`, `chunk_hash_mismatch`, or
`publication_missing`.

## 5. Download lifecycle

1. `download.start` validates the selector and ordered sources, creates a
   control, and submits the job.
2. The worker fetches metadata, verifies it, stores the manifest by root, and
   registers the download with its trusted manifest root and source peer.
3. Chunks are fetched, verified, written at the chunk offset, flushed, and
   checkpointed. The store truncates any unverified tail and charges the
   verified delta against quota and free space.
4. A pause stops before the next chunk; the durable prefix is unchanged. A
   pause requested before metadata registration is deferred until the row
   exists, so the download stays visible and resumable.
5. A failure after all attempts ends as `download.failed` with the verified
   prefix intact.
6. Completion re-verifies the whole file, promotes it into the managed store,
   adds a `retained_download` reference, and deletes the download row.

On start, any row left in `downloading` is changed to `paused` with
`interrupted by restart`. Resume re-verifies the stored manifest or fetches it
again, requires a chunk-aligned prefix, truncates unverified bytes, and starts
at the first missing chunk. A graceful `stop` asks active downloads to pause
before networking shuts down, waits a bounded time for workers to checkpoint,
and marks any remaining `downloading` row paused.

## 6. Core integration

Stage 12 adds six methods to the ABI 1 catalog:

| ID | Method | Mode | Request | Result |
|---|---|---|---|---|
| 0x0801 | `download.start` | A | selector, `peer_id` or `sources`, optional `download_id` | `{download_id, accepted}` |
| 0x0802 | `download.list` | S | `state?`, `cursor`, `limit` | `{items, next_cursor?}` |
| 0x0803 | `download.pause` | A | `download_id` | Record or accepted |
| 0x0804 | `download.resume` | A | `download_id`, optional selector and sources | Stored record |
| 0x0805 | `download.cancel` | A | `download_id`, `keep_partial?` | Record or accepted |
| 0x0806 | `download.remove` | A | `download_id`, `confirmation` | `{download_id, removed}` |

New events: `download.started`, `download.progress`, `download.resumed`,
`download.paused`, `download.completed`, `download.failed`,
`download.cancelled`, and `download.removed`. Terminal events are durable;
progress events are coalesced per download.

`download.list` returns at most 100 records per page and includes the content
ID, display name, expected size, verified prefix, state, source peer, manifest
root, last error, and timestamps.

Schema version 5 adds `manifest_root` and `source_peer` to `downloads` so a
restart can resume without guessing a source or trusting a fresh manifest.

## 7. Protections and bounds

- Only verified content is served. The provider checks the content record and
  manifest on every metadata and chunk request.
- Every chunk request must stay inside one 1 MiB chunk and inside the manifest
  entry.
- Every wire decoder rejects unknown keys, wrong versions, mixed response
  shapes, oversized messages, and zero-length slices.
- A display name from a publication is passed through `safe_display_name`;
  separators, control characters, Windows-forbidden characters, trailing
  dots/spaces, and reserved device names are neutralized.
- Partial files use generated store paths, never user text. A download ID is
  limited to 128 ASCII alphanumeric, `-`, or `_` bytes.
- Quota and the free-space reserve are checked before registration and after
  each verified chunk.
- A corrupt chunk is never written and never advances the prefix.
- A completed download is stored by content ID, so a filename collision cannot
  overwrite another file.
- `download.remove` requires explicit confirmation and refuses running or
  completed entries.

## 8. Interface and UI

- The demo Downloads screen now shows a speed label and the serving peer for
  active transfers, in addition to progress, source count, status, and
  verification state.
- Every value remains demo data from `DemoCatalog`; the existing demo notice
  remains visible. No Kotlin screen is wired to the core yet.
- The screen remains a prototype for Stage 17, not evidence of a live transfer
  UI.

## 9. Verification evidence

### 9.1 `nexus-protocol` unit tests

6 new tests, 0 failures:

| Test | Covers |
|---|---|
| `metadata_request_round_trips_both_selectors` | Content-ID and publication-ID metadata requests. |
| `chunk_request_round_trips_within_one_chunk` | Chunk index, offset, and length encoding. |
| `oversized_or_offset_chunk_slices_are_rejected` | 64 KiB cap and offset past one chunk. |
| `metadata_and_chunk_responses_round_trip` | Manifest, publication, and full-size slice responses. |
| `typed_errors_round_trip_and_oversized_errors_fail` | Stable error text and the 128-byte cap. |
| `mixed_or_unknown_shapes_are_rejected` | Mixed response shapes, truncation, and trailing bytes. |

### 9.2 `nexus-net` Stage 12 tests

2 loopback integration tests, 0 failures:

| Test | Covers |
|---|---|
| `metadata_and_bounded_chunk_slices_cross_the_wire` | Real two-node metadata by content and publication, exact 64 KiB slices, and recorded slice bounds. |
| `unknown_content_and_out_of_bounds_slices_are_rejected` | `content_unavailable`, missing publication, chunk past the manifest, and an oversized slice. |

### 9.3 `nexus-core` unit tests

13 new download-manager tests, 0 failures:

| Test | Covers |
|---|---|
| `downloads_a_real_file_and_reports_progress` | Completion, progress events, verified library bytes. |
| `chunk_slices_stay_bounded_for_large_files` | 64 KiB slices and bounded memory for a multi-chunk file. |
| `a_corrupt_chunk_is_rejected_and_refetched` | Hash mismatch retry and eventual success. |
| `a_persistently_corrupt_source_fails_without_importing_content` | Terminal `verification_failed`, no library entry. |
| `a_mismatched_publication_manifest_root_is_rejected` | Publication/manifest binding. |
| `pause_stops_after_a_verified_chunk_and_resume_fetches_only_the_rest` | Pause boundary and resume from the prefix. |
| `pause_before_metadata_registration_keeps_a_resumable_row` | Deferred pause with no row yet; resume still works. |
| `pause_before_metadata_failure_reports_the_failure` | Early pause with failed metadata reports `failed`, not a phantom paused row. |
| `cancel_can_keep_or_delete_the_verified_partial` | Both cancellation modes. |
| `a_restart_pauses_interrupted_downloads_and_resume_continues_from_verified_bytes` | Process-death recovery. |
| `a_disconnected_source_fails_over_to_the_next_verified_peer` | Source failover mid-transfer. |
| `a_total_peer_disconnect_reports_a_failed_event` | Terminal failure when every source is gone. |
| `a_publication_display_name_cannot_escape_the_managed_store` | Path and display-name safety. |

### 9.4 `nexus-core` Stage 12 test

1 end-to-end test, 0 failures:

| Test | Covers |
|---|---|
| `engines_exchange_a_verified_file_and_keep_it_in_the_library` | Two real engines, 2.4 MiB import, connect, download by content ID, terminal completion, exact re-export, empty download queue after promotion, and a typed `not_found` for missing content. |

### 9.5 Workspace totals

`cargo test --workspace -- --test-threads=1` passed with 251 tests and 0
failures:

| Suite | Tests |
|---|---:|
| `nexus-core` unit tests | 41 |
| `nexus-core` Stage 09 tests | 4 |
| `nexus-core` Stage 10 tests | 2 |
| `nexus-core` Stage 11 tests | 4 |
| `nexus-core` Stage 12 tests | 1 |
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
| `nexus-protocol` unit tests | 31 |
| `nexus-protocol` signed-record tests | 9 |
| `nexus-protocol` Stage 07 tests | 8 |
| `nexus-store` unit tests | 17 |
| `nexus-store` local-import tests | 4 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-store` storage tests | 12 |
| `nexus-transfer` unit tests | 7 |
| `nexus-transfer` integration tests | 7 |
| `nexus-types` unit tests | 1 |
| **Total** | **251** |

Stage 11 had 229 tests; Stage 12 adds 22: 13 core download-manager tests, 6
protocol wire tests, 2 network Stage 12 tests, and 1 core Stage 12 test.

### 9.6 Kotlin checks

- `gradle spotlessCheck detekt :shared:presentation:jvmTest :shared:ui:jvmTest`
  passes.
- The demo catalogue and Downloads screen changes add no new Kotlin test cases;
  the existing presentation and UI tests still pass.

### 9.7 Formatting, linting, and dependencies

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo deny check` reports `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo audit` scans 411 dependencies with no vulnerabilities and one allowed
  unmaintained warning (`paste`, RUSTSEC-2024-0436, Linux-only through
  `if-watch`/netlink).
- No dependency was added; `Cargo.lock` remains at 411 packages.
- The FFI interface document is at version 1.5.0 with twenty-five callable
  methods.

## 10. Exit criteria

| Criterion | Evidence |
|---|---|
| Devices can exchange real files. | `engines_exchange_a_verified_file_and_keep_it_in_the_library` transfers a 2.4 MiB file between two engines over `/nexus/file/1` and re-exports identical bytes. |
| Corrupt chunks are rejected and retried. | `a_corrupt_chunk_is_rejected_and_refetched`, `a_persistently_corrupt_source_fails_without_importing_content`, and the 4-attempt failover loop. |
| Interrupted downloads can resume without restarting from zero. | `pause_stops_after_a_verified_chunk_and_resume_fetches_only_the_rest` and `a_restart_pauses_interrupted_downloads_and_resume_continues_from_verified_bytes` assert the second attempt requests only missing chunks. |
| Memory usage remains bounded for large files. | 64 KiB wire slices, one 1 MiB chunk assembly, `chunk_slices_stay_bounded_for_large_files`, and the 512 KiB frame cap. |
| Path traversal and unsafe overwrite are prevented. | `a_publication_display_name_cannot_escape_the_managed_store`, generated partial paths, and content-ID storage. |
| Free disk space is checked before and during transfer. | `begin_download` calls `ensure_capacity`; each checkpoint calls `ensure_capacity(delta)`. |
| Transfer speed, progress, source peer, and errors are shown. | `download.progress` carries `rate_bps` and `source_peer`; `download.failed` carries a typed error; the demo card shows speed and peer. |
| A mid-transfer disconnect is handled safely. | `a_disconnected_source_fails_over_to_the_next_verified_peer` and `a_total_peer_disconnect_reports_a_failed_event`. |

## 11. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries cross-build with the same transfer
  code.
- The Stage 12 proofs run on Windows loopback through the same Rust code that
  Android will load.
- Android runtime transfer behaviour, mobile network switching, background
  limits, and battery impact remain deferred.

This is not claimed as Android runtime evidence.

## 12. Failure evidence and fixes

1. The migration tests still asserted schema version 4 after migration 5 was
   added. They now assert `LATEST_VERSION`, and the checkpoint test asserts the
   current version instead of a hard-coded 4.
2. `mixed_or_unknown_shapes_are_rejected` changed a valid content-ID byte
   instead of testing truncation and trailing bytes, so it did not exercise
   the decoder's length checks. The test now truncates and appends bytes.
3. Clippy rejected `i64::from(LATEST_VERSION)` as a useless conversion. The
   assertion now compares the integer directly.
4. Spotless walked the local Gradle cache inside `NeededToolsHere` and tried to
   format generated Kotlin DSL accessors. The directory is now excluded from
   Spotless and ignored by Git.
5. `download.started` needed more than the reserved documentation fields to be
   useful to the UI, so it also carries `total_bytes`, `source_peer`,
   `publication_id`, and `display_name`.
6. A pause request can race a chunk failure. The worker re-reads the control
   before recording a failure, so the persisted state and terminal event always
   match the user's action.

## 13. Decisions and unresolved questions

- One source is used at a time. Parallel chunk swarms and erasure coding are
  deferred.
- A source must already be connected; the download manager does not dial.
- Chunk slices are 64 KiB on the wire even though chunks are 1 MiB. This trades
  request count for relay compatibility and bounded frames.
- Only complete chunks are checkpointed. A mid-chunk prefix is rejected on
  resume rather than re-verified in place.
- The manifest root and source peer are persisted so a restart can resume
  without trusting a fresh metadata response.
- Completed downloads leave the download queue and become verified content with
  a `retained_download` reference.
- The Kotlin Downloads UI remains labeled demo data; core wiring is a later UI
  stage.
- Android runtime transfer evidence and remote GitHub Actions execution remain
  outstanding.

## 14. Next stage

Stage 13 adds profiles and signed public metadata: the public profile schema,
username collision behavior without a central registry, signed profile
publication and retrieval, stale-record handling, and profile pages. It must
not begin until Stage 12 is reviewed and approved by the project owner.
