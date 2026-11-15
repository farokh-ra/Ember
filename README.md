# Ember

Ember is a local-first file sharing application for Android and Windows. It uses
a shared Kotlin Multiplatform and Compose Multiplatform interface, a shared Rust
core, and peer-to-peer networking without a mandatory central account or content
service.

Ember is the product name. Internal identifiers keep the historical `nexus`
name for ABI and format stability: Kotlin packages (`io.nexus`), Rust crates
(`nexus-*`), the native library (`nexus_core`), and signed record tags
(`NEXUS-*`).

This repository is at the end of **Stage 08: C ABI, JNI/JNA adapters,
lifecycle, and streaming**. The Android and Windows shells still run the shared,
clickable UI prototype with visibly labelled mock data. Underneath it, the Rust
core now owns an embedded SQLite schema, migrations, a content-addressed managed
file store, quota accounting, reference-counted cleanup, startup reconciliation,
metadata export/restore, local Ed25519 identities, stable `nx1...` identity IDs,
canonical signed records, encrypted identity storage, passphrase-encrypted
backups, streaming content IDs, chunk manifests, publication revisions, signed
tombstones, and the versioned native boundary. A C harness, Android JNI shim,
and Windows JNA adapter exercise the same lifecycle, dispatch, event,
cancellation, and streaming contract. Networking and hosting remain later
stages.

## Current status

| Area | State |
|---|---|
| Repository and module structure | Complete |
| Gradle Kotlin Multiplatform and Compose build | Complete |
| Android debug application | Builds and runs the shared UI |
| Windows desktop application image | Builds and runs the shared UI |
| Rust workspace and library | Builds and tests |
| Design system and component library | Complete |
| Responsive navigation shell | Complete; side rail and bottom bar |
| Clickable screen prototypes | Complete; eleven screens with demo data |
| Dark, light, and system themes | Complete; selection persists |
| Loading, empty, offline, error, and retry states | Complete on data-driven screens |
| Accessibility and keyboard support | Implemented; focus, keyboard, and Escape checks passed |
| Stage 04 desktop test pass | Complete; journeys, five states, themes, and compact layout verified |
| SQLite schema and transactional migrations | Complete; schema version 4 |
| Managed content-addressed file store | Complete; atomic import, no-copy staged adoption, and exclusive lock |
| Quota, free-space reserve, and file limits | Complete; 10 GiB default, 1 GiB reserve, 4 GiB file cap |
| Reference-counted deletion and cleanup | Complete; published originals and replicas require confirmation |
| Crash reconciliation and corrupt-file quarantine | Complete; verified by restart tests |
| Metadata export and validated restore | Complete; CBOR manifest plus SQLite snapshot |
| Local Ed25519 identity and stable identity ID | Complete; Bech32m `nx1...` IDs and committed vectors |
| Canonical CBOR and signed records | Complete; exact-byte envelopes, profile and publication v1 |
| Streaming content IDs and chunk manifests | Complete; raw SHA-256, fixed 1 MiB chunks, signed manifest roots |
| Publication revisions and signed tombstones | Complete; immutability, attribution, and revision rules |
| Encrypted identity at rest and backup | Complete; LEK/HKDF at rest, Argon2id/XChaCha20-Poly1305 backup |
| C ABI and versioned CBOR envelopes | Complete; 17 functions, opaque handles, panic containment |
| Core lifecycle, dispatch, and event queue | Complete; six methods, four workers, coalescing queue |
| Streaming import/export through the ABI | Complete; 1 MiB chunks, staged adoption, bounded export |
| JNI and JNA adapters | Complete on Windows JNA and a Windows-hosted JNI proxy; Android runtime execution deferred |
| C harness and adapter contract tests | Complete; 100 lifecycle cycles and 2.5 MB round-trip |
| Formatting, linting, and static analysis | Enforced locally and in CI |
| CI workflow and dependency review | Defined |
| Developer setup and contribution guides | Complete |
| Product features | Mock UI prototypes; storage, crypto, publication, and native foundations complete, networking starts in Stage 09 |

See [docs/progress/STAGE_08.md](docs/progress/STAGE_08.md) for the exact
evidence and commands used to verify this stage. The byte-level record, identity,
manifest, tombstone, at-rest, and backup formats are specified in
[docs/architecture/SIGNED_RECORD_FORMATS.md](docs/architecture/SIGNED_RECORD_FORMATS.md),
and the native boundary is specified in
[docs/architecture/FFI_INTERFACE.md](docs/architecture/FFI_INTERFACE.md).

## Repository layout

| Path | Purpose |
|---|---|
| `apps/androidApp` | Android shell and platform integration. |
| `apps/desktopApp` | Windows desktop shell and packaging. |
| `shared/model` | Serializable DTOs, IDs, events, and redaction helpers. |
| `shared/core-api` | Platform-independent `NexusCore` contract and fake core. |
| `shared/core-client` | Native JNI/JNA bridge, CBOR codec, event pump, and typed `NexusCore` client. |
| `shared/presentation` | View models, state reduction, and screen models. |
| `shared/ui` | Shared Compose screens, components, and theme. |
| `crates/nexus-*` | Twelve Rust crates for types, crypto, protocol, identity, store, transfer, networking, search, core, FFI, observability, and testkit. |
| `docs` | Architecture, decisions, setup, workflow, logging, and stage records. |
| `scripts` | Portable setup, build, hygiene, and clean-checkout verification. |
| `.github` | CI, dependency review, Dependabot, and contribution templates. |

The module and crate responsibilities are defined in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Do not move responsibilities across
those boundaries without an architecture decision record.

## Quick start on Windows

Prerequisites:

- Windows 10 or 11 x64.
- PowerShell 5.1 or later.
- Git 2.56 or later.
- Android SDK command-line tools with accepted licenses at
  `%LOCALAPPDATA%\Android\Sdk`.

From a fresh clone:

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
.\scripts\setup-windows.ps1
. .\scripts\dev-env.ps1
.\scripts\build.ps1
```

The execution-policy command allows local scripts for the current PowerShell
process only; it does not change the machine policy.

`setup-windows.ps1` installs the pinned portable toolchain under
`%LOCALAPPDATA%\NEXUS\toolchain`. It does not require administrator rights and
does not modify machine-wide environment variables. `dev-env.ps1` configures the
current PowerShell process only.

The full guide, including the manual path and troubleshooting, is
[docs/DEVELOPER_SETUP.md](docs/DEVELOPER_SETUP.md).

## Direct commands

After `. .\scripts\dev-env.ps1`:

```powershell
# Formatting, static analysis, native build, Android, desktop, and shared tests
.\scripts\build.ps1

# Native artifacts, export checks, and the C harness only
.\scripts\build-native.ps1 -Target All -Configuration Release

# Individual Gradle checks
.\gradlew.bat spotlessCheck detekt
.\gradlew.bat :apps:androidApp:assembleDebug
.\gradlew.bat :apps:desktopApp:createDistributable
.\gradlew.bat :shared:model:jvmTest :shared:core-api:jvmTest `
    :shared:core-client:jvmTest :shared:presentation:jvmTest :shared:ui:jvmTest

# Rust checks
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo audit
```

Artifacts:

- Android debug APK: `apps/androidApp/build/outputs/apk/debug/androidApp-debug.apk`
- Windows application image: `apps/desktopApp/build/compose/binaries/main/app/Ember`
- Windows native core: `build/native/windows-x64/nexus_core.dll`
- Android native cores: `build/native/android-arm64-v8a/libnexus_core.so` and `build/native/android-x86_64/libnexus_core.so`

The Windows image is not yet an installer. The Inno Setup per-user installer is
scheduled for Stage 19, as defined by ADR-0009.

## Pinned toolchain

| Tool | Pin |
|---|---|
| JDK | Eclipse Temurin 21.0.13+9 |
| Gradle | 9.9.1 (wrapper committed) |
| Kotlin | 2.4.21 |
| Compose Multiplatform | 1.12.2 |
| Compose Material3 | 1.9.0 |
| Android Gradle Plugin | 9.5.1 |
| Android compile and target SDK | 37 |
| Android minimum SDK | 28 |
| Android build tools | 36.0.0 |
| Android NDK | 29.0.14206865 |
| Rust | 1.99.1, edition 2024 |
| Windows Rust host | `x86_64-pc-windows-msvc` in CI and release; `x86_64-pc-windows-gnu` fallback for local non-admin machines without MSVC C++ tools |
| Inno Setup | 6.4.3 (later packaging stage) |

Version pins live in `gradle/libs.versions.toml`, `rust-toolchain.toml`, and
`Cargo.toml`. Do not change a pin without updating
[docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md](docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md).

## Quality gates

Every change must pass:

1. Repository hygiene: no secrets, keystores, signing material, or local paths.
2. Kotlin formatting with Spotless and ktlint 1.8.0.
3. Kotlin static analysis with detekt 1.23.8.
4. Native Windows and Android artifacts build, required exports are present, and
   the C harness passes.
5. Android debug assembly.
6. Windows desktop application image assembly.
7. Shared presentation and UI JVM tests, including the native adapter contract.
8. Rust formatting, Clippy with warnings denied, and workspace tests.
9. Rust dependency, license, and advisory policy checks.
10. Conventional Commit pull request titles.

The same commands are used by `.github/workflows/ci.yml`. See
[docs/DEVELOPMENT_WORKFLOW.md](docs/DEVELOPMENT_WORKFLOW.md) for the daily
workflow and release conventions.

## Documentation

| Document | Contents |
|---|---|
| [docs/PRODUCT_REQUIREMENTS.md](docs/PRODUCT_REQUIREMENTS.md) | Product scope and v1 requirements. |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System, module, crate, storage, security, and lifecycle design. |
| [docs/architecture/SIGNED_RECORD_FORMATS.md](docs/architecture/SIGNED_RECORD_FORMATS.md) | Exact v1 identity, CBOR, signed-record, manifest, tombstone, at-rest, and backup formats. |
| [docs/architecture/FFI_INTERFACE.md](docs/architecture/FFI_INTERFACE.md) | C ABI, JNI/JNA mapping, envelopes, events, streaming, and ABI compatibility. |
| [docs/progress/STAGE_08.md](docs/progress/STAGE_08.md) | Stage 08 scope, architecture, tests, and verification evidence. |
| [docs/decisions](docs/decisions/README.md) | Accepted architecture decision records. |
| [docs/DEVELOPER_SETUP.md](docs/DEVELOPER_SETUP.md) | Repeatable Windows setup and troubleshooting. |
| [docs/DEVELOPMENT_WORKFLOW.md](docs/DEVELOPMENT_WORKFLOW.md) | Branches, commits, pull requests, releases, and CI. |
| [docs/LOGGING_POLICY.md](docs/LOGGING_POLICY.md) | Secret-safe debug logging and redaction rules. |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution rules and definition of done. |
| [CHANGELOG.md](CHANGELOG.md) | User-visible changes. |

## Security

Do not commit API keys, private keys, keystores, signing certificates,
recovery phrases, or personal data. Report vulnerabilities privately as
described in [SECURITY.md](SECURITY.md); do not open a public issue with exploit
details. Debug logs must be redacted according to
[docs/LOGGING_POLICY.md](docs/LOGGING_POLICY.md).

## Next stage

Stage 09 adds QUIC/TCP connections, peer authentication, network-change
handling, and resource limits. It must not begin until Stage 08 is reviewed and
approved by the project owner. Android JNI runtime verification and remote CI
execution remain outstanding from Stage 08.
