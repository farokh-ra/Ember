# Stage 14 - Publishing and File Pages

**Status:** Complete
**Completed:** 2026-11-22
**Progress guide:** 65-70%
**Next stage:** 15 - voluntary replication and availability (requires user permission)

## 1. Scope

Stage 14 turns the Stage 07 publication format and the Stage 12 transfer stack
into an explicit publishing workflow. A user imports a file, chooses metadata,
and publishes a signed record. Other peers discover the exact record through the
DHT, verify it, see the uploader, and download the content from the publisher
while it is online. Unpublishing signs a withdrawal and stops local serving, but
never promises remote deletion.

The stage covers:

- Explicit publishing of content that already exists in the managed store.
- Description, category, tags, display filename, size, and content ID in the
  signed publication record.
- DHT announcement of the exact signed publication and a provider record for the
  content ID.
- A serving gate that shares verified bytes only while a local publication is
  serving, an active replica exists, or a completed download is retained.
- `publication.get` and `publication.list_local` for local and cached records.
- `publication.lookup` with signature, publication ID, and DHT key verification,
  signed-tombstone lookup, caching, provider discovery, and a separate
  availability result.
- Unpublish with explicit confirmation, owner checks, a signed tombstone, a
  provider withdrawal, and retained local bytes.
- Metadata-only preview classification for plain text and raster images, with no
  execution and no automatic opening.
- A demo publish flow and file page that keep signature, serving, and
  availability visibly separate.

## 2. Architecture

| Component | Behaviour |
|---|---|
| `nexus-protocol::publication` | Existing canonical v1 publication body with category, tags, size, manifest root, revision, and parents; adds `Category::as_str` and `Category::parse`. |
| `publication_dht_key` / `tombstone_dht_key` | Derived DHT keys for the exact signed record and withdrawal. |
| `PublicationTransport` | Core-side publish, lookup, start/stop providing, and provider lookup abstraction; production forwards to `Network`, tests inject records and failures. |
| Operation pool | Publish, unpublish, and lookup run as cancellable jobs on the existing four-worker pool. |
| `Store` | Durable publication metadata, exact signed bytes, tags, local/serving/announced state, availability, tombstones, and the serving gate. |
| `CoreFileProvider` | Consults `Store::content_serveable` before answering a metadata or chunk request. |
| `Network::stop_providing` | New driver command that withdraws this node as a content provider. |

The core protocol version is `0.1.0-stage-14`; the ABI remains 1. The identify
protocol remains `/nexus/0.1.0-stage-12` because Stage 14 adds no new network
protocol or frame type.

## 3. Signed record and metadata

Publishing does not move, copy, or re-encrypt file bytes. The streaming import
from Stage 12 first stores and verifies the content; `publication.create` then
binds that content ID to a signed record.

| Item | Value |
|---|---|
| Publication schema | 1 |
| Category | `documents`, `images`, `audio`, `video`, `software`, `data`, `archives`, or `other` |
| Tags | 10, each 1-32 UTF-8 bytes, order-preserving and deduplicated |
| Description | 4,096 UTF-8 bytes |
| Display filename | Metadata only, at most 255 UTF-8 bytes; never a path |
| Signed publication envelope | 64 KiB |
| Publication revision | 1 for Stage 14 |
| DHT record value cap | 64 KiB |

The identity vault signs the canonical publication body with the publication
domain tag. The private key never enters CBOR, the store, an event, or Kotlin.
The exact signed envelope is verified and stored before any network work starts.

The publication ID is the SHA-256 of the signed record. The local display
tombstone ID is `"{publication_id}:{revision}"`.

## 4. Lifecycle

1. The host imports bytes with `nexus_core_import_begin/write/finish`. The
   import returns a verified `content_id` and retains a `local_import`
   reference.
2. `publication.create` validates the content ID, display filename, description,
   category, and tags, then requires a verified content row and readable bytes.
3. The core ensures a canonical manifest exists for the content, builds a
   revision-1 publication body, signs it through the identity vault, and stores
   the exact signed bytes with `local = 1`, `serving = 1`, and
   `availability = local`.
4. The core adopts the pending `local_import` reference into a
   `publication_original` reference so cleanup cannot remove the published file.
5. A worker announces the signed record under its derived DHT key and starts
   providing the content ID. Success marks the row `announced = 1`.
6. Networking disabled or unavailable does not roll back the publication. The
   record stays local and serving, and `publication.published` reports
   `announced = false` with a typed error. A later publish path can announce it
   again.
7. `publication.create` returns immediately with `accepted`, `publication_id`,
   `content_id`, `revision`, and `op_id`; the terminal event reports the
   announcement outcome.

Publication creation stays at revision 1 in Stage 14. Editing a publication and
revision chains are later work; the protocol already carries a revision and
parent fields.

## 5. Serving gate

A file is served only when all of these are true:

- The content row exists and is `verified`.
- At least one local reason to keep and share it exists:
  - a local, non-tombstoned publication with `serving = 1`, or
  - an active replica, or
  - a retained completed download.

`CoreFileProvider` checks this gate for both metadata and chunk requests. A
cached remote publication never makes this device a source. A tombstoned
publication stops serving immediately, even while its local bytes and signed
record remain.

The gate is evaluated from stored state on each request, so unpublishing does not
depend on a cached in-memory flag or on a restart.

## 6. Lookup, availability, and tombstones

`publication.lookup` runs on a worker and always ends in one terminal event.

- The returned record must parse, verify against the publication domain tag,
  match the requested publication ID, and match the DHT key derived from that
  ID.
- A local row is returned as `source = "local"` without being replaced by a
  remote record.
- A verified remote record is cached with its exact bytes and metadata.
- A different record already cached under the same publication ID is a
  `conflict`: the accepted row is retained and the event reports
  `conflict = true`.
- The core also looks up the signed tombstone key. A valid tombstone is cached
  and forces `tombstoned = true` and `availability = record_only`. A malformed or
  mismatched tombstone fails the lookup with `verification_failed` instead of
  being cached.
- Provider lookup returns bounded peer IDs and addresses for the content ID.
  Providers are advisory hints, not proof of availability.

Signature status, serving, and availability are independent:

| Field | Meaning |
|---|---|
| `signature_verified` | The exact signed record verifies and binds the requested publication ID. |
| `serving` | This device is willing to serve verified bytes now. |
| `availability` | The last observed ability to reach the content. |

Availability values are `local`, `available_now`, `seen_recently`,
`no_known_source`, and `record_only`. A successful provider lookup yields
`available_now`; a recent earlier success without a current provider yields
`seen_recently`; a miss with no source yields `no_known_source`; a tombstone or
missing record yields `record_only`. A local serving publication reports
`local`. The store updates availability on each completed lookup.

## 7. Unpublish limits

`publication.unpublish` requires `confirmation = true` and a local publication
owned by the current identity.

1. The core parses the stored signed publication and builds a canonical
   tombstone bound to its identity, publication ID, content ID, and revision.
2. The identity vault signs the tombstone; the core verifies it against the
   publication before storing it.
3. The publication row becomes `tombstoned = 1`, `serving = 0`,
   `announced = 0`, and `availability = record_only`.
4. A worker announces the exact tombstone under the tombstone DHT key.
5. The provider record is withdrawn unless another local serving reason remains
   for the same content (another serving publication, an active replica, or a
   retained download).
6. Local bytes and the local original reference are retained.

Every result repeats the limits: unpublishing stops this device from serving and
announces a signed withdrawal; copies held by other peers are not deleted; local
bytes are kept. The UI must never call this a global delete.

## 8. Preview contract

`publication.get`, `publication.list_local`, and lookup results include a preview
object:

| Field | Meaning |
|---|---|
| `kind` | `text`, `image`, or `none` |
| `supported` | Whether a safe preview is offered |
| `auto_execute` | Always `false` |
| `reason` | `plain_text`, `raster_image`, or `unsupported_format` |

Only plain-text extensions (for example `txt`, `md`, `csv`, `json`, `rs`, `kt`)
and raster-image extensions (`png`, `jpg`, `gif`, `webp`, `bmp`, `tiff`, `avif`,
`ico`) are eligible. SVG, HTML, PDF, archives, binaries, and executable script
extensions such as `sh`, `ps1`, and `bat` are never previewed. Unsupported
formats may be opened only after an explicit download and verification.

The preview is metadata classification only. The core does not decode, render,
or execute the file, and no UI path opens a downloaded file automatically.

## 9. Core integration

Stage 14 implements the five methods reserved in the ABI 1 catalog:

| ID | Method | Mode | Request | Result |
|---|---|---|---|---|
| 0x0501 | `publication.create` | A | `content_id`, `display_filename` (or `filename`), `description?`, `category?`, `tags?`, `parent_content_id?`, `parent_publication_id?` | `publication.published` with publication ID, announcement and provider outcome, serving, availability, and preview. |
| 0x0502 | `publication.unpublish` | A | `publication_id`, `confirmation` | `publication.unpublished` with tombstone ID, withdrawal outcome, limits, and retained local bytes. |
| 0x0503 | `publication.get` | S | `publication_id` | Local or cached record, tombstone, signature, serving, availability, and preview. |
| 0x0504 | `publication.list_local` | S | `cursor?`, `limit?` | Local publications with metadata, revision, serving, announced, availability, and preview. |
| 0x0505 | `publication.lookup` | A | `publication_id` | `publication.lookup_completed` with the verified record, source, providers, availability, conflict state, and preview. |

New events: `publication.published`, `publication.publish_failed`,
`publication.unpublished`, `publication.publish_cancelled`,
`publication.unpublish_cancelled`, `publication.lookup_completed`,
`publication.lookup_failed`, and `publication.lookup_cancelled`.

Schema version 7 rebuilds `replicas` first to drop its foreign key to
`publications`, then rebuilds `publications` without the local identity and
content foreign keys so a verified remote cache can be stored. It adds
`category`, `size_bytes`, `local`, `serving`, `announced`, `availability`, and
`last_checked_at`, plus a `publication_tags` table. Existing rows are backfilled
as local; serving or published rows become `serving = 1` with
`availability = 'local'`, while tombstoned rows become `serving = 0` with
`availability = 'record_only'`. Category defaults to `other`, size comes from
`contents`, and `announced` starts at 0.

## 10. Protections and bounds

- Publishing requires a verified local content row and readable bytes.
- Every metadata field is length-checked; tags are deduplicated in order.
- The vault signs the publication and tombstone; private keys never leave it.
- Every lookup re-verifies the envelope, requested ID, and derived DHT key.
- A cached record conflict is reported, not silently overwritten.
- A tombstone must verify against the exact publication before it is cached.
- Only the owning identity may unpublish, and confirmation is mandatory.
- Serving is gated by verified content plus a local serving reason.
- Unpublishing never deletes local bytes or claims to erase remote copies.
- Preview classification is metadata only and never executes content.
- Cancellation is cooperative and emits exactly one terminal event.
- Terminal events are durable; failures carry typed errors.

## 11. Interface and UI

- The demo Publish screen now describes the real flow: import and hash, sign,
  announce, serve while online, and unpublish. It shows sample content ID,
  signature, serving, and availability states with a visible demo label.
- The demo File Details screen shows an availability badge, a separate serving
  badge, signature/serving/availability rows, a safe-preview card, and the
  unpublish limits. Unpublished fixtures keep the record-only state.
- `Coastal Field Notes` is a `.txt` fixture so the demo can show a supported
  plain-text preview. Other fixtures exercise unsupported formats.
- Every value remains demo data from `DemoCatalog`; no Kotlin screen is wired to
  the core yet. The screens remain a prototype for a later UI stage.

## 12. Verification evidence

### 12.1 `nexus-core` publication unit tests

9 new tests, 0 failures:

| Test | Covers |
|---|---|
| `previews_only_inert_text_and_raster_images` | Plain-text and raster-image preview metadata with `auto_execute = false`. |
| `preview_rejects_markup_executables_scripts_and_unknown_formats` | SVG, HTML, PDF, archives, and executable script extensions are not previewed. |
| `create_defaults_category_and_deduplicates_tags_in_order` | Default category, tag order, and deduplication. |
| `create_rejects_unknown_categories_bad_ids_and_bad_tags` | Invalid category, content ID, empty tag, oversized tag, and excess tags. |
| `publish_worker_announces_record_and_provider` | Signed record announcement, provider start, serving state, and terminal event. |
| `publish_worker_defers_without_a_transport_but_keeps_serving` | Offline publish keeps the record and serving state with a typed error. |
| `unpublish_worker_announces_tombstone_and_withdraws_provider` | Tombstone announcement, provider withdrawal, limits, and retained bytes. |
| `lookup_caches_a_verified_record_and_signed_tombstone` | Verified caching, tombstone verification, provider list, and record-only availability. |
| `lookup_rejects_a_record_for_a_different_publication_id` | Publication ID and DHT key binding. |

### 12.2 `nexus-store` Stage 14 tests

5 new tests, 0 failures:

| Test | Covers |
|---|---|
| `publications_round_trip_tags_and_filter_local_rows` | Metadata and tag persistence, exact-byte idempotence, and local filtering. |
| `remote_publication_cache_is_exact_and_conflict_safe` | Remote caching, newer replacement, and conflict retention. |
| `serving_gate_requires_verified_bytes_and_a_retained_reason` | Serving gate for publications, replicas, retained downloads, and tombstones. |
| `unpublish_marks_record_only_and_retains_local_bytes` | Tombstone state, availability, and retained content. |
| `ensure_manifest_builds_and_persists_a_verified_manifest` | Manifest creation and persistence for an imported file. |

The migration suite adds `upgrades_version_six_publications_with_explicit_serving_state`,
which verifies the v7 rebuild, backfill, tags table, and removed foreign keys.

### 12.3 `nexus-core` Stage 14 test

1 end-to-end test, 0 failures:

| Test | Covers |
|---|---|
| `publisher_announces_serves_and_withdraws_a_signed_publication` | Two real engines import, publish, discover, verify, list, download, and unpublish through loopback DHT servers. The test asserts signature, serving, and availability separately; checks metadata and tags; confirms the publisher appears as a provider; verifies the downloaded bytes; rejects unpublish without confirmation and by a non-owner; observes the signed tombstone on both sides; and confirms local bytes survive. |

### 12.4 Workspace totals

`cargo test --workspace -- --test-threads=1` passed with 281 tests and 0
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
| `nexus-store` Stage 14 tests | 5 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-store` storage tests | 12 |
| `nexus-transfer` unit tests | 7 |
| `nexus-transfer` integration tests | 7 |
| `nexus-types` unit tests | 1 |
| **Total** | **281** |

Stage 13 had 265 tests; Stage 14 adds 16: 9 core publication tests, 5 store
Stage 14 tests, 1 migration test, and 1 core Stage 14 test.

### 12.5 Kotlin checks

- `:shared:presentation:jvmTest` and `:shared:ui:jvmTest` pass.
- `DemoCatalogTest` adds two cases: serving is claimed only for owned active
  publications, and previews never auto-execute and match the supported-format
  contract.
- The full gate passes:
  `.\gradlew.bat --no-daemon --console=plain spotlessCheck detekt
  :shared:presentation:jvmTest :shared:ui:jvmTest
  :apps:desktopApp:createDistributable :apps:androidApp:assembleDebug`.
- Gradle uses `GRADLE_USER_HOME=NeededToolsHere/gradle-home`, so the wrapper,
  dependency cache, and WiX packaging download stay inside the repository.

### 12.6 Formatting, linting, and dependencies

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Stage 14 adds no direct dependency; `Cargo.lock` remains at 411 packages.
- The first CI run failed only `cargo deny check` with RUSTSEC-2026-0592, a stack
  overflow in `Url::origin()` on nested `blob:` URLs in the transitive `url`
  crate 2.5.8. The lockfile was updated to `url` 2.5.9 and
  `form_urlencoded` 1.2.3; no package was added or removed.
- The FFI interface document is at version 1.7.0 with thirty-three callable
  methods.

## 13. Exit criteria

| Criterion | Evidence |
|---|---|
| Users can publish a file from Android and Windows. | The same core methods and streaming import are built for both platforms. The Windows-loopback Stage 14 test publishes through `publication.create`; Android runtime evidence is deferred. |
| Other peers can discover the publication record. | The reader engine looks up the record through the DHT, verifies its signature and derived key, caches it, and reads its metadata. |
| The source device can serve the file while online. | The reader downloads and re-exports the exact bytes while the publisher is running; the publisher is listed as a provider. |
| The UI never promises global deletion or permanent availability. | Signature, serving, and availability are separate fields; unpublish results repeat the limits; the demo screens state that remote copies may remain and serving stops when the app closes. |

## 14. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries cross-build with the same
  publication, serving, and preview code.
- The Stage 14 proofs run on Windows loopback through the same Rust code that
  Android will load.
- Android runtime publishing, file picker integration, mobile network switching,
  and background serving limits remain deferred.

This is not claimed as Android runtime evidence.

## 15. Failure evidence and fixes

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

## 16. Decisions and unresolved questions

- Stage 14 branches from `main` after Stage 13 merged in PR #12.
- `publication.create` accepts an existing verified content ID instead of file
  paths. File selection and streaming import remain host responsibilities.
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
- The Kotlin publishing and file pages remain labeled demo data; core wiring is
  a later UI stage.
- Android runtime publication evidence remains outstanding.

## 17. Next stage

Stage 15 adds voluntary replication and availability: an explicit host action,
replica verification before advertising, pause/remove controls, storage and
bandwidth limits, and Android metered-network and battery controls. It must not
begin until Stage 14 is reviewed and approved by the project owner.
