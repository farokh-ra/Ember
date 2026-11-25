# Direct File Transfer

| Field | Value |
|---|---|
| Document ID | NEXUS-ARCH-TRANSFER-001 |
| Version | 1.2.0 |
| Status | Implemented in Stage 12; publication serving gate added in Stage 14; multi-source scheduling added in Stage 17; Android runtime verification deferred |
| Date | 2026-11-25 |
| Related | [ADR-0005](../decisions/ADR-0005-libp2p-transports-discovery.md), [ADR-0007](../decisions/ADR-0007-protocol-boundaries-versioning.md), [FFI_INTERFACE.md](FFI_INTERFACE.md), [DHT_DESIGN.md](DHT_DESIGN.md) |

## 1. Purpose

Stage 12 adds direct, peer-to-peer file transfer on top of the authenticated
libp2p transport from Stage 09. A download fetches a canonical manifest and
bounded chunk slices from connected peers, verifies every chunk before writing
it, and promotes the file only after the complete content matches its SHA-256
content ID. Stage 17 lets one download take different chunks from several peers
at once.

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

The core runs downloads on a dedicated two-thread job pool
(`DOWNLOAD_WORKERS = 2`). Each job has a cooperative `DownloadControl` with
pause, cancel, and keep-partial flags, and spawns one transfer worker per usable
source, capped at four. One job runs these phases:

1. **Metadata.** Try each ordered source, up to two rounds, with a bounded
   backoff. The first valid metadata wins; its peer becomes the source.
2. **Verification.** Parse the manifest, require canonical bytes, check it
   against the signed publication when one is present, and check the requested
   content ID or publication ID. A mismatch fails with
   `verification_failed` and a stable `reason` detail.
3. **Registration.** Store the manifest by root, then create the download row
   and an empty partial file. Registration checks the file-size cap, managed
   quota, and free-space reserve.
4. **Chunks.** Up to four peers are scheduled concurrently, one chunk lease per
   peer at a time. Each leased chunk is fetched as 64 KiB slices, concatenated
   in memory, and compared with the manifest's SHA-256 for that chunk. A
   verified chunk is written at its offset, flushed, and recorded in the
   per-chunk map. Chunks may arrive out of order.
5. **Promotion.** After every chunk is verified, the complete file is checked
   against the content ID, promoted into the managed store, given a
   `retained_download` reference, and the download row is deleted.

### 5.1 Multi-source scheduling

Stage 17 adds a pure scheduler in `nexus-transfer`. It owns the pending chunk
set, at most one in-flight lease per chunk, and one lease per peer, so a chunk
is never fetched twice concurrently. A peer's rank comes only from observed
transfer outcomes: authenticated chunks served, transport failures, corrupt
responses, and measured chunk latency. Identity, publication popularity, and
provider claims never affect the score. A peer is marked `proven` only after it
serves a chunk whose hash matches the signed manifest, and proven peers are
preferred.

One worker thread is started per usable source, capped at four. With one source
the scheduler reproduces the Stage 12 sequential behavior; with several sources
it fills the free peers with the lowest pending chunks.

### 5.2 Source failover

A chunk has four total attempts across every source. A failed slice or a hash
mismatch returns the chunk to the pending set and the next lease prefers a
different peer; after a failure the worker waits 150 ms before retrying. A peer
that returns two corrupt responses is quarantined for the rest of the download.
A peer that fails transport three times consecutively is quarantined as well.
The download fails only when no usable source remains or one chunk exhausts its
attempts.

Metadata uses two rounds over the source list. Invalid peer IDs fail
immediately as `invalid_argument`; transport failures are retried and then
mapped to a typed network or `not_found` error. Once metadata is verified, its
peer is the first proven source but does not have to serve every chunk.

### 5.3 Corruption handling

A corrupt chunk is never written and never recorded in the chunk map. The
downloader discards the assembled bytes, returns the chunk to the scheduler,
and refetches it, normally from another peer. A single malicious source
therefore cannot corrupt the file: only chunks matching the signed manifest
reach the partial. If every attempt returns corrupt bytes, the download ends as
`verification_failed` with reason `chunk_hash_mismatch`, and every previously
verified chunk stays resumable.

### 5.4 Disconnect handling

A peer that disconnects mid-transfer produces a transport error on the next
slice. That lease is returned to the scheduler and the remaining chunks go to
the other peers. A quarantined or disconnected peer is skipped for the rest of
the download. If no usable source remains, the download ends as
`download.failed` with a typed error and its verified chunks intact. A later
`download.resume` continues from those chunks, possibly through a different
peer.

## 6. Pause, resume, cancellation, and restart

### 6.1 Pause

`download.pause` sets a cooperative flag. Every worker checks it between slices,
between chunks, and during retry waits. When observed, an in-flight chunk is not
written; already checkpointed chunks stay durable. The store records `paused`,
and a `download.paused` event reports `verified_bytes` and `reason`.

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
- The stored chunk map has one bit per manifest chunk.
- Every recorded chunk fits inside the partial file.

Every chunk the database records as verified is re-hashed from the partial file
before any peer is trusted again. A chunk that fails is cleared from the map and
refetched; a truncated or missing chunk is treated the same way. The store then
truncates the unverified tail to the end of the highest verified chunk and
resets the state to `downloading`. The scheduler resumes with only the missing
chunks, so chunks verified before a pause or restart are never requested again.

A resume without an explicit source uses the stored `source_peer`. A resume
with explicit `peer_id` or `sources` overrides it, and later chunks may come from
any listed source, not only the one that supplied earlier chunks.

### 6.3 Process death

On engine start, `pause_interrupted_downloads` changes every row left in
`downloading` to `paused` with `last_error = "interrupted by restart"`. The
partial file and chunk map are unchanged. The next resume re-verifies the
manifest and every recorded chunk, then continues from the missing chunks.

A graceful `stop` asks every active download worker to pause before networking
shuts down, waits a bounded time for workers to checkpoint and exit, then marks
any remaining `downloading` row paused. A normal application exit therefore
stays resumable instead of becoming a failure.

Reconciliation truncates a partial that is longer than its highest verified
chunk. If a partial is shorter than a recorded chunk, the download is marked
failed with the prefix aligned down to the last complete chunk, so a later
resume never starts from a mid-chunk position.

### 6.4 Cancellation and removal

`download.cancel` stops a running job and either deletes the partial or keeps
it. `keep_partial = true` leaves the verified bytes in place and marks the row
`cancelled`; a later resume can adopt them. `download.remove` requires
`confirmation = true`, refuses a running download, and refuses to delete a
completed library entry. Removal deletes the partial file and the row.

## 7. Disk space, quota, and paths

- `begin_download` checks the file-size cap, managed quota, and free-space
  reserve before any bytes are written.
- Each verified chunk charges its newly verified bytes, so quota and free space
  are re-checked as the transfer grows. Out-of-order chunks are charged once
  each and count toward usage immediately.
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
`verified_prefix`, `verified_bytes`, `state`, `source_peer`, `manifest_root`,
`last_error`, `created_at`, and `updated_at`. `verified_prefix` is the longest
contiguous verified prefix; `verified_bytes` counts every verified chunk,
including chunks beyond a gap.

Events:

| Event | Data |
|---|---|
| `download.started` | `download_id`, `content_id`, `total_bytes`, `total_chunks`, `source_peer`, `sources`, `publication_id?`, `display_name` |
| `download.progress` | `download_id`, `verified_bytes`, `total_bytes`, `rate_bps`, `verified_chunks`, `total_chunks`, `source_peer`, `sources` |
| `download.resumed` | `download_id`, `content_id`, `verified_bytes`, `total_bytes`, `verified_chunks`, `total_chunks`, `sources` |
| `download.paused` | `download_id`, `verified_bytes`, `reason` |
| `download.completed` | `download_id`, `content_id`, `size` |
| `download.failed` | `download_id`, `error`, `verified_bytes` |
| `download.cancelled` | `download_id`, `partial_kept` |
| `download.removed` | `download_id`, `removed` |

In `download.started` and `download.resumed`, `sources` is the ordered list of
peer IDs this download may use. In `download.progress`, `sources` is an array of
`{peer_id, attempts, successes, failures, corruptions, bytes, latency_ms,
proven, quarantined}` observed for this download only.

Progress events are coalesced to at most one per 250 ms per download; the final
progress before promotion is forced. A terminal event is durable and is never
dropped.

## 9. State model

| State | Meaning |
|---|---|
| `pending` | Accepted; the worker has not registered the row yet. A pause or cancel requested in this window is applied after registration. |
| `downloading` | Workers are fetching and checkpointing chunks. |
| `paused` | Stopped with durable verified chunks. |
| `failed` | Terminal error; verified chunks may still be resumed. |
| `cancelled` | Stopped by the user; the partial may have been kept. |
| `completed` | Transient only; promotion deletes the download row. |

A completed file becomes a normal verified content record with a
`retained_download` reference. It is no longer listed as a download.

## 10. Limitations

- One source at a time was the Stage 12 model. Stage 17 schedules up to four
  peers concurrently, one chunk lease per peer, and never more than four
  workers. Ember still does not use erasure coding.
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
  failover, total disconnect, display-name safety, multi-peer combination,
  malicious-peer isolation, source loss, cross-peer resume, and local-chunk
  repair: `crates/nexus-core/src/downloads.rs` unit tests.
- Scheduler ordering, one lease per chunk and peer, scoring from observed
  outcomes, corruption and failure quarantine, and a simulated two-source
  speedup: `crates/nexus-transfer/src/scheduler.rs` unit tests.
- Per-chunk persistence, sparse verified bytes, quota charging, clearing a
  chunk, and completion requiring every chunk:
  `crates/nexus-store/tests/storage.rs` and `crates/nexus-store/src/chunk_map.rs`.
- Two real engines exchanging a 2.4 MiB file end to end, then re-exporting the
  exact bytes: `crates/nexus-core/tests/stage12.rs`.
- One download taking verified chunks from two live engines, and completing
  when one listed peer cannot serve the file:
  `crates/nexus-core/tests/stage17.rs`.
- A publisher importing, signing, announcing, serving, and withdrawing a file,
  and a second engine discovering the record, downloading the bytes, and
  observing the tombstone: `crates/nexus-core/tests/stage14.rs`.
- Serving-gate, remote-cache, tombstone-retention, and manifest persistence
  tests: `crates/nexus-store/tests/stage14.rs`.

See [STAGE_12.md](../progress/STAGE_12.md), [STAGE_14.md](../progress/STAGE_14.md),
and [STAGE_17.md](../progress/STAGE_17.md) for commands and totals.
