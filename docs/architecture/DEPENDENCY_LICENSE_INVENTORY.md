# NEXUS Dependency and License Inventory

| Field | Value |
|---|---|
| Document ID | NEXUS-ARCH-DEPS-001 |
| Version | 1.5.0 |
| Status | Stage 08 C ABI, JNI, JNA, and native packaging dependencies realized; later-stage direct dependencies remain planned |
| Date | 2026-11-14 |
| Policy source | [ADR-0009](../decisions/ADR-0009-build-packaging-dependency-policy.md) |
| Architecture | [ARCHITECTURE.md](../ARCHITECTURE.md) |

## 1. Purpose and scope

This inventory records the direct runtime, test, and build dependencies planned for NEXUS v1, their pinned compatibility versions, licenses, platform use, purpose, and fallback. Stage 03 realizes the build-baseline dependencies needed by the empty shells and shared foundation. Stage 05 realizes the storage dependencies listed in section 4.1: bundled SQLite through `rusqlite`/`libsqlite3-sys`, CBOR metadata archives, SHA-256 hashing, typed errors, and managed-store locking. Stage 06 realizes the cryptographic primitives, canonical encoding, identity storage, and signed-record validation dependencies: Ed25519, Bech32m, XChaCha20-Poly1305, HKDF, Argon2id, NFC normalization, OS randomness, and secret zeroization. Stage 07 realizes the streaming manifest and verification path through `sha2` plus the existing `nexus-crypto` and `nexus-protocol` crates, and wires both into `nexus-transfer` and `nexus-store`. Stage 08 realizes the FFI boundary through the `jni` crate on both native targets and JNA 5.14.0 on the Windows JVM. Every other row in this table is planned for the stage that implements the owning feature and is not resolved by the current workspace. `Cargo.lock` contains 131 packages, including the twelve workspace crates and the transitive graphs of the realized dependencies.

The inventory covers:

- Rust core runtime and test crates.
- Kotlin, Compose, AndroidX, and JVM runtime libraries.
- Android and Windows toolchains.
- Build, packaging, test, and audit tools.
- Known transitive and platform-specific risks.

It does not list every transitive crate. `cargo-deny`, Gradle license reporting, and the CycloneDX SBOM are authoritative for the full graph at release time.

## 2. License policy summary

Allowed for distributed code: Apache-2.0, MIT, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, MPL-2.0 (unmodified, with notice), Unicode-3.0, OFL-1.1, and public domain.

Not allowed for linked or distributed code: GPL, AGPL, SSPL, and any license requiring NEXUS source disclosure or network-service obligations. LGPL is not allowed for linked code unless a future ADR approves dynamic linking and relinking rights. JNA is used under its Apache-2.0 option, not LGPL.

Build-only tools may use other licenses because they are not distributed with the application: Git, Inno Setup, MSVC/Windows SDK, Android SDK, and Python. The JDK runtime bundled by `jpackage` is distributed under GPLv2 with the Classpath Exception and requires notices and source availability.

## 3. Toolchain pins

| Tool | Pin | License | Use | Notes |
|---|---|---|---|---|
| JDK | Eclipse Temurin 21.0.13+9 | GPL-2.0-with-Classpath-Exception | Compile and bundle Windows runtime | Portable extraction; exact patch verified in Stage 03. |
| Gradle | 9.9.1 | Apache-2.0 | Build orchestration | Wrapper committed; no system Gradle required. |
| Kotlin | 2.4.21 | Apache-2.0 | Shared Kotlin and Compose code | Compose compiler aligned; AGP built-in Kotlin. |
| Compose Multiplatform | 1.12.2 | Apache-2.0 | Shared UI and desktop runtime | Runtime and foundation 1.12.2. |
| Compose Material3 | 1.9.0 | Apache-2.0 | Material components and theming | Current compatible Material3 line. |
| Android Gradle Plugin | 9.5.1 | Apache-2.0 | Android build | Paired with Gradle 9.9.1 and compile SDK 37. |
| Android compile SDK | 37 | Android SDK License | Compile Android app | Platform `android-37.0`; build-tools 36.0.0. |
| Android target SDK | 37 | Android SDK License | Runtime behavior target | Re-check Google Play requirement at each release. |
| Android minimum SDK | 28 | Android SDK License | Android 9 baseline | arm64-v8a required; x86_64 for emulator. |
| Android NDK | 29.0.14206865 | Android SDK License | Cross-compile Rust core | Installed and verified in Stage 03. |
| Rust | 1.99.1, edition 2024 | MIT OR Apache-2.0 | Core and headless node | `rust-toolchain.toml` pinned. |
| MSVC | 19.4x (VS 2022 17.14+ or VS 2026 18.x) | Proprietary | Windows Rust and packaging | CI and release only; not present on the Stage 03 local machine. |
| Windows SDK | 10.0.26100 | Proprietary | Windows native build and signing | CI and release only. |
| MinGW-w64 GCC | 16.2.0 | GPL-3.0-with-GCC-exception, runtime libs permissive | Local Windows Rust linker fallback | Build-only; not distributed. Used only when MSVC C++ tools are absent. |
| Inno Setup | 6.4.3 | Inno Setup License (freeware) | Per-user Windows installer | Build-time only; not linked or distributed. |
| Git | 2.56.0 | GPL-2.0-only | Version control | Build-only; not distributed. |
| Python | 3.11 | PSF-2.0 | Documentation validators and hygiene scripts | Build-only; not distributed. |
| Spotless | 8.11.0 | Apache-2.0 | Kotlin formatting | Build-only. |
| ktlint | 1.8.0 | Apache-2.0 | Kotlin lint rules | Build-only. |
| detekt | 1.23.8 | Apache-2.0 | Kotlin static analysis | Build-only. |
| cargo-ndk | 4.1.2 | Apache-2.0 OR MIT | Android Rust build | Planned for the native packaging stage; not active in Stage 03. |
| cargo-deny | 0.20.2 | MIT OR Apache-2.0 | License, advisory, and source gates | CI and release. |
| cargo-audit | 0.23.0 | Apache-2.0 OR MIT | Advisory scan | CI and release. |
| cargo-nextest | 0.9.151 | Apache-2.0 OR MIT | Rust test runner | CI. |
| CycloneDX tooling | Version pinned before release | Apache-2.0 | SBOM generation | CI and release. |

## 4. Rust direct dependencies

Stage 03 realizes `tracing` and `tracing-subscriber`; Stage 05 adds the storage
dependencies listed in section 4.1; Stage 06 adds the crypto, identity, and
signed-record dependencies; Stage 07 adds the streaming transfer and manifest
store wiring over those same crates; Stage 08 adds the `jni` shim. Every other
row in this table is planned for the stage that implements the owning feature and
is not resolved by the current workspace. `Cargo.lock` contains 131 packages,
including the twelve workspace crates and the transitive graphs of the realized
dependencies.

| Dependency | Pin | License | Platform | Purpose | Fallback |
|---|---|---|---|---|---|
| libp2p | 0.57.1 | MIT | Both | Transports, peer identity, DHT, request/response. | Hold 0.56 line; TCP-only configuration. |
| tokio | 1.54.1 | MIT | Both | Async runtime, timers, synchronization. | Bounded std threads with async bridge. |
| rusqlite | 0.42.0 | MIT | Both | SQLite access and migrations. | sqlx 0.9 with bundled SQLite. |
| libsqlite3-sys | 0.40.0 | MIT | Both | Bundled SQLite and FTS5. | System SQLite with verified FTS5. |
| serde | 1.0.229 | MIT OR Apache-2.0 | Both | Serialization traits and derives. | Manual codec at the boundary. |
| serde_bytes | 0.11.19 | MIT OR Apache-2.0 | Both | CBOR byte strings for IDs and hashes. | Manual byte-array wrapper. |
| ciborium | 0.2.2 | Apache-2.0 | Both | CBOR decoding and encoding. | minicbor after license review. |
| ed25519-dalek | 3.0.0 | BSD-3-Clause | Both | Ed25519 key generation, signing, verification. | 2.x line; wire-compatible signatures. |
| sha2 | 0.11.1 | MIT OR Apache-2.0 | Both | SHA-256 content, chunk, and manifest hashes. | ring SHA-256. |
| chacha20poly1305 | 0.11.0 | Apache-2.0 OR MIT | Both | Identity-at-rest and backup encryption. | AES-256-GCM with a new format version. |
| argon2 | 0.6.0 | MIT OR Apache-2.0 | Both | Backup passphrase KDF. | scrypt with a new format version. |
| hkdf | 0.13.0 | MIT OR Apache-2.0 | Both | Local-secret key derivation. | HMAC-SHA256 construction. |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | Both | Secret wiping. | Manual volatile wiping after review. |
| rand | 0.10.3 | MIT OR Apache-2.0 | Both | OS-backed key and nonce generation. | getrandom directly. |
| bech32 | 0.12.0 | MIT | Both | `nx1...` identity ID encoding and checksum. | Hex identity ID with a format version. |
| unicode-normalization | 0.1.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | Both | NFC normalization of user text and passphrases. | No normalization with a format version change. |
| tracing | 0.1.44 | MIT | Both | Structured, redacted logging. | log crate facade. |
| tracing-subscriber | 0.3.23 | MIT | Both | Log formatting and filtering. | Minimal custom logger. |
| tracing-appender | 0.2.5 | MIT | Both | Rotating log files. | Size-based rotation in observability. |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | Both | Typed internal errors. | Manual error enums. |
| parking_lot | 0.13.0 | MIT OR Apache-2.0 | Both | Bounded locks and runtime state. | std synchronization. |
| futures | 0.3.34 | MIT OR Apache-2.0 | Both | Stream and future combinators. | futures-util subset. |
| bytes | 1.12.1 | MIT | Both | Bounded network and transfer buffers. | Vec with explicit caps. |
| hex | 0.4.3 | MIT OR Apache-2.0 | Both | ID display, parsing, and test vectors. | Manual hex codec. |
| fs2 | 0.4.3 | MIT OR Apache-2.0 | Both | Store lock and free-space queries. | Platform-specific file lock and statvfs APIs. |
| jni | 0.21.1 | MIT OR Apache-2.0 | Both | JNI shim for the C ABI; compiled on Windows for the JVM proxy test and on Android for packaging. | JNA on Android. |

### 4.1 Stage 03, Stage 05, Stage 06, Stage 07, and Stage 08 locked Rust graph

| Direct crate | Version | License | Purpose |
|---|---|---|---|
| `tracing` | 0.1.44 | MIT | Structured event emission. |
| `tracing-subscriber` | 0.3.23 | MIT | Environment filter and stderr formatter. |
| `rusqlite` | 0.42.0 | MIT | SQLite access, transactions, backup API, and migrations. Enabled with `backup`, `bundled`, and `fallible_uint`. |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | CBOR manifest serialization traits and derives. |
| `ciborium` | 0.2.2 | Apache-2.0 | Versioned CBOR metadata manifests. |
| `ed25519-dalek` | 3.0.0 | BSD-3-Clause | Ed25519 key generation, signing, and strict verification. Enabled with `zeroize`. |
| `bech32` | 0.12.0 | MIT | Bech32m `nx1...` identity IDs. |
| `chacha20poly1305` | 0.11.0 | Apache-2.0 OR MIT | XChaCha20-Poly1305 identity and backup encryption. Enabled with `zeroize`. |
| `argon2` | 0.6.0 | MIT OR Apache-2.0 | Argon2id backup passphrase derivation. Enabled with `zeroize`. |
| `hkdf` | 0.13.0 | MIT OR Apache-2.0 | HKDF-SHA256 identity-at-rest key derivation. |
| `getrandom` | 0.3.4 | MIT OR Apache-2.0 | OS CSPRNG for seeds, salts, and nonces. Used directly instead of adding `rand`. |
| `unicode-normalization` | 0.1.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | NFC normalization of user text and passphrases. |
| `zeroize` | 1.9.1 | Apache-2.0 OR MIT | Secret zeroization for keys and derived material. |
| `hex` | 0.4.3 | MIT OR Apache-2.0 | ID display and committed test vectors. |
| `sha2` | 0.11.1 | MIT OR Apache-2.0 | SHA-256 content IDs, chunk and manifest hashes, archive hashes, migration checksums, and domain-separated hashes. |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | Typed crypto, identity, protocol, transfer, and storage errors with actionable context. |
| `fs2` | 0.4.3 | MIT OR Apache-2.0 | Exclusive managed-store lock and free-space reporting. |
| `jni` | 0.21.1 | MIT OR Apache-2.0 | Static JNI symbols, byte-array copies, and panic-guarded JNI entry points. |

`libsqlite3-sys` 0.40.0 is realized transitively and compiles the bundled SQLite amalgamation with FTS5. The SQLite graph adds `bitflags`, `fallible-iterator`, `fallible-streaming-iterator`, `smallvec`, and the build-only `cc`, `find-msvc-tools`, `pkg-config`, `shlex`, and `vcpkg` crates. The CBOR, serde, hashing, and error graphs add `ciborium-io`, `ciborium-ll`, `half`, `zerocopy`, `zerocopy-derive`, `serde_core`, `serde_derive`, `proc-macro2`, `quote`, `syn` 2 and 3, `unicode-ident`, `cfg-if`, `cpufeatures`, `digest`, `block-buffer`, `hybrid-array`, `typenum`, `const-oid`, `crypto-common`, `thiserror-impl`, `winapi`, and `winapi-x86_64-pc-windows-gnu`. The Stage 06 crypto graph adds `aead`, `chacha20`, `poly1305`, `cipher`, `cmov`, `crypto-common`, `curve25519-dalek`, `curve25519-dalek-derive`, `ed25519`, `signature`, `rand_core`, `blake2`, `password-hash`, `phc`, `universal-hash`, `subtle`, `inout`, `base64ct`, `hmac`, and `zeroize_derive`. The Stage 08 JNI graph adds `cesu8`, `combine`, `jni-sys` 0.3.1 and 0.4.1, `log`, `once_cell`, `thiserror` 1.0.69, and `windows-sys` 0.45.0. All are MIT, Apache-2.0, BSD, Zlib, or Unicode-3.0 licensed and are checked by `cargo-deny`. Run `cargo tree -p nexus-core -p nexus-ffi -p nexus-crypto -p nexus-identity -p nexus-protocol -p nexus-transfer -p nexus-store` for the exact realized graph.

## 5. Kotlin, Compose, and JVM dependencies

Stage 03 resolves Kotlin, Compose runtime/foundation/Material3, coroutines,
AndroidX activity/core/lifecycle, Kotlin test, JUnit Jupiter, and
coroutines-test. Stage 08 adds JNA 5.14.0 for the Windows adapter.
kotlinx-serialization-cbor, Turbine, and Compose UI test remain planned for later
stages and are not current build dependencies.

| Dependency | Pin | License | Platform | Purpose | Fallback |
|---|---|---|---|---|---|
| Kotlin standard library | 2.4.21 | Apache-2.0 | Both | Kotlin runtime. | Same Kotlin line only. |
| Compose Multiplatform | 1.12.2 | Apache-2.0 | Both | Shared UI and desktop runtime. | Previous stable Compose line. |
| Compose Material3 | 1.9.0 | Apache-2.0 | Both | Material components and theming. | Compose Material2 subset. |
| kotlinx-coroutines-core | 1.11.0 | Apache-2.0 | Both | Structured concurrency, flows, dispatchers. | Java executors and callbacks. |
| kotlinx-serialization-cbor | 1.13.0 | Apache-2.0 | Both | FFI envelope codec. | Hand-written CBOR codec. |
| JNA | 5.14.0 | LGPL-2.1-or-later OR Apache-2.0 | Windows | C ABI binding. | JNI on Windows. |
| androidx.activity:activity-compose | 1.13.0 | Apache-2.0 | Android | Compose activity host and result contracts. | Platform Activity integration without the Compose helper. |
| androidx.core:core-ktx | 1.19.1 | Apache-2.0 | Android | Android compatibility helpers. | Android framework APIs with API checks. |
| androidx.lifecycle:lifecycle-runtime-ktx | 2.11.0 | Apache-2.0 | Android | Lifecycle-aware coroutine scopes. | Explicit activity lifecycle observers. |
| kotlin-test | 2.4.21 | Apache-2.0 | Both | Common assertions. | JUnit assertions directly. |
| JUnit Jupiter | 6.1.3 | EPL-2.0 | Both | JVM test engine. | JUnit 5.13 line. |
| kotlinx-coroutines-test | 1.11.0 | Apache-2.0 | Both | Deterministic coroutine tests. | Manual test dispatchers. |
| Turbine | 1.2.1 | Apache-2.0 | Both | Flow testing. | Manual flow collection. |
| Compose UI test | Compose-managed | Apache-2.0 | Both | Compose interaction and semantics tests. | Platform instrumentation tests. |

JNA is used under the Apache-2.0 option. The LGPL option is not selected. Test-only EPL code is not distributed in release artifacts.

## 6. Build and packaging components

| Component | License | Distributed? | Obligation |
|---|---|---|---|
| Temurin JDK 21 runtime | GPL-2.0-with-Classpath-Exception | Yes, bundled by jpackage | Include license, notices, and source availability information. |
| Compose Desktop and Skiko | Apache-2.0 | Yes | Include license and notices. |
| Skia native library | BSD-3-Clause | Yes, via Skiko | Include copyright and license notice. |
| SQLite | Public domain | Yes, compiled into the core | Attribution in notices; no copyleft obligation. |
| AndroidX and Jetpack libraries | Apache-2.0 | Yes, in the Android app | Include license and notices. |
| Kotlin and Compose compilers | Apache-2.0 | No, build-time | No runtime obligation. |
| Inno Setup | Inno Setup License | No, build-time only | Do not redistribute the installer compiler. |
| MSVC and Windows SDK | Proprietary | No, build-time only | Build machine license only. |
| Android SDK and NDK | Android SDK License | No, build-time only | Do not redistribute SDK tools. |
| Git | GPL-2.0-only | No, build-time only | No distribution obligation. |
| Python | PSF-2.0 | No, build-time only | No distribution obligation. |
| JUnit Jupiter | EPL-2.0 | No, test-only | Excluded from release artifacts. |

## 7. Transitive and platform risks

| Risk | Why it matters | Control |
|---|---|---|
| QUIC crypto provider | libp2p QUIC may pull `ring` or `aws-lc-rs`; `aws-lc-rs` adds CMake/NASM cross-compilation complexity. | Prefer the `ring` provider; if a platform cannot build it, fall back to TCP + Noise + Yamux. |
| rustls and quinn | Large transitive surface for TLS and QUIC. | Pin through libp2p, audit with `cargo-deny`, and keep TCP fallback. |
| Skiko and Skia natives | Platform-specific native binaries and notices. | Validate Windows and Android packaging; include BSD notice. |
| SQLite/FTS5 build flags | A system SQLite may lack FTS5 or have a different version. | Prefer bundled SQLite; fall back to normalized `LIKE` search if FTS5 is unavailable. |
| JNA dual license | LGPL option could impose relinking obligations if selected accidentally. | Declare Apache-2.0 in the build and notices; legal review before any change. |
| JDK redistribution | jpackage bundles a runtime under GPLv2+CE. | Include the JDK license, notices, and source availability; verify at Stage 19. |
| Unicode data | `unicode-normalization` includes Unicode-3.0 data. | Include Unicode license notice. |
| Fonts and icons | Material icons, Roboto, or Noto may carry Apache-2.0 or OFL-1.1 terms. | Inventory bundled fonts at Stage 03 and include notices. |
| AndroidX/Kotlin version skew | Compose and lifecycle artifacts must align with Kotlin 2.4.21. | Use the Compose-managed versions and Gradle dependency locking; build in Stage 03. |
| Test libraries in release | JUnit, Turbine, and test dispatchers must not ship. | Separate test configurations; inspect release artifacts at Stage 19. |
| No analytics or crash SDK | A convenience SDK could become a hidden central dependency. | Prohibited by ADR-0009; dependency review rejects it. |

## 8. License obligations and notices

- Apache-2.0: include the license, preserve NOTICE files, and state modifications if any.
- MIT, BSD-2-Clause, BSD-3-Clause, ISC, Zlib: include copyright and license text in third-party notices.
- MPL-2.0: allowed only for unmodified dependencies; include the license and source location. Modifications require review and source publication for the modified files.
- Unicode-3.0: include the Unicode license and data notice.
- OFL-1.1: include the font license; do not sell fonts separately; preserve reserved names.
- Public domain SQLite: attribute in the inventory and notices.
- GPLv2+CE JDK: include the license, notices, and source availability information for the bundled runtime.
- EPL-2.0 JUnit: test-only; keep out of release artifacts.

The release pipeline generates a third-party notices file from the SBOM and license reports. The file is packaged with the Windows application and linked from the Android About screen and the project release notes.

## 9. Verification and update process

| Gate | Command or evidence | Stage |
|---|---|---|
| Rust license, ban, source, advisory check | `cargo deny check` | 03 and every release |
| Rust advisory scan | `cargo audit` | 03 and every release |
| Gradle dependency locking and verification | Enable and commit lockfiles plus verification metadata; clean build with locks | Before first signed release |
| Gradle license report | Dependency license report reviewed against section 2 | Before first signed release and every dependency change |
| SBOM | CycloneDX for Android, Windows, and Rust | 19 |
| Release artifact inspection | No test, debug, or build-only package in the app | 19 |
| New dependency review | Reason, license, platform support, owner, fallback recorded here | Before merge |

Update cadence:

- Security advisories are triaged immediately; a high-severity vulnerability blocks release unless a documented exception is approved.
- Patch updates are reviewed monthly.
- Minor updates are reviewed quarterly with the full test suite.
- Major updates require an ADR or an amendment to this inventory and a cross-platform build.
- Lockfiles are committed. Release builds use `--locked` and fail on an unlocked dependency.

## 10. Stage 03, Stage 05, Stage 06, Stage 07, and Stage 08 verification status

Stage 03 verifies the build baseline, Stage 05 verifies the realized storage
graph, Stage 06 verifies the realized crypto and identity graph, Stage 07
verifies the streaming manifest, transfer, and store integration, and Stage 08
verifies the C ABI, JNI/JNA adapters, and native packaging. Feature-specific
verification is deferred to the stage that activates the dependency, so the
project does not claim runtime evidence it does not have.

| Task | Stage | Status and evidence |
|---|---|---|
| Install and record the exact JDK, Gradle, Rust, NDK, and local linker versions. | 03 | Complete. `docs/progress/STAGE_03.md` records the exact versions. MSVC and the Windows SDK are absent locally; CI and release remain MSVC. |
| Generate `Cargo.lock`; confirm the realized Rust graph resolves. | 03, 05, 06, 07, 08 | Complete. Stage 08 resolves 131 packages; `cargo tree -p nexus-core -p nexus-ffi -p nexus-crypto -p nexus-identity -p nexus-protocol -p nexus-transfer -p nexus-store` and `cargo test --workspace` pass. |
| Run `cargo deny check` and `cargo audit`; record the transitive license and advisory reports. | 03, 05, 06, 07, 08 | Complete for the realized tracing, storage, crypto, transfer, and JNI graphs. Stage 08 reports `advisories ok, bans ok, licenses ok, sources ok`; `cargo audit` scanned 131 dependencies with no vulnerability reported. |
| Confirm the committed Ed25519, Bech32m, canonical CBOR, signed-record, content, manifest, tombstone, at-rest, and backup vectors. | 06, 07 | Complete on Windows. `nexus-crypto`, `nexus-identity`, `nexus-protocol`, `nexus-transfer`, and `nexus-store` assert all vectors; Android runtime cross-verification remains deferred. |
| Confirm secret material stays out of SQLite, logs, temporary files, and Debug output. | 06, 08 | Complete in Rust and the native create path. The identity file is encrypted with a host-supplied LEK, temporary identity writes are removed on failure, secret types redact `Debug`, and `NativeCoreClient` zeroes the caller's secret after create. |
| Confirm FTS5, WAL, and bundled SQLite compile and run on Windows x64. | 05 | Complete. `nexus-store` builds bundled SQLite 3.x through `libsqlite3-sys` 0.40.0, applies WAL, foreign keys, a busy timeout, and a bounded cache, and creates the FTS5 `search_index` table in migration 2. All 37 `nexus-store` tests pass on the local GNU/MinGW host. |
| Confirm content IDs, chunk manifests, manifest roots, revisions, and tombstones round-trip and reject corruption. | 07 | Complete on Windows. `nexus-transfer` and `nexus-store` assert the committed 2.5 MB vector, one-byte change, chunk corruption, reordering, truncation, root mismatch, immutable publications, monotonic tombstones, manifest repair, and reconciliation quarantine. |
| Confirm FTS5, WAL, and bundled SQLite compile for Android arm64 and x86_64. | 05, 08 | Complete for compilation. `scripts/build-native.ps1` cross-compiles bundled SQLite into both Android `.so` files; runtime storage is not exercised because no emulator or device is available. |
| Confirm the CBOR metadata manifest and archive hashes round-trip. | 05 | Complete. `data_007_metadata_export_and_restore_round_trip` reads the archive magic, manifest length, raw snapshot hash, and CBOR manifest, then restores into a separate store and verifies its contents. |
| Confirm QUIC builds with the chosen crypto provider on both platforms, or activate the TCP fallback. | 09 | Deferred; libp2p is not a dependency yet. |
| Confirm JNA 5.14.0 loads the Windows DLL and JNI loads the Android `.so`. | 08 | JNA and the JNI shim load `nexus_core.dll` on a Windows JVM and pass the shared contract. Android `.so` loading is deferred until an emulator or device is available. |
| Confirm Android Rust cross-compilation and native library packaging. | 08 | Complete for arm64-v8a and x86_64. Both `libnexus_core.so` files build, package, and export-check; runtime execution is deferred. |
| Confirm the C ABI compiles and runs through an independent C consumer. | 08 | Complete on Windows. The C harness compiles against `nexus_core.h` with `-Wall -Wextra -Werror`, passes 100 lifecycle cycles, and verifies events, errors, panic containment, cancellation, and streaming. |
| Confirm Inno Setup 6.4.3 produces a per-user installer and prompts on uninstall. | 19 | Deferred; releases produce an app image only until Stage 19. |
| Confirm the bundled JDK notices and source availability text are packaged. | 19 | Deferred to release packaging. |

## 11. Dependency ownership

| Area | Owner | Responsibility |
|---|---|---|
| Rust core | Core maintainers | Runtime crates, licenses, advisories, MSRV, cross-platform builds. |
| Kotlin and Compose | Application maintainers | UI, coroutines, serialization, JNA, AndroidX alignment. |
| Android build | Android maintainers | SDK, NDK, AGP, Gradle, signing. |
| Windows build | Desktop maintainers | JDK, jpackage, Inno Setup, signing, Windows SDK. |
| Release and SBOM | Release maintainers | Lockfiles, notices, SBOM, artifact inspection. |

A dependency without an owner is not approved for release.
