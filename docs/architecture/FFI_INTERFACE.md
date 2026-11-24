# Ember Core FFI Interface Draft

| Field | Value |
|---|---|
| Document ID | NEXUS-ARCH-FFI-001 |
| Version | 1.9.0 (ABI 1) |
| Status | Implemented through Stage 16; Android runtime verification deferred |
| Date | 2026-11-24 |
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
- The config is CBOR and contains app-private paths, locale, network settings, quota, and log level. It never contains the local secret. The required `abi_version` is checked before any directory, database, or network work, so a mismatch cannot create the data root.
- The local secret is exactly 32 bytes. A different length returns `NEXUS_STATUS_INVALID_ARGUMENT`. The host zeroes its copy after the call.
- `start` loads the identity, starts networking if enabled, recovers download state, and schedules replica verification. It is idempotent. Since Stage 09, `network_enabled` starts the transport stack; a networking failure never fails `start`. The core remains usable offline and reports the failure as `network.state` = `unavailable` with a stable `last_error` code.
- `stop` is idempotent and safe to call from the event-pump shutdown path. It closes the event queue after flushing durable state and does not destroy the handle.
- `destroy` stops if needed, stops the event queue, and removes the handle. Double destroy returns `NEXUS_STATUS_INVALID_HANDLE`; an unknown handle is rejected by every call helper before dispatch.
- A handle is valid until `destroy` returns. No concurrent operation may use a handle after destroy begins.

### 4.1.1 Config map

The create-time config is a canonical CBOR text-keyed map with a maximum of 64 KiB and 256 entries. Unknown keys are ignored so a newer host can talk to an older library.

| Key | Type | Required | Rule |
|---|---|---|---|
| `abi_version` | uint | Yes | Must equal `NEXUS_ABI_VERSION` (1). A mismatch returns `NEXUS_STATUS_ABI_MISMATCH` before storage work. |
| `data_root` | text | Yes | Absolute path with no `.` or `..` components; the core creates and locks it. |
| `app_version` | text | No | 1-64 bytes; defaults to `0.0.0`. |
| `locale` | text | No | 1-35 bytes; defaults to `en`. |
| `log_level` | text | No | `trace`, `debug`, `info`, `warn`, or `error`; defaults to `info`. |
| `network_enabled` | bool | No | Starts QUIC and TCP/Noise/Yamux transports with the core. A start failure never fails `start`; it is reported through `network.status`. Defaults to `false`. |
| `dht_mode` | text | No | `client` or `server`; defaults to `client`. A bootstrap node uses `server`. |
| `relay_fallback` | bool | No | Allows a failed direct dial to fall back to a connected community relay. Defaults to `true`. |
| `relay_server` | bool | No | Serves bounded Circuit Relay v2 circuits for other peers. Defaults to `false`. |
| `search_provider` | bool | No | Opts this node into the optional `/nexus/index/1` index-provider role. The role is announced best-effort after the DHT has a peer. Defaults to `false`. |
| `quota_bytes` | uint | No | 1-500 GiB; defaults to 10 GiB. |
| `enable_test_methods` | bool | No | Exposes `test.*`; defaults to `false`. Release callers must leave this false. |

`NativeConfig.encode()` in `shared/core-client` emits the application keys. It
omits optional role keys such as `dht_mode`, so an ordinary app node defaults to
`client`; a headless deployment sets `server` explicitly.

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
| Index wire message | 1 MiB |
| Event queue | 1,024 events, with progress coalescing |
| CBOR nesting | 16 levels |
| CBOR map entries | 256 |
| CBOR items | 8,192 |

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
| 0x0301 | `profile.get_local` | S | none | Local identity ID, profile or null, lifecycle state, freshness, retained conflicts, and verification state. |
| 0x0302 | `profile.publish` | A | `display_name?`, `username?`, `bio?`, `avatar_content_id?`, `publication_refs?`, `publish?`, `expected_revision?` | `profile.draft_saved` when `publish` is false, otherwise `profile.published` with revision and announcement outcome. |
| 0x0303 | `profile.lookup` | A | `identity_id` | `profile.lookup_completed` with a verified record, freshness, conflicts, and username-label collisions. |
| 0x0401 | `store.usage` | S | none | Quota, used bytes by category, free space, reserve. |
| 0x0402 | `store.set_quota` | S | `quota_bytes` | Effective quota, usage, free space, reserve, and storage health. |
| 0x0403 | `store.cleanup_preview` | S | none | Reclaimable bytes and items requiring confirmation. |
| 0x0404 | `store.cleanup` | A | `selection` | `store.cleanup_completed` with reclaimed bytes. |
| 0x0501 | `publication.create` | A | `content_id`, `display_filename`, `description?`, `category?`, `tags?`, `parent_content_id?`, `parent_publication_id?` | `publication.published` with publication ID, announcement and provider outcome, serving, availability, and preview. |
| 0x0502 | `publication.unpublish` | A | `publication_id`, `confirmation` | `publication.unpublished` with tombstone ID, withdrawal outcome, limits, and retained local bytes. |
| 0x0503 | `publication.get` | S | `publication_id` | Local or cached record, tombstone, signature, serving, availability, and preview. |
| 0x0504 | `publication.list_local` | S | `cursor?`, `limit?` | Local publications with metadata, revision, serving, announced, availability, and preview. |
| 0x0505 | `publication.lookup` | A | `publication_id` | `publication.lookup_completed` with verified record, source, providers, availability, conflict state, and preview. |
| 0x0601 | `network.status` | S | none | Network state, local peer ID, transports, listen/observed/confirmed addresses, peer and connection counts, counters, last error. |
| 0x0602 | `network.peers` | S | `cursor`, `limit` | Observed peers with state, direction, transport, address, last seen, RTT, protocols, agent version. |
| 0x0603 | `network.bootstrap_list` | S | none | Configured bootstrap and manual peers. |
| 0x0604 | `network.bootstrap_add` | S | `multiaddr` | Validated and persisted address; updated list. |
| 0x0605 | `network.bootstrap_remove` | S | `multiaddr` | Updated list. |
| 0x0606 | `network.reconnect` | A | `peer_id?` | `network.reconnect_completed` or bounded failure. |
| 0x0607 | `network.connect` | A | `multiaddr` | `network.connect_completed` with peer ID, transport, and address. |
| 0x0608 | `network.send_test` | A | `peer_id`, `payload` (bytes) | `network.test_message` with echoed payload and round-trip time. |
| 0x0609 | `network.discover` | A | `target?` (32-byte key) | `network.discovery_completed` with bounded peer hints, routing count, and elapsed time. |
| 0x060A | `network.relay_list` | S | none | Configured community relays. |
| 0x060B | `network.relay_add` | S | `multiaddr` | Validated and persisted relay address; updated list. |
| 0x060C | `network.relay_remove` | S | `multiaddr` | Updated relay list. |
| 0x060D | `network.connectivity` | S | none | Connectivity classification, NAT status, relay counts and limits, fallback counters, and a troubleshooting hint. |
| 0x0701 | `discovery.explore` | S | `mode?`, `category?`, `seed?`, `cursor?`, `limit?` | `mode`, `categories`, `results`, `seed`, `next_cursor?`, `page_complete`, `coverage`. |
| 0x0702 | `search.query` | S/A | `query`, `kind?`, `category?`, `tags?`, `sort?`, `cursor?`, `limit?`, optional `filters` map | Synchronous local page with `op_id`, `results`, `next_cursor?`, `coverage`; on the first page, `search.result_page` events and one terminal `search.completed`. |
| 0x0703 | `search.cancel` | S | `op_id` | `op_id`, `outcome` (`requested`, `already_terminal`, or `unknown`), `cancelled`. |
| 0x0801 | `download.start` | A | `publication_id` or `content_id` | `download.started` with download ID; terminal on completion. |
| 0x0802 | `download.list` | S | `state?`, `cursor`, `limit` | Downloads with verified progress and source. |
| 0x0803 | `download.pause` | A | `download_id` | `download.paused`. |
| 0x0804 | `download.resume` | A | `download_id` | `download.resumed` and transfer events. |
| 0x0805 | `download.cancel` | A | `download_id`, `keep_partial` | `download.cancelled`. |
| 0x0806 | `download.remove` | A | `download_id`, `confirmation` | `download.removed`; bytes removed only if unreferenced. |
| 0x0901 | `replica.add` | A | `publication_id?`, `content_id?`, `source_peer?`, `confirmation`, `metered?`, `battery_percent?` | `replica.verifying` then `replica.active` or `replica.failed`; only an already verified local copy can be hosted. |
| 0x0902 | `replica.list` | S | `cursor?`, `limit?`, `include_removed?` | Replicas with publication attribution, size, verification and serving state, timestamps, usage, and hosting policy. |
| 0x0903 | `replica.pause` | A | `replica_id` | `replica.paused`; bytes and the original record are retained. |
| 0x0904 | `replica.resume` | A | `replica_id`, `metered?`, `battery_percent?` | Re-verifies, then `replica.resumed` and `replica.active`, or `replica.failed`. |
| 0x0905 | `replica.remove` | A | `replica_id`, `confirmation` | `replica.removed`; stops serving and releases the local hosting reference. |
| 0x0906 | `replica.verify` | A | `replica_id` | `replica.verified` or `replica.verification_failed`; a paused replica stays paused. |
| 0x0907 | `hosting.limits_get` | S | none | Effective hosting limits, metered and battery signals, serving decision, and limits notice. |
| 0x0908 | `hosting.limits_set` | S | `upload_bytes_per_second?`, `allow_metered?`, `battery_min_percent?`, `metered?`, `battery_percent?` | Persisted limits and current serving decision; emits `hosting.limits_changed`. |
| 0x0A01 | `settings.get` | S | none | Persisted settings. |
| 0x0A02 | `settings.set` | S | changed fields | Effective settings and restart requirements. |
| 0x0A03 | `diagnostics.export` | A | `sections` | `diagnostics.ready` with redacted archive bytes. |
| 0x0B01 | `test.echo` | D | `payload` | Echo payload for Stage 08 integration. |
| 0x0B02 | `test.fault` | D | `fault`, `delay_ms` | Injected fault for Stage 08 and 19 tests. |

Method IDs are grouped by domain. A future minor ABI may add methods in unused IDs; a released ID is never renumbered. Unknown IDs return `NEXUS_STATUS_UNSUPPORTED` with a domain error.

Stage 16 implements forty-five callable methods: `system.info`, `system.health`, `system.snapshot`, the three `profile.*` methods above, the five `publication.*` methods above, `store.usage`, `store.set_quota`, the thirteen `network.*` methods above, the three search and Explore methods above, the six `download.*` methods, the six `replica.*` methods, the two `hosting.*` methods, `test.echo`, and `test.fault`. Every other row above is reserved for its owning stage and currently returns `NEXUS_STATUS_UNSUPPORTED`. `test.*` additionally requires `enable_test_methods` in the config.

### 6.1 Network method semantics

- `network.status` is always callable. Without a running network it returns `disabled` when `network_enabled` is false, otherwise `unavailable` with `last_error` carrying the start-failure code (for example `peer_key_invalid`).
- `listen_addresses` are local listeners. `observed_addresses` are addresses peers reported for this node. `confirmed_addresses` are only addresses validated end to end. The core never reports an address as publicly reachable or dialable from the internet; an empty `confirmed_addresses` list means exactly that.
- Network states are `disabled`, `starting`, `online` (at least one connected peer), `degraded` (listeners up, no peers), `unavailable`, and `stopping`. Peer states are `connecting`, `connected`, `disconnected`, and `failed`.
- `bootstrap_add` requires a complete `/p2p/<peer-id>` multiaddr, persists the list in the data root, and dials with exponential backoff from 1 s to 60 s. `bootstrap_remove` removes it. Both return the updated `{bootstrap: [...]}` list.
- `network.reconnect` without `peer_id` re-dials every bootstrap target; with `peer_id` it retries that peer if it is known. `network.connect` dials one explicit multiaddr. `network.send_test` sends the bounded `/nexus/test/1` payload to one connected peer.
- `network.discover` runs one bounded Kademlia peer-routing query. `target` is optional: a 32-byte key routes around that point, and an omitted target samples a random key. The terminal event carries at most 32 peers (sorted by peer ID, at most 8 addresses each), `routing_peers`, and `elapsed_ms`. It returns peer hints only; a caller must not treat a returned address as verified or publicly reachable.
- `network.connect` tries a direct dial first and only then a connected relay. `network.connect_completed` reports `path` = `direct` or `relay` and, for a relayed path, `relay_peer_id`. A direct connection is never replaced by a relay while it works. If every direct and relay attempt fails, the terminal event is `network.connect_failed` with `path`, optional `relay_peer_id`, and an `error` map containing `code`, `message`, and `retryable`.
- `network.relay_list` returns the merged compiled, configured, and persisted relay list. `network.relay_add` and `network.relay_remove` require a complete `/p2p/<peer-id>` multiaddr, persist the change in `network/relays.list`, and return the updated `{relays: [...]}` list. The list is bounded to 16 entries and 64 KiB.
- `network.connectivity` returns the single-word classification `disabled`, `connecting`, `relay`, `direct`, or `no_peers`, plus NAT status, relay counts, reservation/circuit limits, fallback and hole-punch counters, and a short `hint`. Relayed data connections take precedence over a direct connection to relay infrastructure.
- Relay and NAT status fields are available from `network.status`: `connectivity`, `nat_status`, `nat_public_address`, `nat_probes`, `relay_client_enabled`, `relay_fallback_enabled`, `relays_configured`, `relays_connected`, `relay_reservations`, `relay_reservation_failures`, `relay_circuits_established`, `relay_data_limit_bytes`, `relay_duration_limit_secs`, `fallback_attempts`, `fallback_successes`, `direct_connections`, `relayed_connections`, `hole_punch_failures`, `direct_upgrades`, `relay_server_enabled`, `relay_server_reservations`, `relay_server_circuits`, `relay_server_circuits_accepted`, `relay_server_circuits_closed`, `relay_server_denied`, `relay_server_max_circuit_bytes`, and `relay_server_max_circuit_duration_secs`.
- `network.peers` reports `path` = `direct` or `relay`, and `relay_peer_id` when the path is relayed.
- DHT status fields are available from `network.status`: `dht_mode`, `dht_routing_peers`, `dht_bootstrapped`, `dht_queries_started`, `dht_queries_failed`, `dht_records_accepted`, `dht_records_rejected`, `dht_providers_accepted`, `dht_providers_rejected`, `dht_publish_successes`, and `dht_publish_failures`.
- Each dial is bounded by the configured connect timeout (10 s by default) and each test exchange by the request timeout (8 s by default). A failed dial or exchange never stops the core or any other peer connection.
- A dial that authenticates to a different peer identity than the one in the multiaddr fails with `verification_failed` and details `net_code` = `wrong_peer_id`; the mismatched peer is never recorded as connected.
- Operational network failures (`disabled`, `unavailable`, `timeout`, `connection_failed`, `wrong_peer_id`, `peer_not_found`, `not_connected`, `rejected`, `message_too_large`) return domain failures. Malformed parameters, such as a missing `multiaddr` field or a payload of the wrong type, remain ABI-level `NEXUS_STATUS_INVALID_ARGUMENT`.
- `network.status.last_error` and every network domain error carry small stable codes (`net_code`) without socket details, addresses of unrelated peers, or key material.
- The Stage 09 stack listens on QUIC v1 and TCP with Noise and Yamux. Stage 10 adds `/nexus/kad/1.0.0` in client mode by default; `dht_mode: "server"` is for bootstrap nodes. Stage 11 adds Circuit Relay v2 fallback, DCUtR hole punching, and AutoNAT v1. mDNS is disabled in v1; discovery uses bootstrap addresses, Kademlia routing, and explicit connects.

### 6.2 Async operation semantics

- `invoke` for an async method validates the request and returns a success response containing `op_id` and any immediately known IDs, such as `download_id`.
- The operation then runs on a fixed pool of four worker threads. It emits zero or more progress events and exactly one terminal event: `*.completed`, `*.failed`, or `*.cancelled`.
- Cancellation is cooperative and idempotent. `nexus_core_cancel` returns `NEXUS_STATUS_OK` for a known running, completed, failed, or cancelled operation; an unknown `op_id` returns a domain `not_found` or `conflict` according to the operation state.
- If the event queue is full, progress events are coalesced per operation. Terminal events apply backpressure and are never dropped. Lifecycle and identity events use a non-blocking push; if the queue is saturated, they are counted as dropped rather than blocking the core.
- `test.fault` accepts `fault` = `async`, `async_progress`, `async_fail`, `not_found`, `invalid_argument`, `conflict`, `quota_exceeded`, `verification_failed`, `unsupported_version`, `network_unavailable`, `timeout`, `cancelled`, `io_error`, `permission_denied`, `locked`, or `panic`, plus optional `steps` (1-100), `delay_ms` (0-60,000), and `fail_code`. Domain faults return a normal response envelope with `ok=false`; `panic` is caught at the ABI boundary and returns `NEXUS_STATUS_INTERNAL`.
- The UI reconciles from `system.snapshot` after suspension, so missing progress events do not corrupt state.

### 6.3 Download method semantics

- `download.start` accepts `publication_id`, `content_id`, or both, as 32-byte hex; at least one is required. When both are present, the publication selects the metadata and the content ID must also match. It also requires `peer_id` or a non-empty `sources` array of at most 8 peer IDs. An optional `download_id` must be 1-128 ASCII alphanumeric, `-`, or `_` bytes and must not already exist. The immediate response is `{download_id, accepted: true}`; the transfer continues on the download pool.
- `download.start` requires a running core and a running network. It does not dial peers. Every source must already be connected, and the first source that returns valid metadata becomes the serving peer.
- `download.list` is a synchronous, paginated query. `state` filters by `pending`, `downloading`, `paused`, `failed`, or `cancelled`; `limit` is 1-100 and defaults to 50; `cursor` is the zero-based offset. Completed downloads are promoted into the library and are not listed.
- `download.pause` is cooperative. A running job stops before writing the next chunk and persists `paused` with its verified prefix. A job that is not running persists the pause directly when its state permits it. A pause requested before metadata registration is deferred until the row exists, so the download stays resumable. A completed download returns `conflict`.
- `download.resume` re-verifies the stored manifest root, or fetches metadata again when no root is stored, against the existing content ID and expected size. Only complete-chunk prefixes are resumable. The source defaults to the stored `source_peer`; `peer_id` or `sources` override it. A completed or already-running download returns `conflict`.
- `download.cancel` stops a running job. `keep_partial` defaults to false; when true the verified partial and row remain resumable. A completed download returns `conflict`.
- `download.remove` requires `confirmation: true`, refuses a running download, and refuses a completed library entry. It removes the download row and partial bytes.
- Every download emits exactly one terminal event: `download.completed`, `download.failed`, or `download.cancelled`. `download.paused` is a durable non-terminal report. A failure that races a pause or cancel is reported as the user's action.
- A manifest, publication, chunk, or whole-file mismatch returns `verification_failed` with a stable `details.reason`. A source that lacks the content returns `not_found`. Quota or free-space exhaustion returns `quota_exceeded`; verified bytes stay resumable.

### 6.4 Profile method semantics

- Profiles are exact signed records published under the DHT key derived from the owner's identity ID. The identity vault signs the canonical profile body with the profile domain tag; private keys never enter CBOR or the store.
- `profile.get_local` is synchronous and returns `identity_id`, `has_profile`, `profile` (or null), and `conflicts`. The profile map includes `revision`, `display_name`, `username`, `username_is_label`, `bio`, `avatar_content_id`, `publication_refs`, `state`, `freshness`, `stale`, `discoverable`, `signed`, `record_verified`, `updated_at_ms`, `published_at`, `last_checked_at`, and `conflict_count`.
- Profile states are `draft` (signed locally, not advertised), `pending` (signed and intended for publication, not yet announced), `published` (announced), and `remote` (a verified cache from another identity). Freshness values are `current`, `stale`, and `unknown`.
- The first `profile.publish` requires `display_name`. Later calls merge supplied fields with the existing signed record. Omitting every profile field republishes the existing signed bytes without incrementing the revision. `publish` defaults to true.
- `expected_revision` provides optimistic concurrency. A mismatch returns `conflict` with `details.current_revision` and leaves the stored record unchanged. Without it, local writes may advance but never roll back to a lower revision.
- Every successful field update increments the revision by one and signs a fresh record. The signed bytes are stored before network work starts. `publish: false` emits `profile.draft_saved` with `discoverable: false`; no DHT announcement is attempted.
- A publish attempt with networking disabled or unavailable keeps the row `pending` and emits `profile.published` with `discoverable: false`, `announcement: "deferred"`, and an `error` map. The signed record is not lost and can be republished later.
- `profile.lookup` verifies the signature, the requested identity ID, and the DHT key before caching. A newer revision replaces the cached row. An older revision is retained as the accepted record and reported with `stale: true`. A same-revision record with different bytes is retained in `conflicts` without replacing the accepted row.
- A cache served because the network returned no record or failed is labeled `stale`, including when the previous check was recent. A local row is returned with `source: "local"` and `stale: false`. A cached remote row whose last check is older than seven days is also presented as `stale`.
- Usernames are labels, not unique reservations. `username_collisions` lists every other known profile whose label matches case-insensitively (Unicode-aware lowercasing), each with its distinct `identity_id`. `username_is_label` is always true so the UI never presents a label as a global handle.
- `profile.publish` and `profile.lookup` are cancellable. Cancellation emits `profile.publish_cancelled` or `profile.lookup_cancelled`; a verification or storage failure during lookup emits `profile.lookup_failed`.
- Terminal events are `profile.published`, `profile.draft_saved`, `profile.lookup_completed`, `profile.lookup_failed`, `profile.publish_cancelled`, and `profile.lookup_cancelled`.

### 6.5 Publication method semantics

- Publishing is explicit. The host first streams bytes through the import API and passes the resulting verified `content_id` to `publication.create`; the method never accepts a local path. The signed record binds the uploader identity, content ID, size, manifest root, display filename, description, category, tags, revision, and timestamp.
- `publication.create` accepts `display_filename` or the legacy `filename`, an optional description of at most 4,096 UTF-8 bytes, one of `documents`, `images`, `audio`, `video`, `software`, `data`, `archives`, or `other`, and at most 10 tags of 1-32 UTF-8 bytes each. Tags are deduplicated in order. The immediate response is `{op_id, accepted: true, publication_id, content_id, revision: 1}`.
- The exact signed record is stored before network work starts. The publication row becomes local and serving immediately, and the pending `local_import` reference is adopted into a `publication_original` reference. A worker announces the record and starts providing the content ID. If networking is unavailable, the record and serving state are kept and `publication.published` reports `announced: false` with an error.
- `publication.published` carries `publication_id`, `content_id`, `revision`, `record_verified`, `announced`, `provider_announced`, `serving`, `availability`, `preview`, and an optional `error` map.
- `publication.get` is synchronous and returns `found`, `publication`, `tombstone`, `serving`, `availability`, `signature_verified`, and `preview`. The record map includes `identity_id`, `content_id`, `revision`, `title`, `description`, `filename`, `category`, `size_bytes`, `tags`, `published_at_ms`, `updated_at_ms`, `tombstoned`, `local`, `serving`, `announced`, `availability`, `signature_verified`, `preview`, and `tombstone`.
- `publication.list_local` lists rows owned by the current identity, sorted by publication ID. `limit` is 1-100 and defaults to 50; `cursor` is the last publication ID of the previous page. The result is `{identity_id, publications, next_cursor, total}`.
- `publication.lookup` verifies the record signature, the requested publication ID, and the derived DHT key before caching. It also looks up the signed tombstone key; a valid tombstone is cached and forces `tombstoned: true` and `availability: "record_only"`. A malformed or mismatched tombstone fails the lookup instead of being cached.
- A local row is returned with `source: "local"` and is not replaced by a remote record. A different record already cached under the same publication ID is a `conflict`: the accepted row is retained and the event reports `conflict: true`.
- Provider lookup returns bounded `{peer_id, addresses}` hints. Availability values are `local`, `available_now`, `seen_recently`, `no_known_source`, and `record_only`. A provider hint does not prove the peer will answer; availability is a last-observed hint, not a guarantee.
- `publication.unpublish` requires `confirmation: true` and a local publication owned by the current identity. It signs a tombstone bound to the identity, publication ID, content ID, and revision, verifies it against the publication, stores it, marks the row tombstoned and not serving, announces the tombstone, and withdraws the provider unless another local serving reason remains. Local bytes and the local original reference are retained. A non-owner, missing confirmation, or already-tombstoned row returns `conflict`.
- `publication.unpublished` carries `publication_id`, `tombstone_id` (`"{publication_id}:{revision}"`), `revision`, `record_verified`, `tombstone_announced`, `provider_withdrawn`, `serving`, `local_bytes_retained`, `record_removed`, `availability`, `limits`, and an optional `error` map. `record_removed` is always false.
- A serving node answers file metadata and chunk requests only when the content is verified, the hosting policy allows serving, and a local serving publication, active replica, or retained completed download exists. A cached remote publication never makes this device a source.
- Previews are metadata only: `{kind, supported, auto_execute: false, reason}`. `kind` is `text`, `image`, or `none`; `reason` is `plain_text`, `raster_image`, or `unsupported_format`. Only plain-text and raster-image extensions are eligible. SVG, HTML, PDF, archives, binaries, and executable script extensions are never previewed, and no path executes a file automatically.
- `publication.create`, `publication.unpublish`, and `publication.lookup` are cancellable. Cancellation emits `publication.publish_cancelled`, `publication.unpublish_cancelled`, or `publication.lookup_cancelled`. A verification or storage failure during lookup emits `publication.lookup_failed`.

### 6.6 Replica method semantics

- Hosting is explicit. `replica.add` requires `confirmation: true` and a running core. It accepts `publication_id`, `content_id`, or both as 32-byte hex; when both are present, the publication must describe the content. `source_peer` is optional attribution, not a download source: the method never downloads bytes.
- The content must already exist locally, verify against its content ID, and have a canonical manifest. When a publication is named, its exact signed bytes must verify, bind the publication ID, and validate the manifest. An incomplete or corrupt copy returns `verification_failed`; no active row or provider advertisement is created.
- A metered network that is not allowed, or a known battery level below a nonzero threshold, refuses `replica.add` and `replica.resume` with `conflict` and a `reason` detail before verification work begins.
- A successful add stores or reactivates the deterministic row `replica-<content_id>`, adopts an `active_replica` reference, and returns `{op_id, accepted, replica, usage, attribution_preserved, limits}` immediately. A worker starts providing the content ID and emits `replica.active`; if networking is unavailable, the replica stays active and the event reports `announced: false` with an error.
- `replica.list` is paginated by replica ID. `limit` is 1-100 and defaults to 50; `include_removed` defaults to false. Each replica includes `replica_id`, `content_id`, `publication_id`, `state`, `source_peer`, `verified_size_bytes`, `last_verified_at_ms`, `last_checked_at_ms`, `last_seen_at_ms`, `failure_count`, `last_error`, `announced`, `serving`, `attribution_preserved`, `created_at_ms`, and `updated_at_ms`. The result also includes `usage` (`total_count`, `active_count`, `paused_count`, `failed_count`, `storage_bytes`) and `hosting`.
- `serving` is true only while the row is active and the hosting policy allows serving. It is not a claim that another peer downloaded recently. `last_seen_at_ms` records the last successful advertisement or served metadata/chunk request.
- `replica.pause` is idempotent, keeps bytes, marks the row paused and not announced, and withdraws the provider if no other local serving reason remains. `replica.resume` re-verifies the complete file and manifest before the row becomes active; a corrupt copy returns `verification_failed` and stays failed. `replica.remove` requires `confirmation: true`, stops serving, releases the active replica reference, and never rewrites the original signed publication. Re-adding the same content reactivates the same row.
- `replica.verify` runs a full verification and emits `replica.verified` or `replica.verification_failed`. A successful verification of a paused replica updates its size and timestamps but does not resume it. A failed replica becomes active only on a later explicit `replica.resume`.
- Startup verifies active replicas before advertising them. A maintenance thread re-checks replicas whose last successful verification is older than seven days and emits `replica.maintenance_completed`. A failed verification marks the row `verification_failed`, clears advertisement, and withdraws the provider unless another serving reason remains.
- `replica.add`, `replica.resume`, `replica.remove`, and `replica.verify` are cancellable. Missing confirmation or an invalid lifecycle transition returns `conflict`.

### 6.7 Hosting limits and quota

- `hosting.limits_get` returns `limits` (`upload_bytes_per_second`, `allow_metered`, `battery_min_percent`), `metered`, `battery_percent`, `serving_allowed`, `blocked_reason`, and `limits_notice`.
- `hosting.limits_set` merges supplied fields. The upload limit is `0` (unlimited) or 256 KiB/s-64 MiB/s. `battery_min_percent` is 0-100; 0 disables the battery gate. `allow_metered` defaults to false. Optional `metered` and `battery_percent` update host-reported signals without changing the persisted limits. The method persists the limits and applies the upload rate to the running network.
- `blocked_reason` is `metered_network` when the network is metered and not allowed, or `battery_low` when a known battery level is below a nonzero threshold. An unknown battery level never blocks. While blocked, inbound file metadata and chunk requests, including publication serving, are refused; stored replicas and their records remain intact.
- `store.set_quota` sets the managed-blob quota from 1 to 500 GiB, persists it, and returns quota, managed usage, free space, reserve, and storage health. A replica add or resume checks the quota first and returns `quota_exceeded` rather than evicting an existing publication or replica.
- Upload throttling spaces inbound file-chunk responses to the configured average rate. Metadata and error responses are not delayed. The current response is kept inside the peer request deadline, and bursts cannot reset the average.

### 6.8 Search and Explore method semantics

- `search.query` always returns a synchronous local page from the derived FTS5 index. The local index contains only verified profiles in `published` or `remote` state and non-tombstoned verified publications. Draft, pending, quarantined, invalid, and unsigned records never enter it.
- `query` is required and bounded to 128 UTF-8 bytes, 12 searchable tokens, and no C0/C1 controls or bidi overrides. `kind` is `profile` or `publication`. `category` is a closed publication category. `tags` contains at most 5 exact tags and every tag must match. `sort` is `relevance` (default), `recency`, or `availability`. `limit` is 1-20 and defaults to 20. The same fields may be supplied inside an optional `filters` map; top-level fields win.
- The synchronous response contains `op_id`, `query_hash`, `generation`, `sort`, `indexed_at_ms`, `deadline_ms`, `results`, `total_matches`, `next_cursor` (nullable), `page_complete`, `providers_pending`, `candidate_limit_reached`, and `coverage`. Every result carries `kind`, `record_id`, `content_id` (publications), `identity_id`, sanitized `title` and `summary`, `category`, `tags`, `revision`, `published_at_ms`, `updated_at_ms`, `availability`, `source` (`local` or `provider`), `cached`, `rank`, `conflicts`, `same_content_count`, and `verified`.
- `next_cursor` is opaque and bound to both the index generation and the normalized query hash, including text tokens, kind, category, tags, and sort. Reusing it with a different query returns `conflict`. Any local index write advances the generation, so a cursor from an older snapshot returns `conflict` with "The search index changed; restart the search." Callers must restart rather than retry the same cursor.
- On a cursorless first page while the network is `online`, the core discovers up to 8 optional `/nexus/index/1` providers through the DHT and fans out under one 10-second total deadline. Local results are never delayed. Each provider response streams as a `search.result_page` event; one terminal `search.completed` event is always emitted, including for local-only searches.
- Provider records are parsed and signature-verified in the core before display. Invalid, wrong-kind, oversized, or unsigned records are rejected and counted, and cannot affect local or other-provider results. Provider records are not written to local storage during search. Duplicates against the whole local index and against other providers are collapsed; differing revisions or metadata stay visible in `conflicts`.
- `coverage.label` is `local_only` (no provider participated), `cached_local` (only cached local rows), `partial` (at least one attempted provider did not complete), or `complete` (every attempted provider returned a complete valid page). `coverage.may_be_incomplete` is true only for distributed coverage that is not complete. `coverage.notes` is bounded and includes stable hints such as `local_index_only`, `no_index_providers`, `distributed_results_pending`, `provider_protocol_unsupported`, `provider_page_truncated`, and `search_cancelled`. Per-provider `state` is `ok`, `failed`, `timeout`, `unavailable`, `unsupported`, or `truncated`.
- Provider fan-out runs only for the cursorless first page. Later pages are local-index pages bound to the same generation; callers that need a fresh provider view start a new first-page search.
- `search.cancel` accepts the `op_id` returned by `search.query`. `outcome` is `requested` while the fan-out is running, `already_terminal` after it finished, or `unknown` for an unknown operation. Cancellation stops discovery and pending provider waits, retains already streamed local and verified provider rows, and emits the terminal event with `cancelled` true and the `search_cancelled` note.
- `discovery.explore` is local-only and synchronous. `mode` is `categories` (default), `recent`, or `random`. Categories mode returns counts for all eight closed categories and no result rows. Recent mode orders non-tombstoned publications by publish time. Random mode uses a bounded seeded shuffle; pass the returned `seed` with `next_cursor` for a stable next page. `limit` is 1-50 and defaults to 20.
- Sensitivity: query text, result rows, and provider IDs are safe plain text. Titles and summaries are sanitized and truncated before they leave the core. Provider IDs are peer identifiers, not identities or authorities, and `verified` means signature-valid, not endorsed.
- The optional provider role is enabled by `search_provider` at create time. A provider answers bounded `/nexus/index/1` requests from the same verified local index, re-reads exact signed bytes, and never returns draft or tombstoned records. The role announcement is best-effort and retried for up to 60 seconds after a DHT peer appears.

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

Network failures add a `net_code` detail from a small stable set: `disabled`, `invalid_config`, `invalid_address`, `missing_peer_id`, `invalid_peer_id`, `message_too_large`, `peer_key_locked`, `peer_key_unavailable`, `peer_key_invalid`, `not_running`, `busy`, `network_unavailable`, `timeout`, `connection_failed`, `wrong_peer_id`, `unsupported_protocol`, `not_found`, `not_connected`, `rejected`, `io_error`, `crypto_error`, and `driver_failed`. A wrong remote identity maps to `verification_failed` with `net_code` = `wrong_peer_id`.

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

- `import_begin` validates size, quota, and free space before accepting bytes. `expected_size` is required, non-zero, and bounded to 4 GiB.
- `import_write` accepts at most 1 MiB per call and must be called sequentially for one import ID. The core writes to `store/tmp`, updates the raw SHA-256 and per-chunk SHA-256 state incrementally, and never buffers the whole file.
- `import_finish` verifies the declared size, builds the canonical flat 1 MiB manifest, stores it under its trusted root, and adopts the staged file into the blob tree without a second copy. The returned summary contains `content_id`, `size`, `manifest_root`, `chunk_size`, and `display_name`.
- A completed import is retained by a `local_import` content reference until a publication, download, or replica takes ownership. Importing identical bytes again discards the new staged copy and keeps the existing verified blob.
- `import_abort` deletes the temporary file. Abandoned imports and staging files are cleaned on startup.
- Empty imports, a final size that differs from `expected_size`, more than `expected_size` bytes, writes above 1 MiB, and I/O failures are rejected without publishing partial content.
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
- `export_read` returns at most 1 MiB through a bounded read cursor. A zero-length buffer indicates end of file.
- The host writes to a user-selected destination and must not overwrite silently.
- `export_close` releases the read cursor. The core never writes to the user's destination directly.
- Export is read-only and does not change reference counts. A closed or unknown export ID returns `NEXUS_STATUS_INVALID_HANDLE`.

### 8.3 Streaming thread rules

- Calls are synchronous but bounded. The Kotlin wrapper runs them on `Dispatchers.IO`.
- One import or export ID is used by one host coroutine at a time.
- No import or export call may run on the UI thread.
- A cancelled coroutine closes or aborts the stream; the core cleans up abandoned temporary state.

## 9. Event catalog

Events use the envelope in section 5.4.

Stage 08 emits `core.state` (`created`, `starting`, `running`, `stopping`, `stopped`, `failed`), `core.stopped`, `identity.ready`, `identity.locked`, and the `test.progress` / `test.completed` / `test.failed` / `test.cancelled` family. Stage 09 adds the `network.state` / `network.peer_event` reports and the `network.reconnect_*`, `network.connect_*`, and `network.test_*` terminal events. Stage 10 adds the `network.discovery_*` terminal events. Stage 11 extends the peer and connect events with path and relay fields; no new event type is added. Stage 12 adds the `download.*` transfer family. Stage 13 adds the `profile.*` signed-profile family. Stage 14 adds the `publication.*` publishing family. Stage 15 adds the `replica.*` hosting family and `hosting.limits_changed`. Stage 16 adds `search.result_page` and `search.completed`; Explore is synchronous and emits no events. The remaining rows are reserved for their owning stages; they are documented now so IDs and semantics do not drift.

| Event | Meaning | Data |
|---|---|---|
| `core.state` | Core lifecycle changed. | `state`, `reason?` |
| `identity.ready` | Identity unlocked. | `identity_id` |
| `identity.locked` | LEK or identity unavailable. | `reason` |
| `identity.backup_ready` | Backup export completed. | `backup_bytes`, `format_version` |
| `identity.restored` | Backup restore completed. | `identity_id` |
| `network.state` | Network state changed. | `state`, `peer_count`, `transports`, `reason?` |
| `network.peer_event` | Peer connected, disconnected, or failed. | `peer_id`, `event`, `direction?`, `transport?`, `path?`, `relay_peer_id?`, `reason?` |
| `network.reconnect_completed` | Reconnect request finished. | `peer_id` (null for all bootstrap targets) |
| `network.reconnect_failed` | Reconnect request failed. | `peer_id`, `error` |
| `network.reconnect_cancelled` | Reconnect request cancelled. | `peer_id` |
| `network.connect_completed` | Connection established and authenticated. | `peer_id`, `transport`, `remote_address`, `established_ms`, `multiaddr`, `path`, `relay_peer_id?` |
| `network.connect_failed` | Direct and relay attempts failed or identity did not match. | `multiaddr`, `path?`, `relay_peer_id?`, `error` |
| `network.connect_cancelled` | Direct connection cancelled. | `multiaddr` |
| `network.test_message` | Transport-check exchange completed. | `peer_id`, `payload`, `round_trip_ms` |
| `network.test_failed` | Transport-check exchange failed. | `peer_id`, `error` |
| `network.test_cancelled` | Transport-check exchange cancelled. | `peer_id` |
| `network.discovery_completed` | Peer-routing query finished. | `target` (null when sampled), `peers` (bounded `{peer_id, addresses}` list), `routing_peers`, `elapsed_ms` |
| `network.discovery_failed` | Peer-routing query failed or timed out. | `target`, `error` |
| `network.discovery_cancelled` | Peer-routing query cancelled before completion. | `target` |
| `profile.draft_saved` | Signed local draft stored without announcement. | `identity_id`, `revision`, `state`, `discoverable`, `record_verified`, `updated_at_ms` |
| `profile.published` | Signed profile announcement finished or was deferred. | `identity_id`, `revision`, `state`, `discoverable`, `announcement`, `record_verified`, `updated_at_ms`, `error?` |
| `profile.lookup_completed` | Profile lookup finished, from the network or a cache. | `identity_id`, `found`, `source`, `freshness`, `stale`, `conflict`, `record`, `conflicts`, `username_collisions`, `username_is_label`, `checked_at_ms`, `error?` |
| `profile.lookup_failed` | Returned record failed verification or storage. | `identity_id`, `error` |
| `profile.publish_cancelled` | Publish cancelled before completion. | `identity_id`, `revision` |
| `profile.lookup_cancelled` | Lookup cancelled before completion. | `identity_id` |
| `publication.published` | Publication stored and announcement attempted. | `publication_id`, `content_id`, `revision`, `record_verified`, `announced`, `provider_announced`, `serving`, `availability`, `preview`, `error?` |
| `publication.publish_failed` | The stored publication could not be read or its ID was malformed. | `publication_id`, `error` |
| `publication.unpublished` | Tombstone stored and announced; local bytes retained. | `publication_id`, `tombstone_id`, `revision`, `record_verified`, `tombstone_announced`, `provider_withdrawn`, `serving`, `local_bytes_retained`, `record_removed`, `availability`, `limits`, `error?` |
| `publication.publish_cancelled` | Publish cancelled before completion. | `publication_id`, `serving` |
| `publication.unpublish_cancelled` | Unpublish cancelled before completion. | `publication_id`, `tombstone_id`, `local_bytes_retained` |
| `publication.lookup_completed` | Lookup finished from the network or a cache. | `publication_id`, `found`, `source`, `availability`, `stale`, `conflict`, `record`, `providers`, `checked_at_ms`, `error?` |
| `publication.lookup_failed` | Returned record or tombstone failed verification or storage. | `publication_id`, `error` |
| `publication.lookup_cancelled` | Lookup cancelled before completion. | `publication_id` |
| `download.started` | Download accepted and metadata verified. | `download_id`, `content_id`, `total_bytes`, `source_peer`, `publication_id?`, `display_name` |
| `download.progress` | Verified progress. | `download_id`, `verified_bytes`, `total_bytes`, `rate_bps`, `source_peer` |
| `download.resumed` | Transfer resumed from a verified prefix. | `download_id`, `content_id`, `verified_bytes`, `total_bytes` |
| `download.completed` | Final verification passed. | `download_id`, `content_id`, `size` |
| `download.paused` | Transfer paused with resumable state. | `download_id`, `verified_bytes`, `reason` |
| `download.failed` | Terminal failure. | `download_id`, `error`, `verified_bytes` |
| `download.cancelled` | Cancelled. | `download_id`, `partial_kept` |
| `download.removed` | Incomplete download removed. | `download_id`, `removed` |
| `replica.verifying` | A full replica verification started. | `replica_id`, `content_id` |
| `replica.active` | Replica verified and provider advertisement attempted. | `replica_id`, `content_id`, `state`, `announced`, `verified`, `attribution_preserved`, `error?` |
| `replica.failed` | Hosting or resume verification failed. | `replica_id`, `error` |
| `replica.paused` | Hosting paused; bytes and the original record retained. | `replica_id`, `content_id` |
| `replica.resumed` | A paused or failed replica verified and resumed. | `replica_id`, `content_id` |
| `replica.removed` | Hosting stopped and the local reference released. | `replica_id`, `content_id`, `publication_id`, `publication_preserved`, `attribution_preserved` |
| `replica.verified` | Explicit re-verification succeeded. | `replica_id`, `content_id`, `verified`, `verified_size_bytes`, `state`, `announced`, `checked_at_ms`, `error?` |
| `replica.verification_failed` | Full verification failed; advertising and serving stopped. | `replica_id`, `content_id`, `error` |
| `replica.cancelled` | Replica verification or announcement cancelled. | `replica_id`, `action` |
| `replica.maintenance_completed` | Startup or periodic verification pass finished. | `checked`, `failed`, `skipped`, `cancelled` |
| `hosting.limits_changed` | Hosting limits or host signals changed. | `limits`, `hosting` |
| `search.result_page` | Verified provider additions for one running search. | `op_id`, `query_hash`, `provider_id`, `items`, `conflicts`, `coverage` |
| `search.completed` | One search fan-out reached a terminal state; local-only searches also emit it. | `op_id`, `query_hash`, `coverage`, `new_results`, `rejected_records`, `ignored_records`, `conflict_count`, `cancelled`, `elapsed_ms` |
| `diagnostics.ready` | Redacted diagnostics archive ready. | `archive_bytes`, `sections` |
| `core.stopped` | Core stopped. | `reason` |
| `test.progress` | Debug operation progress; coalescible. | `op_id`, `step`, `total` |
| `test.completed` | Debug operation completed. | `op_id`, `steps` |
| `test.failed` | Debug operation failed. | `op_id`, `error` |
| `test.cancelled` | Debug operation was cancelled. | `op_id`, `steps` |

Terminal naming convention: an async operation emits `*.completed`, `*.failed`, or `*.cancelled` unless its catalog entry names a more specific terminal event. A terminal event always includes `op_id`.

## 10. JNI and JNA mapping

### 10.1 Shared Kotlin client

`shared/core-client` exposes `NativeCoreClient`, which implements the common `NexusCore` lifecycle/events contract and adds the generic native surface:

- `invoke(methodId, params)` encodes `{v, params}`, throws `NexusException.Domain` for `ok=false`, and returns the decoded `result`.
- `cancel(opId)` requests cooperative cancellation.
- `importBegin` / `importWrite` / `importFinish` / `importAbort` expose streaming imports.
- `exportOpen` / `exportRead` / `exportClose` expose bounded exports.
- `create(bridge, config, localSecret, scope, dispatcher)` checks the ABI before calling `create`, zeroes the caller's secret in a `finally` block, and starts one event-pump coroutine. The pump is the only `next_event` consumer and publishes decoded events into `nativeEvents`.

The fake `NexusCore` remains the in-memory implementation for UI tests.

### 10.2 Android JNI

- Kotlin `internal object io.nexus.core.client.NexusNative` declares `@JvmStatic external` functions. The JVM emits them as static native methods on class `io.nexus.core.client.NexusNative`.
- Rust therefore exports static symbols named `Java_io_nexus_core_client_NexusNative_nativeAbiVersion`, `nativeVersion`, `nativeCreate`, `nativeStart`, `nativeStop`, `nativeDestroy`, `nativeInvoke`, `nativeNextEvent`, `nativeCancel`, and the import/export methods. There is no `RegisterNatives` table; the names are checked into the shared library and by `javap`.
- `JNI_OnLoad` returns `JNI_ERR` when the compiled ABI does not match `nexus_core_abi_version()`. Per-call config validation repeats the check before storage work.
- `jbyteArray` inputs are copied to a Rust slice; outputs are copied to a new `jbyteArray`. No Rust pointer or native buffer is retained after the call.
- Result encodings:
  - `LongArray` of exactly two entries: `[status, value]` for create, import-begin, and export-open.
  - `ByteArray`: four little-endian status bytes followed by the payload, for invoke, next-event, import-finish, and export-read.
  - `Int`: status only, for start, stop, destroy, cancel, write, abort, and close.
- Every entry point is wrapped in `catch_unwind`. The byte-returning fallback is a status-payload array carrying `NEXUS_STATUS_INTERNAL`; no panic payload crosses JNI.
- `System.loadLibrary("nexus_core")` loads `libnexus_core.so`. The Windows DLL exports the same JNI symbols so the shim can be exercised on a Windows JVM.

### 10.3 Windows JNA

- JNA 5.14.0. `NexusLibrary : Library` maps the C functions with `Pointer`, `LongByReference`, and a `NexusBuffer` structure.
- `Native.load("nexus_core", NexusLibrary::class.java)` loads the packaged `nexus_core.dll`; tests load the absolute path from the `nexus.native.path` system property.
- `NexusBuffer` is `@Structure.FieldOrder("ptr", "len")` with a `Pointer` and a 64-bit `Long`, matching the x64 C layout. `NexusBuffer.ByValue` is used only for `nexus_core_free_buffer`.
- Output buffers are copied into `ByteArray` with `Pointer.getByteArray`, then freed exactly once in a `finally` block.
- `NativeCoreClient` runs `next_event` with a 50 ms timeout on the event-pump coroutine. JNA calls are synchronous and must not run on the UI thread.

### 10.4 Adapter parity and runtime evidence

The same adapter-independent contract runs against Windows JNA and the Windows-hosted JNI proxy:

- ABI version and library version.
- ABI mismatch rejected before create; the data root is not created.
- Unknown handles return `INVALID_HANDLE` for start, stop, destroy, and invoke.
- Create, idempotent start, stop/restart, destroy, and secret zeroing.
- `test.echo` request/response round-trip and `test.fault` domain error.
- Panic containment followed by a successful echo.
- Ordered asynchronous progress, coalesced progress count, terminal completion, and cancellation.
- 2.5 MB streaming import and export round-trip with 1 MiB bounds and EOF.
- 100 create/start/stop/destroy cycles in the Rust C ABI suite and the C harness.

The C harness compiles against `nexus_core.h` and covers the same lifecycle, echo, domain error, panic, progress, cancellation, ABI mismatch, and streaming contract on Windows. Android JNI runtime execution remains deferred until an emulator or device is available; the Android arm64-v8a and x86_64 builds are compiled and export-checked but not executed locally.

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

- `nexus_core_abi_version()` must equal `NativeCoreClient.ABI_VERSION`.
- The app checks the ABI before opening the database or starting the network.
- Protocol and database versions are independent of the ABI.
- The core reports its version and capabilities in `system.info`; the About and diagnostics screens display them.
- `test.*` methods are available only when the host sets `enable_test_methods`. Otherwise they return `NEXUS_STATUS_UNSUPPORTED`; a release host must never enable them.

## 12. Security requirements

- The local secret is passed as a separate buffer, never inside config CBOR.
- The host zeroes the Kotlin byte array immediately after `create`. The core zeroizes its copy after deriving the key.
- The device peer key is separate from the user signing identity, is stored encrypted at `network/peer_key.enc`, and is never included in identity backup or restore.
- Config, requests, and events never contain private keys, passphrases, or full identity backups except the explicit backup export result.
- Backup bytes are returned only to the requesting operation and are not logged or cached.
- Error details and logs redact local paths, keys, and unnecessary peer identifiers.
- The FFI validates every pointer, length, handle, method ID, and CBOR limit before use.
- Unsafe code is confined to `nexus-ffi` and the JNI shim, with documented invariants; fuzzing is scheduled for Stage 19.
- Panic payloads are not exposed to the UI; only a redacted internal error and diagnostics marker are returned.
- The handle registry prevents use-after-free; stale handles return `NEXUS_STATUS_INVALID_HANDLE`.

## 13. Verification plan

| Stage | Check | Status |
|---|---|---|
| 08 | C harness compiles against the header and exercises lifecycle, invoke, events, streaming, and errors. | Passed on Windows. |
| 08 | Android JNI and Windows JNA pass the same contract suite. | Windows JNA passed. The JNI shim passed through a Windows-hosted JVM proxy. Android runtime execution is deferred. |
| 08 | ABI mismatch is rejected before database or network work. | Passed in the Rust C ABI test, C harness, and both Kotlin adapters. |
| 08 | 100 start/stop cycles show no leaked memory, threads, or handles. | Passed in the Rust C ABI test and C harness. |
| 08 | A forced panic returns an internal error without unwinding into the JVM. | Passed through the C ABI, JNI proxy, and JNA. |
| 08 | Streaming import/export round-trips with 1 MiB bounds. | Passed at 2.5 MB in Rust, C, JNI proxy, and JNA. |
| 09 | Two nodes connect and exchange a test message over QUIC and over TCP/Noise/Yamux. | Passed in `nexus-net` and `nexus-core` loopback tests. |
| 09 | A dial that authenticates to the wrong peer identity fails with `verification_failed` and leaves the core usable. | Passed in the `nexus-core` Stage 09 suite. |
| 09 | Unreachable peers, a peer restart, and repeatable start/stop do not crash or hang the node. | Passed in the `nexus-net` Stage 09 suite. |
| 10 | A fresh node joins through bootstrap, discovers peers, and publishes/looks up a signed record and provider. | Passed in the `nexus-net` and `nexus-core` Stage 10 suites. |
| 10 | Losing one bootstrap peer does not disable discovery, and churn does not disable the entry point. | Passed in the `nexus-net` Stage 10 suite. |
| 11 | Direct connections are preferred, a failed direct dial falls back to a reachable relay, and no path reports `NoViablePath`. | Passed in the `nexus-net` and `nexus-core` Stage 11 suites. |
| 11 | Relay reservations report negotiated limits, relay lists persist across restart, and `network.connectivity` reports the expected classification and hint. | Passed in the `nexus-net` and `nexus-core` Stage 11 suites. |
| 12 | `/nexus/file/1` metadata and bounded chunk slices cross the wire, corrupt chunks are rejected, and pause/resume continues from verified bytes. | Passed in the `nexus-protocol`, `nexus-net`, and `nexus-core` Stage 12 suites. |
| 12 | A multi-megabyte file transfers between two engines and re-exports identical bytes with bounded memory. | Passed at 2.4 MiB in `nexus-core/tests/stage12.rs`; a 4 GiB transfer is not exercised at that size. |
| 13 | Two identities publish signed profiles through the DHT, verify and cache each other's records, and expose case-insensitive username-label collisions. | Passed in `nexus-core/tests/stage13.rs`. |
| 13 | A newer revision supersedes a cached one, and an unreachable publisher leaves a stale-labeled cache instead of a false success. | Passed in `nexus-core/tests/stage13.rs` and the `nexus-core` profile worker tests. |
| 14 | A publisher imports, signs, announces, serves, and withdraws a publication; a second engine discovers and verifies the record, downloads the content, and observes the signed tombstone. | Passed in `nexus-core/tests/stage14.rs`. |
| 14 | The serving gate refuses unowned or tombstoned content, and unpublish retains local bytes while reporting the limits. | Passed in `nexus-store/tests/stage14.rs` and the `nexus-core` publication worker tests. |
| 15 | A host explicitly replicates a file, verifies it before advertising, serves it to another peer after the original publisher withdraws, and keeps the original signed attribution. | Passed in `nexus-core/tests/stage15.rs`. |
| 15 | Pause, resume, remove, quota, metered, and battery policies stop serving without deleting the original record; a paused replica stays paused through verification. | Passed in `nexus-core/tests/stage15.rs` and `nexus-store/tests/stage15.rs`. |
| 15 | Corrupt local bytes are detected by explicit and startup verification, stop advertising and serving, and recover only after a valid re-verification. | Passed in `nexus-core/tests/stage15.rs`. |
| 16 | A verified local FTS5 index serves filtered, ordered, paginated search and Explore with the network disabled, and generation-bound cursors reject stale pages. | Passed in `nexus-core/tests/stage16.rs` and `nexus-store/tests/search.rs`. |
| 16 | A loopback index provider serves signed records that the searching core verifies and deduplicates, and provider loss keeps local results with honest reduced coverage. | Passed in `nexus-core/tests/stage16.rs`. |
| 16 | Unsigned, tampered, wrong-kind, oversized, and malformed provider records are rejected before display and are never written by search. | Passed in the `nexus-core` and `nexus-search` Stage 16 unit tests and the `nexus-protocol` index tests. |
| 19 | Fuzzing covers malformed CBOR, oversized buffers, invalid handles, and cancellation races. | Not yet implemented. |

The detailed evidence, commands, and test counts are in
[STAGE_08.md](../progress/STAGE_08.md), [STAGE_09.md](../progress/STAGE_09.md),
[STAGE_10.md](../progress/STAGE_10.md), [STAGE_11.md](../progress/STAGE_11.md),
[STAGE_12.md](../progress/STAGE_12.md), [STAGE_13.md](../progress/STAGE_13.md),
[STAGE_14.md](../progress/STAGE_14.md), [STAGE_15.md](../progress/STAGE_15.md), and
[STAGE_16.md](../progress/STAGE_16.md).
