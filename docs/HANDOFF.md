# NEXUS Project Handoff

**Current stage:** 08 - C ABI, JNI/JNA adapters, lifecycle, and streaming
**Stage status:** Complete
**Last updated:** 2026-11-15
**Next stage:** 09 - Networking foundation (waiting for user permission)

## What was implemented

Stage 08 connects the verified Rust core to Kotlin through the stable C ABI and
adds the lifecycle, event, error, and streaming layers required by ADR-0003:

- 17-function C ABI exported from `nexus-ffi`, with opaque handles, panic
  containment, and versioned CBOR envelopes.
- Create-time configuration with an ABI gate that runs before the database or
  data root is opened.
- Idempotent create/start/stop/destroy behavior and a handle registry that
  rejects every unknown or stale token.
- Six callable methods: `system.info`, `system.health`, `system.snapshot`,
  `store.usage`, `test.echo`, and `test.fault`.
- Stable domain error envelopes, retryable flags, and ABI status mapping.
- A 1,024-entry event queue with progress coalescing and non-blocking lifecycle
  pushes.
- Fixed four-worker asynchronous operations with progress, terminal, and
  cancellation events.
- Streaming import to `store/tmp` with incremental SHA-256 and chunk hashing,
  canonical manifests, no-copy adoption, and a `local_import` reference retained
  until another owner takes the bytes.
- Bounded export with a 1 MiB cursor and EOF signaling.
- Store migration 4, which rebuilds `content_refs` to allow `local_import`.
- Android JNI shim using static `Java_io_nexus_core_client_NexusNative_*`
  symbols and `JNI_OnLoad` ABI rejection.
- Windows JNA adapter with the native `NexusBuffer` layout.
- Kotlin `NativeCoreClient` with ABI check, secret zeroing, invoke/cancel,
  streaming calls, and a single event pump.
- C harness, six Rust C ABI contract tests, and a shared Kotlin contract that
  runs against both Windows adapters.
- `scripts/build-native.ps1` for Windows and Android artifacts, export checks,
  and the C harness; `scripts/build.ps1` now runs it before Gradle.

No networking, hosting, search, downloads, or product features are implemented
yet. The Kotlin screens still use visibly labelled demo data.

## Files changed

Core crate:

- `crates/nexus-core/Cargo.toml`
- `crates/nexus-core/src/lib.rs`
- `crates/nexus-core/src/config.rs` (new)
- `crates/nexus-core/src/engine.rs` (new)
- `crates/nexus-core/src/envelope.rs` (new)
- `crates/nexus-core/src/error.rs` (new)
- `crates/nexus-core/src/event.rs` (new)
- `crates/nexus-core/src/ops.rs` (new)
- `crates/nexus-core/src/streaming.rs` (new)

FFI crate:

- `crates/nexus-ffi/Cargo.toml`
- `crates/nexus-ffi/src/lib.rs`
- `crates/nexus-ffi/src/jni_bridge.rs` (new)
- `crates/nexus-ffi/src/registry.rs` (new)
- `crates/nexus-ffi/tests/c_abi.rs` (new)

Store crate:

- `crates/nexus-store/src/managed_store.rs`
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/model.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/tests/storage.rs`
- `crates/nexus-store/migrations/0004_local_import_refs.sql` (new)
- `crates/nexus-store/tests/local_import.rs` (new)

Native and scripts:

- `native/c_harness/main.c` (new)
- `scripts/build-native.ps1` (new)
- `scripts/build.ps1`

Kotlin:

- `gradle/libs.versions.toml`
- `shared/core-client/build.gradle.kts`
- `shared/core-client/src/commonMain/kotlin/io/nexus/core/client/Cbor.kt` (new)
- `shared/core-client/src/commonMain/kotlin/io/nexus/core/client/NativeBridge.kt` (new)
- `shared/core-client/src/commonMain/kotlin/io/nexus/core/client/NativeConfig.kt` (new)
- `shared/core-client/src/commonMain/kotlin/io/nexus/core/client/NativeCoreClient.kt` (new)
- `shared/core-client/src/commonMain/kotlin/io/nexus/core/client/NativeStatusPayload.kt` (new)
- `shared/core-client/src/commonMain/kotlin/io/nexus/core/client/NexusException.kt` (new)
- `shared/core-client/src/jvmMain/kotlin/io/nexus/core/client/JnaNativeBridge.kt` (new)
- `shared/core-client/src/androidMain/kotlin/io/nexus/core/client/NexusNative.kt` (new)
- `shared/core-client/src/androidMain/kotlin/io/nexus/core/client/JniNativeBridge.kt` (new)
- `shared/core-client/src/jvmTest/kotlin/io/nexus/core/client/CborTest.kt` (new)
- `shared/core-client/src/jvmTest/kotlin/io/nexus/core/client/NativeContract.kt` (new)
- `shared/core-client/src/jvmTest/kotlin/io/nexus/core/client/NativeContractTest.kt` (new)
- `shared/core-client/src/jvmTest/kotlin/io/nexus/core/client/NexusNative.kt` (test-only JNI proxy)
- `shared/core-client/src/jvmTest/kotlin/io/nexus/core/client/JvmJniNativeBridge.kt` (new)

Workspace and documentation:

- `Cargo.lock`
- `docs/architecture/FFI_INTERFACE.md`
- `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md`
- `docs/progress/STAGE_08.md` (new)
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

The untracked `LICENSE.md` is unrelated user work and is not part of this stage.

## Commands and results

### Native build

```text
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev-env.ps1 -Quiet
.\scripts\build-native.ps1 -Target All -Configuration Release
```

Result: PASS.

- Windows `build/native/windows-x64/nexus_core.dll` built and packaged.
- `libnexus_core.dll.a` regenerated with `gendef` and `dlltool`.
- 19 required Windows exports present.
- C harness compiled with `-std=c11 -Wall -Wextra -Werror` and passed,
  including 100 lifecycle cycles and the 2.5 MB streaming round-trip.
- Android arm64-v8a and x86_64 `libnexus_core.so` built and export-checked.

### Tests

```text
cargo test --workspace
```

Result: PASS, 166 tests, 0 failures.

```text
.\gradlew.bat --no-daemon --console=plain :shared:core-client:jvmTest
```

Result: PASS, 7 `core-client` tests, 0 failures. The two native-contract tests
cover Windows JNA and the Windows-hosted JNI proxy.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 23 | 0 |
| `nexus-ffi` unit tests | 4 | 0 |
| `nexus-ffi` C ABI tests | 6 | 0 |
| `nexus-crypto` unit tests | 24 | 0 |
| `nexus-identity` unit tests | 9 | 0 |
| `nexus-identity` integration tests | 12 | 0 |
| `nexus-observability` unit tests | 3 | 0 |
| `nexus-protocol` unit tests | 16 | 0 |
| `nexus-protocol` signed-record tests | 9 | 0 |
| `nexus-protocol` Stage 07 tests | 8 | 0 |
| `nexus-store` unit tests | 17 | 0 |
| `nexus-store` local-import tests | 4 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` storage tests | 12 | 0 |
| `nexus-transfer` unit tests | 7 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **166** | **0** |
| `shared:core-api` JVM tests | 1 | 0 |
| `shared:core-client` JVM tests | 7 | 0 |
| `shared:model` JVM tests | 3 | 0 |
| `shared:presentation` JVM tests | 44 | 0 |
| `shared:ui` JVM tests | 21 | 0 |
| **JVM total** | **76** | **0** |

### Full local gate

```text
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
.\scripts\build.ps1 -WithDependencyScan
```

Result: PASS on 2026-11-15.

- Native Windows and Android artifacts built, required exports verified, and the
  C harness passed all checks including 100 lifecycle cycles.
- Gradle Spotless, detekt, Android debug assembly, desktop image, and all five
  JVM test tasks passed with 76 tests and 0 failures.
- Rust formatting, Clippy with warnings denied, and workspace tests passed with
  166 tests and 0 failures.
- `cargo deny check`: advisories, bans, licenses, and sources all OK.
- `cargo audit`: 131 dependencies scanned against 1,509 advisories with no
  reported vulnerabilities.
- Repository hygiene: PASS, 249 files scanned.

### Clean checkout

```text
.\scripts\verify-clean-checkout.ps1
```

Result: PASS on 2026-11-15 at Stage 08 commit `6903384`. The local clone built
the Windows DLL and both Android libraries, passed the C harness, built the
Android APK and desktop image, passed Spotless, detekt, and all five JVM test
tasks, passed all 166 Rust tests, and passed hygiene with 248 files scanned.
The script does not run the dependency scan; that was covered by the full gate
above.

## Errors and failed attempts

1. The JNI dependency was conditional on Android, preventing a Windows JVM JNI
   proxy test. It is now unconditional and `jni_bridge` compiles on Windows.
2. The Rust cdylib is `nexus_ffi.dll`, while the product and JNA loader expect
   `nexus_core.dll`. Packaging renames the artifact and regenerates the import
   library so the recorded DLL name is correct.
3. Android `libsqlite3-sys` could not find a cross C compiler. The native script
   now sets the NDK clang wrappers and `CC_/CXX_/AR_<target>` variables.
4. Unknown handles could reach a dispatch path. All call helpers now return
   `NEXUS_STATUS_INVALID_HANDLE` immediately.
5. A panic in a JNI byte-returning call could have returned null. The fallback
   now returns a status-payload array, and Kotlin decodes null defensively.
6. Progress coalescing made an exact event-count assertion flaky. The contract
   now requires 1-4 progress events and one terminal event.
7. The first full gate after the detekt changes failed to compile
   `DEFAULT_QUOTA_BYTES` because `ULong` arithmetic is not a constant expression.
   The constant is now the literal `10_737_418_240uL`, and the rerun passed.

## Decisions and unresolved questions

- Stage 08 was explicitly approved by the project owner.
- ADR-0003, ADR-0008, and ADR-0009 are implemented without architecture changes.
- The ABI remains version 1; the protocol version is `0.1.0-stage-08`.
- `test.*` is enabled only by config and must stay disabled in release hosts.
- A completed streaming import is retained by a `local_import` reference until a
  publication, download, or replica takes ownership.
- Android runtime verification is deferred because no emulator or device is
  available. The Android libraries are compiled and export-checked; the JNI shim
  is exercised through a Windows-hosted JVM proxy.
- Remote GitHub Actions execution has not run for Stage 08.

## Exact next action

1. Review `docs/progress/STAGE_08.md` and the updated
   `docs/architecture/FFI_INTERFACE.md`.
2. Give explicit approval to start Stage 09.
3. Stage 09 adds QUIC/TCP connections, peer authentication, network-change
   handling, and resource limits. Do not begin it before that approval.
