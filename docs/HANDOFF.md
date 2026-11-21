# Ember Project Handoff

**Current stage:** 13 - profiles and signed public metadata
**Stage status:** Complete
**Last updated:** 2026-11-21
**Branch:** `feat/13-profiles` (stacked on `feat/12-streaming-transfer`)
**Next stage:** 14 - publishing and file pages (waiting for user permission)

## What was implemented

Stage 13 adds signed public profiles on top of the Stage 10 DHT and the Stage 08
identity vault:

- A canonical v1 profile body with display name, optional username label, bio,
  optional avatar content ID, bounded publication references, revision, and
  update time.
- Local editing that signs every field change through the identity vault and
  increments the revision by one.
- `publish: false` stores a signed draft and emits `profile.draft_saved` without
  any DHT announcement.
- DHT publication under `sha256("NEXUS-DHT-PROFILE-v1" + identity_id)`, with
  the exact signed envelope stored before network work starts.
- Lookup that verifies the envelope, requested identity ID, and derived DHT key
  before caching.
- Newer-revision replacement, older-revision stale reporting without rollback,
  and same-revision conflict retention in `profile_conflicts`.
- A cache served after a network miss or failure is labelled stale, and cached
  rows older than seven days are also stale.
- Case-insensitive username-label collision reporting with distinct identity
  IDs. Usernames are labels, not unique accounts.
- `expected_revision` optimistic concurrency and a no-field republish that keeps
  the existing revision.
- Three new core methods: `profile.get_local` (0x0301), `profile.publish`
  (0x0302), and `profile.lookup` (0x0303).
- Six profile events: `profile.draft_saved`, `profile.published`,
  `profile.lookup_completed`, `profile.lookup_failed`,
  `profile.publish_cancelled`, and `profile.lookup_cancelled`.
- Schema version 6 with profile lifecycle, freshness, timestamps, and retained
  conflicts.
- A demo Profile card that shows signed, state, freshness, revision, identity,
  conflict, and collision labels. It remains visibly simulated.
- 14 new Rust tests, for 265 total.

## Files changed

Protocol crate:

- `crates/nexus-protocol/src/envelope.rs`

Core crate:

- `crates/nexus-core/src/profiles.rs` (new)
- `crates/nexus-core/src/engine.rs`
- `crates/nexus-core/src/lib.rs`
- `crates/nexus-core/tests/stage13.rs` (new)

Store crate:

- `crates/nexus-store/migrations/0006_profiles.sql` (new)
- `crates/nexus-store/src/lib.rs`
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/model.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/src/util.rs`
- `crates/nexus-store/tests/local_import.rs`
- `crates/nexus-store/tests/profiles.rs` (new)
- `crates/nexus-store/tests/storage.rs`

Shared Kotlin:

- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoModels.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoCatalog.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/components/Cards.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/components/ProfileCards.kt` (new)
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/screens/ProfileScreen.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/resources/NexusStrings.kt`

Developer tooling:

- `scripts/dev-env.ps1` (repo-local `GRADLE_USER_HOME`)

Documentation:

- `docs/progress/STAGE_13.md` (new)
- `docs/architecture/FFI_INTERFACE.md` (version 1.6.0)
- `docs/architecture/DHT_DESIGN.md`
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

## Commands and results

### Rust tests

```text
cargo test --workspace -- --test-threads=1
```

Result: PASS, 265 tests, 0 failures.

Stage 13-specific suites:

```text
cargo test -p nexus-core --lib profiles:: -- --test-threads=1
cargo test -p nexus-core --test stage13 -- --test-threads=1
cargo test -p nexus-store --test profiles -- --test-threads=1
```

Result: PASS, 7, 1, and 5 tests respectively.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 48 | 0 |
| `nexus-core` Stage 09 tests | 4 | 0 |
| `nexus-core` Stage 10 tests | 2 | 0 |
| `nexus-core` Stage 11 tests | 4 | 0 |
| `nexus-core` Stage 12 tests | 1 | 0 |
| `nexus-core` Stage 13 tests | 1 | 0 |
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
| `nexus-store` unit tests | 17 | 0 |
| `nexus-store` local-import tests | 4 | 0 |
| `nexus-store` profile tests | 5 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` storage tests | 12 | 0 |
| `nexus-transfer` unit tests | 7 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **265** | **0** |

Stage 12 had 251 tests. Stage 13 adds 7 core profile-worker tests, 5 store profile
tests, 1 protocol envelope test, and 1 core Stage 13 test.

### Kotlin checks

```text
gradlew --no-daemon --console=plain spotlessCheck detekt
  :shared:presentation:jvmTest :shared:ui:jvmTest
  :apps:desktopApp:createDistributable :apps:androidApp:assembleDebug
```

Result: PASS, including the desktop distributable and Android debug APK. Gradle
uses `GRADLE_USER_HOME=NeededToolsHere/gradle-home`, so packaging downloads stay
inside the repository.

### Formatting, linting, and dependency policy

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

- Formatting and Clippy with warnings denied: PASS.
- `Cargo.lock` remains at 411 packages. Stage 13 adds no dependency.
- The native artifact build was not re-run because the ABI and exported surface
  did not change. The six C ABI tests still pass in the workspace suite.
- `cargo deny check` and `cargo audit` were not re-run for Stage 13; no
  dependency changed.

## Errors and failed attempts

1. Migration and schema assertions still expected version 5 after migration 6
   was added. They now assert `LATEST_VERSION` and schema 6.
2. The Stage 13 integration test initially used two DHT clients and could not
   satisfy quorum because a client rejects stored records. It now uses two DHT
   servers plus one client reader, which also makes the stale-cache assertion
   deterministic.
3. A publish with no profile fields originally tried to build a new record from
   empty fields. It now republishes the existing signed bytes without a revision
   bump.
4. Detekt counted seventeen functions in `Cards.kt`. The profile card and its
   helpers moved to `ProfileCards.kt`, leaving both files under the threshold.
5. Clippy rejected an eight-argument publish worker. The worker now takes the
   verified `SignedProfile` and derives its identity, revision, timestamp, and
   bytes.
6. Clippy also rejected redundant row closures and a collapsible username
   check; both were simplified.

## Decisions and unresolved questions

- Stage 13 was explicitly authorized by the project owner after Stage 12 was
  completed.
- Stage 13 was stacked on `feat/12-streaming-transfer` while PR #11 was open.
  PR #11 merged on 2026-11-20, and Stage 13 was rebased onto the merge commit;
  PR #12 now targets `main`.
- Profiles are exact signed records under an identity-derived DHT key. There is
  no profile registry or unique handle.
- Username labels are compared case-insensitively for transparency, but the
  stored spelling is preserved. Collisions cover only profiles the local store
  has seen.
- `profile.publish` defaults to publishing. A draft requires `publish: false`.
- Omitting every profile field republishes the existing signed record; it does
  not create an empty revision.
- Same-revision conflicts are retained for review rather than resolved
  automatically. Conflict presentation and resolution remain later work.
- Cached remote profiles are always labelled stale after a network miss or
  failure, even when the previous check was recent.
- The ABI remains 1; the core protocol version is `0.1.0-stage-13`. The identify
  protocol remains `/nexus/0.1.0-stage-12` because no network frame changed.
- The Kotlin Profile UI remains labeled demo data; core wiring is a later UI
  stage.
- Android runtime profile evidence is deferred because no emulator or device is
  available.
- GitHub Actions passed all seven checks for PR #12 on 2026-11-21.

## Exact next action

1. Review `docs/progress/STAGE_13.md`,
   `docs/architecture/DHT_DESIGN.md`, the updated
   `docs/architecture/FFI_INTERFACE.md`, the `nexus-protocol`,
   `nexus-store`, and `nexus-core` Stage 13 changes, and PR #12.
2. Merge Stage 13 (PR #12) after approval. Stage 12 is already merged.
3. Give explicit approval to start Stage 14. Stage 14 adds explicit file
   selection and publishing: descriptions, categories, tags, signed publication
   records, publisher availability, file details, and unpublish behavior. Do not
   begin it before that approval.
