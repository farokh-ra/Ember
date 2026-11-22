# Ember Project Handoff

**Current stage:** 14 - publishing and file pages
**Stage status:** Complete
**Last updated:** 2026-11-22
**Branch:** `feat/14-publishing` (from `main` after Stage 13 merged)
**Next stage:** 15 - voluntary replication and availability (waiting for user permission)

## What was implemented

Stage 14 adds explicit signed file publishing on top of the Stage 07 publication
format, the Stage 10 DHT, the Stage 12 transfer stack, and the Stage 13 identity
workflow:

- A publish flow that takes a verified content ID from the streaming import API
  and binds it to a signed record with display filename, description, category,
  tags, size, manifest root, revision, and timestamp.
- DHT announcement under `sha256("NEXUS-DHT-PUB-v1" + publication_id)`, plus a
  content provider record so other peers can find the publisher by content ID.
- A serving gate that shares verified bytes only when a local non-tombstoned
  publication is serving, an active replica exists, or a completed download is
  retained.
- `publication.get` and `publication.list_local` with metadata, tags, local and
  serving state, announced state, availability, signature status, and preview.
- `publication.lookup` with signature, publication ID, and derived-key
  verification, signed-tombstone lookup, cache conflict retention, provider
  hints, and a separate availability result.
- Unpublish with explicit confirmation and ownership checks. It signs and
  announces a tombstone, stops local serving, withdraws the provider unless
  another local serving reason remains, and retains local bytes.
- A metadata-only preview contract for plain text and raster images with
  `auto_execute` always false. SVG, HTML, PDF, archives, binaries, and executable
  script extensions are never previewed.
- Five new core methods: `publication.create` (0x0501),
  `publication.unpublish` (0x0502), `publication.get` (0x0503),
  `publication.list_local` (0x0504), and `publication.lookup` (0x0505).
- Eight publication events: `publication.published`,
  `publication.publish_failed`, `publication.unpublished`,
  `publication.publish_cancelled`, `publication.unpublish_cancelled`,
  `publication.lookup_completed`, `publication.lookup_failed`, and
  `publication.lookup_cancelled`.
- Schema version 7 with publication category, size, tags, local/serving/
  announced state, availability, and last-check timestamps.
- A demo Publish flow and File Details page that show signature, serving, and
  availability as separate states, with safe-preview and unpublish-limit copy.
- 16 new Rust tests, for 281 total, and 2 new Kotlin demo catalogue tests.

## Files changed

Protocol crate:

- `crates/nexus-protocol/src/publication.rs`

Core crate:

- `crates/nexus-core/src/publications.rs` (new)
- `crates/nexus-core/src/downloads.rs`
- `crates/nexus-core/src/engine.rs`
- `crates/nexus-core/src/lib.rs`
- `crates/nexus-core/tests/stage12.rs`
- `crates/nexus-core/tests/stage14.rs` (new)

Network crate:

- `crates/nexus-net/src/driver.rs`
- `crates/nexus-net/src/lib.rs`

Store crate:

- `crates/nexus-store/migrations/0007_publications.sql` (new)
- `crates/nexus-store/src/lib.rs`
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/model.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/tests/local_import.rs`
- `crates/nexus-store/tests/profiles.rs`
- `crates/nexus-store/tests/stage07.rs`
- `crates/nexus-store/tests/stage14.rs` (new)
- `crates/nexus-store/tests/storage.rs`

Shared Kotlin:

- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoModels.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoCatalog.kt`
- `shared/presentation/src/jvmTest/kotlin/io/nexus/presentation/demo/DemoCatalogTest.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/components/Status.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/resources/NexusStrings.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/screens/FileDetailsScreen.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/screens/PublishScreen.kt`

Documentation:

- `docs/progress/STAGE_14.md` (new)
- `docs/architecture/FFI_INTERFACE.md` (version 1.7.0)
- `docs/architecture/DHT_DESIGN.md`
- `docs/architecture/FILE_TRANSFER.md`
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

## Commands and results

### Rust tests

```text
cargo test --workspace -- --test-threads=1
```

Result: PASS, 281 tests, 0 failures.

Stage 14-specific suites:

```text
cargo test -p nexus-core --lib publications:: -- --test-threads=1
cargo test -p nexus-core --test stage14 -- --test-threads=1
cargo test -p nexus-store --test stage14 -- --test-threads=1
cargo test -p nexus-store --lib migrations:: -- --test-threads=1
```

Result: PASS, 9, 1, 5, and 6 tests respectively. The migration suite includes
the Stage 14 backfill test.

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
| `nexus-store` Stage 14 tests | 5 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` storage tests | 12 | 0 |
| `nexus-transfer` unit tests | 7 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **281** | **0** |

Stage 13 had 265 tests. Stage 14 adds 9 core publication tests, 5 store Stage 14
tests, 1 migration test, and 1 core Stage 14 test.

### Kotlin checks

```text
gradlew --no-daemon --console=plain spotlessCheck detekt
  :shared:presentation:jvmTest :shared:ui:jvmTest
  :apps:desktopApp:createDistributable :apps:androidApp:assembleDebug
```

Result: PASS, including the desktop distributable and Android debug APK. Gradle
uses `GRADLE_USER_HOME=NeededToolsHere/gradle-home`, so packaging downloads stay
inside the repository. The Stage 14 demo changes add two `DemoCatalogTest` cases.

### Formatting, linting, and dependency policy

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

- Formatting and Clippy with warnings denied: PASS.
- `Cargo.lock` remains at 411 packages. Stage 14 adds no dependency.
- The native artifact build was not re-run because the ABI and exported surface
  did not change. The six C ABI tests still pass in the workspace suite.
- `cargo deny check` and `cargo audit` were not re-run for Stage 14; no
  dependency changed.

## Errors and failed attempts

1. Stage 12's integration test assumed imported content could be downloaded
   without a publication. The Stage 14 serving gate correctly refuses that, so
   the test now creates an identity and publishes the imported content first.
2. A publish to a client-only DHT peer fails with `NetError::Rejected`. The
   Stage 14 integration test runs both publisher and reader as DHT servers so
   the publisher can store its record with a reachable peer.
3. The existing `publications` table referenced local identities and contents,
   and `replicas` referenced publications. A verified remote publication has
   neither a local identity row nor local bytes, so migration 7 rebuilds both
   tables and treats `publication_id` on a replica as optional attribution.
4. Clippy rejected `chunks_exact(2)` and a redundant closure in the new code;
   both were simplified.
5. The preview allowlist originally included shell and PowerShell extensions.
   They were removed because a preview must never normalize opening a program.

## Decisions and unresolved questions

- Stage 14 was explicitly authorized by the project owner and branches from
  `main` after Stage 13 merged in PR #12.
- `publication.create` accepts a verified content ID instead of file paths. File
  selection and streaming import remain host responsibilities.
- Publishing sets `serving = 1` immediately. The record is useful locally even
  when the announcement is deferred.
- Unpublishing retains local bytes and the local original reference. A separate
  cleanup action may remove them later.
- A tombstone ID is the local display identifier `"{publication_id}:{revision}"`;
  the DHT key remains derived from the publication ID.
- A missing tombstone is not an error; an invalid tombstone fails the lookup.
- Availability is a last-observed hint, not a guarantee. A provider record does
  not prove the peer will answer.
- Preview support is an extension allowlist, not content sniffing. Unsupported
  formats require an explicit download and verification.
- Publication editing and revision chains are deferred; Stage 14 creates
  revision 1 only.
- Stage 14 announces records on publish and unpublish but does not run a
  periodic re-announcement worker. The 24 h DHT TTL makes long-term
  discoverability a later hosting concern.
- The Kotlin publishing and file pages remain labeled demo data; core wiring is
  a later UI stage.
- The ABI remains 1; the core protocol version is `0.1.0-stage-14`. The identify
  protocol remains `/nexus/0.1.0-stage-12` because no network frame changed.
- Android runtime publishing and serving evidence is deferred because no
  emulator or device is available.

## Exact next action

1. Review `docs/progress/STAGE_14.md`, the updated `docs/architecture/`
   documents, the `nexus-protocol`, `nexus-store`, `nexus-net`, and
   `nexus-core` Stage 14 changes, and the Stage 14 PR.
2. Merge Stage 14 after approval. Stage 13 (PR #12) is already merged.
3. Give explicit approval to start Stage 15. Stage 15 adds voluntary replication
   and availability: an explicit host action, replica verification before
   advertising, pause and remove controls, storage and bandwidth limits, and
   Android metered-network and battery controls. Do not begin it before that
   approval.
