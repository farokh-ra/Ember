# Stage 07 - Publishing, Chunk Manifests, Revisions, and Tombstones

**Status:** Complete
**Completed:** 2026-11-14
**Progress guide:** 31-37%
**Next stage:** 08 - Kotlin/FFI integration and Android runtime verification (requires user permission)

## 1. Scope

Stage 07 implements the file-identity and publication-record layer defined by
[ADR-0006](../decisions/ADR-0006-crypto-and-record-formats.md) and
[ADR-0007](../decisions/ADR-0007-protocol-boundaries-versioning.md) without
changing their architecture. It builds on the exact-byte signed records from
Stage 06.

The stage covers:

- Raw SHA-256 content IDs over complete file bytes.
- Fixed 1 MiB chunking and canonical flat chunk manifests.
- Domain-separated manifest roots signed by publication records.
- Streaming file hashing, manifest construction, and chunk/whole-file
  verification with bounded memory.
- Publication revision rules and successor validation.
- Signed tombstone v1 records with uploader-attributed withdrawal.
- Immutable publication storage, monotonic tombstone storage, manifest files,
  and startup reconciliation.
- Committed cross-platform vectors and rejection tests.

It does not implement publishing over a network, downloads, hosting, DHT records,
Kotlin/FFI integration, or UI screens. Those remain later stages.

## 2. Deliverables

| Deliverable | Path | Status |
|---|---|---|
| Manifest v1 schema, limits, canonical encoding | `crates/nexus-protocol/src/manifest.rs` | Complete; 3 unit tests. |
| Manifest-root hashing and publication cross-checks | `crates/nexus-protocol/src/manifest.rs` | Complete; domain-separated root. |
| Publication revision and successor rules | `crates/nexus-protocol/src/publication.rs` | Complete; revision 1 and successor checks. |
| Tombstone v1 signing, parsing, and matching | `crates/nexus-protocol/src/tombstone.rs` | Complete; 3 unit tests. |
| Signed-record error variants | `crates/nexus-protocol/src/error.rs` | Complete; manifest, revision, and tombstone errors. |
| Manifest and tombstone acceptance tests | `crates/nexus-protocol/tests/stage07.rs` | Complete; 8 tests. |
| Streaming hashing and verification | `crates/nexus-transfer/src/lib.rs` | Complete; 7 unit tests. |
| Transfer acceptance tests and 2.5 MB vector | `crates/nexus-transfer/tests/transfer.rs` | Complete; 7 tests. |
| Schema version 3 signed tombstones | `crates/nexus-store/migrations/0003_signed_tombstones.sql` | Complete; backfills attribution. |
| Immutable publications and tombstone upserts | `crates/nexus-store/src/store.rs` | Complete; conflict preservation. |
| Manifest file storage and reconciliation | `crates/nexus-store/src/managed_store.rs`, `store.rs` | Complete; root verification and quarantine. |
| Store acceptance tests | `crates/nexus-store/tests/stage07.rs` | Complete; 4 tests. |
| Exact format specification | [SIGNED_RECORD_FORMATS.md](../architecture/SIGNED_RECORD_FORMATS.md) | Complete; manifest and tombstone sections plus vectors. |
| Evidence record | This document | Complete. |

## 3. Architecture and ownership

The realized dependency direction is:

```text
nexus-protocol -> nexus-crypto
nexus-transfer -> nexus-protocol, nexus-crypto, sha2
nexus-store    -> nexus-protocol, nexus-crypto
```

- `nexus-protocol` owns manifest and tombstone schemas, revision rules, and
  publication cross-checks. It performs no file or network I/O.
- `nexus-transfer` owns streaming hashing and verification. It reads files or
  readers one chunk at a time and never mutates signed records.
- `nexus-store` owns durable metadata and manifest files. It stores exact signed
  publication and tombstone bytes and never rewrites them.
- Kotlin and the FFI layer still do not link the Rust core. The shared UI
  continues to use demo data and makes no publishing claims.

## 4. File identity and chunk manifests

The content ID is the raw SHA-256 digest of the complete file bytes:

```text
content_id = SHA-256(file_bytes)
```

It is not domain-separated and is not the manifest root. Identical bytes produce
the same ID on every platform and in every store. A one-byte change produces a
different ID.

Files are split into fixed 1 MiB chunks. The final chunk is 1 to 1,048,576 bytes.
Files are 1 byte to 4 GiB; empty files are not publishable in v1. The manifest is
a flat canonical CBOR body:

| Key | Type | Rule |
|---:|---|---|
| 1 | uint | Schema major version, `1`. |
| 2 | bstr(32) | Complete-file SHA-256 content ID. |
| 3 | uint | Total size, 1 byte to 4 GiB. |
| 4 | uint | Chunk size, exactly `1,048,576`. |
| 5 | array | Ordered `[length, SHA-256]` chunk entries, 1 to 4,096. |

Validation enforces:

- at most 4,096 entries;
- every non-final chunk exactly 1 MiB and the final chunk 1..=1 MiB;
- chunk lengths summing exactly to the total size;
- entry count exactly `ceil(total_size / 1,048,576)`;
- canonical manifest bytes at most 262,144 bytes;
- unknown integer keys above 5 preserved and major versions other than 1
  rejected.

```text
manifest_root = SHA-256("NEXUS-MANIFEST-v1" || canonical_manifest_body)
```

The publication record signs this root as field 10. A manifest is accepted only
when its content ID, total size, chunk size, and computed root match the signed
publication. Verification hashes the exact stored manifest bytes and never
re-encodes them first.

## 5. Streaming hashing and verification

`nexus-transfer` provides:

- `hash_reader` / `hash_file` for raw content IDs;
- `build_manifest` / `build_manifest_for_file` for one-pass manifest
  construction with a single 1 MiB chunk buffer;
- `verify_chunk` for one chunk;
- `verify_manifest_bytes` for parsing and root checking;
- `verify_publication_manifest` for publication binding;
- `ManifestVerifier` for in-order streaming verification;
- `verify_file` / `verify_file_path` for complete files.

Verification rejects a chunk with the wrong length or hash, a chunk supplied out
of order, missing chunks, trailing bytes, a total size mismatch, and a final
content-ID mismatch. No function holds more than one chunk and one hash buffer in
memory.

## 6. Publication revisions

A publication body is revision 1 or higher. Revision 1 must not have a
`supersedes` value. A revision greater than 1 must name the previous publication
ID.

`validate_successor_of` requires the successor to keep the identity, content ID,
size, manifest root, and chunk size unchanged, to have exactly the previous
revision plus one, and to name the previous publication ID. A revision chain
cannot switch content, size, manifest, or uploader without violating the signed
rules.

Publication IDs remain `SHA-256("NEXUS-PUB-ID-v1" || body)`, so a new revision
has a new ID while the content ID stays stable for unchanged bytes.

## 7. Tombstones

A tombstone is an independently signed record with schema v1 keys 1-6:

| Key | Type | Rule |
|---:|---|---|
| 1 | uint | Schema major version, `1`. |
| 2 | text | Canonical `nx1...` identity ID of the original uploader. |
| 3 | bstr(32) | Publication ID being withdrawn. |
| 4 | bstr(32) | Content ID of the withdrawn publication. |
| 5 | uint | Tombstone revision, at least 1. |
| 6 | uint | Informational unpublish time, Unix milliseconds. |

The signature domain is `NEXUS-TOMBSTONE-v1`. Validation requires the identity,
publication ID, and content ID to match the signed publication being withdrawn.
A higher tombstone revision supersedes a lower revision and may skip numbers.
Equal revisions with different signed bytes are conflicts. A tombstone cannot
change uploader or content attribution, and a replica cannot substitute its own
identity for the original uploader.

A tombstone never deletes a blob, never rewrites the original publication, and
never proves that other peers deleted their copies. It records the uploader's
withdrawal verdict.

## 8. Store schema and immutability

Schema version 3 rebuilds tombstones as independent signed records with
`identity_id` and `content_id`. The migration backfills these columns from the
existing publication row before dropping the publication foreign key. A tombstone
can now arrive before its publication.

- `upsert_publication` inserts a publication once. Repeating the same record is an
  idempotent no-op. Any difference in identity, content ID, revision, or exact
  signed bytes is a `RecordConflict`, and the original row is preserved.
- `upsert_tombstone` accepts an orphan tombstone, preserves signed attribution,
  ignores lower revisions, replaces higher revisions, and rejects equal-revision
  byte conflicts or attribution changes.
- A late publication inherits an existing tombstone state without changing the
  signed publication bytes.
- Manifests are stored as exact canonical bytes at
  `store/manifests/<root>.cbor`. Writes are atomic, size-bounded, root-verified,
  and idempotent for identical bytes. A corrupt existing file is quarantined
  before valid bytes are written.
- Startup reconciliation verifies named manifests and quarantines corrupt or
  unexpectedly named files.

## 9. Security and rejection rules

Tests commit and exercise these failures:

- a one-byte change produces a different content ID and manifest root;
- corrupted, reordered, truncated, and over-long chunks and whole files are
  rejected;
- malformed manifest CBOR, wrong field types, bad chunk counts, bad chunk
  lengths, bad totals, and oversized manifests are rejected;
- manifest bytes that do not hash to the signed root are rejected;
- a manifest whose content ID, size, chunk size, or root differs from its signed
  publication is rejected;
- publication revision 1 with `supersedes` and revision greater than 1 without it
  are rejected;
- a successor that changes content, size, manifest, or uploader is rejected;
- changed tombstone bodies, changed signatures, and publication-domain replay
  fail verification;
- tombstones that change identity, publication ID, or content ID are rejected;
- lower or equal revisions cannot replace a higher revision, and equal revisions
  with different bytes are conflicts;
- immutable publication conflicts preserve the original attribution.

## 10. Committed vectors

The exact vectors and layouts are in
[SIGNED_RECORD_FORMATS.md](../architecture/SIGNED_RECORD_FORMATS.md). The Rust
tests assert:

| Vector | Assertion |
|---|---|
| 2.5 MB deterministic file | Exact content ID. |
| Three-chunk manifest | Exact chunk lengths and hashes. |
| Canonical manifest body | Exact body hex and domain-separated root. |
| One-byte change | Different content ID and manifest root. |
| Tombstone body | Exact body hex, signature, and envelope SHA-256. |
| Tombstone persistence | Exact signed bytes and attribution survive upserts. |
| Manifest persistence | Exact bytes round-trip by root. |

The 2.5 MB fixture is generated by `(i * 31 + 7) % 251` independently of the
Rust implementation. Its chunks are 1,048,576, 1,048,576, and 402,848 bytes.

## 11. Tests and evidence

### Rust tests

`cargo test --workspace` passed with 131 tests, 0 failures.

| Suite | Tests |
|---|---:|
| `nexus-crypto` unit tests | 24 |
| `nexus-identity` unit tests | 9 |
| `nexus-identity` integration tests | 12 |
| `nexus-protocol` unit tests | 16 |
| `nexus-protocol` signed-record tests | 9 |
| `nexus-protocol` Stage 07 tests | 8 |
| `nexus-store` unit tests | 17 |
| `nexus-store` storage tests | 12 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-transfer` unit tests | 7 |
| `nexus-transfer` integration tests | 7 |
| Other workspace unit tests | 6 |
| **Total** | **131** |

The Stage 07 protocol tests are:

| Test | Covers |
|---|---|
| `committed_manifest_vector_is_exact` | Exact body, root, chunk lengths, and total. |
| `committed_tombstone_vector_is_exact` | Exact body, signature, envelope hash. |
| `tombstone_changes_bad_signatures_and_replay_are_rejected` | Body/signature tampering and domain replay. |
| `manifest_unknown_minor_fields_are_preserved_and_major_versions_rejected` | Forward-compatible minor fields and major-version rejection. |
| `tombstone_unknown_minor_fields_are_preserved_and_major_versions_rejected` | Re-signed minor fields and major-version rejection. |
| `manifest_shape_and_publication_cross_checks_are_enforced` | Chunk count, lengths, totals, chunk size, and publication binding. |
| `publication_revision_rules_are_enforced` | Revision 1, successor chain, changed fields, skipped revision, wrong predecessor. |
| `tombstones_bind_to_the_original_signed_publication_and_revisions` | Attribution matching, skipped revisions, equal-revision conflicts. |

The Stage 07 transfer tests are:

| Test | Covers |
|---|---|
| `deterministic_vector_matches_committed_identity_and_manifest` | Exact content ID, chunk hashes, body, root, and round trip. |
| `one_byte_change_produces_a_different_content_id_and_root` | Byte-level collision check. |
| `corrupted_chunks_are_rejected_by_chunk_and_stream_verification` | First and final chunk corruption. |
| `reordered_and_truncated_chunks_are_rejected` | Reordering, truncation, and trailing bytes. |
| `manifest_bytes_must_match_the_signed_root_and_publication` | Root tampering and publication mismatch. |
| `file_path_helpers_round_trip_the_committed_vector` | File hashing, manifest build, and verification. |
| `empty_files_are_rejected` | Empty-file rule. |

The Stage 07 store tests are:

| Test | Covers |
|---|---|
| `publications_are_immutable_idempotent_and_preserve_original_attribution` | Idempotent insert and all conflict classes. |
| `tombstones_are_independent_monotonic_and_keep_signed_attribution` | Orphan insert, late publication, revision order, conflicts. |
| `manifests_round_trip_by_trusted_root_and_bind_to_signed_publications` | Manifest put/get, bounds, corrupt repair, publication binding. |
| `reconciliation_verifies_named_manifests_and_quarantines_corrupt_files` | Verified count and quarantine. |

### Full local gate

```text
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
.\scripts\build.ps1 -WithDependencyScan
```

Result: PASS, exit 0, `All requested local checks passed.`

- Gradle `BUILD SUCCESSFUL`; Spotless and detekt passed.
- Android `assembleDebug` and desktop `createDistributable` passed.
- All 65 JVM tests passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace` passed with 131 Rust tests.
- `cargo deny check` reported `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo audit` reported no vulnerability.
- Repository hygiene scanned the working tree with no forbidden file or secret
  pattern.

### Clean checkout

After commit `STAGE07_COMMIT`, the clean-checkout gate cloned `HEAD` into:

```text
CLEAN_CHECKOUT_PATH
```

and ran:

```text
.\scripts\verify-clean-checkout.ps1
```

Result: PASS, exit 0, `All requested local checks passed.`

The clone built the Android debug APK and Windows app image, passed all 65 JVM
tests, passed Rust formatting and Clippy, and passed all 131 Rust tests.
`verify-clean-checkout.ps1` runs `build.ps1` without `-WithDependencyScan`, so the
clean-checkout run does not repeat `cargo deny` or `cargo audit`. Those policies
were verified separately in the full local gate against the same committed
dependency graph.

### Kotlin boundary check

The Kotlin/JVM tests remain at 65 passing. No Kotlin source links
`nexus-protocol`, `nexus-transfer`, or `nexus-store`, and no publishing or
tombstone path exists in the shared UI. The Stage 04 screens continue to use demo
data.

## 12. Failure evidence and fixes

- The first transfer unit test built 1.4 MB of data but expected one chunk. The
  fixture was reduced below 1 MiB so the test asserts one chunk intentionally.
- Clippy rejected `chunks_exact` on the content-ID hex parser. It now uses
  `as_chunks::<2>()` with an explicit remainder check.
- The initial store publication upsert compared the stored revision to itself
  because a local variable shadowed the incoming value. The comparison now uses
  an `incoming_revision` binding, and the conflict test covers revision changes.
- The first store test helper decoded hex through a leaked boxed array. The final
  helper returns `[u8; 32]` directly from `ContentHash::parse_hex`.
- A tombstone test initially pointed at a synthetic publication ID and then
  expected the unrelated signed publication to be marked. The test now uses the
  actual signed publication ID and checks that attribution and exact bytes are
  preserved.
- The manifest and tombstone vector tests initially encoded the tombstone body
  with incorrect line wrapping. They now assert the exact bytes produced by the
  encoder and the independently generated manifest fixture.
- `cargo fmt --all` rewrapped several long expressions in the new protocol,
  transfer, and store code. Formatting and Clippy now pass workspace-wide.

## 13. Limitations and deferred work

- No Android emulator or physical device is available, so Android runtime
  cross-verification of manifest hashing, signature checks, and file access is
  deferred to Stage 08. Stage 07 proves the byte-level vectors on Windows.
- GitHub Actions has not run remotely for this stage.
- Kotlin/FFI integration is deferred to Stage 08. No screen claims that
  publishing, tombstoning, or manifest verification is connected.
- `nexus-transfer` is not yet wired into import or download orchestration. It
  provides the hashing and verification primitives that later stages will call.
- Manifests are stored locally and verified at startup, but no peer-to-peer
  manifest exchange exists until the networking stages.
- Tombstone propagation and DHT record format are reserved for Stage 09.
- Publication revisions are validated as records but there is no UI or network
  flow for publishing a revision yet.
- Empty files remain unpublishable in v1 by design.

## 14. Exit criteria

| Exit criterion | Result | Evidence |
|---|---|---|
| Identical bytes produce the same content ID. | Pass | Committed 2.5 MB vector; independent Python and Rust hashes agree. |
| A one-byte change produces a different content ID. | Pass | `one_byte_change_produces_a_different_content_id_and_root` and the committed changed-ID vector. |
| Invalid signatures are rejected. | Pass | Tombstone and manifest binding tests; changed body, signature, and replay tests. |
| Corrupted chunks are rejected. | Pass | Chunk, stream, reorder, truncation, and trailing-byte tests. |
| A replica cannot alter original-uploader attribution. | Pass | Tombstone attribution checks and immutable store conflict tests. |

## 15. Next stage

Stage 07 is complete. Stage 08 - Kotlin/FFI integration and Android runtime
verification - is next and requires explicit user permission. It will expose the
verified Rust identity, publication, manifest, and tombstone operations through
the FFI boundary without changing the v1 byte formats established here.
