# ADR-0003 — Stable C ABI with JNI and JNA, versioned CBOR, event queue

- **Status:** Accepted
- **Date:** 2026-11-12
- **Deciders:** NEXUS engineering
- **Supersedes:** none
- **Related:** ADR-0001, ADR-0002, ADR-0004, ADR-0007, [FFI interface draft](../architecture/FFI_INTERFACE.md)

## Context

The Rust core must be called from Kotlin on Android and Windows. The two platforms have different native bridges: Android supports JNI directly, while the Windows Compose Desktop application runs on the JVM and needs a JVM-to-native binding. The same core behaviour must be reachable from both, without maintaining two protocol implementations or two core APIs.

The boundary also has to solve:

- Long-running operations that must not block the UI thread.
- Cancellation and progress reporting.
- Bounded memory and safe ownership of native buffers.
- Rust panics and errors that must never unwind across the ABI.
- Version compatibility between an installed app and a bundled core library.
- A testable contract that can be implemented before either platform shell is complete.

A large, hand-written JNI surface would be duplicated on Windows. A callback-based API would require the Rust core to attach to the JVM and would make reentrancy, shutdown, and thread ownership difficult. A generated binding such as UniFFI is attractive, but it adds a second schema toolchain and its Kotlin/Native and JVM output would still need to be validated separately on both platforms.

## Decision

Expose **one versioned C ABI** from `nexus-ffi`, and use thin platform adapters:

- **Android:** JNI shim using the `jni` crate. The JNI functions are a thin translation layer over the same C ABI.
- **Windows:** JNA binding over the C ABI. JNA is a JVM library and does not require a separate Rust build target.
- **Kotlin:** one common `NexusCore` interface. `expect`/`actual` selects the JNI or JNA bridge. Platform shells never call native functions directly.

### ABI shape

The ABI has a small, fixed lifecycle surface and one generic operation call:

```text
nexus_core_abi_version() -> uint32
nexus_core_version() -> const char*
nexus_core_create(config_cbor, config_len, local_secret, secret_len, out_handle) -> status
nexus_core_start(handle) -> status
nexus_core_stop(handle) -> status
nexus_core_destroy(handle) -> status
nexus_core_invoke(handle, method_id, request_cbor, request_len, out_response) -> status
nexus_core_next_event(handle, timeout_ms, out_event) -> status
nexus_core_cancel(handle, op_id) -> status
nexus_core_free_buffer(buffer) -> void
```

Dedicated streaming calls handle file bytes so no file is ever placed in a CBOR message:

```text
nexus_core_import_begin / import_write / import_finish / import_abort
nexus_core_export_open / export_read / export_close
```

- `method_id` is a stable `uint32` allocated from the table in the FFI draft. IDs are never reused or repurposed.
- Requests and responses are UTF-8 CBOR maps with a schema version field. CBOR is used for structured data only; file bytes use the streaming calls.
- Synchronous methods return a response buffer. Asynchronous methods return an operation ID immediately and emit terminal events.
- All returned buffers are allocated by Rust and released by `nexus_core_free_buffer`. Input buffers are borrowed only for the duration of the call and are never retained.
- Handles are opaque `uint64_t` registry tokens, not raw pointers. A destroyed or unknown handle returns a defined status instead of dereferencing freed memory.

### Event model

There are **no native-to-JVM callbacks**. The core owns a bounded event queue. Kotlin runs one dedicated event-pump thread that calls `nexus_core_next_event` with a timeout and publishes decoded events into a `SharedFlow`.

- Every asynchronous operation emits exactly one terminal event: completed, failed, or cancelled.
- Progress events may be coalesced under backpressure. Terminal events are never dropped.
- Events carry a monotonic sequence number. If the UI has been suspended, it re-queries a snapshot and reconciles rather than assuming it saw every progress event.
- Event payloads are CBOR maps and obey the same size limits as responses.

### Error model

The C return status covers ABI-level failures only: invalid handle, invalid argument, timeout, queue closed, ABI mismatch, internal error. Domain failures are returned in the CBOR envelope with a stable string code and a human-readable, redacted message.

- Rust panics are caught at the ABI boundary with `catch_unwind`; the panic payload is logged without secrets and converted to an internal error. Unwinding across the ABI is forbidden.
- Kotlin maps ABI statuses and domain error codes to a sealed `NexusException` hierarchy.
- Native error messages never contain private keys, passphrases, local paths, or peer identifiers that are unnecessary for the user.

### Threading and memory rules

- `invoke` is thread-safe but must not be called on the UI thread. The Kotlin wrapper dispatches calls to `Dispatchers.IO`.
- `next_event` is a blocking call and must have exactly one consumer.
- Long operations run on the Rust runtime or blocking pool and return immediately.
- Import writes and export reads are bounded, synchronous, and called from the Kotlin IO dispatcher. They are flow-controlled by the host; the core does not buffer a whole file.
- Every buffer is length-delimited. Native strings are UTF-8 and are not assumed to be NUL-terminated.
- The Kotlin side zeroes secret byte arrays after passing them to the core. The core zeroes its copies with `zeroize`.

### Versioning

Four versions are independent:

| Version | Owner | Rule |
|---|---|---|
| FFI ABI | `nexus-ffi` | Must match exactly between the app and the bundled library. |
| App version | release | Semantic version; shown in diagnostics and the About screen. |
| Protocol version | `nexus-protocol` | Major version in libp2p protocol IDs and record schemas. |
| Database schema | `nexus-store` | Migrated by the core; never read by Kotlin. |

The app checks the ABI at startup before creating a core. An ABI mismatch is a fatal, actionable startup error, not a silent fallback. Adding an optional response field is backward compatible; removing or changing a field requires a new ABI version.

## Consequences

**Positive**

- One native contract serves both platforms and can be tested with a C harness before the apps exist.
- No JVM attachment from Rust, no callbacks, and no reentrancy across the boundary.
- Bounded native memory ownership and a single event consumer are easy to audit.
- Kotlin can replace the native bridge with a fake `NexusCore` for UI tests.
- The streaming API keeps memory bounded at the 4 GiB file limit.

**Negative / trade-offs**

- The generic `invoke` method loses compile-time type checking at the ABI; the Kotlin wrapper and Rust method table must stay in sync.
- CBOR envelopes add a small encode/decode cost compared with direct JNI methods.
- JNA is slower than JNI; desktop calls are infrequent and long operations are asynchronous.
- Two bridge implementations must be tested, even though they share the C ABI.
- The event queue adds a poll latency equal to the pump timeout (target 50 ms while active).

**Constraints**

- No native pointers are exposed to Kotlin.
- No unbounded response or event payloads. The limits are recorded in the FFI draft.
- No platform-specific method may appear in the shared method catalog.
- The core must remain usable when the event pump is not running; events buffer up to the queue limit and then apply backpressure to progress producers.

## Alternatives considered

1. **UniFFI-generated Kotlin and JVM bindings.** Less hand-written glue and typed APIs, but adds a second IDL/toolchain, generated-code review burden, and separate Kotlin/Native versus JVM behaviour. Rejected for v1; can be reconsidered if the hand-written surface becomes unmaintainable.
2. **Hand-written JNI methods for every operation.** Fast on Android, but duplicates the entire API on Windows and multiplies the unsafe surface. Rejected.
3. **JNA on Android and Windows.** One binding technology, but JNA on Android is slower and less predictable for mobile lifecycle and packaging. Rejected.
4. **Rust calls JVM callbacks for events.** Avoids polling, but requires JVM attachment, careful local-reference management, and shutdown coordination. Rejected.
5. **Separate Rust daemon over a local socket.** Strong isolation, but adds an IPC protocol, process lifecycle, mobile background restrictions, and a larger attack surface. Rejected for v1.
6. **Protobuf or JSON envelopes.** Workable, but CBOR matches the signed-record format and keeps one compact codec family. JSON has weaker binary and map semantics; protobuf adds a schema compiler. Rejected.

## Verification

- Stage 08 compiles the C header against a C harness and calls lifecycle, invoke, and event functions.
- Stage 08 runs the same `test.echo` and fault-injection operations from Android JNI and Windows JNA.
- Stage 08 runs 100 create/start/stop/destroy cycles and checks for leaks, orphaned threads, and use-after-free.
- Stage 08 verifies that an ABI mismatch is rejected before any database or network work begins.
- Stage 19 fuzzes malformed CBOR, oversized buffers, invalid handles, and cancellation races.
