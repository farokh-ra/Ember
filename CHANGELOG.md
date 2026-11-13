# Changelog

All notable changes to NEXUS are recorded here. The format follows
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
- NEXUS design system with light/dark color schemes, semantic status colors,
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

### Changed

- The Android and Windows shells now host the shared clickable UI prototype
  instead of empty placeholder content.
- The local gate and CI now run `:shared:ui:jvmTest`.
- Compose functions are exempt from detekt complexity and naming rules, and
  palette, icon, and brand-art files use a narrow magic-number suppression.
- ktlint ignores function naming for `@Composable` functions.
- Toolchain pins are recorded for JDK 21.0.13+9, Gradle 9.9.1, Kotlin 2.4.21,
  Compose Multiplatform 1.12.2, Material3 1.9.0, AGP 9.5.1, Rust 1.99.1, and
  Android NDK 29.0.14206865.
- Rust now owns durable local storage; Kotlin remains a demo UI until the FFI
  bridge is added in Stage 08.
- Rust now owns local identity keys, canonical signed records, encrypted
  identity storage, and encrypted backups; no private key enters Kotlin.
- The committed `Cargo.lock` now resolves 109 packages, and the dependency and
  license inventory is at version 1.3.0.

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

### Known limitations

- Identity, publication, download, hosting, search, and network data in the UI
  are visibly labelled mock content. The storage foundation is real, but the UI
  is not connected to it until the FFI bridge.
- Publishing, transfer, hosting, and networking behavior is not implemented yet.
  Stage 06 provides identity, canonical signed records, and encrypted backup only.
- Android and Windows runtime cross-verification of the committed crypto vectors
  is deferred to Stage 08 because the Kotlin shells do not link the Rust core and
  no Android device or emulator is available locally.
- Key loss is permanent. v1 has no automatic rotation, central revocation, or
  recovery service; compromise recovery requires a new identity and republish.
- Android SQLite cross-compilation and runtime storage behavior are not locally
  exercised because no emulator or device is available and the Kotlin shells do
  not link `nexus-store` yet.
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
