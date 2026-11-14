# NEXUS Project Handoff

**Current stage:** 07 - Publishing, chunk manifests, revisions, and tombstones
**Stage status:** Complete
**Last updated:** 2026-11-14
**Next stage:** 08 - Kotlin/FFI integration and Android runtime verification (waiting for user permission)

## What was implemented

Stage 07 is complete. The Rust core now derives file identities, builds and
verifies chunk manifests, validates publication revisions and signed tombstones,
and stores the results immutably, while Kotlin remains a demo shell until the FFI
bridge:

- Raw SHA-256 content IDs over complete file bytes.
- Fixed 1 MiB chunks and canonical flat manifests with one SHA-256 per chunk.
- Domain-separated manifest roots signed by publication records.
- Streaming hashing, manifest construction, chunk verification, and whole-file
  verification with bounded memory.
- Publication revision rules: revision 1 has no predecessor; later revisions
  preserve identity, content, size, manifest root, and chunk size and name the
  previous publication ID.
- Signed tombstone v1 records with original-uploader attribution, skip-tolerant
  revisions, and equal-revision conflict detection.
- Schema version 3 with independent signed tombstones, immutable publication
  inserts, monotonic tombstone upserts, content-addressed manifest files, and
  manifest verification during startup reconciliation.
- Committed 2.5 MB cross-platform vector, exact manifest and tombstone vectors,
  and rejection tests for corruption, invalid signatures, and attribution
  changes.
- Exact v1 format specification in
  `docs/architecture/SIGNED_RECORD_FORMATS.md`.
- Stage 07 evidence in `docs/progress/STAGE_07.md`.

No network publishing, downloads, hosting, DHT, or Kotlin/FFI operation is
implemented yet. The Kotlin screens continue to use visibly labelled demo data
and do not claim that Stage 07 publishing behavior is connected.

## Files changed

Protocol crate:

- `crates/nexus-protocol/src/lib.rs`
- `crates/nexus-protocol/src/error.rs`
- `crates/nexus-protocol/src/manifest.rs` (new)
- `crates/nexus-protocol/src/publication.rs`
- `crates/nexus-protocol/src/tombstone.rs` (new)
- `crates/nexus-protocol/tests/stage07.rs` (new)

Transfer crate:

- `crates/nexus-transfer/Cargo.toml`
- `crates/nexus-transfer/src/lib.rs`
- `crates/nexus-transfer/tests/transfer.rs` (new)

Store crate:

- `crates/nexus-store/Cargo.toml`
- `crates/nexus-store/src/error.rs`
- `crates/nexus-store/src/lib.rs`
- `crates/nexus-store/src/managed_store.rs`
- `crates/nexus-store/src/migrations.rs`
- `crates/nexus-store/src/model.rs`
- `crates/nexus-store/src/store.rs`
- `crates/nexus-store/migrations/0003_signed_tombstones.sql` (new)
- `crates/nexus-store/tests/stage07.rs` (new)
- `crates/nexus-store/tests/storage.rs`

Workspace and documentation:

- `Cargo.lock`
- `docs/architecture/SIGNED_RECORD_FORMATS.md`
- `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md`
- `docs/progress/STAGE_07.md` (new)
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

No Kotlin source was changed.

## Commands and results

### Full local gate

```text
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
.\scripts\build.ps1 -WithDependencyScan
```

Result: PASS, exit 0, `All requested local checks passed.`

- Gradle `BUILD SUCCESSFUL`; Spotless and detekt passed.
- Android debug APK and Windows desktop app image built.
- All 70 JVM tests passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace` passed with 131 Rust tests.
- `cargo deny check` reported `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo audit` scanned the realized graph with no vulnerability reported.
- Repository hygiene scanned the working tree with no forbidden file or secret
  pattern.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-crypto` unit tests | 24 | 0 |
| `nexus-identity` unit tests | 9 | 0 |
| `nexus-identity` integration tests | 12 | 0 |
| `nexus-protocol` unit tests | 16 | 0 |
| `nexus-protocol` signed-record tests | 9 | 0 |
| `nexus-protocol` Stage 07 tests | 8 | 0 |
| `nexus-store` unit tests | 17 | 0 |
| `nexus-store` storage tests | 12 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-transfer` unit tests | 7 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| Other Rust workspace tests | 6 | 0 |
| **Rust total** | **131** | **0** |
| `shared:core-api` JVM tests | 1 | 0 |
| `shared:core-client` JVM tests | 1 | 0 |
| `shared:model` JVM tests | 3 | 0 |
| `shared:presentation` JVM tests | 44 | 0 |
| `shared:ui` JVM tests | 21 | 0 |
| **JVM total** | **70** | **0** |

Earlier stage records reported 65 JVM tests because they counted only the
presentation and UI suites. The five `core-api`, `core-client`, and `model` tests
also run; 70 is the correct total.

### Validators

```text
python scripts/check_repo_hygiene.py
python scripts/check_commit_message.py "feat(publish): add manifests, revisions, and tombstones"
```

Both pass.

### Clean checkout

After commit `c85f9b4` (`feat(publish): add manifests, revisions, and
tombstones`), `scripts/verify-clean-checkout.ps1` cloned `HEAD` into
`C:\Users\Elsa\AppData\Local\Temp\nexus-clean-914ef07c` and ran the build, tests,
and hygiene gate. Result: PASS, exit 0, `All requested local checks passed.` The
clone was removed afterwards.

A recheck after commit `3bfd594` cloned `HEAD` into
`C:\Users\Elsa\AppData\Local\Temp\nexus-clean-27fbdb98` and passed again with
exit 0, 131 Rust tests, and 70 JVM tests. The clone was removed afterwards. A
separate Python recomputation reproduced every committed manifest and tombstone
vector, including the Ed25519 tombstone signature.

The clean-checkout script does not run `-WithDependencyScan`; `cargo deny` and
`cargo audit` were verified separately in the full local gate against the same
committed graph.

## Errors and failed attempts

1. The first transfer unit test built 1.4 MB of data but expected one chunk. The
   fixture was reduced below 1 MiB so the test asserts one chunk intentionally.
2. Clippy rejected `chunks_exact` on the content-ID hex parser. It now uses
   `as_chunks::<2>()` with an explicit remainder check.
3. The initial store publication upsert compared the stored revision to itself
   because a local variable shadowed the incoming value. The comparison now uses
   an `incoming_revision` binding, and the conflict test covers revision changes.
4. The first store test helper decoded hex through a leaked boxed array. The final
   helper returns `[u8; 32]` directly from `ContentHash::parse_hex`.
5. A tombstone test initially pointed at a synthetic publication ID and then
   expected an unrelated signed publication to be marked. The test now uses the
   actual signed publication ID and checks that attribution and exact bytes are
   preserved.
6. `cargo fmt --all` rewrapped several long expressions in the new protocol,
   transfer, and store code. Formatting and Clippy now pass workspace-wide.

## Decisions and unresolved questions

- Stage 07 was explicitly approved by the project owner.
- ADR-0006 and ADR-0007 are implemented without changing the architecture.
- Content IDs are raw SHA-256 over complete file bytes. Manifest roots are
  domain-separated and signed by the publication record.
- Chunking is fixed at 1 MiB. There is exactly one manifest format in v1, with
  1-4,096 entries and a 262,144-byte cap.
- Empty files are not publishable in v1.
- Publication revision 1 must not have a predecessor; every later revision must
  preserve content identity and name the previous publication ID.
- Tombstones are independent signed records so they can arrive before a
  publication and cannot inherit mutable local attribution.
- Tombstone revisions may skip numbers. Equal revisions with different bytes are
  conflicts, not replacements.
- Store manifests are stored as exact canonical bytes under their trusted root.
  Corrupt files are quarantined on read repair or reconciliation.
- No Android emulator or physical device is available, so Android runtime
  cross-verification is deferred to Stage 08. The committed vectors are the
  byte-level contract.
- Remote GitHub Actions execution has not run for this stage.
- Kotlin/FFI integration and platform Keystore/DPAPI adapters remain Stage 08
  work.

## Exact next action

1. Review the Stage 07 evidence in `docs/progress/STAGE_07.md` and the exact
   formats in `docs/architecture/SIGNED_RECORD_FORMATS.md`.
2. Give explicit approval to start Stage 08.
3. Stage 08 connects the verified Rust identity, publication, manifest, and
   tombstone operations to Kotlin through the FFI boundary and verifies the
   committed vectors on an Android runtime. Do not begin it before that approval.
