# Ember Project Handoff

**Current stage:** 19 - performance, compatibility, and release engineering
**Stage status:** Release pipeline complete and test-signed; Android runtime and
production signing evidence unavailable
**Last updated:** 2026-11-26
**Branch:** `feat/19-release`, based on Stage 18 commit `4932c69`
**Pull requests:** #25 (`feat/18-security` -> `main`) merged as `8876350`;
Stage 19 is #26 (`feat/19-release` -> `main`), open and awaiting review
**Next stage:** 20 - global launch and ongoing network governance (requires
explicit project-owner approval)

## What was implemented

Stage 19 turns the build into a release pipeline:

- Per-user Windows MSI produced by jpackage with WiX 3.14.1. It installs under
  `%LOCALAPPDATA%\Ember` without elevation, uses the stable upgrade UUID
  `c7c225ee-ae2c-4c58-822d-ea7bda7ac48c`, and supports in-place upgrades.
- Portable Windows ZIP containing the same app image.
- Android release APK and AAB with an environment-driven signing config. The
  native core is packaged for arm64-v8a and x86_64 and debug sections are
  stripped.
- `INTERNET`, `ACCESS_NETWORK_STATE`, and `CHANGE_WIFI_MULTICAST_STATE` Android
  permissions.
- `emberVersion` and `emberVersionCode` Gradle properties for release overrides,
  with `0.1.0` defaults and MSI segment validation.
- Native Windows core staged through `appResourcesRootDir` and loaded by JNA from
  `compose.application.resources.dir`.
- `scripts/fetch-wix.ps1`: pinned WiX download with SHA-256 verification.
- `scripts/package-release.ps1`: builds, signs, checksums, manifests, and
  verifies APK, AAB, MSI, and portable ZIP.
- `scripts/verify-release.ps1`: independent digest, signature, and payload
  verification.
- `scripts/measure-performance.ps1`: startup, working set, idle CPU, soak growth,
  and package sizes.
- `.github/workflows/release.yml`: tag or manual signed release with secrets.
- Release documentation set under `docs/release/`: performance report,
  compatibility matrix, signing and reproducibility, update/rollback, release
  notes, and accessibility/localization review.
- One new JVM adapter test for loading the core from packaged resources. Rust
  tests remain 372.

## Files changed

Build:

- `build.gradle.kts`
- `apps/androidApp/build.gradle.kts`
- `apps/androidApp/src/main/AndroidManifest.xml`
- `apps/desktopApp/build.gradle.kts`
- `shared/core-client/src/jvmMain/kotlin/io/nexus/core/client/JnaNativeBridge.kt`
- `shared/core-client/src/jvmTest/kotlin/io/nexus/core/client/NativeContractTest.kt`

Scripts:

- `scripts/dev-env.ps1`
- `scripts/build-native.ps1`
- `scripts/fetch-wix.ps1` (new)
- `scripts/package-release.ps1` (new)
- `scripts/verify-release.ps1` (new)
- `scripts/measure-performance.ps1` (new)

CI and docs:

- `.github/workflows/release.yml` (new)
- `docs/release/README.md` (new)
- `docs/release/PERFORMANCE_REPORT.md` (new)
- `docs/release/COMPATIBILITY_MATRIX.md` (new)
- `docs/release/SIGNING_AND_REPRODUCIBILITY.md` (new)
- `docs/release/UPDATE_AND_ROLLBACK.md` (new)
- `docs/release/RELEASE_NOTES_0.1.0.md` (new)
- `docs/release/ACCESSIBILITY_AND_LOCALIZATION.md` (new)
- `docs/progress/STAGE_19.md` (new)
- `docs/decisions/ADR-0009-build-packaging-dependency-policy.md`
- `docs/DEVELOPMENT_WORKFLOW.md`
- `README.md`
- `CHANGELOG.md`
- `docs/HANDOFF.md`

## Commands and results

Local gates (Windows, GNU fallback for native tests):

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | 372 Rust tests passed |
| `cargo deny check` | Passed |
| `cargo audit` | No vulnerability reported |
| `.\gradlew.bat spotlessCheck detekt` | Passed |
| Shared JVM tests | Passed, including the new packaged-resource load test |
| `.\scripts\check_repo_hygiene.py` | Passed |

Release packaging:

| Command | Result |
|---|---|
| `.\scripts\package-release.ps1 -Version 0.1.0 -AllowDirty` with test signing | Passed; APK, AAB, MSI, ZIP, checksums, manifest |
| `.\scripts\verify-release.ps1 -Directory .\dist\0.1.0 -RequireSigned` | Passed |
| `:apps:androidApp:assembleRelease` / `bundleRelease` | Passed |
| `:apps:desktopApp:createDistributable` / `packageMsi` | Passed |
| MSI install | Exit 0, installed 0.1.0 |
| Installed launch | 2,887 ms to main window |
| MSI upgrade 0.1.0 -> 0.1.1 | Exit 0, product version 0.1.1 |
| MSI uninstall | Exit 0, install directory and uninstall entry removed |
| Portable ZIP launch | 2,890 ms to main window |
| APK signature | `apksigner` v2 verified, 1 signer |
| AAB signature | `jarsigner` `jar verified` with self-signed warnings |

Performance (3 runs, 180-second soak, Windows 10 19045, i5-10300H, 16 GB):

| Metric | Result |
|---|---|
| Median startup | 2,805 ms |
| Median working set | 207.1 MiB |
| Idle CPU | 0.021% of the machine |
| 180-second idle growth | 0.9 MiB |
| App image | 154.3 MiB |
| MSI | 71.0 MiB |
| APK / AAB | 61.3 MiB / 22.2 MiB |

## Errors and failed attempts

- Unquoted `-PemberVersion=1.2.3` reached Gradle as `1`. The packaging script
  now passes the argument as a quoted string.
- The MSI needs an explicit package version and Windows segment limits. Both are
  enforced in the script.
- Windows PowerShell 5.1 mishandled `@lockArgs` for native commands; cargo
  received a bare `-`. The script now builds an explicit argument array.
- jpackage marks `Ember.exe` read-only; signing now clears and restores the
  attribute.
- `apksigner.bat` returns -1 to PowerShell on success. Verification calls it
  through `cmd.exe` and checks the `Verifies` line.
- `Compress-Archive` writes backslash ZIP entries. Verification accepts both
  separators.
- Reinstalling the same MSI from a different path failed with 1603 while a stale
  0.1.0 product was registered. Uninstalling first produced clean evidence.
- `--strip-unneeded` removed symbols read by verification; `--strip-debug` keeps
  exported symbols and removes debug sections.
- An early app image contained duplicate native DLLs from manual staging. Gradle
  now owns the staged copy.

## Decisions and unresolved questions

Decisions:

- The Windows package is a per-user MSI. ADR-0009 is amended from Inno Setup.
- Release signing is entirely environment-driven; builds fail closed without it.
- The portable ZIP is described as an unsigned container with a signed
  executable, not as a signed artifact.
- Test signing used an ephemeral Android key and a self-signed Windows
  certificate, both deleted after the run.
- No telemetry or crash reporting was added. A future reporter needs explicit
  opt-in, local preview, and redaction.
- No chunk, hash, or index tuning was made without a realistic workload.
- Production versions remain `0.1.0`; release overrides use Gradle properties.

Unresolved:

- Android runtime, low-memory device, battery, Doze, and network evidence.
- Poor Wi-Fi, mobile, IPv6, and network-change testing.
- Production Android keystore and Windows code-signing certificate.
- Windows 11 runtime testing.
- Uninstall keep/remove prompt for future user data.
- Gradle dependency locking, verification metadata, and CycloneDX SBOM.
- Localization and screen-reader testing.
- Closed alpha and beta with real users.

## Exact next action

1. Stage 19 is committed, pushed, and verified in PR #26 against `main`.
2. Keep PR #26 open for review; do not merge it without project-owner approval.
3. Do not start Stage 20. It requires explicit approval and a decision on how to
   collect the missing Android and network evidence.
