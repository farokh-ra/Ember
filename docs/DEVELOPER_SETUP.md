# NEXUS Developer Setup

This guide makes a clean Windows checkout build with the pinned toolchain. The
setup is portable, per-user, and does not require administrator rights. The same
commands are used locally and in CI.

## 1. Supported environment

| Item | Requirement |
|---|---|
| Operating system | Windows 10 or 11 x64 for both applications. CI also builds Android and Rust on Ubuntu. |
| Shell | Windows PowerShell 5.1 or PowerShell 7. |
| Git | 2.56 or later. |
| Android SDK | Command-line tools installed at `%LOCALAPPDATA%\Android\Sdk`, with accepted licenses. |
| Disk space | About 6 GB for the portable toolchain, Gradle caches, Rust target output, and build output. |
| Network | Required for the first setup and the first dependency resolution. |

No system-wide JDK, Gradle, Cargo, Rust, NDK, or MinGW installation is required.
Do not install a different JDK or Gradle on `PATH` and expect the build to use it;
`dev-env.ps1` puts the pinned tools first.

## 2. Install the Android command-line tools

The Android SDK is the only manual prerequisite because its license must be
accepted interactively. If `android.exe` already exists under
`%LOCALAPPDATA%\Android\Sdk\cmdline-tools\latest\bin`, skip this section.

1. Download the current "Command line tools only" package for Windows from
   <https://developer.android.com/studio#command-line-tools-only>.
2. Extract it so the layout is:

   ```text
   %LOCALAPPDATA%\Android\Sdk\cmdline-tools\latest\bin\android.exe
   %LOCALAPPDATA%\Android\Sdk\cmdline-tools\latest\bin\sdkmanager.bat
   ```

3. Accept the SDK licenses:

   ```powershell
   $sdk = "$env:LOCALAPPDATA\Android\Sdk"
   & "$sdk\cmdline-tools\latest\bin\sdkmanager.bat" --sdk_root=$sdk --licenses
   ```

The current Android CLI replaces the deprecated `sdkmanager` interface. Use
`android sdk install "<package>"` for new automation; `setup-windows.ps1`
supports both. Package identifiers use a slash, for example
`platforms/android-37.0`, not the old semicolon form.

## 3. Automated setup

If PowerShell blocks local scripts, allow them for the current process only:

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
```

Do not weaken the machine-wide policy just for this repository. Then run:

```powershell
.\scripts\setup-windows.ps1
```

The script:

1. Downloads JDK 21.0.13+9, Gradle 9.9.1, Rust 1.99.1, the Android NDK r29, and
   the Android platform/build-tools if missing.
2. Verifies every downloaded archive against a pinned SHA-256 checksum.
3. Detects a complete Visual Studio C++ workload through `vswhere.exe`.
4. Installs portable MinGW-w64 GCC 16.2.0 only when MSVC C++ tools are absent.
5. Installs the Rust `aarch64-linux-android` and `x86_64-linux-android` targets,
   plus Clippy and rustfmt.
6. Installs the Android platform `android-37.0` and build-tools `36.0.0` if they
   are missing.

Everything is installed under `%LOCALAPPDATA%\NEXUS\toolchain`:

```text
%LOCALAPPDATA%\NEXUS\toolchain
├── jdk\jdk-21.0.13+9
├── gradle\gradle-9.9.1
├── mingw\mingw64
├── rustup
├── cargo
└── downloads
```

`setup-windows.ps1` is idempotent. Use `-Force` only when you intentionally want
to re-download and reinstall a component.

### MSVC versus MinGW

CI and release builds use the Microsoft toolchain:

```text
x86_64-pc-windows-msvc
```

A local machine without the Visual Studio C++ workload and Windows SDK cannot
link MSVC binaries. In that case the script installs portable MinGW-w64 and uses:

```text
x86_64-pc-windows-gnu
```

This is a developer fallback, not a change to the release target. It is recorded
in [ADR-0009](decisions/ADR-0009-build-packaging-dependency-policy.md). If a
complete MSVC installation is available, `dev-env.ps1` uses it by default and the
MinGW fallback is not activated. To force MSVC on a machine that has both:

```powershell
. .\scripts\dev-env.ps1 -UseMsvc
```

To build a truly MSVC-compatible Windows release, use the CI workflow or a
machine with the Visual Studio C++ workload and Windows SDK 10.0.26100.

## 4. Load the environment

The environment changes apply to the current PowerShell process only:

```powershell
. .\scripts\dev-env.ps1
```

This sets `JAVA_HOME`, `GRADLE_HOME`, `ANDROID_HOME`, `ANDROID_SDK_ROOT`,
`ANDROID_NDK_HOME`, `CARGO_HOME`, and `RUSTUP_HOME`; prepends the pinned tools to
`PATH`; selects the Rust host toolchain; and writes `local.properties`.

Open a new shell and dot-source the script again in every new session. The script
does not modify the user or machine environment, so nothing leaks into unrelated
projects.

Verify the active versions:

```powershell
java -version
gradle --version
rustc --version --verbose
cargo --version
python --version
```

Expected highlights:

```text
openjdk version "21.0.13" 2026-10-20 LTS
Gradle 9.9.1
rustc 1.99.1
cargo 1.99.1
```

`rustc --version --verbose` reports either `host: x86_64-pc-windows-msvc` in CI
or `host: x86_64-pc-windows-gnu` on the local fallback. Both compile the same
Rust source.

## 5. Build and test

```powershell
.\scripts\build.ps1
```

This runs, in order:

1. Spotless formatting check.
2. detekt static analysis.
3. Android debug APK assembly.
4. Windows desktop application image assembly.
5. Shared Kotlin JVM tests.
6. Rust formatting, Clippy, and workspace tests.
7. Repository hygiene and secret scanning.

Useful narrower commands:

```powershell
# Kotlin only
.\scripts\build.ps1 -SkipRust -SkipHygiene

# Rust only
.\scripts\build.ps1 -SkipGradle -SkipHygiene

# Add dependency and advisory gates
.\scripts\build.ps1 -WithDependencyScan
```

`-WithDependencyScan` requires `cargo-deny` and `cargo-audit`:

```powershell
cargo install cargo-deny --version 0.20.2 --locked
cargo install cargo-audit --version 0.23.0 --locked
```

### Artifacts

| Artifact | Path |
|---|---|
| Android debug APK | `apps/androidApp/build/outputs/apk/debug/androidApp-debug.apk` |
| Windows app image | `apps/desktopApp/build/compose/binaries/main/app/NEXUS` |
| detekt reports | `build/reports/detekt/` |
| Kotlin test reports | `shared/*/build/reports/tests/jvmTest/` |
| Rust test output | Console, or `cargo test --workspace` |

The Windows app image is not an installer. Inno Setup packaging is Stage 19.

## 6. Clean-checkout proof

The exit criterion for Stage 03 is that a clean checkout builds with documented
steps. After committing your work:

```powershell
.\scripts\verify-clean-checkout.ps1
```

The script clones `HEAD` into a temporary directory with `git clone --local`,
runs `scripts\build.ps1` there, and removes the clone unless `-Keep` is passed.
This proves the build does not depend on ignored files, local caches, or
uncommitted changes.

## 7. Manual setup

Use this path only when the automated script cannot run, for example on a locked
down machine.

| Tool | Version | Source |
|---|---|---|
| Eclipse Temurin JDK | 21.0.13+9 | <https://github.com/adoptium/temurin21-binaries/releases> |
| Gradle | 9.9.1 | <https://services.gradle.org/distributions/gradle-9.9.1-bin.zip> |
| Kotlin | 2.4.21 | Resolved by Gradle from Maven Central and Google Maven. |
| Rust | 1.99.1 | rustup, channel pinned by `rust-toolchain.toml`. |
| Android NDK | 29.0.14206865 | Android SDK package `ndk/29.0.14206865`. |
| Android platform | 37.0 | Android SDK package `platforms/android-37.0`. |
| Android build tools | 36.0.0 | Android SDK package `build-tools/36.0.0`. |
| MinGW-w64 GCC | 16.2.0 | <https://github.com/brechtsanders/winlibs_mingw/releases> only if MSVC C++ tools are absent. |

Set the environment variables from section 4, then run:

```powershell
.\gradlew.bat --no-daemon --console=plain spotlessCheck detekt
.\gradlew.bat --no-daemon --console=plain :apps:androidApp:assembleDebug
.\gradlew.bat --no-daemon --console=plain :apps:desktopApp:createDistributable
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Do not commit `local.properties`; it is machine-specific and ignored.

## 8. IDE notes

- **Android Studio / IntelliJ IDEA:** open the repository root as a Gradle
  project. Select the pinned JDK 21 as the Gradle JDK. Let the IDE use the
  Gradle wrapper, not a bundled Gradle distribution.
- **Rust:** install the Rust plugin and point it at the same rustup home and
  toolchain. On the MinGW fallback, set the project toolchain to
  `1.99.1-x86_64-pc-windows-gnu`.
- **Compose:** the Compose Multiplatform plugin supplies the desktop preview
  tooling. Shared UI lives in `shared/ui`; platform entry points stay thin.
- **Line endings:** `.gitattributes` enforces LF for source and CRLF for
  PowerShell and batch files. Do not disable `core.autocrlf=false` in this
  repository.

## 9. Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `cannot be loaded because running scripts is disabled` | Allow scripts for the current process: `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass`, then run the script. Do not change the machine-wide policy. |
| `JAVA_HOME is not set` or Gradle picks another JDK | Dot-source `scripts\dev-env.ps1` in the current shell. Close terminals that still hold an older `JAVA_HOME`. |
| `SDK location not found` | Confirm `%LOCALAPPDATA%\Android\Sdk\platforms\android-37.0` exists, then re-run `dev-env.ps1` to rewrite `local.properties`. |
| `linker 'link.exe' not found` | The MSVC C++ workload is missing. Re-run `setup-windows.ps1` for the MinGW fallback, or install the Visual Studio C++ workload and Windows SDK and use `-UseMsvc`. |
| `linker 'gcc.exe' not found` | MinGW is not on `PATH`. Dot-source `dev-env.ps1` and confirm `%LOCALAPPDATA%\NEXUS\toolchain\mingw\mingw64\bin\gcc.exe` exists. |
| `sdkmanager` reports deprecation | Expected. Use `android sdk install`, or the fallback in `setup-windows.ps1`. |
| Native tool output appears as a PowerShell error record | Windows PowerShell 5.1 renders native stderr as `NativeCommandError` even when the tool succeeds. The NEXUS scripts check the process exit code and fail only on a non-zero result. Treat exit code 0 as success. |
| `Package ndk not found` with old syntax | Use `android sdk install "ndk/29.0.14206865"`; the semicolon form is from the deprecated CLI. |
| Spotless configuration-cache failure | Configuration cache is intentionally disabled in `gradle.properties` because Spotless 8.11.0 cannot serialize its line-ending policy under Gradle 9.9.1. Do not re-enable it until the formatter supports the combination. |
| Gradle runs out of memory | The root build sets `-Xmx4g`. Close other Gradle daemons with `.\gradlew.bat --stop`. |
| Kotlin tests are not found | Tests live in `src/jvmTest`, not `src/commonTest`, in Stage 03. Add new tests there unless a later stage enables Android host or common tests. |
| Material3 version looks older than Compose | Compose Multiplatform 1.12.2 resolves Material3 1.9.0. That is the current compatible Material3 line and is pinned explicitly in the version catalog. |
| Rust dependency scan tools are missing | Install the pinned versions shown in section 5, then re-run with `-WithDependencyScan`. |
| Android license error in CI or locally | Run `sdkmanager --licenses`, accept all listed licenses, and verify the SDK path. |
| Corporate proxy blocks downloads | Configure `HTTPS_PROXY` and `NO_PROXY` for the shell before running setup. Checksums still apply. |
| Clean-checkout script clones old code | It clones committed `HEAD`. Commit the intended changes first, or pass `-Ref <commit>`. |

## 10. Security and local state

- `local.properties`, `.gradle`, `build`, `target`, keystores, signing
  properties, and `.env` files are ignored.
- Never put a real API key, private key, recovery phrase, or signing password in
  a source file, test, issue, or log.
- Debug logs are opt-in and redacted by policy. See
  [LOGGING_POLICY.md](LOGGING_POLICY.md).
- `scripts/check_repo_hygiene.py` scans tracked and untracked non-ignored files
  for forbidden file types and common credential patterns. It runs locally and
  in CI.

## 11. Getting help

If setup still fails, collect:

1. The exact command and full error.
2. `java -version`, `gradle --version`, `rustc --version --verbose`, and
   `python --version`.
3. Whether `vswhere.exe` reports a C++ workload.
4. Redacted output of `.\gradlew.bat --version`.

Do not include tokens, private keys, keystores, personal file paths, or
unredacted logs. Open a build issue with the bug report form.
