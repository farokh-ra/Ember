# Stage 03 - Repository, Build System and Developer Workflow

**Status:** Complete
**Completed:** 2026-11-13
**Progress guide:** 9-13%
**Next stage:** 04 - UI foundation and design system (requires user permission)

## 1. Scope

Stage 03 creates the repository and the build foundation for every later stage.
It does not implement product features. The stage covers:

- Git repository hygiene and conventions.
- Gradle with Kotlin Multiplatform and Compose Multiplatform.
- A Cargo workspace with all twelve architecture crates.
- Formatting, linting, and static analysis for Kotlin and Rust.
- Automated Android, Windows, and Rust build checks.
- Branch, commit, issue, and release conventions.
- Debug logging that does not leak secrets.
- Dependency update and vulnerability scanning.
- A repeatable, non-admin developer setup.

## 2. Deliverables

| Deliverable | Path | Status |
|---|---|---|
| Buildable empty Android app | `apps/androidApp` | Complete; debug APK built. |
| Buildable empty Windows desktop app | `apps/desktopApp` | Complete; Compose app image built. |
| Buildable Rust library | `crates/nexus-ffi` and eleven supporting crates | Complete; tests pass. |
| CI workflow | `.github/workflows/ci.yml` | Complete; YAML validated. |
| Dependency review | `.github/workflows/dependency-review.yml` | Complete. |
| Dependabot updates | `.github/dependabot.yml` | Complete. |
| Dependency and advisory policy | `deny.toml`, `scripts/build.ps1 -WithDependencyScan` | Complete; `cargo deny check` and `cargo audit` exit 0. |
| Contribution guide | `CONTRIBUTING.md` | Complete. |
| Developer setup guide | `docs/DEVELOPER_SETUP.md` | Complete. |
| Development workflow | `docs/DEVELOPMENT_WORKFLOW.md` | Complete. |
| Logging policy | `docs/LOGGING_POLICY.md` | Complete. |
| Security policy | `SECURITY.md` | Complete. |
| Changelog | `CHANGELOG.md` | Complete. |
| Portable setup script | `scripts/setup-windows.ps1` | Complete; idempotent run passed. |
| Local full gate | `scripts/build.ps1` | Complete; exit 0. |
| Clean-checkout verifier | `scripts/verify-clean-checkout.ps1` | Complete; result recorded in section 8. |
| Repository hygiene | `scripts/check_repo_hygiene.py` | Complete; pass. |
| Commit validator | `scripts/check_commit_message.py` | Complete; pass and failure cases tested. |
| Architecture and pin updates | ADR-0009 and dependency inventory | Complete. |

## 3. Repository structure

Kotlin modules:

```text
apps/androidApp
apps/desktopApp
shared/model
shared/core-api
shared/core-client
shared/presentation
shared/ui
```

Rust crates:

```text
crates/nexus-types
crates/nexus-crypto
crates/nexus-protocol
crates/nexus-identity
crates/nexus-store
crates/nexus-transfer
crates/nexus-net
crates/nexus-search
crates/nexus-core
crates/nexus-ffi
crates/nexus-observability
crates/nexus-testkit
```

Key structural decisions:

- The Android application lives in `apps/androidApp`, separate from the shared
  KMP modules, because AGP 9 does not allow `com.android.application` and the KMP
  plugin in one subproject.
- Shared modules use `org.jetbrains.kotlin.multiplatform` with
  `com.android.kotlin.multiplatform.library`.
- AGP 9 built-in Kotlin compiles the Android application; the separate
  `org.jetbrains.kotlin.android` plugin is intentionally absent.
- `shared/ui` depends on `shared/presentation`, which depends on
  `shared/core-api`, which depends on `shared/model`. The dependency direction is
  one-way.
- Stage 03 tests are in `src/jvmTest` to avoid Android host-test setup. Common
  tests can return when the platform test strategy is defined.

## 4. Toolchain verified

| Tool | Verified version |
|---|---|
| JDK | Eclipse Temurin 21.0.13+9 |
| Gradle | 9.9.1 (wrapper) |
| Kotlin | 2.4.21 |
| Compose Multiplatform | 1.12.2 |
| Compose Material3 | 1.9.0 |
| Android Gradle Plugin | 9.5.1 |
| compile SDK / target SDK | 37 / 37 |
| minimum SDK | 28 |
| Android build tools | 36.0.0 |
| Android NDK | 29.0.14206865 (r29) |
| Rust | 1.99.1, edition 2024 |
| Rust host (local) | `x86_64-pc-windows-gnu` with MinGW-w64 GCC 16.2.0 |
| Rust host (CI/release) | `x86_64-pc-windows-msvc` |
| Python | 3.11.0 |
| Git | 2.56.0.windows.2 |

The local machine does not have the Visual Studio C++ workload or Windows SDK,
so the local Rust host is the documented GNU fallback. CI and release remain
MSVC, as recorded in ADR-0009. Android Rust targets
`aarch64-linux-android` and `x86_64-linux-android` are installed for later
native packaging.

## 5. Commands and results

### Full local gate

```text
.\scripts\build.ps1
```

Result: exit 0.

- Gradle reported `BUILD SUCCESSFUL`.
- Spotless and detekt passed.
- `:apps:androidApp:assembleDebug` produced a debug APK.
- `:apps:desktopApp:createDistributable` produced a Windows app image.
- Shared JVM tests passed: 6 tests, 0 failures.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace` passed: 6 tests, 0 failures, plus doc tests.
- Repository hygiene passed.

Artifacts:

| Artifact | Path | Size |
|---|---|---|
| Android debug APK | `apps/androidApp/build/outputs/apk/debug/androidApp-debug.apk` | 29,635,374 bytes |
| Windows app image | `apps/desktopApp/build/compose/binaries/main/app/NEXUS` | 130,042,911 bytes total |
| Windows launcher | `.../app/NEXUS/NEXUS.exe` | 540,672 bytes |

### Kotlin tests

```text
.\gradlew.bat --console=plain :shared:model:jvmTest :shared:core-api:jvmTest :shared:core-client:jvmTest :shared:presentation:jvmTest
```

| Test class | Tests | Failures |
|---|---|---|
| `RedactionTest` | 3 | 0 |
| `FakeNexusCoreTest` | 1 | 0 |
| `CoreClientTest` | 1 | 0 |
| `StartupPresenterTest` | 1 | 0 |

### Rust tests

```text
cargo test --workspace
```

| Crate | Tests | Failures |
|---|---|---|
| `nexus-core` | 1 | 0 |
| `nexus-ffi` | 1 | 0 |
| `nexus-observability` | 3 | 0 |
| `nexus-types` | 1 | 0 |
| All other crates | 0 | 0 |

### Formatting and static analysis

```text
.\gradlew.bat spotlessCheck detekt
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Result: exit 0 for all. ktlint 1.8.0 flags the Compose `NexusApp` name only by
the generic function-naming rule; the composable is annotated with a narrow
ktlint suppression. detekt ignores composables for the same naming rule through
`config/detekt/detekt.yml`.

### Dependency and advisory gates

```text
cargo deny check
cargo audit
```

Result: `cargo deny check` exited 0 with `advisories ok, bans ok, licenses ok,
sources ok`. `cargo audit` exited 0 after scanning 34 crate dependencies against
1,490 advisories. Configuration and full output details are in section 8.

### Setup idempotency

```text
.\scripts\setup-windows.ps1
```

Result: exit 0. The second run detected and reused JDK 21.0.13+9, Gradle 9.9.1,
MinGW-w64 GCC 16.2.0, rustup, Android NDK r29, `platforms/android-37.0`, and
`build-tools/36.0.0`. It did not re-download the archives.

### Commit message validation

```text
python scripts/check_commit_message.py "feat(build): scaffold nexus repository"
python scripts/check_commit_message.py "bad message"
```

Result: valid title exit 0; invalid title exit 1 with a Conventional Commits
failure message.

The Windows desktop build was also verified with `local.properties` removed and
`ANDROID_HOME` / `ANDROID_SDK_ROOT` pointed at an empty directory. It still
produced the app image, so the `desktop` CI job does not require the Android SDK.

### CI workflow validation

All files under `.github/` were parsed as YAML with PyYAML 6.0.3. The workflow
defines six jobs: repository hygiene, pull request title, Rust on Ubuntu, native
Windows MSVC Rust build and tests, Android build and tests, and Windows desktop
packaging. A GitHub remote and runner are not configured in this local
environment, so the workflow has not executed on GitHub. The workflow runs the
same commands that were exercised locally. Recorded limitation: remote CI
execution is verified when the repository is pushed to its hosting remote.

## 6. Failure detection evidence

### Kotlin test failure

A temporary test asserted `false`. Command:

```text
.\gradlew.bat --console=plain :shared:model:cleanJvmTest :shared:model:jvmTest
```

Observed:

```text
Execution failed for task ':shared:model:jvmTest'.
> There were failing tests.
BUILD FAILED
exit code 1
```

The temporary test was removed, the task was rerun, and it returned exit 0.

### Rust test failure

A temporary integration test asserted `false`. Command:

```text
cargo test -p nexus-types
```

Observed:

```text
test deliberately_fails ... FAILED
test result: FAILED. 0 passed; 1 failed
error: test failed
exit code 101
```

The temporary test was removed and `cargo test -p nexus-types` returned exit 0.

These are the same commands used by the Android and Rust CI jobs, which proves
that a compilation or test failure produces a non-zero job result once CI runs
on a hosted runner.

## 7. Repository hygiene and secret safety

```text
python scripts/check_repo_hygiene.py
```

Result: pass. The scanner checks:

- forbidden names such as `local.properties`, `.env`, signing properties, and
  private-key files;
- forbidden suffixes such as keystores, certificates, APKs, AABs, MSIs, and EXEs;
- build and local-state directories;
- private key blocks and common provider token shapes;
- required `.gitignore` entries.

`local.properties` exists on the development machine but is ignored by Git and
excluded from the scan. No API key, private key, keystore, or signing material is
committed. Release signing keys are supplied only by the release environment.

## 8. Dependency, advisory, and clean-checkout evidence

### Dependency policy

```text
cargo deny check
```

Result: exit 0.

```text
advisories ok, bans ok, licenses ok, sources ok
```

The realized graph contains 34 packages: 22 external crates and 12 private
workspace crates. External license expressions observed:

| License expression | Packages |
|---|---:|
| `MIT OR Apache-2.0` | 11 |
| `MIT` | 7 |
| `Unlicense OR MIT` | 2 |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | 1 |
| `Apache-2.0 OR MIT` | 1 |

Policy adjustments required for an unpublished workspace:

- `[licenses] private = { ignore = true }` because workspace crates use
  `publish = false` and do not carry an SPDX license expression yet.
- `[bans] allow-wildcard-paths = true` because private path dependencies have no
  registry version requirement. Wildcard registry requirements remain denied.
- `unused-allowed-license = "allow"` keeps the approved license allowlist for
  future dependencies without warning when an approved license is not present.
- All observed licenses are inside the allowlist. No exception or clarification
  was needed.

### Advisory scan

```text
cargo audit
```

Result: exit 0.

```text
Loaded 1490 security advisories
Scanning Cargo.lock for vulnerabilities (34 crate dependencies)
```

No vulnerability was reported. `cargo deny` also reported `advisories ok` for the
same lockfile.

### Clean checkout

After the initial commit, the clean-checkout proof clones `HEAD` and runs the
complete local gate, including Android assembly and Windows packaging:

```powershell
.\scripts\verify-clean-checkout.ps1
```

Result: PASS.

The script cloned commit `526f83c810157d405233f51661aa70c44c342595`
to `C:\Users\Elsa\AppData\Local\Temp\nexus-clean-7393af63`, ran the full gate,
and returned exit 0. Gradle reported `BUILD SUCCESSFUL in 42s`, with
`:apps:androidApp:assembleDebug` and `:apps:desktopApp:createDistributable`
executed. All Kotlin and Rust tests, documentation tests, and the repository
hygiene scan passed. The clone was removed after the run.

`local.properties` is not committed. The clone's `scripts\dev-env.ps1` recreates
it from the documented local SDK location, so the build depends only on
committed source and the pinned toolchain. This proves that a clean checkout can
be built using the documented steps.

A later documentation-only commit records this result; it does not change build
inputs.

## 9. Known limitations

- Remote GitHub Actions execution is not available in this local environment. The
  workflow and its commands are defined and locally exercised.
- The local Rust host is GNU because the machine lacks MSVC C++ tools and the
  Windows SDK. CI and release use MSVC. This does not change the C ABI. The
  `rust-windows` CI job compiles and tests the MSVC target on `windows-latest`,
  but it could not be executed in this local environment.
- The Windows app image is not an installer. Inno Setup packaging is Stage 19.
- Android Rust cross-compilation and `.so` packaging are not wired in Stage 03.
  The NDK and targets are ready for the native packaging stage.
- Gradle configuration cache is disabled because Spotless 8.11.0 cannot
  serialize its line-ending policy under Gradle 9.9.1.
- Gradle dependency locking and verification metadata are required before the
  first signed release, as recorded in ADR-0009 and the dependency inventory.
- Kotlin common tests are deferred; Stage 03 uses JVM tests.
- Product features and real screens begin in Stage 04.

## 10. Reproducing this stage

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
.\scripts\setup-windows.ps1
. .\scripts\dev-env.ps1
.\scripts\build.ps1
python scripts\check_repo_hygiene.py
python scripts\check_commit_message.py "feat(build): scaffold nexus repository"
```

After committing:

```powershell
.\scripts\verify-clean-checkout.ps1
```

## 11. Next stage

Stage 04 establishes the UI foundation and design system. It must not start
until Stage 03 is reviewed and the project owner explicitly approves it.
