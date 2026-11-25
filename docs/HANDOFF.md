# Ember Project Handoff

**Current stage:** 16 - verified local and distributed search
**Stage status:** Complete on Windows loopback; review and merge pending
**Last updated:** 2026-11-24
**Branch:** `feat/16-search`, based on `main` at `05385de`; committed and pushed
to `origin`, merge pending
**Next stage:** 17 - full UI, accessibility, settings, foreground service, and
Windows tray (requires explicit project-owner approval)

## What was implemented

Stage 16 turns the reserved search surface into a local-first discovery flow on
top of the Stage 05 derived index, the Stage 10 DHT, the Stage 13 profile and
Stage 14 publication records, and the Stage 15 serving rules:

- A verified local FTS5 index over published and remote profiles and
  non-tombstoned publications, maintained incrementally and rebuildable from
  canonical signed records.
- Schema version 9 with `search_meta` generation state and a `search_docs` rowid
  map for logarithmic incremental replacement.
- Bounded query text (128 bytes, 12 tokens), kind, category, up to 5 exact tags,
  relevance/recency/availability ordering, and 1-20 row pages.
- Synchronous local results with total matches, candidate-limit state, coverage,
  and generation- and query-bound opaque cursors.
- Local Explore modes for category counts, recent records, and seeded bounded
  random samples without popularity or engagement signals.
- Optional signed index providers over `/nexus/index/1`: bounded CBOR messages,
  at most 8 providers, one 10-second total deadline, and 120 inbound requests
  per peer per minute.
- In-memory verification of every provider record. Unsigned, tampered,
  wrong-kind, oversized, and malformed records are rejected and never written by
  search.
- Provider additions stream as `search.result_page`; every search emits exactly
  one terminal `search.completed` with coverage, per-provider state, conflicts,
  rejected and ignored counts, cancellation, and elapsed time.
- `search.cancel`, the opt-in `search_provider` config flag, and best-effort DHT
  announcement of the optional provider role.
- Three implemented methods: `discovery.explore` (0x0701), `search.query`
  (0x0702), and `search.cancel` (0x0703).
- Two new events: `search.result_page` and `search.completed`.
- Kotlin Explore and Search demo states for coverage, partial, cached, offline,
  and pagination labels over sample data.
- 44 new Rust tests, for 330 total.

## Files changed

Search crate:

- `crates/nexus-search/Cargo.toml`
- `crates/nexus-search/src/lib.rs`
- `crates/nexus-search/src/query.rs` (new)
- `crates/nexus-search/src/cursor.rs` (new)
- `crates/nexus-search/src/coverage.rs` (new)
- `crates/nexus-search/src/explore.rs` (new)
- `crates/nexus-search/src/result.rs` (new)
- `crates/nexus-search/src/sanitize.rs` (new)

Protocol crate:

- `crates/nexus-protocol/src/index.rs` (new)
- `crates/nexus-protocol/src/lib.rs`
- `crates/nexus-crypto/src/domain.rs` (`DhtIndex` domain tag)

Store crate:

- `crates/nexus-store/migrations/0009_search_state.sql` (new)
- `crates/nexus-store/src/search_store.rs` (new)
- `crates/nexus-store/src/lib.rs`
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/src/error.rs`
- `crates/nexus-store/tests/search.rs` (new)
- Existing store test schema assertions updated to version 9.

Network crate:

- `crates/nexus-net/src/index.rs` (new)
- `crates/nexus-net/src/lib.rs`
- `crates/nexus-net/src/behaviour.rs`
- `crates/nexus-net/src/codec.rs`
- `crates/nexus-net/src/driver.rs`

Core crate:

- `crates/nexus-core/src/search.rs` (new)
- `crates/nexus-core/src/engine.rs`
- `crates/nexus-core/src/config.rs`
- `crates/nexus-core/src/error.rs`
- `crates/nexus-core/src/lib.rs`
- `crates/nexus-core/Cargo.toml`
- `crates/nexus-core/tests/stage16.rs` (new)
- Existing Stage 09-15 test configs updated for `search_provider`.

FFI:

- `crates/nexus-ffi/src/registry.rs` (test config only; methods invoke
  generically).

Shared Kotlin:

- `shared/ui/src/commonMain/kotlin/io/nexus/ui/screens/ExploreScreen.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/screens/SearchScreen.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/resources/NexusStrings.kt`

Documentation:

- `docs/progress/STAGE_16.md` (new)
- `docs/architecture/FFI_INTERFACE.md` (version 1.9.0)
- `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md` (version 1.9.0)
- `docs/ARCHITECTURE.md`
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`
- `docs/decisions/ADR-0009-build-packaging-dependency-policy.md`,
  `docs/progress/STAGE_02.md`, `docs/progress/STAGE_03.md`, and
  `docs/DEVELOPER_SETUP.md` now state the committed wrapper pins (Gradle 9.9.2,
  AGP 9.5.2) that the earlier dependency bump left stale.
- `docs/progress/STAGE_09.md` escapes literal pipes inside the `network.*` event
  names in a table.
- `scripts/validate_stage01_docs.py` and `scripts/validate_stage02_docs.py` now
  split table rows on unescaped pipes only, so an escaped `\|` no longer inflates
  a cell count.

Dependency lock:

- `Cargo.lock` is updated by the workspace build; the resolved package count
  remains 411.

## Commands and results

### Rust tests

```text
cargo test --workspace -- --test-threads=1
```

Result: PASS, 330 tests, 0 failures.

Stage 16-specific suites:

```text
cargo test -p nexus-core --test stage16 -- --test-threads=1
cargo test -p nexus-store --test search -- --test-threads=1
```

Result: PASS, 4 and 4 tests respectively.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 60 | 0 |
| `nexus-core` Stage 09 tests | 4 | 0 |
| `nexus-core` Stage 10 tests | 2 | 0 |
| `nexus-core` Stage 11 tests | 4 | 0 |
| `nexus-core` Stage 12 tests | 1 | 0 |
| `nexus-core` Stage 13 tests | 1 | 0 |
| `nexus-core` Stage 14 tests | 1 | 0 |
| `nexus-core` Stage 15 tests | 3 | 0 |
| `nexus-core` Stage 16 tests | 4 | 0 |
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
| `nexus-store` unit tests | 18 | 0 |
| `nexus-store` local-import tests | 4 | 0 |
| `nexus-store` profile tests | 5 | 0 |
| `nexus-store` search tests | 4 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` Stage 14 tests | 5 | 0 |
| `nexus-store` Stage 15 tests | 2 | 0 |
| `nexus-store` storage tests | 12 | 0 |
| `nexus-transfer` unit tests | 7 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **330** | **0** |

Stage 15 had 286 tests. Stage 16 adds 3 core unit tests, 4 core integration
tests, 6 protocol unit tests, 27 search unit tests, and 4 store integration
tests.

### Kotlin checks and packaging

```text
gradlew --no-daemon --console=plain spotlessCheck detekt
  :shared:presentation:jvmTest :shared:ui:jvmTest
gradlew --no-daemon --console=plain
  :apps:desktopApp:createDistributable :apps:androidApp:assembleDebug
```

Result: PASS. Spotless, detekt, both JVM test suites, the desktop
distributable, and the Android debug APK all succeeded. The build used the
pinned JDK 21 and `GRADLE_USER_HOME=F:\Ember\NeededToolsHere\gradle-home`.

### Native builds

```text
.\scripts\build-native.ps1 -Target All -Configuration Release
```

Result: PASS. The Windows `nexus_core.dll` built with all 19 required exports,
the C harness built and ran, and both Android libraries cross-built with
verified exports.

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
- Stage 16 adds no direct dependency; `Cargo.lock` remains at 411 packages.
- The FFI interface document is at version 1.9.0 with forty-five callable
  methods.
- The ABI remains 1; the core protocol version is `0.1.0-stage-16`. The identify
  protocol remains `/nexus/0.1.0-stage-12`; Stage 16 adds the separate
  `/nexus/index/1` protocol.

## Errors and failed attempts

1. The 10,000-record rebuild initially took 85.1 seconds because deleting FTS
   rows by the `UNINDEXED` kind and owner columns scanned the virtual table. A
   `search_docs` rowid map was added, and the rebuild now passes the 30-second
   bound.
2. The stale-cursor store test expected a string. The error is the typed
   `SearchError::StaleCursor`; the test now matches that variant.
3. Clippy rejected a shadowed test helper, a redundant closure, two
   `format!`-wrapped constant queries, and a field reassignment after
   `Default::default()`. The code was corrected without relaxing the gate.
4. Stage 16 publication fixtures publish asynchronously. The core integration
   test now waits for `publication.published`, so a late index write cannot
   invalidate a cursor mid-test.
5. The first Gradle invocation used JVM 8 and failed. The environment script was
   blocked by the PowerShell execution policy, so the pinned JDK 21 was set
   directly for the gate. The wrapper downloaded Gradle 9.9.2 into the
   repository cache.
6. A first Stage 16 README test estimate was low. A source audit and the full
   suite established the final 330-test total.
7. The Stage 01 and Stage 02 documentation validators failed on unescaped pipes
   inside `docs/progress/STAGE_09.md` table cells. The cells now escape them and
   both validators split on unescaped pipes rather than unescaping first.
8. The dependency bump that moved the wrapper to Gradle 9.9.2 and AGP to 9.5.2
   left the pin statements on 9.9.1/9.5.1. The ADR, Stage 02 and Stage 03
   records, dependency inventory, README, and Stage 02 validator now state the
   committed pins; the optional local Gradle install stays 9.9.1 and is
   documented as non-authoritative.

## Decisions and unresolved questions

- Search is local-first. Provider fan-out never delays the local page.
- Provider results are verified in memory and are not written to local storage
  by search. A later profile or publication lookup may cache verified bytes.
- Fan-out runs only on the cursorless first page. Later pages are local pages
  bound to one generation; a fresh provider view requires a new search.
- A generation change rejects outstanding cursors rather than returning a
  snapshot that may have shifted.
- `search_provider` is opt-in. The role is announced best-effort and retried
  until the DHT has a peer.
- `cached` remains false for provider-only rows.
- Conflicting revisions and metadata stay visible; there is no conflict
  resolution UI yet.
- The Kotlin Explore and Search screens remain labelled demo data with honest
  coverage, partial, cached, offline, and pagination states.
- Android runtime search, index-provider discovery, on-device latency, and
  Kotlin-to-core wiring remain deferred because no emulator or device is
  available.

## Exact next action

1. Open a pull request from the pushed `feat/16-search` branch and review it
   against the gates recorded above.
2. Merge only after review, then delete the merged branch.
3. Do not start Stage 17 without explicit project-owner approval.
4. Stage 17 covers the full UI, accessibility, settings, Android foreground
   service, and Windows tray; it must wire the screens to the core rather than
   extend demo data.
