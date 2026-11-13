# NEXUS Project Handoff

**Current stage:** 05 - Local database and file-store foundation
**Stage status:** Complete
**Last updated:** 2026-11-13
**Next stage:** 06 - Identity and cryptographic foundations (waiting for user permission)

## What was implemented

Stage 05 is complete. Rust now owns every read and write of durable local
state, and Kotlin remains a shell that will reach storage through the future
FFI boundary:

- An embedded SQLite schema with two transactional migrations, checksummed
  history, `PRAGMA user_version`, and rollback on failure.
- A dedicated writer thread, serialized `query_only` readers, WAL, foreign
  keys, a busy timeout, and a bounded cache.
- Tables for content, references, identities, profiles, publications,
  tombstones, downloads, replicas, peers, settings, and rebuildable FTS5
  search.
- A locked, content-addressed managed store under the app-private root.
- Read-only import staging, SHA-256 verification, atomic promotion, and an
  untouched external original.
- Safe display names, traversal and reserved-name handling, 255-byte limits,
  and collision-free export paths.
- A 10 GiB default quota, 1-500 GiB product range, 1 GiB free-space reserve,
  and 4 GiB maximum file size.
- Reference-counted deletion, with explicit confirmation required before the
  last publication-original or active-replica reference removes bytes.
- Resumable download rows with verified-prefix checkpoints counted from the
  first allocated byte.
- Startup reconciliation that adopts crash-window downloads, truncates
  partials, quarantines corrupt or unexpected files, and preserves missing and
  quarantined metadata.
- Metadata export as a versioned CBOR manifest plus a consistent SQLite
  snapshot, with validated restore that never overwrites current data with a
  corrupt or incompatible archive.
- 28 `nexus-store` tests: 16 unit and 12 integration.
- Stage 05 evidence in `docs/progress/STAGE_05.md`.

No identity key, signature, publication, transfer, hosting, or network
operation is implemented yet. The Kotlin screens continue to use visibly
labelled demo data and do not claim that Stage 05 storage is connected.

## Files changed

Storage crate:

- `crates/nexus-store/Cargo.toml`
- `crates/nexus-store/migrations/0001_initial.sql`
- `crates/nexus-store/migrations/0002_search_index.sql`
- `crates/nexus-store/src/lib.rs`
- `crates/nexus-store/src/backup.rs`
- `crates/nexus-store/src/db.rs`
- `crates/nexus-store/src/error.rs`
- `crates/nexus-store/src/managed_store.rs`
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/model.rs`
- `crates/nexus-store/src/paths.rs`
- `crates/nexus-store/src/quota.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/src/util.rs`
- `crates/nexus-store/tests/storage.rs`

Workspace and documentation:

- `Cargo.toml`, `Cargo.lock`
- `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md`
- `docs/progress/STAGE_05.md`
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

No Kotlin source was changed. A storage-access scan of `*.kt`, `*.kts`, and
`*.java` found no SQLite, JDBC, managed-store, or generated blob-path access.

## Commands and results

### Full local gate

```text
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
.\scripts\build.ps1 -WithDependencyScan
```

Result: PASS, exit 0, `All requested local checks passed.`

- Gradle `BUILD SUCCESSFUL in 43s`; Spotless and detekt passed.
- Android debug APK and Windows desktop app image built.
- All 65 JVM tests passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy -p nexus-store --all-targets -- -D warnings` passed.
- `cargo test --workspace` passed with 34 Rust tests.
- `cargo deny check` reported `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo audit` scanned 71 crate dependencies against 1,509 advisories with no
  vulnerability reported.
- Repository hygiene scanned 186 files with 7 forbidden names, 10 forbidden
  suffixes, and 7 secret patterns.

`cargo deny` reports a non-fatal duplicate-version warning for `syn` 2.0.119
and 3.0.10. Both are expected from the pinned serde and zerocopy derive graphs
and pass the license check.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-store` unit tests | 16 | 0 |
| `nexus-store` integration tests | 12 | 0 |
| Other Rust workspace tests | 6 | 0 |
| **Rust total** | **34** | **0** |
| `NexusAppStateTest` | 10 | 0 |
| `NavigationStateTest` | 11 | 0 |
| `ContentStateTest` | 4 | 0 |
| `DemoCatalogTest` | 11 | 0 |
| `IdentityDraftTest` | 7 | 0 |
| `StartupPresenterTest` | 1 | 0 |
| `NexusAppControllerTest` | 9 | 0 |
| `StateLabelsTest` | 5 | 0 |
| `ThemeContrastTest` | 7 | 0 |
| **JVM total** | **65** | **0** |

Presentation subtotal: 44. Shared UI subtotal: 21.

### Artifacts

- Android debug APK:
  `apps/androidApp/build/outputs/apk/debug/androidApp-debug.apk`,
  31,039,671 bytes.
- Windows app image:
  `apps/desktopApp/build/compose/binaries/main/app/NEXUS`, 130,622,747 bytes
  across 200 files.
- Windows launcher: `.../app/NEXUS/NEXUS.exe`.

The APK and desktop image still build from the same UI sources. Stage 05 adds
no Kotlin runtime dependency and no screen behavior.

### Validators

```text
python scripts/check_repo_hygiene.py
python scripts/check_commit_message.py "feat(store): add SQLite and managed file store"
```

Both pass.

### Clean checkout

`scripts/verify-clean-checkout.ps1` cloned the Stage 05 test-pass commit
`700f4138b440a6d2541f1ee47fcba1e7a2aca310` into
`C:\Users\Elsa\AppData\Local\Temp\nexus-clean-aeafcf3e` and ran the build,
tests, and hygiene gate. It returned exit 0: Gradle reported
`BUILD SUCCESSFUL in 46s`, the Android debug APK and Windows app image were
assembled, all 65 JVM tests passed, Rust format, Clippy, and all 34 workspace
tests passed, and repository hygiene scanned 186 files. Some Kotlin tasks were
served from the local Gradle build cache with matching inputs; Android
packaging, desktop runtime-image creation, and the Rust workspace compiled
freshly. The clone was removed afterwards.

The clean-checkout script does not run `-WithDependencyScan`; `cargo deny` and
`cargo audit` were verified separately in the full local gate against the same
committed graph. The follow-up evidence commit changes only documentation, so it
does not alter build inputs.

## Errors and failed attempts

1. The metadata export test compared the snapshot hash against a placeholder
   zero-length slice. It now parses the archive header, extracts the payload,
   and compares its real SHA-256.
2. A canonicalized Windows export path was compared with the non-canonical
   path. The test now canonicalizes the destination first.
3. The restore fixture omitted the publication row, so the restored feed was
   empty. The fixture now inserts and verifies the publication record.
4. `VACUUM INTO` was initially run on a `query_only` reader. It now runs on the
   writer connection through `Database::snapshot_to`.
5. A corrupt blob could be marked `quarantined` and then downgraded to
   `missing` on the next open. Reconciliation now keeps quarantined IDs out of
   the missing-state pass.
6. Unreferenced partial files were counted but left in `partials/`. They are now
   moved to `quarantine/`.
7. Failed partial downloads stopped counting toward quota even though their
   bytes remained on disk. Failed states now remain in the usage sum.
8. Identical bytes under a full quota were rejected before hashing. The importer
   now stages and hashes first, still checks the free-space reserve, and charges
   only new content IDs.
9. Directory scans followed symlinks and reparse points. Scans now use
   `symlink_metadata`, never recurse through a link, and quarantine unexpected
   links or files.
10. Clippy rejected a nested `if`; the reconciliation code now uses a let-chain.

## Decisions and unresolved questions

- Stage 05 was explicitly approved by the project owner.
- Rust remains the only SQLite and managed-store reader and writer, per
  ADR-0004. Kotlin never opens these files.
- The schema is two migrations: version 1 base schema and version 2 FTS5
  `search_index`. Search remains derived state and is not populated until
  Stage 16.
- `QuotaPolicy::new` enforces the 1-500 GiB product range; `custom` supports
  deterministic small-quota tests.
- The managed store requires an absolute root and takes an exclusive lock.
- Reconciliation never follows symlinks, preserves missing and quarantined
  metadata, and deletes only unreferenced generated blobs.
- Metadata archives contain metadata only. File bytes remain in the managed
  store and are reconciled separately.
- The storage-management screen is still a Stage 04 prototype. The settings
  and quota model are ready for the FFI bridge.
- No Android emulator or physical device is available, so Android SQLite
  cross-compilation and runtime storage behavior are not locally exercised.
- Remote GitHub Actions execution has not run for this stage.
- The Windows app image is not an installer. Inno Setup packaging is Stage 19.

## Exact next action

1. Review the Stage 05 evidence in `docs/progress/STAGE_05.md`.
2. Give explicit approval to start Stage 06.
3. Stage 06 adds local Ed25519 identity generation, stable public identity
   IDs, signed-record verification, encrypted identity backup, and
   cross-platform cryptographic vectors. Do not begin it before that approval.
