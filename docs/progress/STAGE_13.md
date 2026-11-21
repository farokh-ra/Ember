# Stage 13 - Profiles and Signed Public Metadata

**Status:** Complete
**Completed:** 2026-11-21
**Progress guide:** 60-65%
**Next stage:** 14 - publishing and file pages (requires user permission)

## 1. Scope

Stage 13 implements signed public profiles on top of the Stage 10 DHT and the
Stage 08 identity vault. It adds local profile editing, a signed local draft,
DHT publication and lookup, revision handling, stale-cache labelling,
same-revision conflict retention, and transparent username-label collisions. It
adds no central account service, unique-handle registry, or profile authority.

The stage covers:

- A signed profile record with display name, optional username label, bio,
  optional avatar content ID, and bounded publication references.
- Local editing that signs each change and increments the revision.
- A draft state that stores signed bytes without announcing them.
- DHT publication under a key derived from the owner's identity ID.
- Lookup that verifies the signature, identity ID, and derived key before
  caching.
- Newer-revision replacement, older-revision stale reporting, and
  same-revision conflict retention.
- Case-insensitive username-label collision reporting with distinct identity
  IDs.
- A demo profile card that shows signed, state, freshness, revision, identity,
  conflict, and collision labels.

## 2. Architecture

| Component | Behaviour |
|---|---|
| `nexus-protocol::profile` | Canonical v1 profile body, exact signed envelope, text and reference bounds. |
| `SignedRecord::from_parts` | Builds an envelope from an externally produced signature so the vault can sign the profile body without exporting the key. |
| `profile_dht_key` | `sha256("NEXUS-DHT-PROFILE-v1" + identity_id)`; the record is stored and served under the same signed-record rules as other DHT values. |
| `ProfileTransport` | Core-side publish/lookup abstraction; production forwards to `Network`, tests inject records and failures. |
| Operation pool | Publish and lookup run as cancellable jobs on the existing four-worker operation pool. |
| Store | Durable profile row with state, revision, signed bytes, freshness, timestamps, and retained conflicts. |

The core protocol version is `0.1.0-stage-13`; the ABI remains 1. The identify
protocol remains `/nexus/0.1.0-stage-12` because Stage 13 adds no new network
protocol or frame type.

## 3. Signed record and bounds

The profile body is canonical CBOR signed with the profile domain tag. The
identity vault signs the body; private keys never enter CBOR, the store, an
event, or Kotlin.

| Item | Value |
|---|---|
| Profile schema | 1 |
| Maximum display name | 64 Unicode scalar values, at least 1 |
| Username label | 3-32 Unicode scalar values, optional |
| Bio | 500 Unicode scalar values |
| Publication references | 64 |
| Signed profile envelope | 64 KiB |
| DHT record value cap | 64 KiB |

A revision starts at 1. Every successful field change signs a new record and
increments the revision by one. Republishing the existing signed bytes without
new fields keeps the revision unchanged.

## 4. Lifecycle

1. `profile.publish` validates the supplied fields and the optional
   `expected_revision`.
2. The first publish requires `display_name`. Later calls merge supplied fields
   with the existing signed record; omitted fields keep their previous value.
3. The core signs a fresh profile body, verifies the resulting envelope, and
   stores the exact signed bytes before any network work begins.
4. With `publish: false`, the row is stored as `draft` and
   `profile.draft_saved` is emitted. No announcement is attempted.
5. With `publish: true`, the row is stored as `pending`, the DHT publish runs on
   a worker, and the row becomes `published` when the announcement succeeds.
6. A publish with networking disabled or unavailable keeps the row `pending`
   and emits `profile.published` with `discoverable: false`,
   `announcement: "deferred"`, and an error map. The signed record is retained
   and can be republished later.
7. Calling `profile.publish` with no profile fields republishes the existing
   signed record. If it was a draft, the same revision becomes `pending` and is
   announced; a published record is announced again without a new revision.

`expected_revision` provides optimistic concurrency. A mismatch returns a
`conflict` with `details.current_revision` and leaves the stored row unchanged.
Without it, local writes may advance but never roll back to a lower revision.

## 5. Lookup, cache, stale, and conflicts

`profile.lookup` runs on a worker and always ends in a terminal event.

- A returned record must parse, verify against the profile domain tag, match
  the requested identity ID, and match the DHT key derived from that identity.
- A newer revision replaces the cached remote row.
- An older revision is reported with `stale: true`; the accepted newer row is
  retained.
- A same-revision record with different bytes is retained in
  `profile_conflicts` without replacing the accepted row.
- A cache served because the network returned no record or failed is labeled
  `stale`, even when the previous check was recent.
- A cached remote row whose last successful check is older than seven days is
  also presented as `stale`.
- Local rows are always `current`.

Profile states are `draft`, `pending`, `published`, and `remote`. Freshness
values are `current`, `stale`, and `unknown`.

## 6. Username collisions

Usernames are labels, not unique reservations.

- Collision detection compares labels case-insensitively with Unicode-aware
  lowercasing.
- Every other known profile with the same label is returned with its distinct
  `identity_id`.
- `username_is_label` is always true so the UI never presents a label as a
  global handle.
- Two identities may publish the same label without either record becoming
  invalid.
- No central registry, reservation, or first-claim rule exists.

## 7. Core integration

Stage 13 implements three methods that were reserved in the ABI 1 catalog:

| ID | Method | Mode | Request | Result |
|---|---|---|---|---|
| 0x0301 | `profile.get_local` | S | none | Local identity ID, profile or null, state, freshness, conflicts, and verification state. |
| 0x0302 | `profile.publish` | A | `display_name?`, `username?`, `bio?`, `avatar_content_id?`, `publication_refs?`, `publish?`, `expected_revision?` | `profile.draft_saved` or `profile.published` with revision and announcement outcome. |
| 0x0303 | `profile.lookup` | A | `identity_id` | `profile.lookup_completed` with a verified record, freshness, conflicts, and collisions. |

New events: `profile.draft_saved`, `profile.published`,
`profile.lookup_completed`, `profile.lookup_failed`,
`profile.publish_cancelled`, and `profile.lookup_cancelled`.

Schema version 6 rebuilds `profiles` with millisecond update time, lifecycle
state, publish and check timestamps, and freshness; it removes the old local
identity and avatar foreign keys so a verified remote cache can be stored. It
adds `profile_conflicts` with a uniqueness constraint on identity, revision, and
exact bytes, plus a case-insensitive username index.

## 8. Protections and bounds

- Every profile field is length-checked and normalized before signing.
- The vault signs the body; the private key never leaves the identity slot.
- Every lookup re-verifies the envelope and checks the requested identity ID
  and DHT key before caching.
- A cached remote profile never overwrites a local profile row.
- A same-revision conflict is retained, not silently accepted or discarded.
- Usernames are always labelled as non-unique.
- Cancellation is cooperative and emits exactly one terminal event.
- Terminal events are durable; publish and lookup failures carry typed errors.

## 9. Interface and UI

- The demo Profile card now shows the signed-record badge, lifecycle state,
  freshness, revision, identity ID, publish and check labels, retained conflict
  count, and case-insensitive username-label collisions.
- The card explains that a signature proves key control, not real-world
  identity, and that usernames are labels rather than unique accounts.
- Every value remains demo data from `DemoCatalog`; the existing demo notice
  remains visible. No Kotlin screen is wired to the core yet.
- The screen remains a prototype for a later UI stage, not evidence of live
  profile editing.

## 10. Verification evidence

### 10.1 `nexus-protocol` unit tests

1 new test, 0 failures:

| Test | Covers |
|---|---|
| `envelope_can_be_built_from_external_signing` | Building and verifying a signed envelope from a signature produced outside `SignedRecord`. |

### 10.2 `nexus-store` profile tests

5 new tests, 0 failures:

| Test | Covers |
|---|---|
| `local_profile_moves_from_draft_to_pending_to_published` | Local lifecycle transitions, timestamps, and freshness. |
| `local_writes_honor_expected_revision_and_never_roll_back` | Optimistic concurrency and revision monotonicity. |
| `remote_cache_replaces_newer_and_retains_conflicts` | Newer replacement, older retention, and same-revision conflict rows. |
| `remote_cache_never_overwrites_a_local_profile` | Local ownership of the identity row. |
| `username_collisions_are_case_insensitive_and_keep_distinct_identities` | Case-insensitive label queries with distinct IDs. |

### 10.3 `nexus-core` profile worker tests

7 new tests, 0 failures:

| Test | Covers |
|---|---|
| `publish_worker_marks_published_and_emits_a_verified_event` | Successful announcement and stored state. |
| `publish_worker_defers_without_a_transport` | Deferred publication with a typed error. |
| `lookup_caches_a_verified_record_and_reports_username_collisions` | Verified caching and collision reporting. |
| `lookup_keeps_a_newer_cache_when_the_network_returns_an_older_revision` | Older-revision stale reporting without rollback. |
| `lookup_labels_a_cache_stale_when_the_network_fails` | Cache fallback labelled stale. |
| `lookup_keeps_a_local_profile_current_when_the_network_fails` | A local row stays current and is not labelled stale. |
| `lookup_rejects_a_record_for_a_different_identity` | Identity and key binding. |

### 10.4 `nexus-core` Stage 13 test

1 end-to-end test, 0 failures:

| Test | Covers |
|---|---|
| `engines_publish_verify_supersede_and_label_stale_profiles` | Two real engines publish signed profiles through loopback DHT servers, verify and cache each other's records, reject a stale expected revision, republish a draft without a revision bump, report case-insensitive collisions with distinct IDs, observe revision 2 supersede revision 1, and serve a stale-labelled cache after both servers stop. |

### 10.5 Workspace totals

`cargo test --workspace -- --test-threads=1` passed with 265 tests and 0
failures:

| Suite | Tests |
|---|---:|
| `nexus-core` unit tests | 48 |
| `nexus-core` Stage 09 tests | 4 |
| `nexus-core` Stage 10 tests | 2 |
| `nexus-core` Stage 11 tests | 4 |
| `nexus-core` Stage 12 tests | 1 |
| `nexus-core` Stage 13 tests | 1 |
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
| `nexus-store` unit tests | 17 |
| `nexus-store` local-import tests | 4 |
| `nexus-store` profile tests | 5 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-store` storage tests | 12 |
| `nexus-transfer` unit tests | 7 |
| `nexus-transfer` integration tests | 7 |
| `nexus-types` unit tests | 1 |
| **Total** | **265** |

Stage 12 had 251 tests; Stage 13 adds 14: 7 core profile-worker tests, 5 store
profile tests, 1 protocol envelope test, and 1 core Stage 13 test.

### 10.6 Kotlin checks

- The full gate passes:
  `.\gradlew.bat --no-daemon --console=plain spotlessCheck detekt
  :shared:presentation:jvmTest :shared:ui:jvmTest
  :apps:desktopApp:createDistributable :apps:androidApp:assembleDebug`.
- The desktop distributable and the Android debug APK package the same Kotlin
  UI and Rust core. The demo catalogue and Profile card changes add no new
  Kotlin test cases; the existing presentation and UI tests still pass.
- Gradle uses `GRADLE_USER_HOME=NeededToolsHere/gradle-home`, so the wrapper,
  dependency cache, and WiX packaging download stay inside the repository.

### 10.7 Formatting, linting, and dependencies

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- No dependency was added; `Cargo.lock` remains at 411 packages.
- The FFI interface document is at version 1.6.0 with twenty-eight callable
  methods.
- GitHub Actions passed all seven checks for PR #12: Rust format/lint/test and
  policy, Rust Windows MSVC build and tests, Windows desktop build, Android
  build/tests/static analysis, repository hygiene, PR title, and dependency
  review.

## 11. Exit criteria

| Criterion | Evidence |
|---|---|
| A user can publish a profile and another peer can retrieve and verify it. | `engines_publish_verify_supersede_and_label_stale_profiles` publishes through two DHT servers and retrieves the exact signed record from a third engine. |
| Profile changes can be distinguished from stale records. | Revisions are stored and compared; an older lookup is reported `stale: true` and a cache after network failure is labelled stale. |
| Username collisions are handled transparently. | Case-insensitive collision queries and the Stage 13 test's `ADA`/`ada` records return distinct identity IDs. |
| Local editing and drafts work without publishing. | `profile.publish` with `publish: false` stores a signed `draft`, emits `profile.draft_saved`, and performs no announcement. |
| Conflicts do not silently replace accepted data. | `profile_conflicts` retains same-revision differing bytes and the accepted row is unchanged. |
| The UI never presents a username as a unique account. | `username_is_label` is always true and the demo card explains the collision rule. |

## 12. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries cross-build with the same profile
  code.
- The Stage 13 proofs run on Windows loopback through the same Rust code that
  Android will load.
- Android runtime profile publication, mobile network switching, and background
  limits remain deferred.

This is not claimed as Android runtime evidence.

## 13. Failure evidence and fixes

1. Migration and schema assertions still expected version 5 after migration 6
   was added. They now assert `LATEST_VERSION` and schema 6.
2. The Stage 13 integration test initially used two DHT clients and could not
   satisfy quorum because a client rejects stored records. It now uses two DHT
   servers plus one client reader, which also makes the stale-cache assertion
   deterministic.
3. A publish with no profile fields originally attempted to build a new record
   from empty fields. It now republishes the existing signed bytes without a
   revision bump.
4. Detekt counted seventeen functions in `Cards.kt`. The profile card and its
   helpers moved to `ProfileCards.kt`, leaving both files under the threshold.
5. Clippy rejected an eight-argument publish worker. The worker now takes the
   verified `SignedProfile` and derives its identity, revision, timestamp, and
   bytes.
6. Clippy also rejected redundant row closures and a collapsible username
   check; both were simplified.
7. The first Android package attempt reused stale intermediates that referenced
   an old checkout path. `gradlew clean` removed them, and the full Gradle gate
   then passed with the desktop distributable and Android debug APK.

## 14. Decisions and unresolved questions

- Stage 13 is a stacked branch from `feat/12-streaming-transfer` because Stage
  12 is not merged yet. The Stage 12 branch and PR are preserved.
- Profiles are exact signed records under an identity-derived DHT key. No
  profile registry or unique handle exists.
- Username labels are compared case-insensitively for transparency, but the
  stored spelling is preserved.
- `profile.publish` defaults to publishing. A draft requires `publish: false`.
- Omitting every profile field republishes the existing signed record; it does
  not create an empty revision.
- Same-revision conflicts are retained for review rather than resolved
  automatically. Conflict presentation and resolution remain later work.
- Cached remote profiles are always labelled stale after a network miss or
  failure, even when the previous check was recent.
- The Kotlin Profile UI remains labeled demo data; core wiring is a later UI
  stage.
- Android runtime profile evidence remains outstanding; GitHub Actions passed
  all seven checks for PR #12.

## 15. Next stage

Stage 14 adds explicit file selection and publishing: descriptions, categories,
tags, signed publication records, publisher availability, file details, and
unpublish behavior. It must not begin until Stage 13 is reviewed and approved by
the project owner.
