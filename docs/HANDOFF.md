# Ember Project Handoff

**Current stage:** 15 - voluntary replication and availability
**Stage status:** Complete and merged
**Last updated:** 2026-11-23
**Branch:** `main` at `264dcbb` (Stage 15 merged through PR #18; feature branch
`feat/15-replication` at `3558a03`)
**Next stage:** 16 - local and distributed search (not started; requires
explicit project-owner approval)

## What was implemented

Stage 15 turns the Stage 14 serving gate into explicit, user-controlled hosting
on top of the Stage 05 managed store, the Stage 10 DHT and provider records, the
Stage 12 transfer stack, and the Stage 14 signed publication workflow:

- Explicit per-file hosting through `replica.add`, with mandatory confirmation,
  a locally verified copy, and no automatic download or silent seeding.
- Full verification of the content ID, canonical manifest, and signed
  publication binding before any provider advertisement.
- Replica lifecycle states `active`, `paused`, `verification_failed`, and
  `removed`, with pause, resume, remove, and manual verify operations.
- Startup re-verification and an hourly maintenance pass that re-checks replicas
  whose last successful verification is older than seven days.
- Storage quota control through `store.set_quota` across the 1-500 GiB product
  range.
- Upload bandwidth, metered-network, and battery-threshold policy through
  `hosting.limits_get` and `hosting.limits_set`, enforced for all inbound file
  serving.
- A serving gate that combines verified content, a local serving reason, and the
  current hosting policy.
- Last-seen, last-verified, last-checked, failure-count, redacted-error, and
  announcement tracking per replica.
- Nine new core methods: `store.set_quota` (0x0402), `replica.add` (0x0901),
  `replica.list` (0x0902), `replica.pause` (0x0903), `replica.resume` (0x0904),
  `replica.remove` (0x0905), `replica.verify` (0x0906),
  `hosting.limits_get` (0x0907), and `hosting.limits_set` (0x0908).
- Eleven hosting events: `replica.verifying`, `replica.active`,
  `replica.failed`, `replica.paused`, `replica.resumed`, `replica.removed`,
  `replica.verified`, `replica.verification_failed`, `replica.cancelled`,
  `replica.maintenance_completed`, and `hosting.limits_changed`.
- Schema version 8 with replica source attribution, verified size, check and
  seen timestamps, failure count, redacted error, announcement state, and a
  state index.
- A demo Hosted Files screen that shows publication, size, verification, last
  verification time, serving state, storage use, and pause, resume, remove, and
  restore actions with visible demo labels.
- 5 new Rust tests, for 286 total, and extended Kotlin demo catalogue checks.

## Files changed

Protocol crate:

- No changes. Stage 15 reuses the existing manifest, publication, provider, and
  file-transfer formats.

Core crate:

- `crates/nexus-core/src/hosting.rs` (new)
- `crates/nexus-core/src/replicas.rs` (new)
- `crates/nexus-core/src/downloads.rs`
- `crates/nexus-core/src/engine.rs`
- `crates/nexus-core/src/lib.rs`
- `crates/nexus-core/tests/stage15.rs` (new)

Network crate:

- `crates/nexus-net/src/config.rs`
- `crates/nexus-net/src/driver.rs`
- `crates/nexus-net/src/lib.rs`

Store crate:

- `crates/nexus-store/migrations/0008_replication.sql` (new)
- `crates/nexus-store/src/lib.rs`
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/model.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/tests/local_import.rs`
- `crates/nexus-store/tests/profiles.rs`
- `crates/nexus-store/tests/stage14.rs`
- `crates/nexus-store/tests/stage15.rs` (new)
- `crates/nexus-store/tests/storage.rs`

Shared Kotlin:

- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoModels.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoCatalog.kt`
- `shared/presentation/src/jvmTest/kotlin/io/nexus/presentation/demo/DemoCatalogTest.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/components/Cards.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/resources/NexusStrings.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/screens/HostedFilesScreen.kt`

Documentation:

- `docs/progress/STAGE_15.md` (new)
- `docs/architecture/FFI_INTERFACE.md` (version 1.8.0)
- `docs/ARCHITECTURE.md`
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

Dependency lock:

- No change. `Cargo.lock` remains at 411 packages.

## Commands and results

### Rust tests

```text
cargo test --workspace -- --test-threads=1
```

Result: PASS, 286 tests, 0 failures.

Stage 15-specific suites:

```text
cargo test -p nexus-core --test stage15 -- --test-threads=1
cargo test -p nexus-store --test stage15 -- --test-threads=1
```

Result: PASS, 3 and 2 tests respectively.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 57 | 0 |
| `nexus-core` Stage 09 tests | 4 | 0 |
| `nexus-core` Stage 10 tests | 2 | 0 |
| `nexus-core` Stage 11 tests | 4 | 0 |
| `nexus-core` Stage 12 tests | 1 | 0 |
| `nexus-core` Stage 13 tests | 1 | 0 |
| `nexus-core` Stage 14 tests | 1 | 0 |
| `nexus-core` Stage 15 tests | 3 | 0 |
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
| `nexus-protocol` unit tests | 32 | 0 |
| `nexus-protocol` signed-record tests | 9 | 0 |
| `nexus-protocol` Stage 07 tests | 8 | 0 |
| `nexus-store` unit tests | 18 | 0 |
| `nexus-store` local-import tests | 4 | 0 |
| `nexus-store` profile tests | 5 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` Stage 14 tests | 5 | 0 |
| `nexus-store` Stage 15 tests | 2 | 0 |
| `nexus-store` storage tests | 12 | 0 |
| `nexus-transfer` unit tests | 7 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **286** | **0** |

Stage 14 had 281 tests. Stage 15 adds 3 core integration tests and 2 store
integration tests.

### Kotlin checks

```text
gradlew --no-daemon --console=plain spotlessCheck detekt
  :shared:presentation:jvmTest :shared:ui:jvmTest
  :apps:desktopApp:createDistributable :apps:androidApp:assembleDebug
```

Result: PASS, including the desktop distributable and Android debug APK. Gradle
uses `GRADLE_USER_HOME=F:\Ember\NeededToolsHere\gradle-home`, so the wrapper,
dependency cache, and WiX packaging download stay inside the repository. The
Stage 15 demo changes extend `DemoCatalogTest` with publication-ID, storage,
serving-state, and verification invariants.

### Formatting, linting, and dependency policy

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

- Formatting and Clippy with warnings denied: PASS.
- Stage 15 adds no direct dependency; `Cargo.lock` remains at 411 packages.
- `cargo deny check` and `cargo audit` run in CI with the Stage 14 lockfile.
- The FFI interface document is at version 1.8.0 with forty-two callable
  methods.
- The ABI remains 1; the core protocol version is `0.1.0-stage-15`. The identify
  protocol remains `/nexus/0.1.0-stage-12` because no network frame changed.

## Errors and failed attempts

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
7. The first Gradle run in the new worktree tried to resolve Android Gradle
   Plugin jars from Google Maven and received 404 responses. The populated
   repository cache at `F:\Ember\NeededToolsHere\gradle-home` was used instead,
   and the gate passed.

## Decisions and unresolved questions

- Replica IDs are deterministic (`replica-<content_id>`) so re-adding the same
  file reactivates one row and preserves its audit history.
- Hosting requires a locally verified copy. Stage 15 does not auto-download a
  file as part of `replica.add`.
- A removed row is retained. `replica.list` hides it unless
  `include_removed: true`.
- Verification of a paused replica updates metadata but does not resume it.
- Metered and battery signals are host-reported; Android and Windows platform
  integrations are later work. An unknown battery level never blocks.
- Upload throttling delays only file-chunk responses. Metadata and errors stay
  immediate so discovery and failure reporting are not slowed.
- The provider is withdrawn only when no other local serving reason remains for
  the content.
- The original signed publication is never modified by replica operations.
- The Kotlin Hosted Files screen remains labeled demo data; core wiring is a
  later UI stage.
- Android runtime replication, metered detection, and battery-aware serving
  evidence is deferred because no emulator or device is available.

## Exact next action

1. Stage 15 is reviewed, approved, and merged into `main` as `264dcbb`; no
   further merge action is required.
2. Give explicit approval to start Stage 16. Stage 16 adds local and distributed
   search, Explore, coverage, pagination, and provider-loss handling. Do not
   begin it before that approval.
3. Stage 16 should begin from `main` at `264dcbb`, with a new feature branch,
   and must keep FTS5 indexing limited to verified local records.
