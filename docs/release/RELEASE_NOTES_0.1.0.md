# Ember 0.1.0 release notes

**Release date:** not yet published
**Version:** 0.1.0 (`versionCode` 100)
**Protocol:** `0.1.0-stage-18`
**Database schema:** 11
**Status:** Stage 19 release candidate. Test-signed only; production signing,
Android runtime testing, and real-network testing remain open.

## What Ember is

Ember is a decentralized application for discovering people and publicly shared
files. It uses a Rust core for identity, signed records, storage, discovery,
transfer, search, and security policy, with a shared Compose UI for Android and
Windows. There is no account server, central index, or mandatory relay.

## What is included

- Local Ed25519 identities with Bech32m `nx1...` IDs, encrypted key storage, and
  passphrase-encrypted backup.
- Signed profiles and file publications with revisions and tombstones.
- SQLite schema 11 with crash reconciliation, corrupt-file quarantine, download
  state, and local moderation tables.
- Kademlia discovery, signed provider records, QUIC and TCP/Noise connections,
  relay fallback, and hole punching.
- Verified chunked transfer with up to four sources, resume, failover, and
  corrupt-source quarantine.
- Hosting, local and distributed search, and multi-peer resilience.
- Local peer blocking, local abuse reports, advisory file-safety notices, and
  delegated previews. Downloaded files are never executed.
- Per-user Windows MSI and portable ZIP.
- Signed Android APK and AAB build pipeline.

## Artifacts

| Artifact | Purpose |
|---|---|
| `Ember-0.1.0-windows.msi` | Per-user Windows installer |
| `Ember-0.1.0-windows-portable.zip` | No-install Windows package |
| `Ember-0.1.0-android.apk` | Direct Android install |
| `Ember-0.1.0-android.aab` | Store delivery |
| `SHA256SUMS` | SHA-256 checksums |
| `release-manifest.json` | Commit, toolchain, sizes, digests, and signing state |

## Windows install

1. Verify the MSI checksum against `SHA256SUMS`.
2. Close any running Ember instance.
3. Open `Ember-0.1.0-windows.msi`.
4. The app installs per user under `%LOCALAPPDATA%\Ember`; no administrator
   rights are required.
5. Start Ember from the Start menu or the install directory.

Requirements: Windows 10 or 11 x64. Java is bundled; do not install a JDK.

To use the portable package, extract the ZIP and run `Ember.exe`. It does not
register an uninstaller or Start menu entry.

## Android install

1. Verify the APK checksum.
2. Allow installation from the file manager or browser used to download it.
3. Open the APK and install.
4. Android 9 (API 28) or later on arm64-v8a or x86_64 is required.

AAB installation is not a user action; the bundle is for store delivery.

## Upgrade

Install the newer MSI or APK over the existing version. Do not change the Windows
upgrade UUID or the Android signing key. See
[UPDATE_AND_ROLLBACK.md](UPDATE_AND_ROLLBACK.md).

## Known limits

- The packaged UI is a clickable demo shell. It is not yet wired to the Rust core
  for networking, publishing, hosting, or search flows.
- Android runtime behaviour, battery use, and low-memory performance were not
  measured. Only build and static package checks are claimed.
- No real poor-network, mobile, IPv6, or network-change testing was performed.
- No independent security review has been completed.
- The Windows MSI removes its install directory on uninstall. User data placement
  and an uninstall keep/remove choice are required before core data is stored by
  the packaged app.
- Crash reporting and telemetry are not included. There is no automatic upload of
  crash data or diagnostics.
- Production signing certificates were not available for this candidate.
  Artifacts were test-signed with an ephemeral Android key and a self-signed
  Windows certificate. Verify signatures and checksums before distributing any
  build from this stage.
- Windows 11 and Android devices were not available for runtime testing.
- Third-party license notices for the bundled JDK and Compose runtime are not
  packaged with the artifacts yet.

## Verification

```powershell
.\scripts\verify-release.ps1 -Directory .\dist\0.1.0 -RequireSigned
```

The script checks artifact sizes and digests, `SHA256SUMS`, APK and AAB
signatures, MSI signer presence, and native-core payloads.
