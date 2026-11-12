# NEXUS Core FFI Interface Draft

| Field | Value |
|---|---|
| Document ID | NEXUS-ARCH-FFI-001 |
| Version | 1.0.0 (ABI 1) |
| Status | Draft accepted for Stage 02; implemented in Stage 08 |
| Date | 2026-11-12 |
| Header | [nexus_core.h](nexus_core.h) |
| Related | [ADR-0003](../decisions/ADR-0003-ffi-c-abi-jni-jna.md), [Architecture](../ARCHITECTURE.md) |

## 1. Purpose

This document is the contract between the Rust core and the Kotlin application on Android and Windows. It defines the C ABI, memory ownership, error model, CBOR envelopes, operation catalog, event catalog, streaming file APIs, threading rules, and version compatibility.

The ABI is deliberately smaller than the product API. Kotlin exposes typed, idiomatic methods over a generic native operation call. Both platforms use the same catalog and the same native functions; only the adapter differs.

## 2. Design rules

1. One C ABI, two adapters: JNI on Android, JNA on Windows.
2. No native-to-JVM callbacks. Events are pulled from a bounded queue.
3. No long-running work blocks the caller. Async operations return an operation ID.
4. No file bytes in CBOR. File import and export use dedicated streaming calls.
5. No raw pointers in Kotlin. Handles are opaque registry tokens.
6. Every returned buffer is Rust-owned and freed exactly once by the host.
7. Rust panics never unwind across the ABI.
8. No platform-specific operation appears in the shared method catalog.
9. Every request and response is bounded before allocation.
10. Errors are typed, stable, and redacted.

## 3. Status codes

`NexusStatus` covers ABI-level outcomes. Domain failures are encoded in the CBOR response envelope.

| Value | Name | Meaning |
|---|---|---|
| 0 | `NEXUS_STATUS_OK` | Call succeeded. |
| 1 | `NEXUS_STATUS_TIMEOUT` | `next_event` reached its timeout with no event. |
| 2 | `NEXUS_STATUS_INVALID_HANDLE` | Handle is unknown, stopped, or destroyed. |
| 3 | `NEXUS_STATUS_INVALID_ARGUMENT` | Null or invalid pointer, length, or method ID. |
| 4 | `NEXUS_STATUS_ABI_MISMATCH` | Host and library ABI versions differ. |
| 5 | `NEXUS_STATUS_QUEUE_CLOSED` | Event queue is closed; the core is stopping or stopped. |
| 6 | `NEXUS_STATUS_INTERNAL` | Guarded internal fault; diagnostics may be available. |
| 7 | `NEXUS_STATUS_BUFFER_TOO_LARGE` | A request or event exceeds the ABI size limit. |
| 8 | `NEXUS_STATUS_UNSUPPORTED` | The build does not support the requested debug or optional method. |

Domain errors use the CBOR error envelope and the stable codes in section 7.

## 4. Functions

```c
uint32_t nexus_core_abi_version(void);
const char *nexus_core_version(void);

NexusStatus nexus_core_create(
    const uint8_t *config_cbor,
    size_t config_len,
    const uint8_t *local_secret,
    size_t local_secret_len,
    uint64_t *out_handle);

NexusStatus nexus_core_start(uint64_t handle);
NexusStatus nexus_core_stop(uint64_t handle);
NexusStatus nexus_core_destroy(uint64_t handle);

NexusStatus nexus_core_invoke(
    uint64_t handle,
    uint32_t method_id,
    const uint8_t *request_cbor,
    size_t request_len,
    NexusBuffer *out_response);

NexusStatus nexus_core_next_event(
    uint64_t handle,
    uint32_t timeout_ms,
    NexusBuffer *out_event);

NexusStatus nexus_core_cancel(uint64_t handle, uint64_t op_id);

void nexus_core_free_buffer(NexusBuffer buffer);
```

Streaming functions are defined in section 8.

### 4.1 Lifecycle

- `nexus_core_create` validates the config, copies the local secret into zeroizing memory, opens the database and store, runs migrations, and returns a handle. It does not start networking.
- The config is CBOR and contains app-private paths, locale, network settings, quota, and log level. It never contains the local secret.
- The local secret is exactly 32 bytes. A different length returns `NEXUS_STATUS_INVALID_ARGUMENT`. The host zeroes its copy after the call.
- `start` loads the identity, starts networking if enabled, recovers download state, and schedules replica verification. It is idempotent.
- `stop` is idempotent and safe to call from the event-pump shutdown path. It does not destroy the handle.
- `destroy` stops if needed, stops the event queue, and removes the handle. Double destroy returns `NEXUS_STATUS_INVALID_HANDLE`.
- A handle is valid until `destroy` returns. No concurrent operation may use a handle after destroy begins.

### 4.2 Handle and memory ownership

| Item | Owner | Rule |
|---|---|---|
| Handle | Host | Opaque `uint64_t`; never dereferenced or forged. |
| Config input | Host | Borrowed for the call only; core copies what it needs. |
| Local secret input | Host | Borrowed and copied into zeroizing memory; host zeroes its array. |
| Request input | Host | Borrowed for the call only; not retained. |
| Response buffer | Core | Host must call `nexus_core_free_buffer` after copying. |
| Event buffer | Core | Host must call `nexus_core_free_buffer` after copying. |
| Streaming chunk input | Host | Borrowed for the call only; core copies to its buffer. |
| Export chunk output | Core | Host frees after writing to the destination. |

- Buffers are length-delimited; strings are UTF-8 and not NUL-terminated.
- A zero-length buffer may have a null pointer.
- Freeing a buffer twice is a host bug; the allocator metadata is not exposed.
- The host copies response bytes into Kotlin-managed arrays and frees the native buffer in a `finally` block.

## 5. CBOR envelopes

### 5.1 Request

```text
{
  "v": 1,
  "params": { ... }
}
```

### 5.2 Success response

```text
{
  "v": 1,
  "ok": true,
  "result": { ... }
}
```

### 5.3 Domain error response

```text
{
  "v": 1,
  "ok": false,
  "error": {
    "code": "quota_exceeded",
    "message": "Not enough managed storage for this file.",
    "retryable": false,
    "details": { ... }
  }
}
```

### 5.4 Event

```text
{
  "v": 1,
  "seq": 42,
  "ts": 1760000000000,
  "type": "download.progress",
  "op_id": 7,
  "download_id": "hex-or-null",
  "data": { ... }
}
```

Envelope fields and string keys are fixed. `details` is optional and bounded. Messages are user-readable and contain no keys, passphrases, local paths, or unnecessary peer identifiers.

### 5.5 Size limits

| Item | Limit |
|---|---|
| Config CBOR | 64 KiB |
| Request CBOR | 1 MiB |
| Response CBOR | 8 MiB |
| Event CBOR | 1 MiB |
| Import chunk | 1 MiB |
| Export read | 1 MiB |
| Event queue | 1,024 events, with progress coalescing |
| CBOR nesting | 16 levels |
| CBOR map entries | 256 |

Oversized input returns `NEXUS_STATUS_BUFFER_TOO_LARGE` or a domain `invalid_argument`; it is never partially parsed.

## 6. Method catalog

All production methods are available on both Android and Windows. Method IDs are stable and never reused. `Mode` is `S` for synchronous query, `A` for asynchronous operation with terminal event, and `D` for debug-only.

| ID | Method | Mode | Request fields | Result or terminal event |
|---|---|---|---|---|
| 0x0101 | `system.info` | S | none | App/core/protocol/schema versions, platform, capabilities. |
| 0x0102 | `system.health` | S | none | Core state, network state, storage health, queue depth. |
| 0x0103 | `system.snapshot` | S | `sections` | Consistent UI snapshot for startup and resume. |
| 0x0201 | `identity.create` | S | `display_name?` | Identity ID, created timestamp, backup warning flag. |
| 0x0202 | `identity.status` | S | none | Locked or ready, identity ID, key format version. |
| 0x0203 | `identity.export_backup` | A | `passphrase` | `identity.backup_ready` with bounded backup bytes. |
| 0x0204 | `identity.restore_backup` | A | `passphrase`, `backup_bytes` | `identity.restored` with identity ID. |
| 0x0205 | `identity.reset` | A | `confirmation` | `identity.reset_completed`; destructive and explicit. |
| 0x0301 | `profile.get_local` | S | none | Local profile body and verification state. |
| 0x0302 | `profile.publish` | A | `display_name`, `bio`, `avatar_content_id?`, `publication_refs` | `profile.published` with profile revision. |
| 0x0303 | `profile.lookup` | A | `identity_id` | `profile.lookup_completed` with verified record or not-found. |
| 0x0401 | `store.usage` | S | none | Quota, used bytes by category, free space, reserve. |
| 0x0402 | `store.set_quota` | S | `quota_bytes` | Effective quota and whether current usage exceeds it. |
| 0x0403 | `store.cleanup_preview` | S | none | Reclaimable bytes and items requiring confirmation. |
| 0x0404 | `store.cleanup` | A | `selection` | `store.cleanup_completed` with reclaimed bytes. |
| 0x0501 | `publication.create` | A | `content_id`, metadata fields, optional parent | `publication.published` with publication ID and record. |
| 0x0502 | `publication.unpublish` | A | `publication_id`, `confirmation` | `publication.unpublished` with tombstone ID. |
| 0x0503 | `publication.get` | S | `publication_id` | Local or cached verified record and tombstone state. |
| 0x0504 | `publication.list_local` | S | `cursor`, `limit` | Local publications with revision and serving state. |
| 0x0505 | `publication.lookup` | A | `publication_id` | `publication.lookup_completed` with verified record and freshness. |
| 0x0601 | `network.status` | S | none | Network state, peer count, transports, bootstrap summary. |
| 0x0602 | `network.peers` | S | `cursor`, `limit` | Observed peers, addresses, direction, last seen. |
| 0x0603 | `network.bootstrap_list` | S | none | Configured bootstrap and manual peers. |
| 0x0604 | `network.bootstrap_add` | S | `multiaddr` | Validated and persisted address. |
| 0x0605 | `network.bootstrap_remove` | S | `multiaddr` | Updated list. |
| 0x0606 | `network.reconnect` | A | none | `network.reconnect_completed` or bounded failure. |
| 0x0701 | `discovery.explore` | A | `category?`, `sort`, `cursor`, `limit` | `discovery.page` and `discovery.completed` with coverage. |
| 0x0702 | `search.query` | A | `query`, `filters`, `cursor`, `limit` | `search.page` and `search.completed` with coverage. |
| 0x0703 | `search.cancel` | S | `op_id` | Cancellation accepted. |
| 0x0801 | `download.start` | A | `publication_id` or `content_id` | `download.started` with download ID; terminal on completion. |
| 0x0802 | `download.list` | S | `state?`, `cursor`, `limit` | Downloads with verified progress and source. |
| 0x0803 | `download.pause` | A | `download_id` | `download.paused`. |
| 0x0804 | `download.resume` | A | `download_id` | `download.resumed` and transfer events. |
| 0x0805 | `download.cancel` | A | `download_id`, `keep_partial` | `download.cancelled`. |
| 0x0806 | `download.remove` | A | `download_id`, `confirmation` | `download.removed`; bytes removed only if unreferenced. |
| 0x0901 | `replica.add` | A | `publication_id` | `replica.verifying` and terminal active or failed. |
| 0x0902 | `replica.list` | S | `cursor`, `limit` | Replicas with size, verification time, serving state. |
| 0x0903 | `replica.pause` | A | `replica_id` | `replica.paused`. |
| 0x0904 | `replica.resume` | A | `replica_id` | `replica.resumed`. |
| 0x0905 | `replica.remove` | A | `replica_id`, `confirmation` | `replica.removed`. |
| 0x0906 | `replica.verify` | A | `replica_id` | `replica.verified` or failed. |
| 0x0A01 | `settings.get` | S | none | Persisted settings. |
| 0x0A02 | `settings.set` | S | changed fields | Effective settings and restart requirements. |
| 0x0A03 | `diagnostics.export` | A | `sections` | `diagnostics.ready` with redacted archive bytes. |
| 0x0B01 | `test.echo` | D | `payload` | Echo payload for Stage 08 integration. |
| 0x0B02 | `test.fault` | D | `fault`, `delay_ms` | Injected fault for Stage 08 and 19 tests. |

Method IDs are grouped by domain. A future minor ABI may add methods in unused IDs; a released ID is never renumbered. Unknown IDs return `NEXUS_STATUS_UNSUPPORTED` with a domain error.

### 6.1 Async operation semantics

- `invoke` for an async method validates the request and returns a success response containing `op_id` and any immediately known IDs, such as `download_id`.
- The operation then runs on the core runtime. It emits zero or more progress events and exactly one terminal event: `*.completed`, `*.failed`, or `*.cancelled`.
- Cancellation is cooperative. `nexus_core_cancel` is idempotent; cancellation of an already terminal operation returns a domain `not_found` or `conflict` according to the operation state.
- If the event queue is full, progress events are coalesced by operation. Terminal events apply backpressure and are never dropped.
- The UI reconciles from `system.snapshot` after suspension, so missing progress events do not corrupt state.

## 7. Domain error codes

| Code | Retryable | Typical cause |
|---|---|---|
| `invalid_argument` | No | Malformed field, bad length, unsupported category. |
| `not_found` | Sometimes | Missing record, content, download, or peer. |
| `conflict` | No automatic merge | Conflicting revisions, duplicate operation, invalid state transition. |
| `quota_exceeded` | After user action | Managed quota or free-space reserve would be exceeded. |
| `verification_failed` | No for the same bytes | Signature, manifest, chunk, or content mismatch. |
| `unsupported_version` | No | Unknown protocol, record, or manifest major version. |
| `network_unavailable` | Yes | No path, bootstrap, or source. |
| `timeout` | Yes | Bounded deadline expired. |
| `cancelled` | Yes | User or lifecycle cancellation. |
| `io_error` | Sometimes | Local file, database, or export failure. |
| `permission_denied` | After user action | Missing platform permission or inaccessible picker result. |
| `locked` | After user action | Identity is locked because the LEK is unavailable. |
| `internal` | With diagnostics | Guarded core fault. |

## 8. Streaming file APIs

File bytes never enter CBOR. The host streams them in bounded chunks.

### 8.1 Import

```c
NexusStatus nexus_core_import_begin(
    uint64_t handle,
    const uint8_t *display_name_utf8,
    size_t display_name_len,
    uint64_t expected_size,
    uint64_t *out_import_id);

NexusStatus nexus_core_import_write(
    uint64_t handle,
    uint64_t import_id,
    const uint8_t *data,
    size_t len);

NexusStatus nexus_core_import_finish(
    uint64_t handle,
    uint64_t import_id,
    NexusBuffer *out_summary);

NexusStatus nexus_core_import_abort(
    uint64_t handle,
    uint64_t import_id);
```

- `import_begin` validates size, quota, and free space before accepting bytes. `expected_size` is required and bounded to 4 GiB.
- `import_write` accepts at most 1 MiB per call and must be called sequentially for one import ID.
- The core writes to `store/tmp`, updates SHA-256 and chunk hashes, and never buffers the whole file.
- `import_finish` verifies the declared size, finalizes the manifest, promotes the blob, and returns a summary with `content_id`, `size`, `manifest_root`, and `chunk_size`.
- `import_abort` deletes the temporary file. Abandoned imports are cleaned on startup.
- The display name is metadata only; it never becomes a path.

### 8.2 Export

```c
NexusStatus nexus_core_export_open(
    uint64_t handle,
    const uint8_t *content_id,
    size_t content_id_len,
    uint64_t *out_export_id);

NexusStatus nexus_core_export_read(
    uint64_t handle,
    uint64_t export_id,
    uint32_t max_len,
    NexusBuffer *out_chunk);

NexusStatus nexus_core_export_close(
    uint64_t handle,
    uint64_t export_id);
```

- `export_open` verifies that the content is locally available and verified.
- `export_read` returns at most 1 MiB. A zero-length buffer indicates end of file.
- The host writes to a user-selected destination and must not overwrite silently.
- `export_close` releases the read cursor. The core never writes to the user's destination directly.
- Export is read-only and does not change reference counts.

### 8.3 Streaming thread rules

- Calls are synchronous but bounded. The Kotlin wrapper runs them on `Dispatchers.IO`.
- One import or export ID is used by one host coroutine at a time.
- No import or export call may run on the UI thread.
- A cancelled coroutine closes or aborts the stream; the core cleans up abandoned temporary state.

## 9. Event catalog

Events use the envelope in section 5.4.

| Event | Meaning | Data |
|---|---|---|
| `core.state` | Core lifecycle changed. | `state`, `reason?` |
| `identity.ready` | Identity unlocked. | `identity_id` |
| `identity.locked` | LEK or identity unavailable. | `reason` |
| `identity.backup_ready` | Backup export completed. | `backup_bytes`, `format_version` |
| `identity.restored` | Backup restore completed. | `identity_id` |
| `network.state` | Network state changed. | `state`, `peer_count`, `transports` |
| `network.peer_event` | Peer connected, disconnected, or failed. | `peer_id`, `event`, `reason?` |
| `publication.published` | Publication stored and announcement attempted. | `publication_id`, `content_id`, `announcement` |
| `publication.unpublished` | Tombstone stored and announced. | `publication_id`, `tombstone_id` |
| `download.started` | Download accepted. | `download_id`, `publication_id`, `content_id` |
| `download.progress` | Verified progress. | `download_id`, `verified_bytes`, `total_bytes`, `rate_bps`, `source_peer?` |
| `download.completed` | Final verification passed. | `download_id`, `content_id`, `size` |
| `download.paused` | Transfer paused with resumable state. | `download_id`, `verified_bytes`, `reason` |
| `download.failed` | Terminal failure. | `download_id`, `error`, `verified_bytes` |
| `download.cancelled` | Cancelled. | `download_id`, `partial_kept` |
| `replica.state` | Replica state changed. | `replica_id`, `state`, `last_verified_at?` |
| `search.page` | Search or Explore results page. | `op_id`, `items`, `next_cursor?`, `coverage` |
| `search.completed` | Search terminal result. | `op_id`, `coverage`, `conflicts` |
| `diagnostics.ready` | Redacted diagnostics archive ready. | `archive_bytes`, `sections` |
| `core.stopped` | Core stopped. | `reason` |

Terminal naming convention: an async operation emits `*.completed`, `*.failed`, or `*.cancelled` unless its catalog entry names a more specific terminal event. A terminal event always includes `op_id`.

## 10. JNI and JNA mapping

### 10.1 Common Kotlin interface

```kotlin
interface NexusCore {
    suspend fun start()
    suspend fun stop()
    fun events(): Flow<NexusEvent>
    suspend fun snapshot(): CoreSnapshot
    suspend fun createIdentity(): Identity
    suspend fun importFile(name: String, size: Long, source: FileSource): ContentSummary
    suspend fun publish(request: PublishRequest): Publication
    suspend fun startDownload(publicationId: PublicationId): DownloadHandle
    suspend fun search(query: String): Flow<SearchPage>
}
```

The real implementation maps these methods onto `invoke` and the streaming functions. The fake implementation uses in-memory state for UI tests.

### 10.2 Android JNI

- Kotlin object `NexusNative` declares `external` functions for the C ABI.
- Rust exports `Java_app_nexus_core_NexusNative_nativeAbiVersion`, `nativeCreate`, `nativeInvoke`, `nativeNextEvent`, and the streaming methods.
- A `JNI_OnLoad` registration table validates the ABI and caches class/method IDs.
- `jbyteArray` inputs are copied to a Rust slice for the call; outputs are copied to a new `jbyteArray`, then the native buffer is freed.
- `System.loadLibrary("nexus_core")` loads the packaged `libnexus_core.so`.
- The JNI shim is thin and calls the same internal C ABI functions as the Windows build.

### 10.3 Windows JNA

- Kotlin interface `NexusLibrary : Library` maps the C functions with JNA `Pointer` and `NexusBuffer.ByValue` structures.
- `Native.load("nexus_core", NexusLibrary::class.java)` loads `nexus_core.dll` from the application directory.
- The buffer structure uses native `size_t` layout for x64: a pointer followed by a 64-bit length.
- Output buffers are copied into `ByteArray` via `Pointer.getByteArray`, then freed with `nexus_core_free_buffer`.
- JNA calls are dispatched on `Dispatchers.IO`; the event pump uses a dedicated thread.

### 10.4 Adapter parity tests

The same contract tests run against both adapters:

- ABI version and core version.
- Create, start, stop, destroy, double stop, double destroy.
- Invalid handle and invalid method ID.
- CBOR request and response round-trip.
- Malformed and oversized CBOR.
- Event ordering, timeout, queue backpressure, and cancellation.
- Import/export chunking and end-of-file.
- Panic containment through `test.fault`.
- 100 lifecycle cycles with no leaked native memory or threads.

## 11. Versioning and compatibility

| Change | Allowed in ABI 1? | Action |
|---|---|---|
| Add an optional request field | Yes | Older core ignores it; Kotlin sends only when supported. |
| Add an optional response field | Yes | Older Kotlin ignores it. |
| Add a new method ID | Yes | Kotlin feature-detects via `system.info` capabilities. |
| Change a field type or meaning | No | New ABI and coordinated release. |
| Remove a method | No | Deprecate, then remove only in a new ABI. |
| Change memory ownership | No | New ABI. |
| Change CBOR envelope shape | No | New ABI. |
| Change event terminal semantics | No | New ABI. |

- `nexus_core_abi_version()` must equal the Kotlin `EXPECTED_ABI`.
- The app checks the ABI before opening the database or starting the network.
- Protocol and database versions are independent of the ABI.
- The core reports its version and capabilities in `system.info`; the About and diagnostics screens display them.
- A debug build may expose `test.*` methods. Release builds return `NEXUS_STATUS_UNSUPPORTED` for them.

## 12. Security requirements

- The local secret is passed as a separate buffer, never inside config CBOR.
- The host zeroes the Kotlin byte array immediately after `create`. The core zeroizes its copy after deriving the key.
- Config, requests, and events never contain private keys, passphrases, or full identity backups except the explicit backup export result.
- Backup bytes are returned only to the requesting operation and are not logged or cached.
- Error details and logs redact local paths, keys, and unnecessary peer identifiers.
- The FFI validates every pointer, length, handle, method ID, and CBOR limit before use.
- Unsafe code is confined to `nexus-ffi` and the JNI shim, with documented invariants and fuzz coverage.
- Panic payloads are not exposed to the UI; only a redacted internal error and diagnostics marker are returned.
- The handle registry prevents use-after-free; stale handles return `NEXUS_STATUS_INVALID_HANDLE`.

## 13. Verification plan

| Stage | Check |
|---|---|
| 08 | C harness compiles against the header and exercises lifecycle, invoke, events, streaming, and errors. |
| 08 | Android JNI and Windows JNA pass the same contract suite. |
| 08 | ABI mismatch is rejected before database or network work. |
| 08 | 100 start/stop cycles show no leaked memory, threads, or handles. |
| 08 | A forced panic returns an internal error without unwinding into the JVM. |
| 12 | Import/export streaming handles a 4 GiB file with bounded memory. |
| 19 | Fuzzing covers malformed CBOR, oversized buffers, invalid handles, and cancellation races. |
