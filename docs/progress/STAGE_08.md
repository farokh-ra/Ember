# Stage 08 - C ABI, JNI/JNA Adapters, Lifecycle, and Streaming

**Status:** Complete
**Completed:** 2026-11-14
**Progress guide:** 38-42%
**Next stage:** 09 - Networking foundation (requires user permission)

## 1. Scope

Stage 08 implements the native boundary defined by
[ADR-0003](../decisions/ADR-0003-ffi-c-abi-jni-jna.md),
[ADR-0008](../decisions/ADR-0008-lifecycle-background-execution.md),
[ADR-0009](../decisions/ADR-0009-build-packaging-dependency-policy.md), and
[FFI_INTERFACE.md](../architecture/FFI_INTERFACE.md), without changing the
architecture or the Stage 06/07 record formats.

The stage covers:

- The 17-function C ABI in `docs/architecture/nexus_core.h`.
- Opaque handle registry with idempotent create/start/stop/destroy behavior.
- Create-time CBOR configuration and an ABI gate that runs before storage work.
- The stable request, response, and event envelopes.
- Domain error envelopes and ABI status mapping.
- Bounded asynchronous operations on a fixed four-worker pool.
- A 1,024-entry event queue with progress coalescing and non-blocking lifecycle
  pushes.
- `test.echo` and `test.fault`, including synchronous panic containment.
- Streaming import/export with 1 MiB chunks and no whole-file buffering.
- Android JNI and Windows JNA adapters over the same C ABI.
- A C harness, Rust C ABI contract tests, and shared Kotlin adapter tests.
- Windows and Android native packaging through `scripts/build-native.ps1`.

Networking, hosting, search, downloads, and the remaining product methods are
out of scope and remain later-stage work.

## 2. Architecture

The new responsibilities are split as follows:

| Layer | Responsibility |
|---|---|
| `nexus-core` | Configuration, lifecycle, dispatch, event queue, asynchronous operations, streaming orchestration. |
| `nexus-ffi` | Stable C ABI, handle registry, panic containment, buffer ownership, status mapping, JNI shim. |
| `shared/core-client` | Kotlin `NativeBridge` interface, JNI and JNA adapters, CBOR codec, typed `NexusException`, `NativeCoreClient`, event pump. |
| `native/c_harness` | Independent C consumer compiled against the shipped header and DLL. |
| `scripts/build-native.ps1` | Windows/Android build, artifact naming, export verification, C harness run. |

`nexus-ffi` depends on `nexus-core`; the platform adapters contain no protocol or
storage logic. Both platforms call the same method IDs and receive the same
envelopes.

## 3. Lifecycle and handle registry

`nexus-ffi` keeps an opaque `u64` registry token for each `Engine`. Tokens are
never raw pointers. Unknown or destroyed tokens return
`NEXUS_STATUS_INVALID_HANDLE` from every call helper, including `start`, `stop`,
`destroy`, `invoke`, `next_event`, `cancel`, and the streaming calls.

Lifecycle rules implemented:

- `create` validates the config and secret, opens the SQLite store, runs
  migrations, and returns a handle. It does not start networking.
- `start` is idempotent. It transitions through `starting` and emits
  `core.state`; a ready identity emits `identity.ready`, otherwise
  `identity.locked`.
- `stop` is idempotent, closes the event queue after durable state is flushed,
  and keeps the handle valid.
- `destroy` stops if needed and removes the registry entry. A second destroy or
  destroy of an unknown handle returns `NEXUS_STATUS_INVALID_HANDLE`.
- `nexus_core_next_event` returns `QUEUE_CLOSED` once the core is stopping or
  stopped, so the Kotlin pump exits cleanly.

The Rust C ABI test and the C harness each run 100
create/start/stop/destroy cycles. Both complete without leaked handles, orphaned
locks, or stale event-pump threads.

## 4. Configuration and ABI gate

The config is a canonical CBOR text-key map limited to 64 KiB:

| Key | Required | Rule |
|---|---|---|
| `abi_version` | Yes | Must equal 1. |
| `data_root` | Yes | Absolute path with no `.` or `..` components. |
| `app_version` | No | 1-64 bytes; default `0.0.0`. |
| `locale` | No | 1-35 bytes; default `en`. |
| `log_level` | No | `trace`, `debug`, `info`, `warn`, or `error`; default `info`. |
| `network_enabled` | No | Recorded but starts no transport in Stage 08; default `false`. |
| `quota_bytes` | No | 1-500 GiB; default 10 GiB. |
| `enable_test_methods` | No | Enables `test.*`; default `false`. |

`CoreConfig::parse` checks `abi_version` before constructing the store. A
mismatch returns `NEXUS_STATUS_ABI_MISMATCH`, writes no handle, and creates no
data root. This is asserted by the Rust C ABI test, the C harness, and both
Kotlin adapters.

`NativeCoreClient.create` also compares the loaded library ABI with
`NativeCoreClient.ABI_VERSION` before calling native `create`, zeroes the
caller's secret in every path, and refuses to open the database on a mismatch.

## 5. Method dispatch and error model

Stage 08 implements six method IDs:

| ID | Method | Mode | Result |
|---|---|---|---|
| 0x0101 | `system.info` | S | Core/ABI/protocol/schema versions, platform, locale, capabilities. |
| 0x0102 | `system.health` | S | Lifecycle state, network state, queue depth, operations, storage, identity. |
| 0x0103 | `system.snapshot` | S | Consistent startup/resume snapshot. |
| 0x0401 | `store.usage` | S | Quota, managed and partial bytes, free space, reserve, health. |
| 0x0B01 | `test.echo` | D | Echoes `payload`. Requires `enable_test_methods`. |
| 0x0B02 | `test.fault` | D | Domain faults, async operations, or a forced panic. |

All other catalog IDs return `NEXUS_STATUS_UNSUPPORTED` with a domain error.
A `test.*` ID with debug methods disabled returns `UNSUPPORTED` before any
dispatch.

ABI statuses cover boundary failures: `OK`, `TIMEOUT`, `INVALID_HANDLE`,
`INVALID_ARGUMENT`, `ABI_MISMATCH`, `QUEUE_CLOSED`, `INTERNAL`,
`BUFFER_TOO_LARGE`, and `UNSUPPORTED`. Domain failures stay inside the CBOR
response:

```text
{
  "v": 1,
  "ok": false,
  "error": {
    "code": "not_found",
    "message": "The requested item was not found.",
    "retryable": false
  }
}
```

The core maps storage, transfer, configuration, and import errors to stable
domain codes. Error messages are bounded and redacted. `catch_unwind` protects
every C and JNI entry point; panic payloads never cross the boundary.

## 6. Events and asynchronous operations

The event queue holds 1,024 events. Durable state and terminal events are never
dropped; progress events are coalesced per operation. Lifecycle and identity
events use `EventQueue::try_push` so a saturated queue cannot block `start` or
`stop`; such drops are counted for diagnostics.

Asynchronous operations run on four worker threads:

1. `invoke` validates the request and returns `{op_id, accepted}`.
2. The worker emits zero or more `test.progress` events.
3. Exactly one terminal event follows: `test.completed`, `test.failed`, or
   `test.cancelled`.
4. `nexus_core_cancel` is idempotent and interrupts waits cooperatively.
5. Event sequence numbers increase monotonically; every progress and terminal
   event carries its `op_id`.

Implemented event families:

| Event | Class | Data |
|---|---|---|
| `core.state` | Durable | `state`, optional `reason` |
| `core.stopped` | Durable | `reason` |
| `identity.ready` | Durable | `identity_id` |
| `identity.locked` | Durable | `reason` |
| `test.progress` | Progress | `step`, `total` |
| `test.completed` | Durable | `steps` |
| `test.failed` | Durable | `error` |
| `test.cancelled` | Durable | `steps` |

Because progress may coalesce under load, the contract asserts 1-4 progress
events for a four-step operation rather than exactly four.

## 7. Streaming import and export

### 7.1 Import

`import_begin` validates the display name, expected size, quota, and free space
before accepting bytes. `import_write` accepts at most 1 MiB per call and writes
to `store/tmp`. The session updates the raw SHA-256 and per-chunk SHA-256 state
incrementally; no function holds the whole file.

`import_finish`:

1. Verifies the final size against `expected_size`.
2. Builds the canonical flat 1 MiB manifest.
3. Stores the manifest under its trusted root.
4. Adopts the staged file into the blob tree without a second copy.
5. Adds a `local_import` content reference so the verified bytes survive until a
   publication, download, or replica takes ownership.

Importing identical bytes again discards the new staged copy and keeps the
existing blob. Empty imports, size mismatches, over-writes, and chunks above
1 MiB are rejected. `import_abort` removes the staged file; startup cleanup
removes abandoned staging files.

The summary result contains `content_id` (32-byte SHA-256 as hex), `size`,
`manifest_root`, `chunk_size`, and `display_name`.

### 7.2 Export

`export_open` requires verified local content and returns a cursor.
`export_read` returns at most 1 MiB through a bounded cursor; an empty buffer
means end of file. `export_close` releases the cursor.
Export never changes reference counts and never writes directly to the user's
destination.

### 7.3 Store migration 4

Migration 4 rebuilds `content_refs` to allow `local_import` alongside
`publication_original`, `active_replica`, and `retained_download`. SQLite cannot
widen a `CHECK` constraint in place, so the small table is rebuilt inside the
migration transaction. Existing references are copied before the old table is
dropped, and both indexes are recreated.

## 8. Kotlin adapters and client

### 8.1 Shared bridge

`NativeBridge` exposes the 14 native operations plus ABI and version accessors.
`NativeStatus` maps all nine statuses and treats unknown codes as `INTERNAL`.
`NexusException` is the shared hierarchy: `AbiMismatch`, `Status`, `Domain`, and
`Decode`.

`NativeCoreClient`:

- Checks ABI before create and zeroes the caller's secret.
- Encodes requests as canonical CBOR `{v: 1, params: {...}}`.
- Decodes success results and domain errors.
- Runs the only `next_event` loop with a 50 ms timeout.
- Publishes decoded events to `nativeEvents` and lifecycle state to `events`.
- Exposes cancel and streaming calls.
- Stops and destroys the handle exactly once in `close()`.

### 8.2 Android JNI

`io.nexus.core.client.NexusNative` is a Kotlin object whose `@JvmStatic external`
members become static native methods. Rust exports matching
`Java_io_nexus_core_client_NexusNative_*` symbols; no `RegisterNatives` table is
used. `JNI_OnLoad` returns `JNI_ERR` on an ABI mismatch.

Result encodings:

| Return type | Meaning |
|---|---|
| `Int` | Status only. |
| `LongArray` | Exactly `[status, value]`. |
| `ByteArray` | Four little-endian status bytes followed by the payload. |

Every JNI entry point is panic-guarded. A panic in a byte-returning call becomes
a status-payload array with `NEXUS_STATUS_INTERNAL`; no payload is exposed.

### 8.3 Windows JNA

JNA 5.14.0 maps the C functions through `NexusLibrary : Library`. `NexusBuffer`
is a `Pointer` plus a 64-bit `Long` with
`@Structure.FieldOrder("ptr", "len")`; the by-value variant exists only for
`nexus_core_free_buffer`. Output buffers are copied and freed in `finally`.

The Gradle test task passes `-Dnexus.native.path=<repo>/build/native/windows-x64/nexus_core.dll`.

## 9. Native packaging and build

`scripts/build-native.ps1` builds and packages:

| Target | Artifact | Verification |
|---|---|---|
| Windows x64 | `build/native/windows-x64/nexus_core.dll` | 19 required exports, import library, C harness. |
| Android arm64-v8a | `build/native/android-arm64-v8a/libnexus_core.so` | 3 required exports. |
| Android x86_64 | `build/native/android-x86_64/libnexus_core.so` | 3 required exports. |

Rust's cdylib is named `nexus_ffi`; the script copies it to the product name
`nexus_core`. Because the linker's import library records the original DLL name,
the script regenerates `nexus_core.def` with `gendef`, patches the `LIBRARY`
line, and rebuilds `libnexus_core.dll.a` with `dlltool`. The C harness links
against the regenerated import library.

Android builds use the NDK clang wrappers and set `CC_`, `CXX_`, and `AR_` for
the target so `libsqlite3-sys` cross-compiles the bundled SQLite amalgamation.
The `.so` files are compiled and export-checked locally but not executed on
Android in this environment.

`scripts/build.ps1` runs `build-native.ps1 -Target All -Configuration Release`
before Gradle and has a `-SkipNative` switch for focused work.

## 10. Verification evidence

### 10.1 C harness

`native/c_harness/main.c` is compiled with:

```text
gcc -std=c11 -Wall -Wextra -Werror -I docs/architecture main.c libnexus_core.dll.a
```

It checks:

- ABI and library version.
- ABI mismatch returns `NEXUS_STATUS_ABI_MISMATCH` and creates no directory.
- A short secret returns `NEXUS_STATUS_INVALID_ARGUMENT`.
- 100 create/start/stop/destroy cycles.
- Idempotent start and stop.
- Unknown handle rejection.
- `test.echo` round-trip.
- Domain `not_found` inside a successful ABI response.
- Forced panic returns `NEXUS_STATUS_INTERNAL` with no buffer.
- Async progress ordering and terminal completion.
- Cooperative cancellation.
- 2.5 MB streaming import/export round-trip with 1 MiB chunks and EOF.

Observed output:

```text
NEXUS C ABI harness
  ABI version: 1
  library version: 0.1.0
  lifecycle: 100 create/start/stop/destroy cycles
  lifecycle cycles: 100/100

C harness passed.
```

The forced-panic check intentionally writes the Rust panic line to stderr. It is
caught before the boundary; the harness still exits successfully.

### 10.2 Rust tests

`cargo test --workspace` passed with 166 tests and 0 failures:

| Suite | Tests |
|---|---:|
| `nexus-core` unit tests | 23 |
| `nexus-ffi` unit tests | 4 |
| `nexus-ffi` C ABI tests | 6 |
| `nexus-crypto` unit tests | 24 |
| `nexus-identity` unit tests | 9 |
| `nexus-identity` integration tests | 12 |
| `nexus-observability` unit tests | 3 |
| `nexus-protocol` unit tests | 16 |
| `nexus-protocol` signed-record tests | 9 |
| `nexus-protocol` Stage 07 tests | 8 |
| `nexus-store` unit tests | 17 |
| `nexus-store` local-import tests | 4 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-store` storage tests | 12 |
| `nexus-transfer` unit tests | 7 |
| `nexus-transfer` integration tests | 7 |
| `nexus-types` unit tests | 1 |
| **Total** | **166** |

The six Rust C ABI tests are:

| Test | Covers |
|---|---|
| `abi_and_version_are_exported` | ABI 1 and non-empty version. |
| `lifecycle_dispatch_and_errors_share_one_contract` | Lifecycle, echo, domain errors, panic, unknown IDs/handles. |
| `abi_mismatch_is_rejected_before_database_work` | No handle and no data root on mismatch. |
| `events_async_progress_and_cancellation_are_ordered` | Event ordering, terminal completion, cancellation, coalescing. |
| `streaming_import_and_export_round_trip` | 2.5 MB import/export, chunk bounds, summary fields, EOF. |
| `one_hundred_lifecycle_cycles_do_not_leak_handles_or_locks` | 100 cycles, stale handle rejection. |

### 10.3 Kotlin tests

A forced rerun of the five JVM test tasks produced 76 tests, 0 failures, and 0
skipped:

| Suite | Tests |
|---|---:|
| `shared:core-api` | 1 |
| `shared:core-client` | 7 |
| `shared:model` | 3 |
| `shared:presentation` | 44 |
| `shared:ui` | 21 |
| **Total** | **76** |

The `core-client` suite contains four CBOR codec tests, the existing client
boundary test, and two adapter-contract tests. `NativeContractTest` runs the same
`runNativeContract` body against:

- `JnaNativeBridge.load(nexus.native.path)`.
- `JvmJniNativeBridge`, using the Windows `nexus_core.dll` JNI exports.

Both contracts cover ABI mismatch before create, unknown handles, secret
zeroing, idempotent start, echo, domain error, panic containment, ordered async
progress, cancellation, restart, and the 2.5 MB streaming round-trip.

### 10.4 Build and formatting

`scripts/build-native.ps1 -Target All -Configuration Release` completed:

- Windows `nexus_core.dll` built, packaged, and export-checked.
- `libnexus_core.dll.a` regenerated and used by the C harness.
- Android arm64-v8a `libnexus_core.so` built and export-checked.
- Android x86_64 `libnexus_core.so` built and export-checked.
- C harness compiled and passed.

`cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets -- -D warnings` pass.

### 10.5 Full local gate

The complete gate completed on 2026-11-15:

```text
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev-env.ps1 -Quiet
.\scripts\build.ps1 -WithDependencyScan
```

Result: PASS.

- Native Windows and Android artifacts built and required exports verified.
- The C harness passed lifecycle, ABI mismatch, echo, domain error, panic,
  async progress, cancellation, 100 lifecycle cycles, and the 2.5 MB streaming
  round-trip.
- Spotless, detekt, Android debug assembly, desktop image, and all five JVM test
  tasks passed; 76 JVM tests, 0 failures, 0 skipped.
- Rust formatting, Clippy with warnings denied, and workspace tests passed; 166
  Rust tests, 0 failures.
- `cargo deny check` reported advisories, bans, licenses, and sources all OK.
- `cargo audit` scanned 131 dependencies against 1,509 advisories and reported
  no vulnerabilities.
- Repository hygiene passed with 249 files scanned.

The clean-checkout replay is recorded after the Stage 08 commit.

## 11. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries are compiled, packaged, and
  export-checked.
- The JNI shim is executed on a Windows JVM as a proxy for symbol names, result
  encodings, panic containment, and the shared contract.
- Android runtime execution, Android JNI linkage, and on-device byte
  verification remain deferred to the first environment with an emulator or
  device.

This is not claimed as Android runtime evidence. The C ABI and JNI shim share
the same underlying call helpers, so the remaining risk is Android-specific
loading, packaging, and runtime behavior rather than a second protocol
implementation.

## 12. Failure evidence and fixes

1. The JNI dependency was initially conditional on Android, so the Windows JVM
   could not load the JNI symbols. It is now unconditional and `jni_bridge`
   compiles on Windows; this enables the Windows-hosted JNI proxy test while
   Android execution stays deferred.
2. The Rust cdylib is `nexus_ffi.dll`, but the product, JNA loader, and C harness
   load `nexus_core.dll`. Packaging now renames the artifact and regenerates the
   import library with `gendef` plus `dlltool`; otherwise the import library
   recorded the wrong DLL name.
3. Android `libsqlite3-sys` initially failed to find a C compiler for the
   cross-target. The build script now sets the NDK clang wrappers through
   `CARGO_TARGET_*_LINKER` and `CC_/CXX_/AR_<target>`.
4. Unknown handles could reach a dispatch path and return a less specific error.
   Every call helper now maps a missing registry entry to
   `NEXUS_STATUS_INVALID_HANDLE`.
5. A panic in a JNI byte-returning call originally risked returning null. The
   fallback now returns a status-payload array with `NEXUS_STATUS_INTERNAL`, and
   Kotlin decodes null defensively.
6. The first contract expected exactly `steps` progress events. The queue
   coalesces progress under load, so the contract asserts 1-4 events for a
   four-step operation and requires a terminal event.
7. The first full gate after the detekt changes failed to compile
   `DEFAULT_QUOTA_BYTES` because `ULong` arithmetic is not a constant expression.
   The constant is now the literal `10_737_418_240uL`; the rerun passed.

## 13. Decisions and unresolved questions

- Stage 08 was explicitly approved by the project owner.
- ADR-0003, ADR-0008, and ADR-0009 are implemented without architecture changes.
- The ABI version remains 1. The protocol version string is
  `0.1.0-stage-08`.
- `nexus-core` owns lifecycle, dispatch, events, and streaming orchestration;
  `nexus-ffi` remains a thin unsafe boundary with panic containment.
- A completed streaming import is retained as a `local_import` reference until
  another owner takes the bytes.
- Progress coalescing is an intentional contract property, not a test
  workaround.
- Android runtime verification and remote GitHub Actions execution remain
  outstanding.
- Networking, product methods, and 4 GiB streaming evidence are later stages.

## 14. Next stage

Stage 09 adds QUIC/TCP connections, peer authentication, network-change
handling, and resource limits. It must not begin until Stage 08 is reviewed and
approved by the project owner.
