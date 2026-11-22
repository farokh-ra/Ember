# Direct File Transfer

| Field | Value |
|---|---|
| Document ID | NEXUS-ARCH-TRANSFER-001 |
| Version | 1.1.0 |
| Status | Implemented in Stage 12; publication serving gate added in Stage 14; Android runtime verification deferred |
| Date | 2026-11-22 |
| Related | [ADR-0005](../decisions/ADR-0005-libp2p-transports-discovery.md), [ADR-0007](../decisions/ADR-0007-protocol-boundaries-versioning.md), [FFI_INTERFACE.md](FFI_INTERFACE.md), [DHT_DESIGN.md](DHT_DESIGN.md) |

## 1. Purpose

Stage 12 adds direct, peer-to-peer file transfer on top of the authenticated
libp2p transport from Stage 09. A download fetches a canonical manifest and
bounded chunk slices from one connected peer at a time, verifies every chunk
before writing it, and promotes the file only after the complete content
matches its SHA-256 content ID.

The design has four properties:

1. **No central content service.** Bytes move directly between peers. The DHT
   supplies hints only; it never stores file bytes.
2. **No whole-file buffering.** At most one 1 MiB chunk is assembled in memory.
3. **No unverified bytes in the library.** A file enters the managed store only
   after manifest, chunk, and whole-file checks pass.
4. **Resume from verified bytes.** A pause, restart, or source failure resumes
   at the last durable chunk boundary, not at zero.

## 2. Wire protocol

The protocol ID is `/nexus/file/1`. Messages are canonical CBOR maps with a
fixed unsigned-key schema, carried inside a four-byte big-endian length prefix.
The protocol is registered next to `/nexus/test/1` in the same libp2p
`request_response` behaviour.

### 2.1 Metadata request

| Key | Field | Type | Meaning |
|---:|---|---|---|
| 1 | `version` | uint | `1` |
| 2 | `kind` | uint | `0` (metadata) |
| 3 | `selector` | uint | `0` content ID, `1` publication ID |
| 4 | `id` | bytes(32) | SHA-256 content ID or signed publication ID |

### 2.2 Chunk request

| Key | Field | Type | Meaning |
|---:|---|---|---|
| 1 | `version` | uint | `1` |
| 2 | `kind` | uint | `1` (chunk) |
| 4 | `content_id` | bytes(32) | SHA-256 of the complete file |
| 5 | `index` | uint | Zero-based 1 MiB chunk index |
| 6 | `offset` | uint | Byte offset inside the chunk |
| 7 | `length` | uint | Requested slice length |

`offset + length` must stay inside one chunk and `length` must be between 1 and
64 KiB. A chunk request carries no publication selector.

### 2.3 Metadata response

| Key | Field | Type | Meaning |
|---:|---|---|---|
| 1 | `version` | uint | `1` |
| 2 | `kind` | uint | `0` (metadata) |
| 3 | `manifest` | bytes | Exact canonical manifest body |
| 4 | `publication` | bytes | Optional exact signed publication envelope |

The manifest is at most 256 KiB. The optional signed publication is at most
64 KiB.

### 2.4 Chunk response

| Key | Field | Type | Meaning |
|---:|---|---|---|
| 1 | `version` | uint | `1` |
| 2 | `kind` | uint | `1` (chunk) |
| 5 | `data` | bytes | Exactly the requested slice, 1 byte to 64 KiB |

### 2.5 Typed error response

| Key | Field | Type | Meaning |
|---:|---|---|---|
| 1 | `version` | uint | `1` |
| 6 | `error` | text | Stable ASCII code, at most 128 bytes |

The source returns `content_unavailable` when it does not hold verified content
for the selector. Mixed responses (for example manifest plus chunk bytes) and
unknown keys are rejected by both encoder and decoder.

### 2.6 Slice sizing

A v1 chunk is 1 MiB, but one wire response is capped at 64 KiB. The downloader
issues 16 sequential slice requests per full chunk. This keeps a chunk message
small enough to cross a relayed circuit with a modest byte limit and keeps the
transport frame far below the 512 KiB message cap.

## 3. Bounds

| Item | Value |
|---|---|
| Protocol | `/nexus/file/1` |
| Wire version | 1 |
| Maximum encoded message | 512 KiB |
| Maximum chunk slice | 64 KiB |
| Maximum manifest body | 256 KiB |
| Maximum signed publication | 64 KiB |
| Maximum typed error | 128 bytes |
| Chunk size | 1 MiB |
| Maximum chunks per file | 4,096 |
| Maximum file size | 4 GiB |
| Inbound file requests per peer | 12,000 per minute |
| Rate-window table | 256 peers |

Every decoder checks the version, the message kind, the exact field set, the
byte lengths, and the slice arithmetic before allocating or serving bytes.

## 4. Serving side

`nexus-net` owns the transport and enforces wire bounds. It delegates content
decisions to the `FileProvider` trait, implemented by the core's
`CoreFileProvider` over the managed store.

A metadata request succeeds only when:

- The selector resolves to a stored manifest and, for a publication selector,
  the exact signed publication record.
- The manifest body parses and matches the signed publication's content ID,
  size, chunk size, and manifest root. The root commits to the exact canonical
  manifest bytes, including every chunk hash.
- The content record is `verified` and its stored size equals the manifest
  total.

A chunk request succeeds only when the content is verified, the index exists in
the manifest, `offset + length` stays inside that chunk, and the store can read
exactly that slice. The provider never constructs a path from request data; it
uses the stored content ID and manifest entry.

The serving node holds no transfer session. Each request is stateless, so a
disconnect or restart cannot leave a half-served file in a special state.

### 4.1 Publication serving gate

Stage 14 adds an explicit sharing decision on top of content verification. The
provider answers only when the content row is `verified` and at least one local
reason to share it exists:

- a local, non-tombstoned publication with `serving = 1`, or
- an active replica, or
- a retained completed download.

A cached remote publication is never a reason to serve. Unpublishing marks the
publication `tombstoned = 1` and `serving = 0`, so the next metadata or chunk
request is refused even though the local bytes remain. The gate is read from
stored state per request; it does not rely on an in-memory flag or a restart.

Publishing also announces a provider record for the content ID, and unpublishing
withdraws it unless another local reason to serve remains. A provider record is a
hint: the downloader still verifies the manifest, the signed publication when
present, every chunk, and the final content ID.

## 5. Download manager

The core runs downloads on a dedicated two-thread pool (`DOWNLOAD_WORKERS = 2`).
Each job has a cooperative `DownloadControl` with pause, cancel, and keep-partial
flags. One job runs these phases:

1. **Metadata.** Try each ordered source, up to two rounds, with a bounded
   backoff. The first valid metadata wins; its peer becomes the source.
2. **Verification.** Parse the manifest, require canonical bytes, check it
   against the signed publication when one is present, and check the requested
   content ID or publication ID. A mismatch fails with
   `verification_failed` and a stable `reason` detail.
3. **Registration.** Store the manifest by root, then create the download row
   and an empty partial file. Registration checks the file-size cap, managed
   quota, and free-space reserve.
4. **Chunks.** For each missing chunk, fetch 64 KiB slices from the current
   source, concatenate one chunk in memory, compare its SHA-256 with the
   manifest entry, write it at the chunk offset, flush, and checkpoint the new
   verified prefix.
5. **Promotion.** After the last chunk, verify the complete file against the
   content ID, promote the partial into the managed store, add a
   `retained_download` reference, and delete the download row.

### 5.1 Source failover

A chunk has four attempts. A failed slice or a hash mismatch advances to the
next source in the ordered list. After the last source, the next attempt wraps
to the first source after a 150 ms backoff. The download fails only after all
four attempts are exhausted.

Metadata uses two rounds over the source list. Invalid peer IDs fail
immediately as `invalid_argument`; transport failures are retried and then
mapped to a typed network or `not_found` error.

### 5.2 Corruption handling

A corrupt chunk is never written. The downloader discards the assembled bytes,
advances to another source, and refetches the same chunk. If every attempt
returns corrupt bytes, the download ends as `verification_failed` with reason
`chunk_hash_mismatch`, and the verified prefix remains resumable.

### 5.3 Disconnect handling

A peer that disconnects mid-transfer produces a transport error on the next
slice. The same retry and failover path applies. If no source remains, the
download ends as `download.failed` with a typed error and its verified prefix
intact. A later `download.resume` continues from that prefix.

## 6. Pause, resume, cancellation, and restart

### 6.1 Pause

`download.pause` sets a cooperative flag. The worker checks it between slices,
between chunks, and during retry waits. When observed, the current chunk is not
written; the last checkpointed prefix stays durable. The store records
`paused`, and a `download.paused` event reports `verified_bytes` and `reason`.

A pause requested before metadata registration is deferred until the download
row exists. The worker still fetches and verifies the manifest, registers the
row, and then pauses at the first chunk boundary, so the download remains
visible and resumable. If metadata fails before a row exists, the download ends
as `download.failed`; a pause is not reported for a row that was never created.
Cancellation is not deferred.

A pause request for a download that is not currently running persists the pause
directly if the row is `pending`, `downloading`, or already `paused`. A
completed download cannot be paused.

### 6.2 Resume

`download.resume` requires a stored row and a running network. It prefers the
stored `manifest_root`; if absent, it fetches metadata again and re-verifies it
against the stored content ID. It then checks:

- The stored content ID and expected size match the manifest.
- The verified prefix is not larger than the total size.
- The prefix is chunk-aligned (except when it equals the total size), because
  only complete chunks are ever checkpointed.

The store truncates any unverified tail, resets the state to `downloading`, and
the worker starts at `verified_prefix / 1 MiB`. Bytes before the prefix are
never requested again.

A resume without an explicit source uses the stored `source_peer`. A resume
with explicit `peer_id` or `sources` overrides it.

### 6.3 Process death

On engine start, `pause_interrupted_downloads` changes every row left in
`downloading` to `paused` with `last_error = "interrupted by restart"`. The
partial file and verified prefix are unchanged. The next resume re-verifies the
manifest and continues from the durable prefix.

A graceful `stop` asks every active download worker to pause before networking
shuts down, waits a bounded time for workers to checkpoint and exit, then marks
any remaining `downloading` row paused. A normal application exit therefore
stays resumable instead of becoming a failure.

### 6.4 Cancellation and removal

`download.cancel` stops a running job and either deletes the partial or keeps
it. `keep_partial = true` leaves the verified bytes in place and marks the row
`cancelled`; a later resume can adopt them. `download.remove` requires
`confirmation = true`, refuses a running download, and refuses to delete a
completed library entry. Removal deletes the partial file and the row.

## 7. Disk space, quota, and paths

- `begin_download` charges the full expected size against the managed quota and
  free-space reserve before any bytes are written.
- Each checkpoint charges the newly verified delta, so quota and free space are
  re-checked as the transfer grows.
- A full quota or an exhausted reserve fails the download with
  `quota_exceeded`; existing verified bytes stay resumable.
- Partial files live under the managed store's generated partial path. The
  download ID is validated as an ASCII component; user input never chooses a
  path.
- The display filename comes from the signed publication or a generated
  `download-<content-prefix>.bin` fallback and is passed through
  `safe_display_name`: path separators, control characters, Windows-forbidden
  characters, trailing dots/spaces, and reserved device names are neutralized.
- A completed download is stored by content ID, not by display name, so two
  files with the same name cannot overwrite each other.

## 8. Core methods and events

The ABI remains 1. Stage 12 adds six methods:

| ID | Method | Mode | Request | Result |
|---|---|---|---|---|
| 0x0801 | `download.start` | A | `publication_id` or `content_id` (or both), `peer_id` or `sources`, optional `download_id` | `{download_id, accepted}` |
| 0x0802 | `download.list` | S | `state?`, `cursor`, `limit` | `{items, next_cursor?}` |
| 0x0803 | `download.pause` | A | `download_id` | Record or accepted |
| 0x0804 | `download.resume` | A | `download_id`, optional selector and sources | Stored record |
| 0x0805 | `download.cancel` | A | `download_id`, `keep_partial?` | Record or accepted |
| 0x0806 | `download.remove` | A | `download_id`, `confirmation` | `{download_id, removed}` |

`download.list` returns at most 100 records per page (50 by default) and
includes `download_id`, `content_id`, `display_name`, `expected_size`,
`verified_prefix`, `state`, `source_peer`, `manifest_root`, `last_error`,
`created_at`, and `updated_at`.

Events:

| Event | Data |
|---|---|
| `download.started` | `download_id`, `content_id`, `total_bytes`, `source_peer`, `publication_id?`, `display_name` |
| `download.progress` | `download_id`, `verified_bytes`, `total_bytes`, `rate_bps`, `source_peer` |
| `download.resumed` | `download_id`, `content_id`, `verified_bytes`, `total_bytes` |
| `download.paused` | `download_id`, `verified_bytes`, `reason` |
| `download.completed` | `download_id`, `content_id`, `size` |
| `download.failed` | `download_id`, `error`, `verified_bytes` |
| `download.cancelled` | `download_id`, `partial_kept` |
| `download.removed` | `download_id`, `removed` |

Progress events are coalesced to at most one per 250 ms per download; the final
progress before promotion is forced. A terminal event is durable and is never
dropped.

## 9. State model

| State | Meaning |
|---|---|
| `pending` | Accepted; the worker has not registered the row yet. A pause or cancel requested in this window is applied after registration. |
| `downloading` | Worker is fetching and checkpointing chunks. |
| `paused` | Stopped with a durable verified prefix. |
| `failed` | Terminal error; the prefix may still be resumed. |
| `cancelled` | Stopped by the user; the partial may have been kept. |
| `completed` | Transient only; promotion deletes the download row. |

A completed file becomes a normal verified content record with a
`retained_download` reference. It is no longer listed as a download.

## 10. Limitations

- One source is used at a time. Ember does not run parallel chunk swarms or
  erasure coding.
- A source must be connected before a download starts. The manager does not
  dial peers itself.
- Resume requires the manifest to remain stored or re-fetchable. If the
  manifest is lost and every source is offline, the partial cannot be resumed.
- The content is served only while the source is online, the content is
  verified, and the source still has a local reason to share it. Unpublishing
  stops local serving but cannot delete copies other peers already hold. There
  is no separate seeding protocol, incentive, or availability guarantee.
- A publication record does not prove availability. The DHT record and provider
  hint can outlive a publisher that has gone offline, so the UI reports
  signature, serving, and availability separately.
- Previews are metadata only and limited to plain text and raster images. SVG,
  HTML, PDF, archives, binaries, and executable script extensions are not
  previewed, and a downloaded file is never opened automatically.
- Transfers are bounded by the 4 GiB file cap and the 1 MiB chunk model from
  Stage 07.
- Android runtime transfer behaviour is not exercised locally. The Android
  libraries cross-build with the same code; Windows loopback is the evidence
  platform.
- The Kotlin Downloads screen remains demo data. It displays speed, source
  peer, and progress fields, but it is not connected to the core yet.

## 11. Evidence

- Wire round-trip, bounds, and mixed-shape rejection:
  `crates/nexus-protocol/src/file.rs` unit tests.
- Loopback metadata and chunk exchange, unknown content, out-of-bounds and
  oversized slices: `crates/nexus-net/tests/stage12.rs`.
- Download manager completion, corruption retry, persistent corruption,
  publication mismatch, pause/resume, cancel, restart recovery, source
  failover, total disconnect, and display-name safety:
  `crates/nexus-core/src/downloads.rs` unit tests.
- Two real engines exchanging a 2.4 MiB file end to end, then re-exporting the
  exact bytes: `crates/nexus-core/tests/stage12.rs`.
- A publisher importing, signing, announcing, serving, and withdrawing a file,
  and a second engine discovering the record, downloading the bytes, and
  observing the tombstone: `crates/nexus-core/tests/stage14.rs`.
- Serving-gate, remote-cache, tombstone-retention, and manifest persistence
  tests: `crates/nexus-store/tests/stage14.rs`.

See [STAGE_12.md](../progress/STAGE_12.md) and
[STAGE_14.md](../progress/STAGE_14.md) for commands and totals.
