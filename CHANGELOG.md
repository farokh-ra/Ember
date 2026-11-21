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

### Known limitations

- Identity, publication, download, hosting, search, and network data in the UI
  are visibly labelled mock content. The native core and Kotlin client exist, but
  the product screens are not connected to them yet.
- Networking, publishing, hosting, and search product flows are not implemented
  yet. The native boundary, lifecycle, events, streaming, and test methods exist;
  networking and direct transfer are implemented at the core and network layers.
- Direct transfer is implemented and tested on Windows loopback. The product
  screens remain mock content and are not connected to it.
- Stage 12 uses one connected source at a time. It does not run parallel chunk
  swarms, erasure coding, or a seeding incentive.
- A source must already be connected; the download manager does not dial peers
  itself.
- Resume requires the manifest to remain stored or re-fetchable. If the manifest
  is lost and every source is offline, the partial cannot be resumed.
- Content is available only while a source is online and serving verified bytes.
  There is no availability guarantee or remote-erasure mechanism.
- The Kotlin Downloads screen shows demo speed, progress, and source-peer fields
  and is not connected to the core yet.
- Profiles are published and verified on Windows loopback. The Kotlin Profile
  screen remains demo data and is not connected to the core yet.
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
- FTS5 search is schema-only and is populated in Stage 16.
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
- The Windows application image is not yet an installer; Inno Setup packaging is
  Stage 19.
- The local non-admin Windows toolchain uses a MinGW GNU Rust host when MSVC C++
  tools are unavailable. CI and release builds use MSVC.
- Android Rust cross-compilation and native library packaging are wired in a
  later stage.
- Gradle configuration cache is disabled because Spotless 8.11.0 cannot
  serialize its line-ending policy under Gradle 9.9.1.
