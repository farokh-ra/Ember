# Stage 16 - Verified Local and Distributed Search

**Status:** Complete on Windows loopback; merged into `main`
**Completed:** 2026-11-24
**Branch:** `feat/16-search`, based on `main` at `05385de`
**Progress guide:** 75-80%
**Next stage:** 17 - full UI, accessibility, settings, foreground service, and
Windows tray (requires explicit project-owner approval)

## 1. Scope

Stage 16 turns the reserved search surface into a working, honest discovery
flow. Search is local-first: the core maintains a derived FTS5 index over
records that already passed schema and signature validation, and it may ask a
bounded set of optional peer index providers for broader coverage. There is no
central search service, no full-text DHT, and no popularity or engagement
ranking.

The stage covers:

- A verified local FTS5 index maintained incrementally from profile and
  publication writes, removal on unpublish, and a transactional rebuild from
  stored signed records.
- Bounded query text, kind, category, tag, sort, page-size, and pagination
  inputs.
- Local Explore modes for category counts, recent records, and a seeded bounded
  random sample.
- Generation-bound cursors that reject stale index snapshots instead of
  silently skipping or repeating records.
- Optional signed index providers over `/nexus/index/1`, with bounded
  fan-out, one total deadline, per-peer rate limiting, and in-memory
  verification of every returned record.
- Coverage labels, partial-result labels, provider-loss handling, duplicate
  suppression, and visible conflicts.
- `search.result_page` streaming and exactly one terminal `search.completed`
  event per search.
- Kotlin Explore and Search demo states that show coverage, partial, cached,
  offline, and pagination labels over sample data.

## 2. Architecture

Search is split by responsibility:

| Crate | Responsibility |
|---|---|
| `nexus-search` | Pure query validation, tokenization, FTS expression, cursor, coverage, deduplication, conflict, ordering, Explore, sanitization, and provider re-check logic. No I/O. |
| `nexus-store` | FTS5 tables, incremental indexing, local and provider pages, Explore pages, index generation, rebuild, and exact signed-record lookup. |
| `nexus-protocol` | Bounded CBOR `/nexus/index/1` request and response types, record kinds, and the fixed provider key. |
| `nexus-net` | Index libp2p protocol, framing, inbound and outbound handlers, timeouts, per-peer rate limiting, and the `IndexProvider` trait. |
| `nexus-core` | `0x0701`-`0x0703` dispatch, local search, provider discovery and fan-out, verification, deduplication, events, cancellation, capabilities, and the optional provider role. |

Search never parses raw network frames. It receives verified signed records
through `nexus-protocol`, reads local index rows through `nexus-store`, and
returns only records with an explicit verification state.

## 3. Derived local index

Migration `0009_search_state.sql` makes the FTS5 table created in migration 2
live derived state:

- `search_meta` stores the index generation and last refresh time.
- `search_docs` maps FTS rowids to `(kind, owner_id)`. FTS5 cannot index an
  `UNINDEXED` column, so deleting one document by kind and owner would scan the
  virtual table. This ordinary table keeps incremental replacement logarithmic.
- Existing published and remote profiles and non-tombstoned publications are
  backfilled once during the upgrade.

The index contains:

- Published and remote profiles: display name, username, bio, and identity ID.
- Non-tombstoned publications: title, filename, description, category, tags,
  owner identity ID, and publication ID.

Draft and pending profiles, tombstoned publications, quarantined records, and
invalid or unsigned records never enter the index. Every profile and
publication write maintains its row in the same transaction as the canonical
metadata. Unpublish removes the publication row, and a rebuild starts from the
canonical tables so it cannot resurrect a tombstone.

The generation advances on every indexed write. A pagination cursor embeds the
generation and the normalized query hash; a mismatch returns a conflict instead
of paging over a changed snapshot.

## 4. Local search contract

`search.query` is synchronous for the local page:

| Field | Bound |
|---|---|
| Query text | 128 UTF-8 bytes, 12 tokens, no C0/C1 controls or bidi overrides |
| Kind | `profile` or `publication` |
| Category | One closed publication category |
| Tags | At most 5 exact tags, each 32 bytes |
| Sort | `relevance` (default), `recency`, or `availability` |
| Limit | 1-20, default 20 |
| Cursor | 128-byte opaque `v1:<generation>:<offset>:<query_hash>` value |

The normalized query hash binds text tokens, kind, category, tags, and sort.
Reusing a cursor with another query returns `conflict`; reusing it after any
indexed write returns a conflict that tells the caller to restart the search.

Each result carries an explicit verification state and:

- Record kind and stable record ID.
- Content ID for publications.
- Owner identity ID.
- Sanitized, truncated title and summary.
- Category and tags.
- Revision and publish/update timestamps.
- Availability, source (`local` or `provider`), cached flag, and rank.
- Conflicts and same-content count.

The local page also returns `total_matches`, `next_cursor`, `page_complete`,
`providers_pending`, `candidate_limit_reached`, and `coverage`.

Text is bounded and sanitized before it leaves the core. FTS input is
tokenized into a quoted expression, so user text is never interpolated as raw
FTS syntax.

## 5. Explore

`discovery.explore` is local-only and synchronous:

| Mode | Behaviour |
|---|---|
| `categories` (default) | Returns counts for all eight closed categories and no result rows. |
| `recent` | Orders non-tombstoned publications by publish time. |
| `random` | Applies a bounded, deterministic seeded shuffle. |

Callers may pass a seed. The returned seed and cursor produce a stable next page.
Explore limit is 1-50, default 20. There are no likes, follows, comments,
popularity counts, or engagement signals in the schema or the query.

## 6. Distributed index providers

`/nexus/index/1` is a full request and response libp2p protocol:

- `IndexCodec` frames bounded CBOR messages with a 1 MiB hard cap.
- Requests carry bounded query text, kind, category, tags, page size, and a
  provider page cursor.
- Responses carry verified signed record bytes, a next cursor, completeness,
  or a short safe error string.
- Records are selected from the same verified local index and re-read from the
  canonical store as exact signed bytes. A provider never invents metadata,
  drafts, or tombstoned records.
- Outbound requests are bounded by a timeout and are not paginated by a search;
  provider fan-out runs only on the cursorless first page.
- Inbound requests are limited to 120 per peer per minute, and malformed,
  oversized, wrong-kind, or unsupported requests receive a bounded error.

Provider discovery uses the DHT key derived by `index_provider_key()`. The core
queries at most eight providers under one 10-second total deadline. Local
results are never delayed by discovery or provider waits. Provider additions
stream as `search.result_page`; exactly one `search.completed` event ends every
search, including local-only searches.

The optional provider role is enabled by `search_provider` at create time. The
role is announced best-effort through `start_providing` after the DHT has a
peer, with a bounded retry window. A node that does not opt in never serves
index requests.

## 7. Verification, coverage, and conflicts

Every provider record is parsed and signature-verified in memory before it can
enter a result page. Unsigned, tampered, wrong-kind, oversized, and malformed
records are rejected and counted. A rejected record cannot affect local results
or another provider's results. Search does not write provider records to local
storage; a later profile or publication lookup may cache verified bytes through
its normal path.

Coverage labels are:

| Label | Meaning |
|---|---|
| `local_only` | No provider participated. |
| `cached_local` | The page contains only locally cached rows. |
| `partial` | At least one attempted provider did not complete. |
| `complete` | Every attempted provider returned a complete valid page. |

`may_be_incomplete` is true only for distributed coverage that is not complete.
Bounded notes include `local_index_only`, `no_index_providers`,
`distributed_results_pending`, `provider_protocol_unsupported`,
`provider_page_truncated`, and `search_cancelled`. Per-provider state is `ok`,
`failed`, `timeout`, `unavailable`, `unsupported`, or `truncated`.

Duplicates against the whole local index and against other providers are
collapsed by record identity. Shared content IDs are grouped with a visible
count. Differing revisions or metadata remain visible in `conflicts`; no
provider can suppress a local row or overwrite another record.

`search.cancel` accepts the returned `op_id`. Cancellation stops discovery and
pending provider waits, keeps rows already streamed, and emits the terminal
event with `cancelled` true and the `search_cancelled` note.

## 8. Schema version 9

Migration `0009_search_state.sql` adds:

- `search_meta` with `generation` and `indexed_at_ms`.
- `search_docs` with a primary-key rowid and a unique `(kind, owner_id)` pair.
- One-time backfill from verified profiles and non-tombstoned publications.

No canonical record bytes, identities, publications, replicas, downloads, or
settings are rewritten. The FTS index remains derived state and can be deleted
and rebuilt without data loss.

## 9. Core integration

Stage 16 implements three methods reserved in the ABI 1 catalog:

| ID | Method | Mode | Request | Result |
|---|---|---|---|---|
| 0x0701 | `discovery.explore` | S | `mode?`, `category?`, `seed?`, `cursor?`, `limit?` | `mode`, `categories`, `results`, `seed`, `next_cursor?`, `page_complete`, `coverage` |
| 0x0702 | `search.query` | S/A | `query`, `kind?`, `category?`, `tags?`, `sort?`, `cursor?`, `limit?`, optional `filters` map | Synchronous local page with `op_id`; on the first page, `search.result_page` events and one terminal `search.completed` |
| 0x0703 | `search.cancel` | S | `op_id` | `op_id`, `outcome`, `cancelled` |

New events are `search.result_page` and `search.completed`. `system.info`
includes `discovery.explore`, `search.query`, and `search.cancel` when the core
is running. `CoreConfig` gains `search_provider`, default `false`.

New repository modules include:

- `crates/nexus-search` query, cursor, coverage, result, Explore, and sanitize
  modules plus 27 unit tests.
- `crates/nexus-store/migrations/0009_search_state.sql` and
  `crates/nexus-store/src/search_store.rs`.
- `crates/nexus-protocol/src/index.rs` with six unit tests.
- `crates/nexus-net/src/index.rs` plus driver and network startup integration.
- `crates/nexus-core/src/search.rs` with three unit tests.
- `crates/nexus-core/tests/stage16.rs` with four integration tests.
- `crates/nexus-store/tests/search.rs` with four integration tests.

## 10. Protections and bounds

- Only verified, published or remote profiles and non-tombstoned publications
  are indexed.
- Unsigned, tampered, wrong-kind, oversized, and malformed provider records are
  rejected before display and never written by search.
- Query length, tokens, tags, page size, cursor, error text, and index messages
  are bounded before allocation.
- Provider fan-out is capped at eight peers, one 10-second total deadline, and
  120 inbound requests per peer per minute.
- Local search returns without waiting for discovery or provider responses.
- Later pages are local pages bound to one generation; they never repeat
  provider fan-out.
- Provider records cannot suppress local rows, overwrite other records, or grant
  verification.
- Titles and summaries are plain text, sanitized, and truncated; no HTML is
  rendered and no downloaded content is executed.
- No popularity, reputation, engagement, or provider ranking affects order.
- A failed provider reduces coverage honestly; it does not fail the search.

## 11. Interface and UI

- `ExploreScreen` now shows an honest coverage label that changes for offline,
  degraded, and online preview states, shows the number of visible sample
  records, and pages through sample results.
- `SearchScreen` shows coverage, partial, cached/offline, and pagination labels
  over sample results with an explicit "showing X of Y" count and a load-more
  action.
- Both screens remain visibly labelled demo content. No Kotlin screen is wired
  to the core yet; wiring is later UI work.
- The demo does not claim provider completeness, live network coverage, or
  availability.

## 12. Verification evidence

### 12.1 `nexus-core` Stage 16 tests

4 tests, 0 failures:

| Test | Covers |
|---|---|
| `local_search_filters_paginates_and_labels_coverage_honestly` | Verified local results, kind/category/tag/sort filters, page bounds, cursor reuse, stale-cursor conflict, cancellation, and local-only coverage. |
| `explore_categories_recent_and_random_are_local_and_bounded` | Category counts, recent ordering, seeded random stability, and Explore bounds. |
| `provider_loss_keeps_local_results_and_reports_reduced_coverage` | Local rows survive provider loss and the terminal event reports reduced coverage. |
| `distributed_provider_results_are_verified_and_bounded` | A real loopback provider serves verified signed records; the searching core verifies, deduplicates, and reports complete distributed coverage. |

### 12.2 `nexus-store` Stage 16 tests

4 tests, 0 failures:

| Test | Covers |
|---|---|
| `lifecycle_changes_update_the_derived_index_and_conflicts_stay_visible` | Draft exclusion, publish indexing, remote conflict retention, publication indexing, unpublish removal, rebuild exclusion, and index stats. |
| `pages_are_generation_bound_and_stale_cursors_conflict` | Two-page pagination, page disjointness, index-generation conflict, recency order, and stale-cursor rejection. |
| `explore_and_provider_pages_use_only_verified_indexed_records` | Category counts, seeded random repeatability, exact provider bytes, and provider page completeness. |
| `local_search_stays_bounded_with_ten_thousand_records` | A 10,000-publication corpus: bounded first page, candidate-limit flag, local-only coverage, sub-second local query, and a bounded rebuild. |

### 12.3 Workspace totals

`cargo test --workspace -- --test-threads=1` passed with 330 tests and 0
failures:

| Suite | Tests |
|---|---:|
| `nexus-core` unit tests | 60 |
| `nexus-core` Stage 09 tests | 4 |
| `nexus-core` Stage 10 tests | 2 |
| `nexus-core` Stage 11 tests | 4 |
| `nexus-core` Stage 12 tests | 1 |
| `nexus-core` Stage 13 tests | 1 |
| `nexus-core` Stage 14 tests | 1 |
| `nexus-core` Stage 15 tests | 3 |
| `nexus-core` Stage 16 tests | 4 |
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
| `nexus-store` unit tests | 18 |
| `nexus-store` local-import tests | 4 |
| `nexus-store` profile tests | 5 |
| `nexus-store` search tests | 4 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-store` Stage 14 tests | 5 |
| `nexus-store` Stage 15 tests | 2 |
| `nexus-store` storage tests | 12 |
| `nexus-transfer` unit tests | 7 |
| `nexus-transfer` integration tests | 7 |
| `nexus-types` unit tests | 1 |
| **Total** | **330** |

Stage 15 had 286 tests. Stage 16 adds 44: 3 core unit tests, 4 core integration
tests, 6 protocol unit tests, 27 search unit tests, and 4 store integration
tests.

### 12.4 Kotlin checks and packaging

- `spotlessCheck`, `detekt`, `:shared:presentation:jvmTest`, and
  `:shared:ui:jvmTest` pass.
- `:apps:desktopApp:createDistributable` produced the Windows application image
  and `:apps:androidApp:assembleDebug` produced the Android debug APK.
- Gradle uses `GRADLE_USER_HOME=F:\Ember\NeededToolsHere\gradle-home`, so
  downloads stay inside the repository.

### 12.5 Native builds

- `.\scripts\build-native.ps1 -Target All -Configuration Release` passes.
- The Windows `nexus_core.dll` is built and its 19 required exported symbols are
  verified.
- The C harness builds and runs against the Windows library.
- The Android arm64-v8a and x86_64 `libnexus_core.so` libraries cross-build and
  their exports are verified.

### 12.6 Formatting, linting, and dependencies

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `python scripts/check_repo_hygiene.py` passes.
- `python scripts/validate_stage01_docs.py` and
  `python scripts/validate_stage02_docs.py` both pass.
- `cargo deny check` passes: advisories, bans, licenses, and sources are all
  allowed.
- `cargo audit` reports only the documented allowed `paste` unmaintained
  warning.
- Stage 16 adds no direct dependency; the workspace lockfile remains at 411
  packages.
- The FFI interface document is at version 1.9.0 with forty-five callable
  methods.

## 13. Exit criteria

| Criterion | Evidence |
|---|---|
| Local search works offline and returns verified results. | The local index is populated from verified writes only; Stage 16 tests search, filter, sort, paginate, and Explore with the network disabled. |
| Invalid or unsigned records never enter the index or results. | Draft, pending, tombstoned, unsigned, tampered, wrong-kind, oversized, and malformed records are rejected by construction or by in-memory verification tests. |
| Explore shows categories, recent records, and random records without popularity. | The Stage 16 store and core tests cover all three modes and repeatable seeds; no engagement fields exist. |
| Pagination is stable and honest. | Cursors bind the query and generation; stale cursors return `conflict` and the tests prove page disjointness. |
| Provider loss does not fail search. | Local results survive, the terminal event reports reduced coverage, and the provider-loss test asserts the notes and states. |
| Distributed results are verified and bounded. | A loopback provider serves signed bytes; the core verifies, deduplicates, and reports complete coverage within the deadline and page bounds. |
| Coverage, partial, cached, and offline states are labelled. | The core emits bounded coverage metadata and the Kotlin Explore and Search screens render honest labels. |
| Search remains bounded under load. | The 10,000-record test enforces a sub-second local search and a bounded rebuild; query, fan-out, message, and rate limits are explicit. |

## 14. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries cross-build with the same search,
  index-provider, and coverage code.
- The Stage 16 proofs run on Windows loopback through the same Rust code that
  Android will load.
- Android runtime search, index-provider discovery, on-device latency, and
  Kotlin-to-core wiring remain deferred.

This is not claimed as Android runtime evidence.

## 15. Failure evidence and fixes

1. The 10,000-record index rebuild initially took 85.1 seconds. Deleting FTS
   rows by the `UNINDEXED` kind and owner columns scanned the virtual table.
   `search_docs` now maps stable record IDs to rowids, and the rebuild passes the
   30-second bound.
2. The stale-cursor store test assumed a string error. `StoreError` exposes the
   typed `SearchError::StaleCursor`, and the test now matches that variant.
3. Clippy rejected a shadowed test helper, a redundant closure, two
   `format!`-wrapped constant queries, and a field reassignment after
   `Default::default()`. The tests and store code were corrected without
   weakening the checks.
4. Stage 16 publication fixtures publish asynchronously. The core test now waits
   for `publication.published` before assuming the derived index is stable, so a
   late index write cannot invalidate a cursor mid-test.
5. The first Gradle invocation used JVM 8 because `JAVA_HOME` was not set and the
   environment script was blocked by execution policy. The pinned JDK 21 was set
   directly for the gate, and the wrapper downloaded Gradle 9.9.2 into the
   repository cache.
6. The Stage 01 and Stage 02 documentation validators failed on a table in
   `docs/progress/STAGE_09.md`: the `network.*` event unions contained literal
   pipes. The cells now escape them, and both validators split on unescaped pipes
   instead of unescaping before splitting.
7. The dependency bump that moved the wrapper to Gradle 9.9.2 and AGP to 9.5.2
   left the pin tables in the ADR, the Stage 02 and Stage 03 records, the
   dependency inventory, and the Stage 02 validator on 9.9.1/9.5.1. They now all
   state the committed pins; the optional local Gradle install from
   `setup-windows.ps1` remains 9.9.1 and is documented as non-authoritative.

## 16. Decisions and unresolved questions

- Search remains local-first. Provider fan-out is an internal detail that never
  blocks the local page.
- Provider results are shown only after in-memory signature verification and are
  not written to local storage by search.
- Fan-out runs only for the cursorless first page. A fresh provider view requires
  a new search.
- A generation change rejects outstanding cursors rather than returning a
  possibly inconsistent page.
- `search_provider` is opt-in. The role is announced best-effort and retried
  until the DHT has a peer; it is not a hard startup requirement.
- `cached` stays false for provider-only rows because search does not cache
  them.
- Conflicts from same-revision records remain visible. There is no automatic
  conflict-resolution UI yet.
- The Kotlin Explore and Search screens remain demo data with honest labels;
  core wiring is later UI work.
- Android runtime search and index-provider evidence remains outstanding.

## 17. Next stage

Stage 17 adds the full UI, accessibility, settings, foreground service, and
Windows tray. It requires explicit project-owner approval. Stage 16 remains on
`feat/16-search`, pushed to `origin`, until review and merge; the merge base is
`main` at `05385de`.
