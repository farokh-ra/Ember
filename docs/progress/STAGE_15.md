# Stage 15 - Voluntary Replication and Availability

**Status:** Complete and merged
**Completed:** 2026-11-23
**Merged:** PR #18 into `main` as `264dcbb` on 2026-11-23
**Progress guide:** 70-75%
**Next stage:** 16 - local and distributed search (not started; requires explicit
project-owner approval)

## 1. Scope

Stage 15 turns the Stage 14 serving gate into explicit, user-controlled
hosting. A user can choose a file that already exists in the verified local
store, advertise it as a replica, pause or resume serving, remove the replica,
and see exactly what is stored and served. The original signed publication is
never rewritten, and downloading a file never creates a hosting obligation.

The stage covers:

- Explicit per-file hosting through `replica.add`, with mandatory
  confirmation and no automatic download or silent seeding.
- Full local verification of the content, manifest, and signed publication
  binding before a provider advertisement is attempted.
- Replica lifecycle states `active`, `paused`, `verification_failed`, and
  `removed`, with pause, resume, remove, and manual verify methods.
- Startup re-verification and a periodic pass that re-checks replicas whose
  last successful verification is older than seven days.
- Storage quota control through `store.set_quota`.
- Upload bandwidth, metered-network, and battery thresholds through
  `hosting.limits_get` and `hosting.limits_set`.
- A serving gate that combines verified content, a local serving reason, and
  the current hosting policy.
- Last-seen, last-verified, last-checked, failure-count, and redacted-error
  tracking per replica.
- A demo Hosted Files screen that shows publication, size, verification,
  last verification time, serving state, storage use, and pause/resume/remove
  controls with visible demo labels.

## 2. Architecture

| Component | Behaviour |
|---|---|
| `nexus-store::ReplicaRecord` | Durable lifecycle, publication attribution, source peer, verified size, timestamps, failure count, redacted error, and announced state. |
| `nexus-store` migration 8 | Extends `replicas` without changing the signed publication bytes; adds state and announcement indexes. |
| `nexus-core::replicas` | Explicit add, list, pause, resume, remove, verify, startup verification, periodic maintenance, provider advertisement, and events. |
| `nexus-core::hosting` | In-memory hosting policy and host-reported metered/battery signals read by every inbound file request. |
| `Engine` dispatch | Implements `0x0402` and `0x0901`-`0x0908`, starts maintenance on `start`, and stops it before shutdown. |
| `CoreFileProvider` | Requires verified content, a local serving reason, and `serving_allowed()` before answering metadata or chunk requests; records successful service. |
| `nexus-net` upload throttle | Spaces inbound file-chunk responses to the configured average rate and applies limit changes at runtime. |

The core protocol version is `0.1.0-stage-15`; the ABI remains 1. The identify
protocol remains `/nexus/0.1.0-stage-12` because Stage 15 adds no new wire
protocol or frame type. Replicas use the existing content-provider records and
`/nexus/file/1` transfer protocol.

## 3. Explicit hosting

Hosting is a per-file decision. `replica.add`:

1. Requires a running core and `confirmation: true`.
2. Accepts `publication_id`, `content_id`, or both as 32-byte hex. When both
   are present, the publication must describe the content.
3. Accepts optional `source_peer` attribution, plus optional `metered` and
   `battery_percent` host signals.
4. Checks the managed quota before creating or reactivating a row.
5. Verifies the complete local content against its content ID and verifies the
   canonical manifest. When a publication is named, the exact signed bytes must
   verify, bind the publication ID, and validate the manifest.
6. Creates or reactivates the deterministic row `replica-<content_id>`, adopts
   an `active_replica` reference, and starts a worker that advertises the
   content ID.

The immediate response is
`{op_id, accepted, replica, usage, attribution_preserved, limits}`. The worker
emits `replica.active` with `announced` and an optional typed error. A network
failure does not discard the verified replica; it stays active and can be
advertised again by resume or a later announcement path.

`replica.add` never downloads bytes. The user must first import or download and
verify the file. An incomplete or corrupt copy returns `verification_failed`,
and no active row or provider advertisement is created.

## 4. Replica lifecycle

| State | Meaning | Serving |
|---|---|---|
| `active` | The user is hosting a verified copy and advertisement was attempted. | Yes, while hosting policy allows. |
| `paused` | The user suspended hosting; bytes and the original record remain. | No. |
| `verification_failed` | A full verification failed. | No. |
| `removed` | The user stopped hosting and the local reference was released. | No. |

- `replica.pause` is idempotent. It marks the row paused, clears advertisement,
  and withdraws the provider if no other local serving reason remains.
- `replica.resume` re-verifies the complete file and manifest before the row
  becomes active. A corrupt copy returns `verification_failed` and stays failed.
- `replica.remove` requires `confirmation: true`. It stops serving, marks the
  row removed, releases the `active_replica` reference, and withdraws the
  provider unless another serving reason remains. The original signed
  publication and any bytes still referenced elsewhere are untouched.
- Re-adding the same content reactivates the same deterministic row, preserving
  its audit history.
- `replica.verify` runs a full verification. A paused replica stays paused after
  a successful check; verification is not an implicit resume.

## 5. Verification and availability

- Startup runs a verification pass over every active replica before advertising.
- A maintenance thread wakes hourly and re-checks replicas whose last
  successful verification is older than seven days.
- A verification failure marks the row `verification_failed`, increments
  `failure_count`, stores a redacted `last_error`, clears advertisement, and
  withdraws the provider unless another serving reason remains.
- A failed replica recovers to active only after a later explicit
  `replica.resume` verifies the bytes again.
- `last_verified_at_ms` records a successful full verification;
  `last_checked_at_ms` records any attempt; `last_seen_at_ms` records the last
  successful advertisement or served metadata/chunk request.
- `replica.list` reports `serving` from the current state and hosting policy.
  It is a local willingness signal, not a promise that another peer is online.

## 6. Hosting limits

| Control | Method | Range and default | Enforcement |
|---|---|---|---|
| Storage quota | `store.set_quota` | 1-500 GiB; 10 GiB default | `replica.add` and `replica.resume` refuse new work with `quota_exceeded`; existing publications and replicas are never silently evicted. |
| Upload rate | `hosting.limits_set` | 0 or 256 KiB/s-64 MiB/s; 0 means unlimited | Inbound file-chunk responses are delayed to the configured average; metadata and errors are not delayed. |
| Metered network | `hosting.limits_set` | `allow_metered` defaults false | While metered and not allowed, all inbound file serving is refused with `blocked_reason = "metered_network"`. |
| Battery threshold | `hosting.limits_set` | 0-100%; 0 disables | A known battery level below the threshold refuses serving with `blocked_reason = "battery_low"`. An unknown level never blocks. |

`hosting.limits_get` returns the persisted limits, current host signals,
`serving_allowed`, `blocked_reason`, and the limits notice. Limits persist in
the store and are loaded when the engine opens. A limit change applies to the
running network immediately and emits `hosting.limits_changed`.

The metered and battery values are host-reported signals in this stage. Android
and Windows platform integrations that supply them are later UI and platform
work; the core never guesses a network cost or battery level.

## 7. Serving gate

A file is served only when all of these are true:

- The content row exists and is `verified`.
- The hosting policy allows serving (`serving_allowed()`).
- At least one local reason to keep and share it exists:
  - a local, non-tombstoned publication with `serving = 1`, or
  - an active replica, or
  - a retained completed download.

`CoreFileProvider` checks this gate for both metadata and chunk requests. A
cached remote publication never makes this device a source. A paused, failed,
or removed replica stops serving immediately even though its bytes and signed
publication record may remain. Every successful metadata or chunk response
updates `last_seen_at` for active replicas of that content.

## 8. Schema version 8

Migration `0008_replication.sql` extends `replicas` with:

- `source_peer`
- `verified_size_bytes`
- `last_checked_at`
- `last_seen_at`
- `failure_count`
- `last_error`
- `announced`

It adds `replicas_state_idx` for lifecycle queries. Existing rows are retained;
removed rows remain as audit history. The original signed publication bytes and
attribution are never rewritten by replica operations.

## 9. Core integration

Stage 15 implements nine methods reserved in the ABI 1 catalog:

| ID | Method | Mode | Request | Result |
|---|---|---|---|---|
| 0x0402 | `store.set_quota` | S | `quota_bytes` | Effective quota, managed usage, free space, reserve, and storage health. |
| 0x0901 | `replica.add` | A | `publication_id?`, `content_id?`, `source_peer?`, `confirmation`, `metered?`, `battery_percent?` | `replica.verifying`, then `replica.active` or `replica.failed`. |
| 0x0902 | `replica.list` | S | `cursor?`, `limit?`, `include_removed?` | Replicas, usage, and hosting policy. |
| 0x0903 | `replica.pause` | A | `replica_id` | Paused replica, usage, and hosting policy; emits `replica.paused`. |
| 0x0904 | `replica.resume` | A | `replica_id`, `metered?`, `battery_percent?` | Re-verifies, then `replica.resumed` and `replica.active`, or `replica.failed`. |
| 0x0905 | `replica.remove` | A | `replica_id`, `confirmation` | Removed replica, usage, and hosting policy; emits `replica.removed`. |
| 0x0906 | `replica.verify` | A | `replica_id` | `replica.verified` or `replica.verification_failed`. |
| 0x0907 | `hosting.limits_get` | S | none | Limits, signals, serving decision, and limits notice. |
| 0x0908 | `hosting.limits_set` | S | `upload_bytes_per_second?`, `allow_metered?`, `battery_min_percent?`, `metered?`, `battery_percent?` | Persisted limits and current serving decision; emits `hosting.limits_changed`. |

New events: `replica.verifying`, `replica.active`, `replica.failed`,
`replica.paused`, `replica.resumed`, `replica.removed`, `replica.verified`,
`replica.verification_failed`, `replica.cancelled`,
`replica.maintenance_completed`, and `hosting.limits_changed`.

The `system.info` capability list includes the six `replica.*` methods and the
two `hosting.*` methods when the core is running. `store.set_quota` is included
with the other storage methods.

## 10. Protections and bounds

- Hosting requires an explicit confirmation and a verified local copy.
- No code path downloads, imports, or seeds a file without a user action.
- Provider advertisement happens only after complete verification.
- The original signed publication is stored and served unchanged; replica
  results always report `attribution_preserved`.
- Pausing or removing a replica never deletes the original uploader's record or
  another peer's copy.
- A verification failure stops advertising and serving before the event is
  reported.
- Quota pressure refuses new work instead of evicting existing data.
- Metered and battery policy gates all inbound serving, including publications.
- Upload throttling is bounded and keeps each response inside the peer request
  deadline.
- Replica list pages are limited to 100 rows; errors are redacted to 512
  characters.
- Maintenance and verification run on bounded workers and stop with the core.

## 11. Interface and UI

- `HostedFileCard` now shows the publication identifier, original uploader,
  size, storage use, verification state, last verification time, serving state,
  peers served, and a last-served label when available.
- `HostedFilesScreen` supports demo pause, resume, remove, and restore actions.
  Pausing keeps the fixture and changes only its demo state; removing hides it
  until the sample list is restored.
- Every screen remains visibly labelled demo content. No Kotlin screen is wired
  to the core yet; the prototype is for a later UI stage.
- The hosted-files demo keeps original-uploader attribution separate from the
  local profile. A replica is never presented as the original uploader.

## 12. Verification evidence

### 12.1 `nexus-core` Stage 15 tests

3 new tests, 0 failures:

| Test | Covers |
|---|---|
| `explicit_replica_serves_after_withdrawal_and_removal_preserves_records` | Two engines publish and download, a third explicitly hosts the verified file, serves it after the original publisher unpublishes, then pauses, resumes, removes, and re-adds it. Asserts attribution, provider withdrawal, last-seen, and that the original record survives. |
| `hosting_limits_block_serving_and_persist_across_restart` | Upload limit, metered opt-in, battery threshold, quota change, blocked reasons, persisted limits, and serving recovery after restart. |
| `verification_detects_corruption_and_paused_replicas_stay_paused` | Explicit verification, corruption detection, failed-state advertisement withdrawal, paused-stays-paused behavior, resume recovery, and startup re-verification. |

### 12.2 `nexus-store` Stage 15 tests

2 new tests, 0 failures:

| Test | Covers |
|---|---|
| `replica_records_round_trip_page_and_report_usage` | Replica persistence, paging, removed-row filtering, usage counts and storage bytes, verification history, announcement state, last-seen updates, and reopen. |
| `hosting_limits_and_quota_persist_across_reopen` | Default limits, quota override, atomic limit persistence, and invalid-limit rejection. |

### 12.3 Workspace totals

`cargo test --workspace -- --test-threads=1` passed with 286 tests and 0
failures:

| Suite | Tests |
|---|---:|
| `nexus-core` unit tests | 57 |
| `nexus-core` Stage 09 tests | 4 |
| `nexus-core` Stage 10 tests | 2 |
| `nexus-core` Stage 11 tests | 4 |
| `nexus-core` Stage 12 tests | 1 |
| `nexus-core` Stage 13 tests | 1 |
| `nexus-core` Stage 14 tests | 1 |
| `nexus-core` Stage 15 tests | 3 |
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
| `nexus-protocol` unit tests | 32 |
| `nexus-protocol` signed-record tests | 9 |
| `nexus-protocol` Stage 07 tests | 8 |
| `nexus-store` unit tests | 18 |
| `nexus-store` local-import tests | 4 |
| `nexus-store` profile tests | 5 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-store` Stage 14 tests | 5 |
| `nexus-store` Stage 15 tests | 2 |
| `nexus-store` storage tests | 12 |
| `nexus-transfer` unit tests | 7 |
| `nexus-transfer` integration tests | 7 |
| `nexus-types` unit tests | 1 |
| **Total** | **286** |

Stage 14 had 281 tests; Stage 15 adds 5: 3 core integration tests and 2 store
integration tests. The workspace suite also includes the unchanged 57 core
unit tests and 22 network unit tests.

### 12.4 Kotlin checks

- `:shared:presentation:jvmTest` and `:shared:ui:jvmTest` pass.
- `DemoCatalogTest` extends the hosted-replica checks to require valid
  publication IDs, non-empty storage labels, and serving only for verified
  replicas.
- The full gate passes:
  `.\gradlew.bat --no-daemon --console=plain spotlessCheck detekt
  :shared:presentation:jvmTest :shared:ui:jvmTest
  :apps:desktopApp:createDistributable :apps:androidApp:assembleDebug`.
- Gradle uses `GRADLE_USER_HOME=NeededToolsHere/gradle-home`, so downloads stay
  inside the repository.

### 12.5 Formatting, linting, and dependencies

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo deny check` passes locally and in CI with the Stage 14 lockfile
  update; `cargo audit` reports only the documented allowed `paste`
  unmaintained warning.
- Stage 15 adds no direct dependency; `Cargo.lock` remains at 411 packages.
- The FFI interface document is at version 1.8.0 with forty-two callable
  methods.

## 13. Exit criteria

| Criterion | Evidence |
|---|---|
| A user can voluntarily replicate a file and later serve it to another peer. | The Stage 15 core test explicitly adds a verified local replica and downloads the exact bytes from that host after the original publisher withdraws. |
| Removing a replica does not affect the original uploader's record. | The test removes the replica, confirms the original signed publication and cached record still verify, and re-adds the same content. |
| The app does not replicate files without explicit user choice. | `replica.add` requires `confirmation: true`; no download, import, or startup path creates a replica. |
| Incomplete or corrupt content is never advertised. | Add, resume, explicit verify, and startup verification check the content ID, manifest, and publication binding before advertisement. |
| Hosted files show publication, size, verification, last verification, serving, and storage use. | The demo Hosted Files screen and `DemoHostedFile` expose every field; the core `replica.list` result carries the real values. |
| Quota, bandwidth, metered, and battery limits are enforced. | The Stage 15 tests block downloads under each policy, verify persisted limits across restart, and pass upload throttling through the network driver. |
| Pause and remove behave honestly. | Pause keeps bytes and stops serving; remove requires confirmation, releases the local reference, and preserves the original record. |
| Re-verification runs at startup and every seven days. | Startup maintenance and the hourly seven-day check are implemented; the corruption test observes the startup failure and serving stop. |

## 14. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries cross-build with the same replica,
  hosting, and throttle code.
- The Stage 15 proofs run on Windows loopback through the same Rust code that
  Android will load.
- Android runtime hosting, metered-network detection, battery reporting,
  background serving limits, and foreground-service integration remain
  deferred.

This is not claimed as Android runtime evidence.

## 15. Failure evidence and fixes

1. `Engine::stop` does not drop the `Store`; restart tests initially held the
   old handle and failed to reopen the same data root. The tests now drop the
   engine before creating the next one.
2. The managed blob path is
   `<data_root>/store/blobs/<id[0..2]>/<id[2..4]>/<content_id>`; the corruption
   test uses that path instead of assuming a flat blob directory.
3. Clippy rejected a `Default` field reassignment in the store test helper; the
   test now builds the record with struct update syntax.
4. The first replica-serving test needed the original publisher to withdraw
   before the replica download, proving the replica was the source rather than
   the original uploader.
5. A failed `replica.add` originally risked leaving an announced provider. The
   implementation now verifies before advertisement and withdraws on failure.
6. A paused replica could be resumed accidentally by verification. The
   implementation now keeps paused replicas paused and requires an explicit
   resume.

## 16. Decisions and unresolved questions

- Replica IDs are deterministic (`replica-<content_id>`) so re-adding the same
  file reactivates one row and preserves its audit history.
- Hosting requires a locally verified copy. Stage 15 does not auto-download a
  file as part of `replica.add`.
- A removed row is retained. `replica.list` hides it unless
  `include_removed: true`.
- Verification of a paused replica updates metadata but does not resume it.
- Metered and battery signals are host-reported; platform integrations are
  later work. An unknown battery level never blocks.
- Upload throttling delays only file-chunk responses. Metadata and errors stay
  immediate so discovery and failure reporting are not slowed.
- The provider is withdrawn only when no other local serving reason remains for
  the content.
- The original signed publication is never modified by replica operations.
- The Kotlin Hosted Files screen remains labeled demo data; core wiring is a
  later UI stage.
- Android runtime replication evidence remains outstanding.

## 17. Next stage

Stage 16 adds local and distributed search, Explore, coverage, pagination, and
provider-loss handling. It has not started and requires explicit project-owner
approval before work begins. Stage 15 is merged into `main` as `264dcbb`, so the
next branch should be created from that commit.
