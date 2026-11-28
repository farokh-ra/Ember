# Security Release Checklist

Use this checklist before tagging any build that is distributed beyond the
development team. A checked item needs an artifact: a test, a command output, a
document, or a signed review note. “Looks fine” is not evidence.

**Release candidate:** ______________________
**Date:** ______________________
**Owner:** ______________________

## 1. Scope and versions

- [ ] The release version, `core_version`, `protocol_version`, FFI document
      version, ABI version, and database schema version are recorded.
- [ ] Every version bump is intentional, documented in `CHANGELOG.md`, and
      reflected in `docs/architecture/FFI_INTERFACE.md`.
- [ ] Database migrations are append-only. No shipped migration file was edited.
- [ ] The ABI version is unchanged unless the C surface actually changed, and a
      compatibility note exists if it did.

## 2. Build and static analysis

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes, including all stage suites.
- [ ] `cargo deny check` passes.
- [ ] `cargo audit` reports no high or critical advisory that is not explicitly
      accepted below.
- [ ] The Android debug and release builds compile, and Gradle tests pass.
- [ ] The Windows desktop application image builds and starts.
- [ ] Formatting, linting, and dependency review CI jobs are green on the
      release commit.
- [ ] Toolchain, JDK, Gradle, Kotlin, Compose, AGP, NDK, and Rust versions match
      the pins in the repository.

## 3. Threat model and known risks

- [ ] `docs/security/THREAT_MODEL.md` matches the shipped code.
- [ ] Every threat marked *partial* or *deferred* has an owner and a disposition.
- [ ] No known high-severity issue is unaddressed. Any accepted high-severity
      risk has a written rationale and an explicit release-block decision.
- [ ] The list of residual risks was reviewed by the project owner.
- [ ] An independent security review has been completed, or the decision to ship
      without one is recorded with its rationale.

## 4. Identity and key handling

- [ ] Private keys, passphrases, and backup bytes do not appear in logs, errors,
      events, diagnostics, metadata exports, or test fixtures.
- [ ] The identity key and device peer key are encrypted at rest with separate
      keys and are excluded from identity backup.
- [ ] Backup export requires an explicit user action and a passphrase, and the
      passphrase is not stored.
- [ ] Wrong-passphrase, corrupt-backup, and incompatible-backup paths fail
      closed without overwriting current data.
- [ ] Key-loss and no-central-recovery warnings are visible in the UI and in
      `security.notices`.
- [ ] Locked-identity paths return a stable error and do not leak key material.

## 5. Network and protocol

- [ ] Every inbound request is decoded with depth, item, map-entry, and byte
      limits; oversized input is rejected before allocation.
- [ ] Truncated, mutated, duplicate-key, non-canonical, and trailing-byte
      messages are rejected without a panic. Evidence: protocol and core tests.
- [ ] Unknown major record, protocol, and manifest versions are rejected; unknown
      minor fields are preserved.
- [ ] Signed records are verified against the claimed identity before storage or
      display.
- [ ] Peer identity mismatches fail with `wrong_peer_id`.
- [ ] Connection, peer, request, provider, search, and worker bounds are active.
- [ ] Index-provider requests are rate-limited and bounded by a total deadline.
- [ ] Discovery and provider results are filtered against the local block list.

## 6. Abuse controls

- [ ] A blocked peer is refused on outbound dial, inbound connection, inbound
      test/file/index request, and as a download source.
- [ ] Unblocking restores normal behavior.
- [ ] Block and report caps are enforced inside the writer transaction.
- [ ] Notes are bounded and truncated on character boundaries.
- [ ] Peer IDs and categories are validated; malformed values are rejected.
- [ ] Block and report rows are marked `local_only` and are never published,
      announced, or uploaded.
- [ ] The UI states that blocking is local and does not remove content from the
      network.
- [ ] `docs/security/PRIVACY_REVIEW.md` reflects the shipped retention and
      deletion behavior.

## 7. Files and platform handlers

- [ ] There is no code path from a completed download to process execution.
- [ ] Every download event and download listing reports `auto_execute: false`
      and `preview: "delegated"`.
- [ ] File safety classification is shown before a platform handler is offered.
- [ ] Dangerous or unknown file types are warned about, not silently opened.
- [ ] Previews are delegated to a platform viewer or an external sandbox; the
      core process never renders untrusted content.
- [ ] Download file names are sanitized and cannot escape the managed directory.
- [ ] Partial files and completed files stay inside app-managed storage.

## 8. Privacy

- [ ] No analytics, advertising, telemetry, or automatic crash reporting is
      linked into the release.
- [ ] The privacy review is current for the release.
- [ ] Public-visibility and no-anonymity disclosures are visible before the first
      publish and in Settings.
- [ ] Logs and diagnostics are redacted and user-exported only.
- [ ] Platform backup behavior for the identity vault and database was verified
      on a real device, or is explicitly marked unverified in the release notes.
- [ ] Permissions requested by each platform are the minimum required.

## 9. Storage and migration

- [ ] Opening an older schema upgrades in one transaction and leaves a usable
      database.
- [ ] Opening a newer schema fails closed instead of downgrading.
- [ ] A corrupt database is not silently replaced.
- [ ] Restore validates the manifest, hash, schema version, and required tables
      before swapping data.
- [ ] Restoring a pre-Stage-18 archive still succeeds.
- [ ] Quota and free-space checks prevent a download from filling the disk.
- [ ] Blocking and reporting survive observation pruning and are unaffected by
      clearing peer observations.

## 10. Dependency and supply chain

- [ ] `Cargo.lock` is committed and matches the built artifacts.
- [ ] The dependency and license inventory is current.
- [ ] New dependencies were reviewed for license, maintenance, and known issues.
- [ ] Gradle lockfiles and `gradle/verification-metadata.xml` are current and CI-verified.
- [ ] The CycloneDX SBOM is attached, parses, and matches the release commit.
- [ ] The third-party notice bundle is attached, contains the licence texts and mapping, and has no unexplained component.
- [ ] No dependency phones home at runtime.
- [ ] Native libraries and binaries ship from pinned, verified sources.

## 11. Release artifacts and signing

- [ ] The release directory contains the APK, AAB, MSI, portable ZIP, CycloneDX
      SBOM, third-party notice bundle, `SHA256SUMS`, and
      `release-manifest.json`.
- [ ] `verify-release.ps1 -RequireSigned` passes on the exact published bytes.
- [ ] Android artifacts carry the production signing identity, not the debug key
      or a test key.
- [ ] Windows executables and the MSI carry the organization's Authenticode
      certificate with a trusted chain.
- [ ] Signatures are timestamped, or the absence of a timestamp authority is
      recorded as an accepted risk.
- [ ] `SHA256SUMS` and the manifest are published next to the artifacts and
      describe those exact bytes.
- [ ] Signing keys, keystores, certificates, and passwords are absent from the
      repository, CI logs, and release notes.
- [ ] The compatibility matrix and performance report match the release commit.
- [ ] Install, upgrade, and uninstall were exercised on each shipped platform.

## 12. Documentation and disclosure

- [ ] `SECURITY.md` lists the current supported versions and reporting channel.
- [ ] `CHANGELOG.md` records security-relevant changes without exploit detail.
- [ ] `docs/security/THREAT_MODEL.md`, `PRIVACY_REVIEW.md`, and this checklist
      are included in the release commit.
- [ ] User-facing release notes explain public visibility, key loss, and local
      blocking in plain language.
- [ ] A security contact and coordinated-disclosure expectation are published.

## 13. Sign-off

| Role | Name | Date | Decision |
|---|---|---|---|
| Project owner | | | |
| Security reviewer | | | |
| Platform reviewer (Android) | | | |
| Platform reviewer (Windows) | | | |

## 14. Accepted risks for this release

Record each accepted risk with severity, rationale, owner, and expiry.

| Risk | Severity | Rationale | Owner | Review by |
|---|---|---|---|---|
| | | | | |

A release with an unrecorded high-severity risk must not ship.
