# Stage 05 - Local Database and File-Store Foundation

**Status:** Complete
**Completed:** 2026-11-13
**Progress guide:** 18-23%
**Next stage:** 06 - Identity and cryptographic foundations (requires user permission)

## 1. Scope

Stage 05 establishes the durable local data foundation. It defines the SQLite
schema and migration path, stores file bytes in a managed content-addressed
store instead of database blobs, and supplies the repository layer that later
stages will call through the Rust core. It does not implement identity keys,
signatures, publishing, networking, transfer, or the final storage-management
screen.

The stage covers:

- Embedded, ordered, transactional SQLite migrations with rollback.
- Tables for identities, profiles, publications, tombstones, replicas,
  downloads, peers, settings, content references, and rebuildable search.
- A single-writer database worker with serialized read connections.
- A locked, generated, content-addressed managed file store.
- Safe filenames, path validation, collision-free export names, and symlink
  rejection.
- Quota accounting, a 1 GiB free-space reserve, and a 4 GiB per-file limit.
- Reference-counted deletion with confirmation for published originals and
  active replicas.
- Resumable download records and verified-prefix checkpointing.
- Startup reconciliation for crashes, orphan blobs, corrupt bytes, and stale
  partial files.
- Metadata snapshot export and validated restore in a versioned archive.

## 2. Deliverables

| Deliverable | Path | Status |
|---|---|---|
| SQLite schema and migrations | `crates/nexus-store/migrations` | Complete; versions 1 and 2. |
| Migration runner | `crates/nexus-store/src/migrations.rs` | Complete; checksums, history, rollback, upgrade test. |
| Database worker and read pool | `crates/nexus-store/src/db.rs` | Complete; one writer thread, bounded readers, WAL. |
| Managed file store | `crates/nexus-store/src/managed_store.rs` | Complete; atomic staging and promotion. |
| Quota policy | `crates/nexus-store/src/quota.rs` | Complete; product range and deterministic test policy. |
| Path and filename safety | `crates/nexus-store/src/paths.rs` | Complete; traversal, reserved names, 255-byte limit. |
| Repository/data-access layer | `crates/nexus-store/src/store.rs` | Complete; public `Store` facade. |
| Typed records | `crates/nexus-store/src/model.rs` | Complete; content, references, downloads, and metadata. |
| Metadata export and restore | `crates/nexus-store/src/backup.rs` | Complete; CBOR manifest plus raw SQLite snapshot. |
| Storage settings model | `settings` table plus `Store::set_setting` / `get_setting` | Complete in the data layer; the Kotlin settings screen remains a Stage 04 prototype until the FFI bridge. |
| Storage tests | `crates/nexus-store/tests/storage.rs` | Complete; 12 integration tests. |
| Evidence record | This document | Complete. |

## 3. Architecture and ownership

Stage 05 follows
[ADR-0004](../decisions/ADR-0004-rust-storage-ownership.md):

- Rust is the only reader and writer of the SQLite database and the managed
  store.
- The app-private root is supplied by the platform shell; in Stage 05 the Rust
  API takes it as an absolute path argument.
- Kotlin never opens, queries, copies, or scans these files.
- The public API lives in `nexus_store::Store`; the database and blob modules
  are private implementation details.
- The database and managed store live under the same app-private root so a
  later transfer stage can reason about one volume and one quota.
- File bytes never become SQLite `BLOB` values. Only metadata, signed record
  bytes, hashes, paths, and counters are stored in the database.

The Kotlin storage-access check is part of the final evidence. The scan
searched all `*.kt`, `*.kts`, and `*.java` files for these patterns:

```text
sqlite, SQLite, jdbc, JDBC, nexus.sqlite3, blobs/, partials/, ManagedStore
```

Result: no matches. The Kotlin shells contain no database or managed-store
access.

## 4. Root layout

```text
<app-private-root>/
  db/
    nexus.sqlite3
    nexus.sqlite3-wal
    nexus.sqlite3-shm
  store/
    .lock
    blobs/<aa>/<bb>/<content_id_hex>
    manifests/<content_id_hex>.cbor   (reserved; manifest import starts in Stage 07)
    partials/<download_id>.part
    tmp/
    quarantine/
```

- The root must be absolute. A relative root returns `UnsafePath`.
- `ManagedStore::open` creates the layout, rejects a symlinked root or
  generated directory, and takes an exclusive `.lock` with `fs2`.
- A second process opening the same root gets `StoreLocked`.
- Blob names are derived from the SHA-256 content ID: the first two hex
  characters, the next two, then the full 64-character lowercase ID.
- Callers cannot choose a blob path, manifest name, or partial name.

## 5. SQLite schema and migrations

### Migrations

| Version | Name | Contents |
|---:|---|---|
| 1 | `initial` | Core metadata schema and default settings. |
| 2 | `search-index` | FTS5 `search_index` virtual table. |

`crates/nexus-store/src/migrations.rs` embeds the SQL files at compile time.
Each migration:

- runs inside a transaction;
- records its version, name, timestamp, and SHA-256 checksum in
  `schema_migrations`;
- updates `PRAGMA user_version`;
- rolls back on failure, leaving the previous schema and data intact.

Startup rejects a database whose `user_version` is newer than the embedded
schema. The upgrade test creates a version 1 database with data, applies
version 2, and confirms the original rows survive. A forced-failure test
confirms the prior schema remains usable.

### Tables

| Table | Purpose |
|---|---|
| `schema_migrations` | Applied migration history and checksums. |
| `settings` | Local settings such as `managed_quota_bytes` and `download_retention_days`. |
| `contents` | Content ID, size, state, generated blob path, and timestamps. |
| `content_refs` | Reference type and owner for each retained content ID. |
| `identity_refs` | Public identity reference and key-file location. No private key bytes. |
| `profiles` | Profile revision, exact record bytes, display data, and avatar content ID. |
| `publications` | Signed publication record bytes, metadata, and tombstone flag. |
| `tombstones` | Signed unpublish records tied to a publication. |
| `downloads` | Resumable download state, expected size, verified prefix, partial path, and safe error text. |
| `replicas` | Explicit hosting choice, verification time, and lifecycle state. |
| `peers` | Redacted peer observation and last-seen data. |
| `search_index` | FTS5 derived index; rebuildable and not authoritative. |

All ordinary tables are `STRICT` and use `CHECK` constraints for lengths,
states, and non-negative counters; the FTS5 `search_index` virtual table is the
exception. Foreign keys are enabled on every connection. Tables that hold signed
records preserve the exact received bytes; the parsed columns exist only for
queries and display.

The schema stores no private keys, passphrases, signing seeds, real user data,
network observations, or file bytes.

## 6. Database connection model

- One dedicated writer thread owns the write connection. `Database::write`
  sends a closure to that thread and waits for its result. A writer closure
  never re-enters `Database::write`.
- Reads borrow from a small serialized pool of `query_only` connections.
- The writer sets `journal_mode=WAL`, `synchronous=FULL`,
  `wal_autocheckpoint=1000`, `temp_store=MEMORY`, `foreign_keys=ON`, a busy
  timeout, and a bounded page cache.
- Readers set `query_only=ON`, `foreign_keys=ON`, the same busy timeout, and
  the same cache bound.
- `Database::snapshot_to` runs `VACUUM INTO` on the writer connection, never on
  a read-only connection.
- Shutdown checkpoints and closes the writer and pooled readers cleanly.

This keeps the UI and future network tasks from writing directly and gives one
serialization point for quota checks, reference changes, and metadata commits.

## 7. Managed imports and path safety

An import follows this order:

1. Validate the source is a regular file, not a symlink or reparse point.
2. Reject a file above the configured maximum (4 GiB by default).
3. Check quota and free space before copying.
4. Copy the source into `tmp/` in 64 KiB chunks while hashing SHA-256.
5. Flush the staged file and verify its byte count.
6. Re-check quota after hashing so identical bytes that already exist are not
   charged twice.
7. Atomically promote the staged file into its generated blob path.
8. Commit the content row and reference in one database transaction.

The source file is opened read-only. It is never renamed, modified, or
deleted. If staging or the pre-promotion check fails, the staged file is
removed and the source remains untouched. If promotion succeeds but the
database commit fails, the promoted blob is left unreferenced and startup
reconciliation deletes it; the source is still untouched.

`safe_display_name` strips directory components and traversal, replaces
forbidden characters, handles Windows reserved names (`CON`, `PRN`, `AUX`,
`NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`), preserves the extension, and enforces 255
UTF-8 bytes. `unique_export_path` appends a numeric suffix instead of silently
overwriting an existing file. Content exports are written only to an explicit
destination outside the managed store.

## 8. Quota, free space, and limits

| Limit | Value |
|---|---|
| Default managed quota | 10 GiB |
| Product quota range | 1-500 GiB |
| Free-space reserve | 1 GiB |
| Maximum single file | 4 GiB |
| Minimum SQLite cache | 256 KiB |
| Default SQLite cache | 4 MiB |
| Busy timeout | 5 seconds |

- `QuotaPolicy::new` enforces the 1-500 GiB product range.
- `QuotaPolicy::custom` exists for deterministic tests with small quotas.
- `QuotaUsage` reports verified managed bytes plus the verified prefix of
  pending, downloading, paused, and failed partial downloads.
- `Store::ensure_capacity` checks the file limit, the managed quota, and the
  free-space reserve before work starts.
- A new download checks its expected size before creating a row. Each verified
  prefix then counts toward usage from the first allocated byte.
- Failed partial downloads remain counted because their bytes still occupy
  disk until the user cleans them up.
- Re-importing identical bytes under a full quota is allowed: the source is
  staged and hashed first, but the free-space reserve is still checked before
  the copy.
- A blocked import or download leaves existing content and references intact.

## 9. References and deletion

Three reference types are defined:

| Reference type | Confirmation before last-reference deletion |
|---|---|
| `publication_original` | Required. |
| `active_replica` | Required. |
| `retained_download` | Not required. |

`Store::add_reference` is idempotent for the same `(content_id, ref_type,
owner_id)`. `Store::release_reference` deletes the reference, then deletes the
content row and its blob only when no reference remains.

- Removing the last publication-original or active-replica reference without
  `confirmed = true` returns `ConfirmationRequired` and changes nothing.
- Removing a retained download while another reference remains returns
  `Retained { remaining }` and keeps the bytes.
- Removing the last retained download returns `BytesDeleted` only after the
  database commit, then removes the blob.
- `cleanup_partials` requires explicit confirmation and removes only
  incomplete download records older than the requested age.

## 10. Downloads, reconciliation, and crash safety

### Downloads

- `begin_download` validates the ID, content ID, expected size, display name,
  quota, and free space, then creates an empty partial and a `downloading` row.
- `checkpoint_partial` requires a monotonic prefix no larger than the expected
  size, verifies the partial is at least that long, truncates any unverified
  tail, records the prefix, and counts it toward quota.
- `complete_download` verifies the full partial, promotes it to a blob,
  creates or updates the content row, adds a retained-download reference, and
  deletes the download row in one transaction.
- `cancel_download` removes the row and partial bytes.

### Reconciliation

`Store::open` runs reconciliation before returning. `Store::reconcile` can be
called explicitly later. The report records:

- verified blobs;
- adopted completed downloads promoted before the database commit;
- quarantined unexpected or invalid files;
- deleted unreferenced generated blobs;
- removed unreferenced content rows;
- missing content rows;
- truncated partials;
- failed downloads whose partial disappeared;
- orphan partials moved to quarantine;
- removed temporary imports;
- bytes reclaimed.

Rules:

- A blob with a matching content row is hashed and size-checked. A mismatch is
  moved to `quarantine/` and marked `quarantined`.
- A quarantined content row is never downgraded to `missing` on a later open.
- A missing blob marks the content row `missing` and preserves its reference and
  metadata.
- A generated blob with no content row is deleted. If it matches a verified
  completed download, it is adopted instead of deleted.
- An unexpected blob path, invalid manifest name, bad partial, symlink, or
  reparse point is moved to `quarantine/` rather than followed or silently used.
- Partial files are truncated to the verified prefix recorded in the database.
- `tmp/` is cleared.
- Scans never recurse through a symlink and never follow one during cleanup.

## 11. Metadata export and restore

`Store::export_metadata` writes a single archive:

```text
offset  size  field
0       15    magic "NEXUS-METADATA\0"
15      2     format version, little-endian u16 (1)
17      4     manifest length, little-endian u32
21      N     CBOR manifest
21+N    ...   raw SQLite snapshot
```

The CBOR manifest contains the format name, format version, schema version,
creation time, database size, and SHA-256 of the snapshot. The snapshot is
produced with `VACUUM INTO` on the writer connection, so it is consistent and
contains no WAL side files.

Export writes to a unique temporary file, flushes it, and renames it into place.
It never overwrites an existing destination.

Restore:

1. Reads the magic, format version, manifest length, and manifest.
2. Rejects unknown magic, unsupported format or schema versions, oversized
   manifests, truncated archives, and hash or size mismatches.
3. Stages the snapshot next to the live database.
4. Opens the staged snapshot read-write, runs `PRAGMA integrity_check` and
   `PRAGMA foreign_key_check`, confirms every required table exists, and checks
   that its `user_version` matches the manifest.
5. Checkpoints and closes the live database, renames it to a rollback copy,
   swaps in the staged snapshot, and reopens it. Reopening applies any pending
   migrations.
6. If the swap or reopen fails, restores the previous database and removes the
   staged copy.

A corrupt or incompatible archive therefore cannot overwrite current data.
The archive contains metadata only, not file bytes or secrets. File bytes are
restored separately by the managed store and its content IDs.

## 12. Security and privacy boundaries

- No private key, signing material, passphrase, or identity seed is written by
  this stage. `identity_refs` holds only a public key reference and key-file
  path.
- No real user, peer, or network data is committed in fixtures or tests.
- Error text is typed and does not include raw SQL, file contents, or secrets.
- Paths stored in the database are generated relative paths, not host paths.
- The store root is app-private and must be absolute.
- Symlinks and reparse points are rejected on import, open, and scan.
- The managed store never modifies or deletes the external original.
- Secrets, keystores, signing files, local paths, build output, and downloads
  remain excluded by repository hygiene.

## 13. Tests and evidence

### Rust tests

`cargo test --workspace` passed with 34 tests, 0 failures.

| Suite | Tests |
|---|---:|
| `nexus-store` unit tests | 16 |
| `nexus-store` integration tests | 12 |
| Other workspace unit and doc tests | 6 |
| **Total** | **34** |

The 12 integration tests are:

| Test | Covers |
|---|---|
| `data_001_schema_and_restart_persistence` | Schema creation, default settings, restart persistence. |
| `data_003_file_bytes_stay_outside_sqlite_and_import_copies` | Bytes in the managed store, database stays bounded, source untouched. |
| `data_004_safe_export_names_and_no_silent_overwrite` | Traversal, reserved names, collision-free export, and refusal to export inside the managed store. |
| `data_005_quota_blocks_new_work_without_damaging_existing_data` | Quota refusal and intact existing content. |
| `data_005_partial_downloads_count_from_the_first_allocated_byte` | Verified-prefix accounting and completed download. |
| `duplicate_bytes_are_counted_and_charged_once` | Deduplication and reference counting. |
| `data_006_reference_counting_and_confirmation_rules` | Last-reference confirmation, shared-byte retention, partial-cleanup age policy, and cleanup confirmation. |
| `data_007_metadata_export_and_restore_round_trip` | Archive header, manifest, hash, restore, data survival, and corrupt/incompatible archive rejection. |
| `reconciliation_removes_orphans_quarantines_unexpected_files_and_truncates_partials` | Crash cleanup, orphan deletion, quarantine, partial truncation. |
| `reconciliation_adopts_a_download_promoted_before_the_database_commit` | Crash window between promotion and commit. |
| `missing_blob_is_preserved_as_missing_metadata` | Missing bytes keep metadata and references. |
| `reconciliation_quarantines_corrupt_blob_and_keeps_that_state` | Corrupt bytes are quarantined and not downgraded. |

The 16 unit tests cover migration history and rollback, version upgrades,
relative-root rejection, duplicate store locking, generated blob paths, path
traversal and reserved names, filename length, export collisions, archive
header constants, and manifest length limits.

### Full local gate

```text
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
.\scripts\build.ps1 -WithDependencyScan
```

Result: PASS, exit 0, `All requested local checks passed.`

- Gradle `BUILD SUCCESSFUL in 43s`; Spotless and detekt passed.
- Android `assembleDebug` and desktop `createDistributable` passed.
- All 65 JVM tests passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy -p nexus-store --all-targets -- -D warnings` passed.
- `cargo test --workspace` passed with 34 Rust tests.
- `cargo deny check` reported `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo audit` scanned 71 crate dependencies against 1,509 advisories with no
  vulnerability reported.
- Repository hygiene scanned 186 files with 7 forbidden names, 10 forbidden
  suffixes, and 7 secret patterns.

`cargo deny` also reports a non-fatal duplicate-version warning because the
realized graph contains `syn` 2.0.119 and 3.0.10. Both versions are expected
from the pinned serde and zerocopy derive graphs and are covered by the license
check.

### Clean checkout

The Stage 05 test-pass commit
`700f4138b440a6d2541f1ee47fcba1e7a2aca310` was cloned into
`C:\Users\Elsa\AppData\Local\Temp\nexus-clean-aeafcf3e`, and the clean-checkout
gate ran there:

```text
.\scripts\verify-clean-checkout.ps1
```

Result: PASS, exit 0, `All requested local checks passed.`

- Gradle reported `BUILD SUCCESSFUL in 46s`; Android debug assembly and the
  Windows app image passed.
- All 65 JVM tests passed. Some Kotlin compile and test tasks were served from
  the local Gradle build cache with matching source inputs; Android packaging,
  desktop runtime-image creation, and the Rust workspace compiled freshly.
- Rust format, Clippy with warnings denied, and all 34 workspace tests passed.
  `nexus-store` ran its 16 unit and 12 integration tests.
- Repository hygiene scanned 186 files.

`verify-clean-checkout.ps1` runs `build.ps1` without `-WithDependencyScan`, so
the clean-checkout run does not repeat `cargo deny` or `cargo audit`. Those
policies were verified separately in the full local gate above against the same
committed dependency graph.

The temporary clone was removed after the run. This proves that the committed
Stage 05 tree builds and tests from a clean checkout without untracked local
files. The follow-up evidence commit changes only documentation and does not
alter build inputs.

### Kotlin boundary check

The Kotlin/JVM tests remain at 65 passing. No Kotlin source contains a SQLite,
JDBC, managed-store, or generated blob-path reference. The Stage 04 screens
continue to use demo data; no screen claims that Stage 05 storage is connected.

## 14. Failure evidence and fixes

- The metadata export test initially compared the snapshot hash with a
  placeholder zero-length slice. It now parses the archive header, extracts
  the payload, and compares its real SHA-256 with the manifest report.
- A canonicalized export path check initially compared against the
  non-canonical path on Windows. The test now canonicalizes the destination
  before comparing.
- The restore fixture omitted the publication row, so the restored feed was
  empty. The fixture now inserts the publication record and verifies it after
  restore.
- `VACUUM INTO` was initially run through a `query_only` reader. It now runs on
  the writer connection through `Database::snapshot_to`.
- A corrupt blob could be marked `quarantined`, then downgraded to `missing` on
  the next open because the quarantine scan did not track IDs. The scan now
  keeps quarantined IDs distinct from missing IDs.
- Unreferenced partial files were counted but left in `partials/`. They are now
  moved to `quarantine/`.
- A failed partial download stopped counting toward quota even though its bytes
  remained on disk. Failed states now stay in the usage sum.
- Re-importing identical bytes under a full quota was rejected before hashing.
  The importer now stages and hashes first, still checks the free-space reserve,
  and charges only new content IDs.
- Directory scans followed symlinks and reparse points. Scans now use
  `symlink_metadata`, never recurse through a link, and quarantine unexpected
  links or files.
- Clippy rejected a nested `if`; the reconciliation code now uses a let-chain.

## 15. Limitations and deferred work

- No Android emulator or physical device is available. The Android APK builds,
  but Android SQLite cross-compilation and runtime storage behavior are not
  exercised. The Kotlin shells do not link `nexus-store` yet.
- GitHub Actions has not run remotely for this stage.
- The storage-management UI is still a Stage 04 prototype. The settings and
  quota model exist, but the screen will be connected through the FFI bridge in
  a later stage.
- FTS5 search is schema-only; Stage 16 populates and queries it.
- The transfer layer is not implemented. Download records are ready for it, but
  no network download writes to these partials yet.
- Identity, signing, publishing, and replication records are schema and
  repository support only. Their cryptographic behavior starts in Stages 06 and
  07.
- Metadata restore swaps the database but does not restore file bytes; the
  managed store is the byte authority and is reconciled separately.
- Desktop GUI automation remains intermittent. Stage 05 is a headless data
  stage, so no desktop UI journey was required.

## 16. Exit criteria

| Exit criterion | Result | Evidence |
|---|---|---|
| Data survives app restarts. | Pass | `data_001_schema_and_restart_persistence`; reconciliation adoption and missing-blob tests also reopen the store. |
| Migrations work from a clean database and previous schema versions. | Pass | Migration history, forced rollback, and version 1 to 2 upgrade tests. |
| Unsafe paths and filename collisions are handled. | Pass | Path unit tests, import symlink rejection, export collision test, and explicit no-overwrite rule. |
| File bytes live outside SQLite. | Pass | `data_003_file_bytes_stay_outside_sqlite_and_import_copies`; database size remains bounded. |
| Quota, reserve, and partial downloads are enforced. | Pass | `data_005` tests and duplicate-byte test. |
| Published originals and active replicas require confirmation. | Pass | `data_006_reference_counting_and_confirmation_rules`. |
| Corrupt or incompatible metadata cannot overwrite current data. | Pass | Archive validation and restore rollback paths; round-trip test for the valid case. |
| Kotlin never opens the database or managed store. | Pass | Kotlin storage-access grep returned no matches. |

## 17. Next stage

Stage 05 is complete. Stage 06 - Identity and cryptographic foundations - is
next and requires explicit user permission. It will add local Ed25519 key
generation, stable public identity IDs, signed-record verification, encrypted
identity backup, and cross-platform cryptographic vectors on top of the
storage layer established here.
