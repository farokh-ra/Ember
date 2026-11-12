# NEXUS Project Handoff

**Current stage:** 03 — Repository, build system and developer workflow
**Stage status:** Complete
**Last updated:** 2026-11-13
**Next stage:** 04 — UI foundation and design system (waiting for user permission)

## What was implemented

Stage 03 is complete. The repository now builds and checks every platform shell:

- Git repository on `main` with LF line endings, repository hygiene rules, a
  Conventional Commits template, and issue/pull-request templates.
- Gradle 9.9.1 Kotlin Multiplatform build with Compose Multiplatform and five
  shared modules: `model`, `core-api`, `core-client`, `presentation`, and `ui`.
- A buildable Android application shell (`io.nexus.android`, version `0.1.0`,
  min SDK 28, target/compile SDK 37) that produces a debug APK.
- A buildable Windows desktop application shell that produces a Compose
  distributable app image with a bundled runtime.
- A Cargo workspace with all twelve architecture crates, a tested native ABI
  version export, redacted logging, and pinned Rust 1.99.1.
- Spotless 8.11.0 with ktlint 1.8.0, detekt 1.23.8, Rustfmt, and Clippy with
  warnings denied.
- Shared JVM tests and Rust tests, plus proven non-zero failure detection for
  both test systems.
- CI workflow for hygiene, commit titles, Rust checks, Android build/tests, and
  Windows desktop packaging; dependency review; Dependabot.
- Portable non-admin Windows toolchain setup and a single local build gate.
- Developer setup, workflow, logging policy, security policy, contributing
  guide, changelog, and Stage 03 evidence record.
- Repository hygiene and commit-message validators.
- `cargo deny` and `cargo audit` dependency and advisory checks.

No product features are implemented. The Android and Windows screens are empty
shells; Stage 04 establishes the UI foundation and design system.

## Files changed

Build and repository foundation:

- `.editorconfig`, `.gitattributes`, `.gitignore`, `.gitmessage`
- `settings.gradle.kts`, `build.gradle.kts`, `gradle.properties`
- `gradle/libs.versions.toml`, `gradle/wrapper/*`, `gradlew`, `gradlew.bat`
- `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`, `deny.toml`

Kotlin modules:

- `apps/androidApp/**`
- `apps/desktopApp/**`
- `shared/model/**`
- `shared/core-api/**`
- `shared/core-client/**`
- `shared/presentation/**`
- `shared/ui/**`

Rust crates:

- `crates/nexus-types`, `nexus-crypto`, `nexus-protocol`, `nexus-identity`,
  `nexus-store`, `nexus-transfer`, `nexus-net`, `nexus-search`, `nexus-core`,
  `nexus-ffi`, `nexus-observability`, `nexus-testkit`

Automation and documentation:

- `.github/workflows/ci.yml`, `.github/workflows/dependency-review.yml`
- `.github/dependabot.yml`, `.github/release.yml`
- `.github/pull_request_template.md`, `.github/ISSUE_TEMPLATE/**`
- `scripts/setup-windows.ps1`, `scripts/dev-env.ps1`, `scripts/build.ps1`
- `scripts/verify-clean-checkout.ps1`
- `scripts/check_repo_hygiene.py`, `scripts/check_commit_message.py`
- `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`
- `docs/DEVELOPER_SETUP.md`, `docs/DEVELOPMENT_WORKFLOW.md`,
  `docs/LOGGING_POLICY.md`
- `docs/progress/STAGE_03.md`, `docs/HANDOFF.md`
- `docs/decisions/ADR-0009-build-packaging-dependency-policy.md` (amended)
- `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md` (updated to 1.1.0)

## Commands and results

### Full local gate

```text
.\scripts\build.ps1
```

Result: exit 0.

- Gradle `BUILD SUCCESSFUL`; Spotless and detekt passed.
- Android debug APK:
  `apps/androidApp/build/outputs/apk/debug/androidApp-debug.apk`,
  29,635,374 bytes.
- Windows app image:
  `apps/desktopApp/build/compose/binaries/main/app/NEXUS`, 130,042,911 bytes;
  launcher 540,672 bytes.
- Shared JVM tests: 6 tests, 0 failures.
- `cargo fmt --all -- --check`, Clippy with `-D warnings`, and
  `cargo test --workspace` passed (6 unit tests, 0 failures, plus doc tests).
- Repository hygiene passed.

The Windows desktop build was also verified with `local.properties` removed and
`ANDROID_HOME` unset, so the Windows CI job does not require the Android SDK.

### Validators

```text
python scripts/check_repo_hygiene.py
python scripts/check_commit_message.py "feat(build): scaffold nexus repository"
python scripts/validate_stage01_docs.py
python scripts/validate_stage02_docs.py
```

Result: all pass. Stage 01 and Stage 02 validators were re-run after the Stage 03
documentation additions and still report `PASS`.

### Failure detection

A temporary Kotlin test asserted `false`: `:shared:model:jvmTest` failed with
exit code 1. After removal it passed with exit code 0. A temporary Rust test
asserted `false`: `cargo test -p nexus-types` failed with exit code 101. After
removal it passed with exit code 0.

### Dependency and advisory gates

`cargo deny check` and `cargo audit` were run with the pinned versions. The exact
exit codes, allowed-license configuration, and any advisory status are recorded
in `docs/progress/STAGE_03.md` section 8.

### Clean checkout

After the initial commit, `scripts/verify-clean-checkout.ps1` cloned `HEAD` into
a temporary directory and ran the full build gate from the clone. The result and
commit are recorded in `docs/progress/STAGE_03.md` section 8.

### CI validation

All `.github/**/*.yml` files parse with PyYAML 6.0.3. GitHub Actions could not be
executed remotely because this environment has no GitHub remote and no Docker
runner. The workflow runs the same commands that pass locally. This limitation
is recorded in the Stage 03 evidence file.

## Errors and failed attempts

1. The machine had no JDK 21, Gradle, Rust, NDK, MSVC C++ workload, or Windows
   SDK. A portable, non-admin toolchain was installed under
   `%LOCALAPPDATA%\NEXUS\toolchain`.
2. Android CLI syntax changed: the working command is
   `android.exe sdk install "ndk/29.0.14206865"`; the older
   `sdkmanager "ndk;..."` path is kept only as a fallback.
3. AGP 9 cannot combine the KMP plugin and `com.android.application` in one
   subproject. Android application code lives in `apps/androidApp`; shared
   modules use `com.android.kotlin.multiplatform.library`.
4. Gradle configuration cache cannot serialize Spotless 8.11.0's line-ending
   policy under Gradle 9.9.1, so it is disabled and documented.
5. PowerShell 5.1 renders native stderr as `NativeCommandError`. The scripts now
   set `$ErrorActionPreference = "Continue"`, merge native stderr into the
   output stream, and fail on non-zero exit codes only.
6. The Compose `NexusApp` name trips ktlint's generic function-naming rule. A
   narrow suppression is used; detekt ignores composables for the same rule.
7. ktlint reordered `import` statements; Spotless normalizes them, so commits
   should run `spotlessApply` before pushing.

## Decisions and unresolved questions

- All architecture decisions remain in `docs/decisions/`. ADR-0009 now records
  exact Stage 03 pins and the MinGW fallback.
- Local Rust builds use `x86_64-pc-windows-gnu` with MinGW-w64 GCC 16.2.0
  because MSVC C++ tools and the Windows SDK are unavailable. CI and release
  remain `x86_64-pc-windows-msvc`; the C ABI is unchanged.
- Gradle dependency locking and verification metadata are deferred until before
  the first signed release, as recorded in ADR-0009 and the dependency
  inventory.
- Common Kotlin tests are deferred; Stage 03 tests live in `src/jvmTest`.
- The Windows app image is not an installer. Inno Setup packaging is Stage 19.
- Android Rust cross-compilation is not wired yet; the NDK and Rust targets are
  installed and ready for the native packaging stage.
- Remote GitHub Actions execution is verified when the repository is pushed to
  its hosting remote.

## Exact next action

1. Review the Stage 03 evidence in `docs/progress/STAGE_03.md`.
2. Give explicit approval to start Stage 04.
3. Stage 04 establishes the UI foundation and design system. Do not begin it
   before that approval.
