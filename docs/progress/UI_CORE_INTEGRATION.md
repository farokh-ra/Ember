# UI and Core Integration: Slice 0 Evidence

**Status:** Slice 0 complete on `feat/21-core-integration`; slices 1-9 not
started
**Branch:** `feat/21-core-integration`, based on `feat/19-release`
**Last updated:** 2026-11-28

Stage 20 closed the repository roadmap. It did not connect the packaged UI to
the Rust core: the screens still render demo data and the shells never created a
`NexusCore`. This document tracks the follow-up integration work, starting with
the lifecycle slice.

## 1. Slice plan

| Slice | Scope | Status |
|---|---|---|
| 0 | Core facade, lifecycle, and sealed local secret in both shells | Complete |
| 1 | Real identity creation and onboarding | Not started |
| 2 | File import and library list | Not started |
| 3 | Publish and file details | Not started |
| 4 | Explore from real published records | Not started |
| 5 | Download start, pause, resume, cancel, and progress | Not started |
| 6 | Hosting controls | Not started |
| 7 | Search against real results | Not started |
| 8 | Network status and settings from core state | Not started |
| 9 | Packaged end-to-end flow on Windows, then Android | Not started |

## 2. What slice 0 delivers

| Change | Location |
|---|---|
| Shell-facing core status model | `shared/core-api/.../CoreStatus.kt` |
| Fallible process-level lifecycle owner | `shared/core-client/.../CoreHost.kt` |
| Platform secret store contract | `shared/core-client/.../LocalSecretStore.kt` |
| Windows DPAPI store | `shared/core-client/.../DpapiSecretStore.kt` |
| Android Keystore store | `shared/core-client/.../AndroidKeystoreSecretStore.kt` |
| Desktop shell creates, starts, and closes the core | `apps/desktopApp/.../Main.kt` |
| Android application owns the core across activity recreation | `apps/androidApp/.../EmberApplication.kt` |
| Settings reports real core availability and protocol version | `shared/ui/.../SettingsScreen.kt`, `LocalCoreStatus.kt` |

Behaviour:

- A missing native library, malformed ABI, unavailable secure store, or failed
  start leaves the shell running with an explicit status instead of crashing.
- The 32-byte local secret is sealed by DPAPI on Windows and by a
  non-exportable Android Keystore key on Android. There is no plaintext
  fallback.
- Desktop uses `%LOCALAPPDATA%\Ember\data`; Android uses `filesDir`, matching
  `ARCHITECTURE.md`.
- Networking is disabled in both shells until slice 4 needs real peer state.
- Screens still show demo content. Only the core status is real.

## 3. Verification

Local gate on 2026-11-28:

| Check | Result |
|---|---|
| `.\scripts\build.ps1 -WithDependencyScan` | Passed |
| Rust tests | 380 passed, 0 failed |
| `cargo fmt`, `cargo clippy -D warnings`, `cargo deny`, `cargo audit` | Passed |
| Native builds | Windows x64 DLL, Android arm64-v8a and x86_64 `.so`; 19 required exports present |
| `spotlessCheck` and `detekt` | Passed |
| `:apps:androidApp:assembleDebug` | Passed |
| `:apps:desktopApp:createDistributable` | Passed |
| New tests | `CoreHostTest` 5 tests; controller core-status test |
| CycloneDX SBOM | 1,133 components (399 Cargo, 734 Maven) |
| Repository hygiene | 385 files scanned; passed |

Dependency changes: `apps/desktopApp/gradle.lockfile` adds
`net.java.dev.jna:jna:5.19.1` because the desktop shell now depends on
`core-client`. `gradle/verification-metadata.xml` already covered JNA.

## 4. Not verified

- The DPAPI and Android Keystore paths compile but were not exercised on a clean
  machine, an Android device, or an emulator in this slice.
- No packaged desktop run created an identity, because identity onboarding is
  slice 1.
- No Android runtime, upgrade, Doze, low-memory, or soak evidence exists.
- The core status label reports only lifecycle. It does not imply that any
  network, publication, or transfer feature works.

## 5. Next slice

Slice 1 adds typed identity operations over `CoreHost.invoke` and wires the
welcome and identity screens to `identity.create`, `identity.status`, restore,
and reset. It removes demo onboarding from the shipped path. The demo catalogue
stays until each screen has a real data source.
