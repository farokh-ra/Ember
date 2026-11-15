# ADR-0004 — Rust owns SQLite and the managed file store

- **Status:** Accepted
- **Date:** 2026-11-12
- **Deciders:** Ember engineering
- **Supersedes:** none
- **Related:** ADR-0002, ADR-0003, ADR-0006, ADR-0008, [Architecture](../ARCHITECTURE.md)

## Context

Ember must persist identity references, profiles, publication records, file metadata, downloads, replicas, peer observations, settings, and search indexes. File bytes can reach 4 GiB and must never be stored as SQLite BLOBs. The product requires a single verified copy per content ID, reference-counted deletion, quota enforcement, partial-download accounting, restart recovery, and safe export.

If Kotlin and Rust both write durable state, there are two writers, two migration paths, and races that can delete a blob still referenced by a publication. If SQLite is exposed to the UI layer, the UI can observe half-applied changes or bypass quota and path rules. If the managed store is a user-visible folder, another application can rename, delete, or replace files while Ember is serving them.

## Decision

**The Rust core is the only writer and reader of durable Ember state.**

- Kotlin never opens the SQLite database, never reads or writes the managed store, and never receives a raw store path for ordinary operations.
- The platform shell supplies two app-private directories at core creation: a database directory and a store directory. The core rejects paths outside the app-private root and creates the layout itself.
- SQLite holds metadata, signed record bytes, state machines, indexes, quota accounting, and reference counts. It does not hold file content, avatars, or chunk manifests as multi-megabyte BLOBs.
- The managed store holds immutable, content-addressed blobs, chunk manifests, resumable partial downloads, and temporary imports.
- Identity private key material lives in a separate encrypted file outside SQLite. Metadata exports therefore cannot accidentally contain the private key.

### SQLite rules

- Use bundled SQLite through `rusqlite`/`libsqlite3-sys`, with FTS5 enabled. The exact pinned version is in the dependency inventory.
- Enable WAL, foreign keys, a busy timeout, and a bounded page cache.
- One writer connection runs on a dedicated database thread. A small read pool serves queries. The network and UI never write directly.
- Migrations are embedded, ordered, transactional, and recorded with `PRAGMA user_version` plus a migration history table. A failed migration rolls back and leaves the previous schema intact.
- A metadata backup is a consistent SQLite snapshot plus a versioned archive manifest. It excludes secrets and file bytes. Restore validates the archive version and integrity before replacing anything.
- Signed records are stored as the exact received bytes alongside parsed query fields. Re-encoding is never required to verify or re-serve them.
- Search tables are derived state. They can be dropped and rebuilt from verified records without data loss.

### Managed store rules

```text
store/
  blobs/<aa>/<bb>/<content_id_hex>
  manifests/<manifest_root_hex>.cbor
  partials/<download_id>.part
  tmp/import-<random>.part
  quarantine/<content_id_hex>-<timestamp>
```

- Blob names are generated from the content ID. User filenames are display metadata only and never become paths.
- Blobs are immutable. A completed blob is written to `tmp`, fully verified, flushed, atomically renamed into `blobs`, and only then referenced by a database transaction.
- A crash may leave an unreferenced blob or a missing reference. Startup reconciliation scans generated directories, quarantines unexpected files, adopts verified unreferenced blobs only when a matching database record exists, and deletes unreferenced blobs under a store lock.
- Deletion commits the reference removal first, then removes bytes. A crash between the two leaves an orphan that the next reconciliation removes. Bytes are never deleted before the database says they are unreferenced.
- Reference types are publication original, replica, and retained download. A blob is deleted only when no reference remains.
- Partial downloads count toward quota from the first allocated byte. The database records the verified contiguous prefix; on restart the partial file is truncated to that prefix before resuming.
- Quota and free-space checks run before work starts and periodically during transfer. The free-space reserve is never consumed to satisfy the managed quota.
- Export is explicit and host-mediated. The core exposes a read cursor; Kotlin writes to a platform file picker or share destination. Existing files are never silently overwritten.
- Path handling rejects traversal, absolute names, Windows reserved names, control characters, NUL, and overlong names. The core does not follow symlinks or reparse points inside the store.

### Identity key custody

- The identity private key is encrypted with a key derived from a 32-byte local secret (LEK) supplied by the host at core creation.
- The LEK is generated and sealed by Android Keystore or Windows DPAPI. The core never stores the LEK in plaintext and never logs it.
- The encrypted identity file has a versioned header, a random nonce, and an authentication tag. A wrong LEK or modified file fails closed.
- If the platform secret is unavailable, the core starts in a locked state with no identity. The user can restore a backup or reset; the core never falls back to plaintext key storage.

## Consequences

**Positive**

- One writer means one schema, one migration path, and no Kotlin/Rust races.
- Signed records remain byte-identical for verification and replica serving.
- Quota, reference counting, and partial-download state are enforced in one place.
- UI tests can use a fake core without touching SQLite.
- Identity keys are outside ordinary database queries and metadata exports.

**Negative / trade-offs**

- Every metadata operation crosses the FFI boundary.
- The core needs a blocking database thread and careful transaction design.
- Kotlin cannot use Room, SQLDelight, or platform file APIs for durable state.
- Reconciliation must handle crash windows and unexpected files.
- Bundled SQLite increases binary size and requires cross-compilation for Android.

**Constraints**

- One core instance per process in v1.
- The database and store must be on the same volume, under one app-private root.
- No user file path is ever passed to the core for publishing. Publishing imports a managed copy.
- No automatic deletion of a published original or active replica to satisfy quota.
- Database size remains bounded; large objects belong in the store.

## Alternatives considered

1. **Kotlin owns SQLite through Room or SQLDelight, Rust owns networking.** Easier UI integration, but creates two writers and duplicates quota and reference-counting logic. Rejected.
2. **Store file bytes as SQLite BLOBs.** Simplifies transactions but violates the 4 GiB streaming and bounded-memory requirements, bloats WAL, and makes partial writes expensive. Rejected.
3. **Use the user's original file path as the served file.** Avoids copying but lets the file change or disappear, breaks content verification, and can expose arbitrary paths. Rejected.
4. **Put the managed store in shared external storage.** Easier export, but other apps can modify it, Android scoped storage complicates access, and path safety is weaker. Rejected.
5. **Use a central database or object store.** Violates the local-first and no-central-backend rules. Rejected.
6. **Store the identity key in SQLite encrypted.** Workable, but a metadata export or ordinary database query could capture it. A separate encrypted file has a smaller blast radius. Rejected.

## Verification

- Stage 05 tests schema creation, migrations, forced-migration rollback, restart persistence, quota accounting, free-space reserve, reference-counted deletion, path traversal, Windows reserved names, and crash-window reconciliation.
- Stage 05 checks that no Kotlin code opens the database or store.
- Stage 07 proves signed record bytes survive storage and replica serving unchanged.
- Stage 12 proves partial downloads resume from the verified prefix after process death.
- Stage 19 runs a 24-hour session and a disk-full test without data loss.
