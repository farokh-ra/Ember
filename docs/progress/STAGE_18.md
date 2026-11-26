# Stage 18 - Security, Abuse Resistance, and Privacy Hardening

**Status:** Complete on Windows loopback; merged into `main` as `8876350`
**Completed:** 2026-11-26
**Branch:** `feat/18-security`, based on `feat/16-search` at `8abaa50`
**Pull request:** #25 into `main`, merged; Stages 16 and 17 are merged through #24
**Progress guide:** 85-90%
**Next stage:** 19 - performance, compatibility, and release engineering
(requires explicit project-owner approval)

## 1. Scope

Stage 18 hardens the existing product rather than adding a new user feature. It
covers the Stage 18 work items and deliverables:

- A threat model for identity and key theft, fake records, Sybil behaviour, DHT
  poisoning, malicious peers, spam, and resource exhaustion.
- Validation, bounds, and malformed-input tests for incoming protocol messages
  and metadata.
- Local blocking and local abuse reporting without a central authority.
- Safe-file warnings and delegated previews. Downloaded content is never
  executed.
- Key-loss and public-visibility guidance.
- A log and diagnostics privacy review.
- Dependency and static analysis in CI.
- A security release checklist.

The stage does not add a central moderation service, malware scanning, network
anonymity, erasure coding, or a new file-transfer protocol.

## 2. Architecture

| Crate | Responsibility |
|---|---|
| `nexus-store` | Schema v11, `blocked_peers` and `abuse_reports`, caps, note bounds, ID validation, and local-only persistence. |
| `nexus-net` | Local block list state and enforcement on dial, inbound connection, inbound test/file/index request, discovery, and providers. |
| `nexus-core` | `0x0C01`-`0x0C06` security methods, blocklist refresh, blocked-source filtering, safety classification, and notices. |
| `nexus-protocol` | Malformed-input regression tests for file request and response decoding. |

No new network protocol, frame type, or Kotlin API is required. The FFI
namespace `0x0C` is new; `0x0A` remains reserved for settings and diagnostics.

## 3. Threat model and privacy review

The full documents are:

- [`docs/security/THREAT_MODEL.md`](../security/THREAT_MODEL.md)
- [`docs/security/PRIVACY_REVIEW.md`](../security/PRIVACY_REVIEW.md)
- [`docs/security/SECURITY_RELEASE_CHECKLIST.md`](../security/SECURITY_RELEASE_CHECKLIST.md)

The threat model records 34 threats across identity and key theft, fake records,
Sybil and DHT poisoning, malformed messages, abuse, unsafe files, privacy, local
device access, and supply chain. Each row records impact, controls, status, and
residual risk. It explicitly lists non-goals so the product does not imply
anonymity, network-wide takedown, or malware detection.

The privacy review records a data inventory, what is public by design, what stays
local, telemetry status, logging rules, retention, platform permissions, and a
list of findings. The main honest findings are that a metadata export is a full
local database snapshot, abuse reports have no per-report deletion control yet,
and Android platform backup behaviour is unverified.

## 4. Storage: schema version 11

`crates/nexus-store/migrations/0011_security_hardening.sql` adds:

| Table | Columns | Notes |
|---|---|---|
| `blocked_peers` | `peer_id` (primary key), `reason`, `note`, `created_at`, `updated_at` | No foreign key to observed peers, so a block survives observation pruning. |
| `abuse_reports` | `report_id` (autoincrement), `peer_id`, `category`, `note`, `created_at` | Append-only local reports. |

Indexes cover block creation time and reason, and report peer plus creation
time. `CHECK` constraints restrict reasons and categories to the documented
vocabulary, cap the note at the byte bound, and require a non-empty peer ID.

Store behavior:

- `block_peer` is idempotent per peer. Re-blocking updates the reason and note
  and keeps the original `created_at`.
- `unblock_peer` returns whether a row was removed.
- `record_abuse_report` inserts and returns the stored row.
- Caps come from `StoreConfig`: `max_blocked_peers` and `max_abuse_reports`,
  default 4096 each, validated from 1 to 65,536. The cap is checked and enforced
  inside the writer transaction, so concurrent attempts cannot exceed it.
- Notes are trimmed, empty notes become `null`, and notes are truncated at
  `MAX_SECURITY_NOTE_BYTES` (512) on a UTF-8 character boundary.
- Peer IDs use the existing safe-ID rule: at most 128 ASCII alphanumeric or
  `- _ . :` characters.
- `record_peer_observation` now truncates addresses to 512 bytes and error text
  to 256 bytes, so a hostile peer cannot grow the peer table with unbounded
  strings.
- Security tables are not added to the archive required-table list, so a
  pre-Stage-18 archive still restores. Migration 11 creates both tables empty
  when an older archive is restored.

The schema version is now 11. Hard-coded version assertions in storage, profile,
and local-import tests were updated to 11.

## 5. Network enforcement

`nexus-net` keeps a local `BTreeSet` of blocked peer IDs behind the existing
shared state. It is never announced, published, or included in a signed record.

`Network` exposes:

- `set_blocked_peers(Vec<String>)` replaces the list and deduplicates it.
- `blocked_peer_count()`.
- `is_peer_blocked(&str)`.

Enforcement points:

| Point | Behavior |
|---|---|
| Outbound dial | A blocked peer fails with `NetError::PeerBlocked` before a connection is attempted. |
| Inbound request (`test`, `file`, `index`) | Returns a `blocked` typed response without handling the payload. |
| Inbound connection | A blocked peer's connection is closed on the next driver step. |
| Discovery results | Blocked peers are removed from candidate and provider lists. |
| Existing request | Later requests on an established connection are refused. |

`NetError::PeerBlocked` has the stable code `peer_blocked`. The core maps it to
`permission_denied` with the message “The peer is blocked on this device.”

The core refreshes the network block list when the network starts and after every
successful block or unblock, so the store remains the source of truth.

## 6. Core security methods

| ID | Method | Behavior |
|---|---|---|
| `0x0C01` | `security.block_list` | `{count, cap, peers}`; each peer includes reason, note, timestamps, and `local_only: true`. |
| `0x0C02` | `security.block_add` | Blocks or updates a peer; emits `security.peer_blocked`. |
| `0x0C03` | `security.block_remove` | Returns `{peer_id, removed}`; emits `security.peer_unblocked` only when a row was removed. |
| `0x0C04` | `security.report_add` | Appends a local report and returns its `report_id`. |
| `0x0C05` | `security.report_list` | Newest first, bounded `limit` from 1 to 200, returns `count`, `total`, and `local_only: true`. |
| `0x0C06` | `security.notices` | Standing safety notices with `auto_execute: false` and `previews: "delegated"`. |

`system.info` advertises the six new capabilities. All six methods require a
running core and never contact another peer.

Error behavior:

- Unknown reason or category, malformed note type, and notes above 512 bytes
  return `invalid_argument` before touching the store.
- A malformed peer ID is rejected by the store as an invalid ID and maps to
  `invalid_argument`.
- A block cap or report cap returns `invalid_config`, which maps to
  `invalid_argument` with a safe message.
- Unknown methods and stopped-core calls keep their existing behavior.

## 7. Blocked download sources

`download.start` and `download.resume` drop locally blocked sources before any
network request. When every source is blocked, the call returns
`permission_denied` and starts nothing. When some sources remain, the download
uses only the allowed peers. If a download is already running and its source is
blocked, the network layer refuses the next request and normal failover moves
the chunk to another source.

The check is deliberately before `require_network`, so a fully blocked source
list returns the block error even when the network is unavailable.

## 8. Safe files and previews

`crates/nexus-core/src/safety.rs` classifies a file by extension into
`document`, `media`, `archive`, `executable`, `script`, or `unknown`. It returns
a warning flag and a short message. Dangerous, archive, and unknown extensions
warn; ordinary documents and media do not.

Every `download.started`, `download.resumed`, and `download.completed` event and
every `download.list` row carries:

```text
safety: {
  category: "document" | "media" | "archive" | "executable" | "script" | "unknown",
  warn: bool,
  message: string,
  auto_execute: false,
  preview: "delegated"
}
```

`security.notices` returns five standing notices:

- `no_auto_execution`: Ember never runs a downloaded file.
- `delegated_preview`: previews are handed to a platform viewer or sandbox.
- `key_backup`: losing the identity key or backup loses the identity.
- `public_visibility`: published profiles and files are public.
- `local_blocking`: blocking and reporting are local and do not remove content
  from the network.

The classification is advisory. It has no execution path and does not claim to
detect malware.

## 9. Protocol hardening

- `nexus-protocol` file decoding gained a sweep that truncates a valid request at
  every length and mutates every byte, asserting no panic. It also checks an
  oversized message and a malformed response.
- `nexus-core` gained a malformed-request test covering empty input, invalid
  CBOR, deeply nested CBOR, and an oversized request buffer, plus a stopped-core
  test for all six security methods.
- The existing decoder limits (16 levels, 256 map entries, 8,192 items, 1 MiB
  file messages) remain authoritative.

## 10. Privacy and logging review

- No telemetry, analytics, advertising, remote logging, or automatic crash
  reporting exists.
- Secrets are never logged. Peer IDs, addresses, file names, and local paths are
  debug-only and redacted in shared output.
- Block and report notes are local user text and are never logged.
- Events and errors expose stable codes, bounded counts, and IDs, not file bytes
  or key material.
- A user-initiated metadata export is a full database snapshot and therefore
  includes local security rows. It is documented as private. Diagnostics export,
  when implemented, must exclude them.

## 11. Versioning

| Item | Value |
|---|---|
| Protocol | `0.1.0-stage-18` |
| FFI interface document | 1.11.0 |
| ABI | 1 (unchanged) |
| Database schema | 11 |
| Callable methods | 51 |

## 12. Verification evidence

Commands run on Windows with the pinned GNU Rust toolchain:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo audit
```

Results:

| Check | Result |
|---|---|
| Formatting | Passed |
| Clippy with `-D warnings` | Passed |
| Workspace tests | 372 passed, 0 failed |
| Stage 18 new tests | 19 |
| `cargo deny check` | Passed |
| `cargo audit` | No vulnerability reported |

New test count by area:

| Area | Tests |
|---|---|
| Store blocks, caps, notes, reports, and validation | 5 |
| Core methods, events, persistence, blocked sources, notices, malformed input | 7 |
| Migration 10 to 11 | 1 |
| Network block-list state | 1 |
| Protocol truncation and mutation | 1 |
| Safety classification and notices | 2 |
| Security value shaping | 2 |
| **Total** | **19** |

Additional evidence:

- `crates/nexus-store/tests/stage18.rs`: idempotent blocks, persistence across
  reopen, atomic caps, newest-first reports, UTF-8 note truncation, malformed
  IDs, and invalid config.
- `crates/nexus-core/tests/stage18.rs`: block round trip and events, restart
  persistence, report limits and validation, blocked source rejection, notices
  and capabilities, malformed requests, and stopped-core behavior.
- `crates/nexus-net/src/shared.rs`: block-list replace, deduplicate, clear, and
  lookup.
- `crates/nexus-protocol/src/file.rs`: truncation and single-byte mutation
  sweep.
- `crates/nexus-core/src/safety.rs`: category matrix and notice contents.
- `crates/nexus-core/src/security.rs`: local-only value shaping.

## 13. Exit criteria

| Criterion | Status |
|---|---|
| Known high-severity issues are fixed or explicitly blocked from release | No known high-severity issue in this stage; independent review still pending |
| Malformed peer messages cannot crash the app in tested cases | Passed for truncation, mutation, oversized input, deep nesting, and invalid CBOR |
| Downloaded content is never executed automatically | Passed; no execution path and every event says `auto_execute: false` |
| Users understand public visibility and key-loss risks | Identity, publish, and settings screens already carry public-visibility and no-recovery warnings; the new `security.notices` payload is not yet bound to those screens |
| Threat model, security tests, abuse controls, privacy review, and release checklist exist | Complete |

## 14. Deferred Android runtime evidence

Android runtime behavior remains deferred because no emulator or device is
available in this environment. The Android libraries cross-build against the same
Rust core, but Stage 18 does not claim on-device verification for blocking,
notification of notices, platform file handling, or platform backup behavior.
The release checklist records these gaps.

## 15. Failure evidence and fixes

- The first core Stage 18 test run failed because the test config used a 64 MiB
  quota, below the core's 1 GiB minimum. The test config now uses 1 GiB.
- Category and note validation originally surfaced as a raw `CoreError` from
  `invoke` instead of a domain error. The test now asserts the actual contract:
  parameter errors return `InvalidArgument`, and store ID errors return `Store`.
- The first workspace build could not find the MinGW linker because the PATH
  pointed at the toolchain root instead of `mingw64\bin`. The corrected path
  builds and links every test.
- Clippy found a useless `vec!` in the new protocol test and a needless borrow
  in the new store test; both were fixed.
- Formatting was applied after the first check.

## 16. Decisions and unresolved questions

Decisions:

- Blocking and reporting are local by design. There is no central moderation
  authority, no automatic upload, and no gossip of moderation data.
- Security tables do not reference observed peers, so moderation survives
  observation pruning.
- New tables are not required by the archive validator, preserving restore of
  pre-Stage-18 archives.
- The FFI namespace is `0x0C`, leaving `0x0A` reserved.
- File safety classification is advisory and has no execution path.
- Enforcement lives in the network layer and in download source selection, not
  only in the UI.

Unresolved:

- Per-report deletion is not implemented.
- Android runtime evidence is unavailable.
- No independent security review has been commissioned.
- Fuzzing and cancellation-race stress belong to Stage 19.
- The Kotlin UI does not yet render the safety map or the standing notices.

## 17. Next stage

Stage 19 covers performance, compatibility, release engineering, fuzzing, and
release evidence. It requires explicit project-owner approval before work starts.
