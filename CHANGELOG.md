# Changelog

All notable changes to Ember are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Repository foundation for Stage 03, including Git hygiene files, editor
  configuration, commit template, and contribution templates.
- Gradle Kotlin Multiplatform and Compose Multiplatform build with shared
  modules for model, core API, core client, presentation, and UI.
- Empty Android application shell with a debug APK build.
- Empty Windows desktop application shell with a Compose desktop application
  image build.
- Cargo workspace with twelve architecture crates and a tested native ABI
  version export.
- Redacted structured logging helpers in Kotlin and Rust.
- Spotless and ktlint formatting, detekt static analysis, Rustfmt, and Clippy.
- Shared JVM tests for redaction, the fake core contract, the core client
  boundary, and startup presentation state.
- CI workflow for repository hygiene, Conventional Commit titles, Rust checks,
  Android builds and tests, a native Windows MSVC Rust build and test job, and
  Windows desktop packaging with artifact upload.
- Dependency review workflow, Dependabot configuration, and release-note
  categories.
- Portable non-admin Windows setup, local build, and clean-checkout verification
  scripts.
- Repository hygiene and commit-message validators.
- Developer setup, development workflow, logging policy, contributing guide, and
  Stage 03 evidence record.
- Ember design system with light/dark color schemes, semantic status colors,
  typography, spacing, shape, size, and a custom Compose icon set.
- Reusable component library for navigation chrome, cards, status labels,
  buttons, selections, dialogs, state panels, avatars, brand art, and prototype
  controls.
- Responsive navigation shell with a side rail at 840 dp and above and a compact
  top bar plus bottom navigation below it.
- Eleven clickable screen prototypes: welcome, identity setup, Explore, Search,
  profile, file details, Publish, Downloads, hosted files, Settings, and network
  status.
- Immutable application state with per-section navigation stacks, demo
  onboarding, versioned persistence tokens, and a shared controller.
- Persisted dark, light, and system theme selection on Android and Windows.
- Loading, empty, offline, error, and retry presentations for every data-driven
  screen, plus a visibly simulated preview-state bar and network selector.
- Accessibility support: WCAG AA contrast tests, 48 dp targets, visible focus
  rings, roles and selection semantics, live regions, text-plus-icon status,
  Android back, Escape back, and reduced-motion handling on Android.
- 21 shared UI JVM tests and 44 presentation JVM tests, including palette
  contrast, navigation, state restoration, identity validation, demo search, and
  status-label mapping.
- Stage 04 evidence record.
- Embedded SQLite schema with two transactional migrations, checksummed history,
  `PRAGMA user_version`, and rollback on failure.
- Dedicated SQLite writer thread, serialized read pool, WAL, foreign keys, busy
  timeout, and bounded page cache.
- Metadata tables for identities, profiles, publications, tombstones, replicas,
  downloads, peers, settings, content references, and rebuildable FTS5 search.
- Locked content-addressed managed file store with generated paths, read-only
  import staging, SHA-256 verification, and atomic promotion.
- Quota accounting, a 1 GiB free-space reserve, a 4 GiB file limit, and a
  1-500 GiB product quota range.
- Reference-counted deletion with explicit confirmation for published originals
  and active replicas.
- Resumable download records with verified-prefix checkpoints and partial
  cleanup.
- Startup reconciliation for crash-window downloads, orphan blobs, corrupt
  bytes, missing blobs, and stale partials.
- Metadata export and restore using a versioned CBOR manifest and a consistent
  SQLite snapshot.
- Safe filename and export-path handling for traversal, Windows reserved names,
  control characters, and collisions.
- 16 `nexus-store` unit tests and 12 integration tests, plus Stage 05 evidence.
- Local Ed25519 identity generation with a stable Bech32m `nx1...` identity ID.
- Canonical CBOR encoder and decoder with shortest integers, sorted map keys,
  duplicate rejection, NFC user text, and bounded depth, items, and input.
- Exact-byte `{1: body, 2: sig}` signed-record envelopes with domain-separated
  signatures and byte-preserving verification.
- Signed profile v1 and publication v1 schemas, unknown minor-field preservation,
  unknown major-version rejection, and stable publication IDs.
- Encrypted `secrets/identity.enc` storage using a host-supplied LEK, HKDF-SHA256,
  XChaCha20-Poly1305, atomic restrictive writes, and redacted secret `Debug`.
- User-controlled encrypted backup and restore with a versioned Argon2id header,
  fixed v1 defaults, NFC passphrases, generic wrong-passphrase failure, and no
  central recovery path.
- 21 Stage 06 integration tests plus committed identity, signature, canonical
  CBOR, signed-record, at-rest, and backup vectors.
- Exact signed-record and identity format specification in
  `docs/architecture/SIGNED_RECORD_FORMATS.md`.
- Raw SHA-256 content IDs and fixed 1 MiB chunking for publishable files up to
  4 GiB.
- Canonical flat chunk manifests with per-chunk SHA-256 hashes, a 4,096-entry
  limit, a 256 KiB cap, and domain-separated manifest roots.
- Streaming file hashing, one-pass manifest construction, chunk verification,
  and whole-file verification with bounded memory in `nexus-transfer`.
- Publication revision rules, successor validation, and exact manifest binding.
- Signed tombstone v1 records with original-uploader attribution, skip-tolerant
  revisions, and equal-revision conflict detection.
- Schema version 3 with independent signed tombstones, immutable publication
  upserts, monotonic tombstone upserts, content-addressed manifest files, and
  manifest verification during startup reconciliation.
- 33 Stage 07 Rust tests plus committed 2.5 MB content, chunk, manifest, root,
  tombstone, and rejection vectors.
- Stage 07 evidence record.
- A 17-function C ABI with opaque handles, panic containment, length-delimited
  buffers, and versioned CBOR envelopes.
- Create-time configuration with an ABI check that runs before the database or
  data root is opened.
- Idempotent create/start/stop/destroy behavior, unknown-handle rejection, and
  100-cycle lifecycle evidence.
- `system.info`, `system.health`, `system.snapshot`, `store.usage`, `test.echo`,
  and `test.fault` dispatch with stable domain errors.
- A 1,024-entry coalescing event queue and four-worker asynchronous operations
  with progress, terminal, and cancellation events.
- Forced-panic containment at the C and JNI boundaries.
- Streaming import to `store/tmp` with incremental SHA-256 and chunk hashing,
  canonical manifests, no-copy adoption, and bounded export with EOF.
- Schema version 4 and a `local_import` reference type that retains verified
  staged imports until another owner takes the bytes.
- Android JNI symbols, `JNI_OnLoad` ABI rejection, static result encodings, and a
  Windows-hosted JNI proxy test.
- Windows JNA adapter with the native `NexusBuffer` layout.
- Shared Kotlin `NativeCoreClient`, CBOR codec, typed `NexusException`, event
  pump, cancel, and streaming APIs.
- A C harness compiled against the shipped header and DLL, plus six Rust C ABI
  tests and a shared Kotlin adapter contract.
- `scripts/build-native.ps1` for Windows and Android artifacts, export checks,
  and the C harness; the full build gate runs it before Gradle.
- 35 new Stage 08 Rust tests and six new JVM tests, for 166 Rust and 76 JVM
  tests total.
- Stage 08 evidence record.
- An encrypted device peer key at `network/peer_key.enc`, independent of the
  user signing identity and excluded from identity backup and restore.
- Pinned libp2p `0.57.1` transports: QUIC v1 preferred and TCP + Noise + Yamux
  fallback, with DNS resolution and identify/ping health checks.
- Authenticated direct connections, explicit multiaddress dialing, and
  wrong-peer-identity rejection before application protocols run.
- A bounded `/nexus/test/1` request/response exchange with a 4 KiB payload cap,
  an 8 KiB frame cap, and canonical CBOR.
- Network and peer state reporting, including separate listen, observed, and
  confirmed address classes and stable counters.
- Persisted bootstrap and manual peers in `network/bootstrap.list`, bounded to
  64 entries, with atomic add/remove.
- Exponential reconnect backoff from 1 s to 60 s, network-change reset, and a
  bounded synchronous driver API.
- Connection, stream, frame, request-rate, and idle-timeout limits.
- Eight `network.*` core methods and the `network.state`, `network.peer_event`,
  and `network.reconnect_*` / `network.connect_*` / `network.test_*` events.
- 35 new Stage 09 Rust tests, for 201 Rust tests total.
- Stage 09 evidence record.
- libp2p Kademlia `/nexus/kad/1.0.0` with client and server modes, a 24 h
  record TTL, 12 h republication, replication factor 8, and disjoint query
  paths.
- Exact signed-envelope DHT values for publications, profiles, and tombstones,
  with key-derived verification and monotonic revision rules.
- Raw content-ID provider records with bounded address lists and provider
  discovery.
- A bounded in-memory DHT store with `StoreInserts::FilterBoth` inbound
  validation and caps of 1,024 records, 20 providers per key, and 512 locally
  advertised provider keys.
- Bootstrap entry points merged from compile-time `EMBER_BOOTSTRAP_PEERS`,
  runtime configuration, and persisted `network/bootstrap.list`, deduplicated
  and bounded to 64 entries.
- `network.discover` (0x0609) with bounded peer routing, sampled targets, and
  `network.discovery_completed|failed|cancelled` terminal events.
- Eleven DHT status fields in `network.status` and a `dht_mode` create-time
  config key.
- DHT record and routing design in `docs/architecture/DHT_DESIGN.md` and
  bootstrap operator guidance in `docs/operations/BOOTSTRAP_NODES.md`.
- 13 new Stage 10 Rust tests, for 214 Rust tests total.
- Stage 10 evidence record.
- Circuit Relay v2 fallback, including relay-client reservations, bounded relay
  server circuits, relay lists merged from compiled, configured, and persisted
  sources, and optional community relay nodes.
- DCUtR hole punching and AutoNAT v1 reachability, with path reporting for
  direct and relayed connections.
- A direct-first fallback order with bounded direct and relay deadlines and a
  terminal `NoViablePath` result when every candidate fails.
- Four new core methods: `network.relay_list` (0x060A), `network.relay_add`
  (0x060B), `network.relay_remove` (0x060C), and `network.connectivity`
  (0x060D).
- NAT, relay, path, fallback, hole-punch, and relay-server fields in
  `network.status`, `network.peers`, `network.connect_*`, and
  `network.peer_event`, plus a connectivity troubleshooting hint.
- Relay cost visibility through negotiated circuit byte and duration limits and
  accepted/closed circuit counters.
- NAT traversal and relay design in `docs/architecture/NAT_TRAVERSAL.md` and
  relay operator guidance in `docs/operations/RELAY_NODES.md`.
- 15 new Stage 11 Rust tests, for 229 Rust tests total.
- Stage 11 evidence record.
- A bounded `/nexus/file/1` metadata and chunk-slice protocol with canonical
  CBOR, exact field sets, a 512 KiB message cap, and 64 KiB slices.
- A storage-backed `FileProvider` that serves only verified content whose
  manifest and signed publication agree.
- Direct peer-to-peer downloads with per-chunk SHA-256 verification, ordered
  source failover, and bounded retries.
- Pause, resume, cancellation with optional partial retention, and process-death
  recovery from durable chunk checkpoints.
- Quota and free-space checks at registration and after every verified chunk.
- Safe display names and generated partial paths that cannot traverse or
  overwrite the managed store.
- Six new core methods: `download.start` (0x0801), `download.list` (0x0802),
  `download.pause` (0x0803), `download.resume` (0x0804),
  `download.cancel` (0x0805), and `download.remove` (0x0806).
- Eight transfer events: `download.started`, `download.progress`,
  `download.resumed`, `download.paused`, `download.completed`,
  `download.failed`, `download.cancelled`, and `download.removed`.
- Schema version 5 with `manifest_root` and `source_peer` on downloads.
- Transfer speed and serving-peer fields in the demo Downloads screen.
- Direct-transfer design in `docs/architecture/FILE_TRANSFER.md`.
- 22 new Stage 12 Rust tests, for 251 Rust tests total.
- Stage 12 evidence record.
- Signed public profiles with local editing and drafts, identity-derived DHT
  publication and lookup, revision checks, stale-cache labelling, same-revision
  conflict retention, and case-insensitive username-label collision reporting.
- Three new core methods: `profile.get_local` (0x0301), `profile.publish`
  (0x0302), and `profile.lookup` (0x0303).
- Six profile events: `profile.draft_saved`, `profile.published`,
  `profile.lookup_completed`, `profile.lookup_failed`,
  `profile.publish_cancelled`, and `profile.lookup_cancelled`.
- Schema version 6 with profile lifecycle, freshness, timestamps, and retained
  profile conflicts.
- Signed-state, freshness, revision, identity, conflict, and collision labels in
  the demo Profile screen.
- 14 new Stage 13 Rust tests, for 265 Rust tests total.
- Stage 13 evidence record.
- Explicit signed file publishing with descriptions, categories, tags, content
  IDs, manifest roots, and size metadata.
- DHT publication and provider announcement, lookup with signature, publication
  ID, and derived-key verification, signed tombstone lookup, provider hints, and
  availability reported separately from signature and serving.
- A serving gate that shares verified bytes only for a local non-tombstoned
  serving publication, an active replica, or a retained completed download.
- Unpublish with explicit confirmation and ownership checks, a signed tombstone,
  provider withdrawal, retained local bytes, and repeated limits that copies
  held by other peers are not deleted.
- A metadata-only preview contract for plain text and raster images with
  `auto_execute` always false; SVG, HTML, PDF, archives, binaries, and executable
  script extensions are never previewed.
- Five new core methods: `publication.create` (0x0501),
  `publication.unpublish` (0x0502), `publication.get` (0x0503),
  `publication.list_local` (0x0504), and `publication.lookup` (0x0505).
- Eight publication events: `publication.published`,
  `publication.publish_failed`, `publication.unpublished`,
  `publication.publish_cancelled`, `publication.unpublish_cancelled`,
  `publication.lookup_completed`, `publication.lookup_failed`, and
  `publication.lookup_cancelled`.
- Schema version 7 with publication metadata, local/serving/announced state,
  availability and last-check timestamps, a `publication_tags` table, and a
  rebuild that allows verified remote records without local foreign keys.
- Separate signature, serving, and availability states, safe-preview cards, and
  unpublish limits in the demo Publish and File Details screens.
- 16 new Stage 14 Rust tests, for 281 Rust tests total.
- Stage 14 evidence record.
- Explicit voluntary file hosting with mandatory per-file confirmation, no
  automatic download or seeding, and a deterministic `replica-<content_id>` row
  that reactivates on re-add.
- Full verification of the content ID, canonical manifest, and signed
  publication binding before any provider advertisement.
- Replica lifecycle states `active`, `paused`, `verification_failed`, and
  `removed`, with pause, resume, remove, and manual verify operations.
- Startup re-verification and an hourly maintenance pass that re-checks
  replicas whose last successful verification is older than seven days.
- Storage quota control through `store.set_quota` across the 1-500 GiB product
  range.
- Upload bandwidth, metered-network, and battery-threshold policy through
  `hosting.limits_get` and `hosting.limits_set`, enforced for all inbound file
  serving.
- A serving gate that combines verified content, a local serving reason, and the
  current hosting policy.
- Last-seen, last-verified, last-checked, failure-count, redacted-error, and
  announcement tracking per replica.
- A demo Hosted Files screen showing publication, size, verification, last
  verification time, serving state, storage use, and pause, resume, remove, and
  restore actions.
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
- 5 new Stage 15 Rust tests, for 286 Rust tests total.
- Stage 15 evidence record.
- A verified local FTS5 search index over published and remote profiles and
  non-tombstoned publications, maintained incrementally and rebuildable from
  stored signed records.
- Schema version 9 with `search_meta` generation state and a `search_docs`
  rowid map that keeps incremental index replacement logarithmic.
- Bounded search input and filters: 128 UTF-8 bytes, 12 tokens, kind, category,
  up to 5 exact tags, and relevance, recency, or availability ordering.
- Synchronous local `search.query` pages with generation- and query-bound
  opaque cursors, a 20-row page cap, total matches, and candidate-limit
  metadata.
- Local Explore modes for category counts, recent records, and a seeded bounded
  random sample, with no popularity or engagement signal.
- Optional signed index providers over `/nexus/index/1`, with 1 MiB message
  bounds, at most 8 providers, one 10-second total deadline, and 120 requests per
  peer per minute.
- Provider records are verified in memory before display. Unsigned, tampered,
  wrong-kind, oversized, and malformed records are rejected without affecting
  local or other-provider results, and search never writes them to storage.
- `search.result_page` streaming and one terminal `search.completed` event with
  per-provider state, coverage notes, conflict counts, cancellation, new-result
  count, and elapsed time.
- `search.cancel`, the opt-in `search_provider` config flag, and best-effort DHT
  announcement of the optional provider role.
- Kotlin Explore and Search demo states for coverage, partial results, offline
  and cached labels, and pagination.
- 44 new Stage 16 Rust tests, for 330 Rust tests total.
- Stage 16 evidence record.
- Multi-source downloads: up to four connected peers serve one file
  concurrently, each holding at most one chunk lease, so chunks can arrive out
  of order from different sources.
- A pure multi-source scheduler in `nexus-transfer` that ranks peers only by
  observed transfer outcomes: authenticated chunks, transport failures, corrupt
  responses, and measured chunk latency. A peer is trusted only after it serves
  a chunk whose hash matches the signed manifest.
- Per-chunk persistence in schema version 10: a `chunk_map` bitmap and
  `verified_bytes` count sparse verified chunks, while `verified_prefix` remains
  the contiguous prefix for existing callers.
- Reconciliation aligns a short partial's failed state down to the last complete
  chunk, so a truncated file stays resumable instead of keeping a mid-chunk
  prefix.
- Resume now re-hashes every recorded chunk from the partial file, clears
  corrupted or truncated chunks, truncates only the unverified tail, and
  continues from missing chunks across a different peer.
- Corrupt responses are retried from another source; two corrupt responses or
  three consecutive transport failures quarantine a peer for that download. A
  malicious source cannot silently corrupt the file.
- `download.started`, `download.progress`, and `download.resumed` now report
  chunk counters and per-source attempts, successes, failures, corruptions,
  bytes, latency, provenance, and quarantine state.
- 23 new Stage 17 Rust tests, for 353 Rust tests total.
- Stage 17 evidence record.
- Local peer blocking with reasons, bounded notes, timestamps, and a 4096-row
  default cap, enforced inside one writer transaction and persisted in schema
  version 11.
- Local abuse reports with categories, bounded notes, newest-first paging, and a
  separate 4096-row default cap. Reports and blocks are never uploaded,
  announced, or written into signed records.
- Network enforcement for blocked peers: outbound dials, inbound test, file, and
  index requests, inbound connections, discovery results, and providers are all
  refused or filtered.
- Blocked-source filtering in `download.start` and `download.resume`, with
  `permission_denied` when every source is blocked.
- `security.block_list`, `security.block_add`, `security.block_remove`,
  `security.report_add`, `security.report_list`, and `security.notices` under FFI
  namespace `0x0C`.
- Advisory safe-file classification for documents, media, archives, executables,
  scripts, and unknown extensions, with visible warnings and delegated previews.
- `download.started`, `download.resumed`, `download.completed`, and
  `download.list` now include a `safety` map with `auto_execute: false` and
  `preview: "delegated"`.
- Standing safety notices for no automatic execution, delegated previews,
  key-loss risk, public visibility, and local-only blocking.
- Truncation and single-byte mutation tests for file protocol messages, plus
  malformed and oversized request tests in the core.
- Threat model, privacy review, and security release checklist.
- 19 new Stage 18 Rust tests, for 372 Rust tests total.
- Stage 18 evidence record.
- Per-user Windows MSI package, portable Windows ZIP, signed Android release APK
  and AAB, SHA-256 checksums, and a machine-readable release manifest.
- `emberVersion` and `emberVersionCode` Gradle properties for release overrides,
  with a `0.1.0` default, a stable Windows upgrade UUID, and an Android
  `versionCode` derived from the version unless overridden.
- Release scripts for the pinned WiX download, packaging, verification, and
  performance measurement.
- A tag and manual-dispatch release workflow that restores signing material from
  repository secrets, builds, signs, verifies, uploads, and publishes.
- Release, performance, compatibility, signing, reproducibility, update/rollback,
  and accessibility/localization documents.
- A JVM adapter check that loads the native core from the packaged Compose
  resources directory.
- Stage 19 evidence record.
- Public protocol v1 specification covering identity, canonical CBOR, signed
  records, DHT keys, file and index wire messages, limits, errors, and security
  properties.
- Protocol versioning and deprecation policy with change classes, support
  windows, migration rules, and a review checklist.
- Interoperability and conformance documentation plus an eight-test public
  conformance suite for the v1 identity, record, manifest, and rejection
  vectors. Rust tests are now 380.
- Community infrastructure directory policy for independent bootstrap, relay,
  and index operators, explicitly marked as unpublished.
- Privacy-preserving network-health design with local-only diagnostics and a
  not-yet-implemented opt-in aggregate tier.
- Governance, moderation, and security disclosure policy drafts, including
  decision classes, local-first abuse handling, severity targets, embargo, and
  advisory format.
- ADR-0011 records the public-protocol, self-conformance, and launch-gate
  decision as proposed.
- Public known-limitations and post-launch maintenance documents with an
  explicit launch gate.
- Stage 20 evidence record.

### Changed

- The product and user-facing application surfaces are named Ember. Internal
  `io.nexus` packages, `nexus-*` crates, native symbols, storage names, and
  signed record tags are unchanged for compatibility.
- The Android and Windows shells now host the shared clickable UI prototype
  instead of empty placeholder content.
- The local gate and CI now run `:shared:ui:jvmTest`.
- Compose functions are exempt from detekt complexity and naming rules, and
  palette, icon, and brand-art files use a narrow magic-number suppression.
- ktlint ignores function naming for `@Composable` functions.
- Toolchain pins are recorded for JDK 21.0.13+9, Gradle 9.9.1, Kotlin 2.4.21,
  Compose Multiplatform 1.12.2, Material3 1.9.0, AGP 9.5.1, Rust 1.99.1, and
  Android NDK 29.0.14206865.
- Rust now owns durable local storage and the native lifecycle; Kotlin reaches it
  only through the versioned C ABI.
- Rust now owns local identity keys, canonical signed records, encrypted
  identity storage, and encrypted backups; no private key enters Kotlin.
- The committed `Cargo.lock` now resolves 131 packages, and the dependency and
  license inventory is at version 1.5.0.
- The SQLite schema is now version 4; tombstones are independent signed records,
  publications are immutable, and verified staged imports can be retained.
- The full local gate now builds Windows and Android native artifacts and runs
  the C harness before Gradle.
- `network_enabled` now starts the transport stack. A networking failure never
  fails core start; the core stays usable offline and reports the failure through
  `network.status`.
- The protocol version is now `0.1.0-stage-11`; the ABI remains 1.
- The committed `Cargo.lock` now resolves 411 packages, and the dependency and
  license inventory is at version 1.8.0. One unmaintained, non-vulnerable
  Linux-only `paste` advisory is documented in `deny.toml`.
- The FFI interface document is at version 1.4.0 with nineteen callable
  methods.
- The identify protocol version is now `/nexus/0.1.0-stage-11`; the Kademlia
  protocol is `/nexus/kad/1.0.0`.
- `libp2p` now enables the `autonat`, `dcutr`, and `relay` features. The
  optional `libp2p-upnp` crate remains in the graph but is not enabled.
- The SQLite schema is now version 5; downloads remember their trusted manifest
  root and source peer for restart recovery.
- The protocol version is now `0.1.0-stage-12`; the ABI remains 1.
- The FFI interface document is at version 1.5.0 with twenty-five callable
  methods.
- The identify protocol version is now `/nexus/0.1.0-stage-12`; the Kademlia
  protocol is `/nexus/kad/1.0.0` and the file protocol is `/nexus/file/1`.
- The SQLite schema is now version 6; profiles carry lifecycle, freshness, and
  check timestamps, and conflicting same-revision records are retained.
- The protocol version is now `0.1.0-stage-13`; the ABI remains 1.
- The FFI interface document is at version 1.6.0 with twenty-eight callable
  methods.
- The identify protocol remains `/nexus/0.1.0-stage-12` because Stage 13 adds no
  new network protocol or frame type.
- The Windows developer environment sets `GRADLE_USER_HOME` to the gitignored
  `NeededToolsHere/gradle-home`, so Gradle and WiX downloads stay inside the
  repository.
- The SQLite schema is now version 7; publications carry category, size, tags,
  local/serving/announced state, availability, and last-check timestamps.
- The protocol version is now `0.1.0-stage-14`; the ABI remains 1.
- The FFI interface document is at version 1.7.0 with thirty-three callable
  methods.
- The identify protocol remains `/nexus/0.1.0-stage-12` because Stage 14 adds no
  new network protocol or frame type.
- The file provider now applies the publication serving gate, so imported content
  is not shared until a local publication, replica, or retained download
  justifies it.
- The committed `Cargo.lock` now resolves `url` 2.5.9 and `form_urlencoded`
  1.2.3, fixing RUSTSEC-2026-0592. The package count remains 411.
- The SQLite schema is now version 8; replicas carry source peer, verified size,
  check and seen timestamps, failure count, redacted error, and announcement
  state, with a state index for lifecycle queries.
- The protocol version is now `0.1.0-stage-15`; the ABI remains 1.
- The FFI interface document is at version 1.8.0 with forty-two callable
  methods.
- The identify protocol remains `/nexus/0.1.0-stage-12` because Stage 15 adds no
  new network protocol or frame type.
- The core file provider now also applies the hosting policy, so a paused,
  failed, removed, metered, or low-battery host stops serving while its records
  and bytes remain.
- Inbound file-chunk responses are throttled to the configured average upload
  rate; metadata and error responses stay immediate.
- The SQLite schema is now version 9; `search_meta` tracks the index generation
  and last refresh time, and `search_docs` maps record IDs to FTS rowids.
- The protocol version is now `0.1.0-stage-16`; the ABI remains 1.
- The FFI interface document is at version 1.9.0 with forty-five callable
  methods.
- The identify protocol remains `/nexus/0.1.0-stage-12`; Stage 16 adds the
  separate `/nexus/index/1` request and response protocol.
- The SQLite schema is now version 10; `downloads` records a per-chunk bitmap
  and sparse verified byte count, backfilled from the old verified prefix.
- The protocol version is now `0.1.0-stage-17`; the ABI remains 1.
- The FFI interface document is at version 1.10.0 with forty-five callable
  methods.
- The identify protocol remains `/nexus/0.1.0-stage-12`; Stage 17 reuses
  `/nexus/file/1` and adds no new network protocol or frame type.
- Download progress is measured from `verified_bytes`, so out-of-order chunks
  count immediately instead of waiting for a contiguous prefix.
- `search.query` now returns a synchronous local page and exactly one
  `search.completed` event per cursorless first page. Provider results stream as
  `search.result_page`; there is no separate `search.page` event.
- Search config accepts flat `kind`, `category`, `tags`, and `sort` fields, with
  a compatibility fallback to a nested `filters` map.
- The Kotlin Explore and Search screens show sample coverage, partial, cached,
  offline, and pagination states instead of advertising complete results.
- The SQLite schema is now version 11; `blocked_peers` and `abuse_reports` hold
  local moderation state without a foreign key to observed peers.
- The protocol version is now `0.1.0-stage-18`; the ABI remains 1.
- The FFI interface document is at version 1.11.0 with fifty-one callable
  methods.
- The identify protocol remains `/nexus/0.1.0-stage-12`; Stage 18 adds no new
  network protocol or frame type and reuses `/nexus/file/1` and `/nexus/index/1`.
- A malformed peer ID, reason, category, or oversized note is rejected with
  `invalid_argument`; a blocked peer or fully blocked source list maps to
  `permission_denied` with `peer_blocked`.
- Peer observation addresses and error text are truncated at 512 and 256 bytes
  so a hostile peer cannot grow the peer table with unbounded strings.
- The Windows package is now an MSI instead of an application image only. It
  installs per user under `%LOCALAPPDATA%\Ember` and uses a stable upgrade UUID.
- The native Windows core is staged through `appResourcesRootDir` and loaded from
  `compose.application.resources.dir`.
- Android release packaging includes arm64-v8a and x86_64 JNI libraries and strips
  debug sections from the packaged libraries.
- Android release builds declare `INTERNET`, `ACCESS_NETWORK_STATE`, and
  `CHANGE_WIFI_MULTICAST_STATE` permissions.
- `dev-env.ps1` falls back to an operator-provided JDK and Rust toolchain when the
  pinned local toolchain is absent, so the same scripts run in CI.
- `build-native.ps1` supports `--locked` release builds and skips GNU-only import
  library and C harness steps when those tools are unavailable.

### Fixed

- Desktop Escape now navigates back when no Compose child owns focus after
  mouse-only navigation.
- Compact screen headers no longer duplicate the shell top-bar Back button;
  expanded layouts keep the header Back action.
- Corrupt blobs stay quarantined instead of being downgraded to missing on the
  next startup.
- Orphan partial files are quarantined instead of left in the partials
  directory.
- Failed partial downloads remain counted toward quota while their bytes occupy
  disk.
- Re-importing identical bytes under a full quota no longer fails before the
  content can be deduplicated.
- Managed-store scans no longer follow symlinks or reparse points.
- A failed DHT publication now increments `dht_publish_failures` once instead of
  twice.
- A `download.pause` requested before metadata registration is deferred until
  the download row exists, so the download stays visible and resumable instead
  of being discarded. If metadata then fails, the failure is reported instead
  of a paused state with no row.
- `download.start` rejects a user-supplied `download_id` that already exists in
  the store.
- A graceful core stop records the `pause_interrupted_downloads` result and
  still completes the state and identity shutdown before returning the error.
- Signing a read-only jpackage launcher restores the read-only attribute after
  signing instead of failing.
- Release verification accepts Windows ZIP entry separators and self-signed
  `jarsigner` warnings without hiding a failed signature.
- Native argument splatting under Windows PowerShell 5.1 no longer produces a
  bare `-` for release cargo builds.

### Security

- No API keys, private keys, keystores, or signing material are committed.
- Debug logging is redacted by policy and test coverage.
- CI scans for forbidden files and common credential patterns.
- Imported files are opened read-only and the external original is never
  modified or deleted.
- Database paths are generated relative paths; user input cannot choose a blob
  location.
- Metadata archives contain metadata only, never file bytes or private keys.
- Corrupt or incompatible metadata archives are rejected before the live
  database is replaced.
- Identity keys are encrypted at rest in `secrets/identity.enc`, not in SQLite,
  metadata exports, logs, diagnostics, or crash reports.
- Canonical-record verification rejects changed fields, changed signatures,
  non-canonical CBOR, duplicate keys, identity/key mismatch, and domain replay.
- Backup restore validates and decrypts before touching the live identity file;
  a wrong passphrase returns one generic failure.
- The UI copy distinguishes a verified signature from real-world identity and
  states that v1 has no automatic rotation or central revocation.
- The device peer key is encrypted at rest with `DomainTag::PeerKey` and the
  local encryption key; it is never included in backups, exports, logs, or
  diagnostics.
- Both transports authenticate the remote peer before any application protocol
  runs, and a mismatched peer identity is rejected without being recorded as
  connected.
- Status reporting keeps observed and confirmed addresses separate and never
  claims public reachability without evidence.
- Test messages, frames, bootstrap entries, listen addresses, connection counts,
  and per-peer inbound requests are bounded.
- DHT records are accepted only when the exact signed envelope verifies and the
  key derived from its signed identifier matches the storage key.
- Stale revisions, same-revision conflicts, malformed values, invalid provider
  keys, and oversized provider address lists are rejected and counted.
- DHT value, provider, address, store, TTL, query, and frame sizes are bounded.
  The in-memory store refuses new entries instead of growing without limit.
- Bootstrap peers are entry points, not authorities: they cannot sign for a
  user, change a DHT key, or override a signed record.
- DHT queries reveal queried keys to participating peers; sampled targets avoid
  repeating one exact interest.
- File metadata and chunk messages are canonical, bounded, and rejected when
  mixed, truncated, oversized, or carrying unknown fields.
- Only verified content is served. A source checks the content record, manifest,
  publication binding, chunk index, and requested slice bounds on every request.
- A downloaded chunk is hashed before it is written; a corrupt chunk never
  advances the durable verified prefix.
- Partial paths are generated by the store, and publication display names are
  sanitized before storage; request data cannot choose a path.
- Download IDs are limited to 128 ASCII alphanumeric, `-`, or `_` bytes, and
  removal requires explicit confirmation.
- The transitive `url` crate was updated from 2.5.8 to 2.5.9 for
  RUSTSEC-2026-0592, a stack overflow in `Url::origin()` on nested `blob:` URLs.
- File request and response decoders are exercised with every truncation length
  and every single-byte mutation, and malformed or oversized CBOR is rejected
  without panicking.
- Local blocks and abuse reports are device-local policy. They are never
  announced, uploaded, gossiped, or written into signed records, and their
  listings are marked `local_only`.
- Blocked peers are refused at dial, inbound connection, inbound test/file/index
  request, discovery, provider, and download-source layers, including peers
  blocked after a download started.
- Notes, reasons, categories, IDs, and table sizes are bounded; malformed values
  are rejected before reaching SQLite.
- There is no code path that executes a downloaded file. Previews are delegated
  and every download event reports `auto_execute: false`.
- The threat model, privacy review, and security release checklist are checked
  in, and a high-severity finding blocks release until fixed or explicitly
  accepted.
- A user-initiated metadata export is a full local database snapshot and
  includes security rows; it is documented as private, and diagnostics export
  must exclude them.

### Known limitations

- Release artifacts are test-signed only. Production Android keystore and Windows
  code-signing certificates were unavailable, so the 0.1.0 evidence build is
  verifiable but not publicly trusted.
- Android runtime, low-memory device, battery, poor-network, mobile, IPv6, and
  network-change behaviour were not tested; no emulator or device was available.
- The Windows MSI does not yet offer an uninstall keep/remove choice for user
  data. The packaged UI stores no core data, so this must be added before core
  data is wired in.
- Gradle dependency locking, verification metadata, and a CycloneDX SBOM are not
  part of the release build yet.
- Third-party license notices for the bundled JDK and Compose runtime are not
  packaged yet. This is a legal pre-release gate.
- The UI is not localized. It ships English copy only, with no RTL or
  pseudo-localization test.
- Screen-reader and platform high-contrast testing were not performed.

- Identity, publication, download, hosting, search, and network data in the UI
  are visibly labelled mock content. The native core and Kotlin client exist, but
  the product screens are not connected to them yet.
- Networking, publishing, hosting, and search product flows are not implemented
  in the Kotlin client yet. The native boundary, lifecycle, events, streaming,
  and test methods exist; networking, direct transfer, profile publication, file
  publication, hosting, and search are implemented at the core and network
  layers.
- Direct transfer is implemented and tested on Windows loopback. The product
  screens remain mock content and are not connected to it.
- Stage 12 uses one connected source at a time. It does not run parallel chunk
  swarms, erasure coding, or a seeding incentive.
- A source must already be connected; the download manager does not dial peers
  itself.
- Resume requires the manifest to remain stored or re-fetchable. If the manifest
  is lost and every source is offline, the partial cannot be resumed.
- Content is available only while a source is online and serving verified bytes.
  There is no availability guarantee or remote-erasure mechanism. Unpublishing
  stops local serving and announces a signed withdrawal, but cannot delete copies
  other peers already hold.
- The Kotlin Downloads screen shows demo speed, progress, and source-peer fields
  and is not connected to the core yet.
- Profiles and publications are published and verified on Windows loopback. The
  Kotlin Profile, Publish, and File Details screens remain demo data and are not
  connected to the core yet.
- Stage 14 creates publication revision 1 only. Editing a publication and
  revision chains are deferred.
- The DHT record TTL is 24 h. Stage 14 announces on publish and unpublish but
  does not run a periodic re-announcement worker, so long-term discoverability
  depends on later hosting and republication work.
- Previews are metadata classification only. The core does not render text or
  images, and the demo screens do not open files.
- Stage 15 requires a locally verified copy before hosting; `replica.add` never
  downloads bytes for the user.
- Metered-network and battery values are host-reported signals. Android and
  Windows platform integrations that supply them, and foreground-service
  hosting limits, remain later work.
- Hosted-file rows are retained after removal as audit history; there is no
  purge or compaction UI yet.
- Hosting has no seeding incentive, availability guarantee, or remote-erasure
  mechanism. Removing a replica stops local serving and cannot delete copies
  other peers hold.
- The Kotlin Hosted Files screen shows demo data and pause, resume, and remove
  actions; it is not connected to the core yet.
- Android runtime replication, metered detection, and battery-aware serving are
  not exercised locally; the Android libraries cross-build with the same hosting
  code.
- Usernames are labels, not unique accounts. There is no registry, reservation,
  or first-claim rule, and collision detection covers only profiles the local
  store has seen.
- A same-revision profile conflict is retained for review. There is no automatic
  conflict resolution or UI flow to choose between the retained records.
- A cached remote profile is served as stale after a network miss or failure,
  even when the previous check was recent.
- Stage 10 supports Kademlia peer routing, signed record publication and
  lookup, and provider discovery. Stage 11 adds Circuit Relay v2 fallback,
  DCUtR hole punching, and AutoNAT. mDNS is deferred; NAT traversal is not
  guaranteed without a reachable relay.
- Raw relayed byte totals are not exposed by libp2p. Relay cost reporting uses
  negotiated circuit limits and circuit counters, not a byte total.
- UPnP and NAT-PMP port mapping are deferred; `libp2p-upnp` is not enabled.
- The Kotlin Network troubleshooting section is demo data and is not connected
  to the core yet.
- The DHT store is in-memory and bounded. Records are rebuilt by republication
  and lookup after a restart; they are not durable product state.
- DHT records remain hints. An attacker can run many peers and influence which
  records are reachable; global Sybil resistance and provider reputation are
  later work.
- There is no automatic global bootstrap-list update service. Compiled,
  configured, and persisted lists are operator or release decisions.
- Android runtime DHT behavior is not exercised locally; the Android libraries
  cross-build with the same graph.
- Android runtime publication and file-serving behavior is not exercised
  locally; the Android libraries cross-build with the same publishing code.
- Android runtime relay, DCUtR, and AutoNAT behavior is not exercised locally;
  the Android libraries cross-build with the same relay graph.
- The Android lifecycle hook for Wi-Fi switches and sleep/wake is not bound yet;
  `Network::notify_network_change` is implemented and tested at the network
  layer.
- Android JNI runtime execution is deferred because no emulator or device is
  available locally. The Android arm64-v8a and x86_64 libraries are compiled and
  export-checked, and the JNI shim is exercised through a Windows-hosted JVM
  proxy; this is not on-device evidence.
- Key loss is permanent. v1 has no automatic rotation, central revocation, or
  recovery service; compromise recovery requires a new identity and republish.
- Android SQLite runtime storage is not locally exercised because no emulator or
  device is available and the Android application does not yet host the core.
- The storage-management screen remains a prototype; the settings and quota
  model exist in the data layer.
- Stage 16 distributed coverage is inherently partial. Index providers are
  optional peers; search never claims complete network coverage and cannot make
  an offline provider answer.
- Only the cursorless first page queries index providers. Later pages are local
  pages bound to the same index generation; a new provider view requires a new
  search.
- Provider records are verified and shown but are not cached or written to local
  storage by search. A later profile or publication lookup may cache them.
- Search has no global relevance model. Ranking is local and provider results
  are merged without popularity, reputation, or engagement signals.
- The Kotlin Explore and Search screens remain demo data with honest coverage,
  partial, cached, offline, and pagination labels; they are not connected to the
  core yet.
- Android runtime search and index-provider behavior are not exercised locally;
  the Android libraries cross-build with the same search code.
- Local blocking and reporting are not rendered in the Kotlin UI yet; the core
  methods, events, notices, and enforcement exist.
- There is no per-report deletion control yet. Reports are capped and remain
  local, but removing one report currently requires clearing the local store.
- Blocking is local and does not remove content from the network. A blocked peer
  can return under a new identity and must be blocked again.
- Sybil resistance remains partial. Bounded tables and provider limits reduce the
  effect of many colluding peers but do not eliminate it.
- File safety classification is extension-based and advisory. It is not malware
  detection and must not be presented as a guarantee.
- No independent security review has been commissioned. The threat model,
  privacy review, and release checklist record this as a pre-release gap.
- Android runtime blocking, platform file handling, and platform backup behavior
  are not exercised locally; the release checklist tracks the verification.
- Fuzzing, cancellation-race stress, and long-running resource exhaustion tests
  are Stage 19 work.
- Metadata restore replaces the database but does not restore file bytes; the
  managed store is reconciled separately.
- Shared UI copy is English-only; locale-aware formatting and translated
  resources are later-stage work.
- 200% text scaling, TalkBack, and Narrator journeys have structural support but
  were not exercised on a device or emulator in this environment.
- Desktop reduced motion is a no-op; Android reads the system animator scale.
- Desktop GUI automation was intermittent during the Stage 04 test pass; keyboard
  navigation and screenshot review were used to complete the journeys.
- In the identity bio field, Tab inserts a tab character instead of moving
  focus; Shift+Tab moves focus back to the display-name field.
- The Windows application image is now also packaged as a per-user MSI and a
  portable ZIP by Stage 19. The MSI does not yet offer an uninstall keep/remove
  choice for user data.
- The local non-admin Windows toolchain uses a MinGW GNU Rust host when MSVC C++
  tools are unavailable. CI and release builds use MSVC.
- Android Rust cross-compilation and native library packaging are wired in a
  later stage.
- Gradle configuration cache is disabled because Spotless 8.11.0 cannot
  serialize its line-ending policy under Gradle 9.9.1.
