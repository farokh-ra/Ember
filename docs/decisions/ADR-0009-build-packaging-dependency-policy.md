# ADR-0009 — Build, packaging, dependency, and license policy

- **Status:** Accepted
- **Date:** 2026-11-12
- **Amended:** 2026-11-13 (Stage 03 toolchain verification and local GNU fallback)
- **Amended:** 2026-11-26 (Stage 19 shipping MSI instead of Inno Setup)
- **Deciders:** Ember engineering
- **Supersedes:** none
- **Related:** ADR-0001, ADR-0003, ADR-0005, ADR-0006, [Dependency inventory](../architecture/DEPENDENCY_LICENSE_INVENTORY.md)

## Context

Ember must produce a signed Android app and a signed per-user Windows application from one repository, with a shared Kotlin/Compose UI and a shared Rust core. The build must be reproducible enough to audit, must not introduce a mandatory service, and must keep third-party licenses compatible with distribution. Stage 03 verified the toolchain on a non-admin Windows machine that has an Android SDK but no complete Visual Studio C++ workload or Windows SDK. The local fallback uses portable MinGW-w64; CI and release builds remain MSVC.

The Windows installer must install per-user without administrator rights, preserve user data on upgrade, and ask what to do with user data on uninstall. jpackage alone produces an app image but its MSI path depends on WiX and does not provide an interactive uninstall data prompt.

## Decision

### Pinned toolchain

| Tool | Pin | Notes |
|---|---|---|
| JDK | Eclipse Temurin 21.0.13+9 | Portable extraction under `%LOCALAPPDATA%\NEXUS\toolchain`; exact patch verified in Stage 03. |
| Gradle | 9.9.2 | Wrapper committed; no system Gradle required. |
| Kotlin | 2.4.21 | Shared Kotlin/Compose code; AGP built-in Kotlin. |
| Compose Multiplatform | 1.12.2 | Shared UI and desktop runtime. |
| Compose Material3 | 1.9.0 | Current compatible Material3 line with Compose 1.12.2. |
| Android Gradle Plugin | 9.5.2 | Paired with Gradle 9.9.2. |
| Android compile SDK | 37 | Platform `android-37.0`; build-tools 36.0.0. |
| Android target SDK | 37 | Re-check Google Play's current requirement at each release. |
| Android minimum SDK | 28 | Android 9, as specified by the product. |
| Android NDK | 29.0.14206865 | Installed as `ndk/29.0.14206865`; r29 LTS line. |
| Rust | 1.99.1, edition 2024 | `rust-toolchain.toml` pinned. |
| Android Rust targets | `aarch64-linux-android`, `x86_64-linux-android` | arm64 device, x86_64 emulator. |
| Windows Rust target | `x86_64-pc-windows-msvc` | CI and release; no 32-bit build. |
| Local Windows Rust fallback | `x86_64-pc-windows-gnu` with MinGW-w64 GCC 16.2.0 | Development only when the MSVC C++ workload and Windows SDK are absent. |
| Windows toolchain | MSVC 19.4x (VS 2022 17.14+ or VS 2026 18.x), Windows SDK 10.0.26100 | CI and release build machine. |
| Kotlin formatting | Spotless 8.11.0 with ktlint 1.8.0 | Gradle plugin. |
| Kotlin static analysis | detekt 1.23.8 | Gradle plugin. |
| Rust policy tools | cargo-deny 0.20.2, cargo-audit 0.23.0, cargo-nextest 0.9.151 | CI and release gates. |
| Installer | jpackage MSI with WiX 3.14.1.8722 | Build-time only; per-user MSI. Inno Setup 6.4.3 was the original plan and is replaced by the Stage 19 amendment below. |
| SBOM | CycloneDX | Generated in CI for Android, Windows, and Rust. |

Exact transitive Rust versions are locked in `Cargo.lock`. Gradle resolves the pinned direct versions through `gradle/libs.versions.toml` and the committed wrapper; Gradle dependency locking and verification metadata are required before the first signed release. No dynamic versions, version ranges, or snapshot dependencies are allowed in release builds.

### Local non-admin Windows fallback

A machine without the Visual Studio C++ workload cannot link
`x86_64-pc-windows-msvc`. `scripts/setup-windows.ps1` detects the missing
workload through `vswhere.exe` and installs portable MinGW-w64 GCC 16.2.0 under
`%LOCALAPPDATA%\NEXUS\toolchain\mingw`. `scripts/dev-env.ps1` then selects
`1.99.1-x86_64-pc-windows-gnu` for that shell.

The fallback:

- compiles and tests the same Rust source and is sufficient for local
  development and Stage 03 exit criteria;
- does not change the release target, which remains
  `x86_64-pc-windows-msvc`;
- is not used by CI, because GitHub's Windows runner has the MSVC toolchain;
- must not be used to produce a signed release artifact. Release builds run in
  CI or on a machine with the full MSVC C++ workload and Windows SDK.

The `nexus-ffi` crate keeps a C ABI and does not depend on a C++ runtime, so the
host linker choice does not change the exported interface.

### Android packaging

- `apps/androidApp` builds a signed APK for direct distribution and a signed AAB for Play delivery.
- Rust is built as `libnexus_core.so` for arm64-v8a and x86_64, packaged in `jniLibs`.
- The app uses the Android Keystore to seal the local secret and Storage Access Framework for import/export. It does not request broad storage permissions.
- Signing keys are supplied by the release environment and never committed. Debug builds use a debug key and are not release artifacts.
- Android backup rules exclude the sealed local secret and encrypted identity file; identity portability is through the explicit encrypted backup, not Android auto-backup.
- Foreground service permissions and the `dataSync` service type are declared for the supported API levels.

### Windows packaging

Stage 19 amendment (2026-11-26): the shipping Windows package is a **per-user
MSI** produced by jpackage with WiX 3.14.1. Inno Setup was not adopted because it
would add a second installer toolchain and a manual wrapping step, while
jpackage's MSI path is already part of the Compose Desktop build. The MSI:

- installs per user under `%LOCALAPPDATA%\Ember` without administrator rights;
- uses the stable upgrade UUID `c7c225ee-ae2c-4c58-822d-ea7bda7ac48c`, so a newer
  version replaces the older product;
- creates Start menu and optional desktop shortcuts and registers an uninstaller;
- packages `nexus_core.dll` under the application's `app/resources` directory,
  which the JNA loader checks through `compose.application.resources.dir`;
- is Authenticode-signed in the release pipeline, along with the main executable
  in the portable ZIP.

The MSI does not implement the interactive uninstall data prompt that the Inno
Setup plan described. The current packaged UI stores no core user data, so this
does not affect 0.1.0. Before the UI is wired to the core, user data must live
outside the install directory and uninstall must offer a keep/remove choice. That
remains a release requirement.

The rest of the original packaging decision still applies:

- Gradle builds a Compose Desktop app image with `jpackage` and a pinned JDK 21
  runtime.
- The Rust core is `nexus_core.dll` x64, loaded by JNA from the packaged
  resources directory.
- The app does not install a login auto-start entry in v1.
- A portable ZIP is published for users who cannot install an MSI.

### Dependency policy

Allowed licenses for distributed code:

| License | Policy |
|---|---|
| Apache-2.0 | Allowed. |
| MIT | Allowed. |
| BSD-2-Clause / BSD-3-Clause | Allowed with notice. |
| ISC | Allowed. |
| Zlib | Allowed. |
| MPL-2.0 | Allowed for unmodified dependencies with notice; modifications require review. |
| Unicode-3.0 | Allowed for Unicode data. |
| OFL-1.1 | Allowed for bundled fonts with notice. |
| Public domain | Allowed (SQLite). |

Disallowed for linked or distributed code: GPL, AGPL, SSPL, and any license that would require Ember source disclosure or impose network-service obligations. LGPL is not allowed for linked code unless a future ADR approves dynamic linking and provides relinking rights; JNA is used under its Apache-2.0 option, not LGPL.

Build-only tools may carry other licenses because they are not distributed: Git (GPL-2.0), WiX (MS-RL), MSVC/Windows SDK (proprietary), Android SDK (proprietary license), Python (PSF). The JDK runtime bundled by jpackage is GPLv2 with the Classpath Exception and is distributed with notices.

### License and vulnerability gates

- `cargo-deny` checks licenses, banned crates, duplicate versions, and advisory sources on every CI run.
- `cargo-audit` and the GitHub Advisory Database feed are checked before release.
- Gradle resolves only locked versions and produces a dependency license report; the report is reviewed when dependencies change. Dependency locking and SHA-256 verification metadata are enabled and committed as of the Stage 20 launch-readiness follow-up; CI rejects drift.
- A CycloneDX SBOM is required for each release artifact. `scripts/generate_sbom.py` generates it from `Cargo.lock` and the Gradle verification metadata, and CI uploads it.
- The release includes a generated third-party notice bundle and the exact protocol/app versions. The bundle still needs a reviewed pass before a public release.
- New dependencies require a reason, a license check, a platform check, and an owner in the dependency inventory.
- No analytics, crash-reporting, advertising, or account SDK is included by default.

### Reproducibility and release evidence

- Release builds use `--locked` Cargo builds and checksum verification. Gradle dependency locking and verification metadata are committed; a clean build fails when either is stale.
- Build metadata records the toolchain versions, source commit, and artifact SHA-256.
- Android and Windows artifacts are signed; SHA-256 checksums and release notes are published.
- A clean checkout must build with the documented commands. The build must not depend on a developer's global Gradle, Cargo, or JDK configuration beyond the documented toolchain.
- No build step downloads code from an unpinned URL at runtime.

## Consequences

**Positive**

- One repository produces both first-class platforms with pinned, auditable versions.
- Per-user Windows installation avoids administrator rights; the uninstall data choice is deferred until the packaged application stores core user data.
- License and vulnerability gates are defined before code lands.
- The JDK and core runtime are bundled, so users do not install Java or Rust.
- SBOM and checksums support release review. Checksums, the release manifest, the SBOM, and the notice bundle now exist; the bundle still needs legal and release review.

**Negative / trade-offs**

- WiX 3 is an extra build-time dependency, downloaded under a pinned hash; the MSI does not yet provide an uninstall data choice.
- Bundling a JDK increases the Windows artifact size.
- Pinning the latest stable tool lines requires a deliberate upgrade cadence.
- Android and Windows signing certificates are release-operations dependencies.
- The local non-admin fallback adds a second Rust host target to developer
  documentation, but not to release artifacts.
- Spotless 8.11.0 cannot serialize its line-ending policy into Gradle's
  configuration cache under the pinned Gradle 9.9 line, so the configuration
  cache is disabled for now.

**Constraints**

- No release artifact may be produced from an unlocked dependency graph.
- No private signing key or keystore is committed to the repository.
- No mandatory auto-update service or account SDK.
- No GPL/AGPL/SSPL code linked into the distributed app.
- No silent downgrade of the database or protocol.

## Alternatives considered

1. **jpackage MSI with WiX.** Native MSI and familiar enterprise deployment, but requires WiX, does not provide the required interactive uninstall data prompt, and adds MSI authoring complexity. Rejected for Stage 03; **revisited and adopted in the Stage 19 amendment** because it removes a second installer toolchain, and the data prompt is deferred until the packaged app stores core data.
2. **MSIX.** Good Windows integration and signing, but packaging, identity, and uninstall data handling are more complex for a per-user side-loadable app. Rejected for v1.
3. **Electron or a web wrapper.** Violates the first-class native platform rule and adds a browser runtime. Rejected.
4. **Gradle-only Windows packaging without an installer.** Users would manage a folder manually; no uninstall or upgrade story. Rejected.
5. **Unpinned latest versions.** Easier upgrades, but unreproducible and unsafe for signed records and native builds. Rejected.
6. **System Java instead of a bundled runtime.** Smaller download, but users would need a compatible JDK and jpackage behavior would vary. Rejected.

## Verification

- Stage 03 installed and recorded the exact toolchain, built an empty Android
  app, an empty Windows app, and a Rust library, and proved a clean-checkout
  build. Evidence is recorded in `docs/progress/STAGE_03.md`.
- Stage 08 packages the Rust library into both apps and proves the same core API
  is callable.
- Stage 19 produces signed release candidates, verifies signatures and checksums, and installs, upgrades, and uninstalls the Windows MSI. Android APK/AAB install and runtime verification remain blocked on device availability. SBOM and Gradle lock auditing remain pre-release requirements.
- CI fails on an unapproved license, a known high-severity vulnerability without a documented exception, or a dynamic dependency version.
