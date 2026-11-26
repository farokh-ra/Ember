# Compatibility matrix - Ember 0.1.0

**Updated:** 2026-11-26
**Evidence scope:** Windows x64 local build and runtime; Android build and static
packaging checks only.

Compatibility claims are separated into three states:

- **Verified** - built and exercised in this environment.
- **Built, not run** - the artifact builds and passes static checks, but no
  compatible runtime was available.
- **Unsupported** - deliberately outside the 0.1.0 scope.

## 1. Windows

| Item | State | Evidence |
|---|---|---|
| Windows 10 x64 22H2 (19045) | **Verified** | MSI install, launch, portable ZIP launch, upgrade, and uninstall. |
| Windows 11 x64 | Built, not run | Same jpackage runtime and WiX 3 MSI; no Windows 11 machine available. |
| Windows 10/11 on ARM64 | Unsupported | Artifacts target x64 only. |
| Windows Server | Unsupported | No desktop session requirement is tested. |
| Per-user installation without elevation | **Verified** | Installed under `%LOCALAPPDATA%\Ember` with `msiexec /qn`. |
| Upgrade 0.1.0 -> 0.1.1 | **Verified** | Stable upgrade UUID; the newer MSI replaced the older product and kept the uninstall entry. |
| Uninstall | **Verified** | Exit code 0, install directory removed, uninstall entry removed. |
| Portable ZIP | **Verified** | Extracted and launched from a temporary directory. |
| Bundled JDK 21 runtime | **Verified** | The installed app runs without a system Java installation. |
| MSVC release build | Built in CI, not run locally | The local machine uses the GNU fallback; the release workflow uses MSVC. |

Installer details:

| Property | Value |
|---|---|
| Installer type | MSI produced by jpackage with WiX 3.14.1 |
| Scope | Per user |
| Default directory | `%LOCALAPPDATA%\Ember` |
| Upgrade UUID | `c7c225ee-ae2c-4c58-822d-ea7bda7ac48c` |
| Uninstall product code for 0.1.0 | `{E1C5024B-A3A9-3671-831B-D9CEFC767181}` |

## 2. Android

| Item | State | Evidence |
|---|---|---|
| Android 9 (API 28) and later | Built, not run | `minSdk` 28; no device or emulator. |
| Android 9-16 runtime behaviour | Built, not run | No runtime testing. |
| arm64-v8a devices | Built, not run | `libnexus_core.so` present in the APK and AAB. |
| x86_64 emulator | Built, not run | `libnexus_core.so` present in the APK and AAB. |
| armeabi-v7a / x86 | Unsupported | The Rust core is packaged for arm64-v8a and x86_64 only. |
| APK sideload | Built, not run | Signed with the test key and verified with `apksigner`. |
| Play delivery (AAB) | Built, not run | Signed bundle verified with `jarsigner`; no Play Console upload. |
| Runtime permissions | Built, not run | `INTERNET`, `ACCESS_NETWORK_STATE`, and `CHANGE_WIFI_MULTICAST_STATE` are declared. |
| Low-memory devices | Not tested | No 2-4 GB device available. |
| Doze, background limits, battery optimisation | Not tested | No device. |

Target SDK is 37. Google Play's current target-SDK requirement must be checked
again at release time; a Play upload was not attempted.

## 3. Network

| Environment | State |
|---|---|
| Windows loopback, multiple local peers | Verified in earlier stages (Stages 09-18). |
| Direct TCP and QUIC connections | Verified on loopback. |
| Relay fallback and hole punching | Verified on loopback in Stage 11. |
| Public internet bootstrap and DHT | Not verified in this environment. |
| Poor Wi-Fi, packet loss, high latency | Not tested. No network shaping was available. |
| Mobile data and carrier NAT | Not tested. |
| IPv4-only, IPv6-only, dual stack | Not tested end to end. |
| Network change during transfer | Not tested on a real network; scheduler failover is covered by simulation. |

## 4. Storage and data

| Item | State |
|---|---|
| SQLite schema v11 | Verified by Rust tests and migrations. |
| Upgrade from schema 10 to 11 | Verified in Stage 18 tests. |
| Import of a pre-Stage-18 archive | Verified. |
| Windows uninstall preserves user data | **Not applicable yet**: the packaged UI has no core data directory. The install directory is removed on uninstall. Before the UI is wired to the core, user data must live outside the install directory and uninstall must offer a keep/remove choice. |
| Android backup rules | Not verified on device. |

## 5. Toolchain compatibility

| Component | Pin | Notes |
|---|---|---|
| JDK | Temurin 21.0.13+9 | Bundled runtime for Windows; CI uses Temurin 21. |
| Gradle | 9.9.2 | Wrapper committed. |
| Kotlin | 2.4.21 | |
| Compose Multiplatform | 1.12.2 | |
| Android Gradle Plugin | 9.5.2 | |
| Android build tools | 36.0.0 | |
| Android NDK | 29.0.14206865 | Android Rust cross-build. |
| Rust | 1.99.1, edition 2024 | Release uses `--locked`. |
| WiX | 3.14.1.8722 | Downloaded and hash-verified by `scripts/fetch-wix.ps1`. |

## 6. Release-blocking gaps

The following gaps block a public release even though the stage deliverables are
present:

1. No Android runtime, low-memory device, or battery evidence.
2. No real-network, poor-network, mobile, or IPv6 evidence.
3. Production signing keys and certificates were not available; artifacts use
   ephemeral test signing.
4. No Windows 11 machine was tested.
5. The packaged UI is still a demo shell, so end-to-end product flows are not
   part of this compatibility claim.
6. Third-party notices and a CycloneDX SBOM are not generated yet. Both are
   required before a public release.

See [STAGE_19.md](../progress/STAGE_19.md) for the full evidence record.
