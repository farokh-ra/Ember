# Stage 06 - Identity and Cryptographic Foundations

**Status:** Complete
**Completed:** 2026-11-13
**Progress guide:** 24-30%
**Next stage:** 07 - Publishing, manifests, revisions, and tombstones (requires user permission)

## 1. Scope

Stage 06 establishes the local cryptographic identity and the exact signed-record
byte formats that later publishing, transfer, and networking stages depend on.
It follows [ADR-0006](../decisions/ADR-0006-crypto-and-record-formats.md) and
[ADR-0007](../decisions/ADR-0007-protocol-boundaries-versioning.md) without
changing their architecture.

The stage covers:

- Local Ed25519 key generation from the operating-system CSPRNG.
- A stable public identity ID in Bech32m with HRP `nx`.
- Canonical CBOR with bounded depth, item count, and input size.
- Exact-byte signed envelopes and domain-separated signatures.
- Signed profile v1 and publication v1 schemas.
- Encrypted identity storage under `secrets/identity.enc`.
- A user-initiated encrypted backup and restore format.
- Committed cross-platform vectors and security rejection tests.
- Explicit key-loss, rotation, recovery, and real-world-identity limits.

It does not implement publishing, chunk manifests, transfers, hosting, DHT
records, networking, Kotlin/FFI integration, or UI screens. Those remain later
stages.

## 2. Deliverables

| Deliverable | Path | Status |
|---|---|---|
| Canonical CBOR encoder and decoder | `crates/nexus-crypto/src/cbor.rs` | Complete; bounded, canonical, duplicate-key rejection. |
| Ed25519 keys, signing, strict verification | `crates/nexus-crypto/src/key.rs` | Complete; secret seed zeroized and `Debug` redacted. |
| Bech32m identity IDs | `crates/nexus-crypto/src/identity_id.rs` | Complete; multicodec payload and canonical parsing. |
| Domain tags and domain-separated hashing | `crates/nexus-crypto/src/domain.rs`, `hash.rs` | Complete; all ten v1 contexts. |
| XChaCha20-Poly1305, HKDF, Argon2id | `crates/nexus-crypto/src/aead.rs`, `kdf.rs` | Complete; untrusted parameter bounds. |
| NFC text handling | `crates/nexus-crypto/src/text.rs` | Complete; user text and passphrases. |
| OS randomness | `crates/nexus-crypto/src/random.rs` | Complete; `getrandom` directly. |
| Identity lifecycle and notices | `crates/nexus-identity/src/identity.rs` | Complete; redacted `Debug`, key-loss statements. |
| Encrypted identity at rest | `crates/nexus-identity/src/at_rest.rs` | Complete; authenticated 68-byte header. |
| Encrypted backup and restore | `crates/nexus-identity/src/backup.rs` | Complete; versioned Argon2id header and generic failures. |
| Atomic vault storage | `crates/nexus-identity/src/vault.rs` | Complete; restrictive writes and symlink/reparse-point rejection. |
| Signed envelope | `crates/nexus-protocol/src/envelope.rs` | Complete; exact raw bytes preserved. |
| Profile v1 | `crates/nexus-protocol/src/profile.rs` | Complete; bounds, NFC, unknown minor fields. |
| Publication v1 | `crates/nexus-protocol/src/publication.rs` | Complete; category enum, publication ID. |
| Identity acceptance tests | `crates/nexus-identity/tests/identity.rs` | Complete; 12 tests. |
| Signed-record acceptance tests | `crates/nexus-protocol/tests/records.rs` | Complete; 9 tests. |
| Exact format specification | [SIGNED_RECORD_FORMATS.md](../architecture/SIGNED_RECORD_FORMATS.md) | Complete. |
| Evidence record | This document | Complete. |

## 3. Architecture and ownership

The realized dependency direction is:

```text
nexus-identity -> nexus-crypto
nexus-protocol -> nexus-crypto
```

- `nexus-crypto` owns every byte-level primitive and encoding rule.
- `nexus-identity` owns local key generation, encrypted key storage, backup, and
  restore. It does not write SQLite.
- `nexus-protocol` owns signed-record schemas and verification. It does not store
  records or perform network I/O.
- Kotlin and the FFI layer receive public identity metadata only. No plaintext
  private key crosses the future FFI boundary.
- The platform shell supplies the 32-byte local encryption key from Android
  Keystore or Windows DPAPI. The Rust core still authenticates the encrypted
  identity file independently.
- Later stages may add store integration and Kotlin bindings without changing the
  v1 byte formats defined here.

## 4. Identity creation and stable ID

`Identity::create` generates a 32-byte Ed25519 seed with the OS CSPRNG and
derives the public key locally. No network call, account, registry, or central
service is involved. The identity ID is:

```text
Bech32m(HRP = "nx", payload = 0xed 0x01 || 32-byte public key)
```

The committed zero-seed vector is:

```text
public key = 3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29
identity   = nx1a5qnk638hn8tdfpdv23635p2duxhxefjz4m3mcjr5cavqj9p3dva52g6l8d37
```

Creating an identity twice in the same vault fails with `AlreadyExists`; it never
silently replaces an existing key. Loading after a process restart returns the
same ID and public key.

## 5. Canonical CBOR and signed records

Signed bodies use the canonical profile specified in
[SIGNED_RECORD_FORMATS.md](../architecture/SIGNED_RECORD_FORMATS.md):

- definite lengths, shortest integers, no tags or floats;
- scalar map keys sorted by canonical byte encoding;
- duplicate keys rejected;
- valid UTF-8 and NFC user-visible text;
- signed-record bounds of depth 16, 512 items, and 65,536 bytes;
- decode, re-encode, and byte-equality checks.

The envelope is:

```text
{1: body bstr, 2: signature bstr}
```

The 64-byte Ed25519 signature covers `domain_tag || body`. Verification uses the
exact stored body bytes and never re-encodes them. The complete envelope is at
most 64 KiB. Envelope extra fields are rejected.

Profile and publication bodies use integer keys 1-9 and 1-16 respectively.
Unknown keys above the known range are preserved for a newer valid minor record.
A schema major version other than 1 is rejected. Changed signed fields, changed
signatures, identity/key mismatch, domain replay, non-canonical CBOR, duplicate
keys, trailing bytes, and non-NFC text all fail verification.

## 6. Identity at rest

The keypair is stored only in `secrets/identity.enc` below the app-private root.
The exact format is:

```text
magic "NEXIDNT1" | version 1 | KDF 1 | flags 0 | 32-byte salt | 24-byte nonce
ciphertext = XChaCha20-Poly1305(
  HKDF-SHA256(LEK, salt, "NEXUS-IDENTITY-KEY-v1"),
  nonce,
  AAD = header,
  plaintext = 84-byte NEXKEY01 payload
)
```

The header is authenticated. A wrong LEK, changed header, truncated file, unknown
version, or unsupported KDF fails closed. A valid file is 168 bytes.

Writes are atomic: a new file is created, flushed, and renamed into place, and
temporary files are removed on failure. Unix uses `0600` for the file and `0700`
for the secrets directory. On Windows the file inherits the app-private parent
directory ACL. Symbolic links, junctions, and other reparse points on the root,
the `secrets` directory, or the identity file are rejected before any read or
write. No plaintext seed is written to a temporary file, SQLite, a log, a crash
report, or a metadata export.

## 7. Backup and restore

A backup is one user-selected binary file with:

```text
magic "NEXBKUP1" | version 1 | Argon2id KDF | flags 0
Argon2 version 0x13 | memory | iterations | parallelism
16-byte salt | 24-byte nonce | identity ID length | identity ID text
```

v1 defaults are 64 MiB, 3 iterations, and 1 lane. The encryption key is
`Argon2id(NFC(passphrase), salt, parameters)`. The associated data is
`header || "NEXUS-BACKUP-v1"`. The plaintext is the same 84-byte `NEXKEY01`
payload used at rest.

- New passphrases are NFC-normalized, limited to 1,024 UTF-8 bytes, and require at
  least 12 Unicode scalar values.
- A wrong passphrase, changed authenticated header, truncated ciphertext, or
  changed ciphertext returns one generic `BackupUnreadable` error. Structural
  damage and unsupported format version, KDF, or Argon2 parameters are rejected
  before key derivation as `MalformedBackup` or `UnsupportedBackupVersion`.
- Restore validates and decrypts the whole file before replacing the live
  identity. A failed restore leaves the current identity untouched.
- The backup contains no device peer key, no SQLite database, and no managed
  files.

## 8. Security, privacy, and identity limits

- Secret seeds and derived keys are held in zeroizing buffers where the API
  allows it. `KeyPair`, `Identity`, and `LocalEncryptionKey` redact `Debug`
  output.
- No private key or passphrase is stored in SQLite or a metadata archive.
- Error text is typed and does not include secret bytes or passphrases.
- Tests use only synthetic seeds, salts, nonces, and the well-known passphrase
  test phrase. No real user secret is committed.
- `SIGNATURE_STATUS_LABEL` is exactly `Signed record verified`. A valid signature
  proves control of the identity key, not a real name, legal authorship, or
  content legality.
- v1 has no automatic rotation, central revocation, password reset, or recovery
  service. Losing the key and all backups loses the identity. Compromise recovery
  requires a new identity and republishing; old records remain valid under the
  old key.

## 9. Committed vectors

The exact vectors and layouts are in
[SIGNED_RECORD_FORMATS.md](../architecture/SIGNED_RECORD_FORMATS.md). The Rust
tests assert:

| Vector | Assertion |
|---|---|
| Zero-seed Ed25519 public key | Exact hex. |
| Zero-seed identity ID | Exact `nx1...` string and decoded public key. |
| Publication domain signature | Exact 64-byte signature for `nexus-stage-06`. |
| Canonical CBOR scalars and map order | Exact encoded bytes. |
| Profile body | Exact full body hex, signature, envelope SHA-256. |
| Publication body | Exact full body hex, signature, envelope SHA-256, publication ID. |
| Deterministic at-rest file | Exact length, header hex, SHA-256, decrypt round trip. |
| Deterministic v1 backup | Exact length, header prefix, SHA-256, restore round trip. |

## 10. Tests and evidence

### Rust tests

`cargo test --workspace` passed with 98 tests, 0 failures.

| Suite | Tests |
|---|---:|
| `nexus-crypto` unit tests | 24 |
| `nexus-identity` unit tests | 9 |
| `nexus-identity` integration tests | 12 |
| `nexus-protocol` unit tests | 10 |
| `nexus-protocol` integration tests | 9 |
| `nexus-store` unit and integration tests | 28 |
| Other workspace unit tests | 6 |
| **Total** | **98** |

The 12 identity integration tests are:

| Test | Covers |
|---|---|
| `id_001_local_identity_has_a_stable_bech32m_id` | Fixed ID vector, embedded public key, no registry. |
| `id_001_identity_survives_a_restart_and_never_needs_a_server` | Vault create/load, stable ID, no silent replacement. |
| `symlinked_secrets_directory_is_rejected_before_read_or_write` | A linked `secrets` directory is rejected for exists, load, and save (where the host permits link creation). |
| `id_003_plaintext_key_material_is_absent_from_storage_and_debug_output` | Encrypted file, no seed or public key in ciphertext, redacted `Debug`, no temp files. |
| `id_004_encrypted_backup_restores_the_same_identity_and_signatures` | Backup round trip, restart signature, wrong-passphrase error. |
| `id_004_vault_backup_round_trips_with_v1_defaults` | Vault backup and restore with the 64 MiB defaults. |
| `id_005_key_loss_and_v1_limits_are_stated_without_fake_recovery` | Key-loss and no-recovery copy. |
| `id_006_unsupported_truncated_and_tampered_backups_do_not_replace_identity` | Version, truncation, and integrity failures preserve the live identity. |
| `id_006_committed_v1_backup_vector_is_exact` | Fixed salt/nonce/passphrase/seed length, header, and SHA-256. |
| `deterministic_at_rest_vector_is_exact_and_round_trips` | Fixed LEK/salt/nonce length, header, SHA-256, wrong-LEK rejection. |
| `id_007_the_same_backup_on_two_devices_signs_verifiable_records` | Two restores, same ID, cross-verifying signatures. |
| `id_008_status_label_does_not_claim_real_world_identity` | Status and real-world-identity notices. |

The 9 signed-record integration tests are:

| Test | Covers |
|---|---|
| `committed_profile_and_publication_vectors_are_exact` | Full body hex, signatures, envelope hashes, publication ID, canonical scalars. |
| `id_002_any_changed_signed_field_or_signature_fails_verification` | Changed body, changed signature, domain replay. |
| `identity_mismatch_and_cross_identity_verification_are_rejected` | Signing with the wrong key and verifying with another identity. |
| `unknown_minor_fields_are_preserved_and_unknown_major_versions_are_rejected` | Forward-compatible minor fields and major-version rejection. |
| `non_canonical_bodies_duplicate_keys_and_trailing_bytes_are_rejected` | Map order, duplicate keys, non-shortest integers, trailing bytes, non-canonical envelope. |
| `cbor_depth_items_and_input_bounds_are_enforced` | Depth and 64 KiB caps. |
| `user_text_is_nfc_on_sign_and_must_be_nfc_on_parse` | NFC normalization on sign and rejection on parse. |
| `publication_and_profile_bounds_are_enforced` | Names, tags, size, chunk size, revision, filenames. |
| `id_007_equal_revision_conflicts_remain_distinct_and_verifiable` | Equal-revision conflicts stay distinct and both verify. |

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
- `cargo test --workspace` passed with 98 Rust tests.
- `cargo deny check` reported `advisories ok, bans ok, licenses ok, sources ok`
  for the 109 locked packages.
- `cargo audit` scanned 109 dependencies against 1,509 advisories and reported
  no vulnerability.
- Repository hygiene scanned the working tree with no forbidden file or secret
  pattern.

### Clean checkout

After commit `fa40fcf` (`feat(crypto): add identity and signed records`), the
clean-checkout gate cloned `HEAD` into:

```text
C:\Users\Elsa\AppData\Local\Temp\nexus-clean-96faa4d6
```

and ran:

```text
.\scripts\verify-clean-checkout.ps1
```

Result: PASS, exit 0, `All requested local checks passed.`

The clone built the Android debug APK and Windows app image, passed all 65 JVM
tests, passed Rust formatting and Clippy, and passed all 96 Rust tests. Repository
hygiene passed, and the script removed the clone afterwards.

`verify-clean-checkout.ps1` runs `build.ps1` without `-WithDependencyScan`, so
the clean-checkout run does not repeat `cargo deny` or `cargo audit`. Those
policies were verified separately in the full local gate against the same
committed dependency graph.

### Kotlin boundary check

The Kotlin/JVM tests remain at 65 passing. No Kotlin source links
`nexus-crypto`, `nexus-identity`, or `nexus-protocol`, and no private key or
passphrase path exists in the shared UI. The Stage 04 screens continue to use
demo data and do not claim that Stage 06 identity behavior is connected.

## 11. Failure evidence and fixes

- The first `encrypt_identity_with_material` refactor passed references to fixed
  arrays where byte slices were expected. The helper now passes the arrays
  directly to HKDF and AEAD.
- The publication vector initially contained an incorrect number of repeated
  `0x11` content-ID bytes. The committed test now asserts the full exact body
  produced by the encoder.
- Signing a profile or publication originally stored the pre-normalized input
  object while the signed body contained NFC text. Signing now parses the signed
  body back into the returned model, so model fields and exact bytes cannot
  diverge.
- Clippy rejected needless explicit lifetimes in shared field helpers and the
  ten-argument publication constructor. Lifetimes were elided and the schema
  constructor documents its deliberate field-mirroring signature.
- The initial non-canonical-envelope test constructed bytes incorrectly. It now
  builds a real map with key 2 before key 1 and is rejected by canonical parsing.
- The backup and at-rest vector tests initially used wrong byte offsets. They now
  compare the exact 44-byte backup prefix and 68-byte at-rest header.
- Unused direct dependencies were removed from crates that did not use them;
  `getrandom` and `hex` remain direct dependencies of `nexus-crypto`, and
  `zeroize` is retained where secret material is held.
- A recheck found that `IdentityVault::load` and `exists` validated only the
  final `identity.enc` component, so a symlinked `secrets` directory could
  redirect reads. The root and `secrets` directory are now checked with symlink
  metadata, Windows reparse attributes, and directory checks before reads or
  writes. Regression tests cover a linked `secrets` directory and a
  non-directory `secrets` path.
- Restore paths copied the seed out of the decoded payload. They now move the
  validated `KeyPair` into the identity, avoiding another secret copy.
- `IdentityId::from_public_key` previously fell back to the invalid sentinel
  `nx1invalid` if Bech32m encoding failed. Encoding is infallible for the fixed
  HRP and 34-byte payload, and the code now asserts that invariant.
- Documentation overstated that every changed backup header returned the generic
  error. Version, KDF, Argon2-version, and parameter failures are now documented
  as pre-derivation structural errors; authenticated-data failures remain
  generic.

## 12. Limitations and deferred work

- No Android emulator or physical device is available, so Android runtime
  cross-verification of signatures, Argon2id, and file permissions is deferred to
  Stage 08. Stage 06 proves the byte-level vectors on Windows.
- GitHub Actions has not run remotely for this stage.
- Kotlin/FFI identity integration is deferred to Stage 08. No screen claims that
  identity creation or backup is connected.
- The Android Keystore and Windows DPAPI adapters are platform-shell work for
  Stage 08. Stage 06 consumes a 32-byte LEK and does not implement the platform
  seal.
- Publishing, chunk manifests, publication IDs in the store, revisions, and
  tombstones are Stage 07.
- Tombstone signing is a reserved domain tag and schema direction, not yet a
  signed body implementation.
- DHT key derivation constants are defined but unused until Stage 09.
- Backup passphrase strength is a UI policy in addition to the 12-character
  minimum; no zxcvbn-style estimator exists yet.
- There is no automatic key rotation or central recovery in v1, by design.

## 13. Exit criteria

| Exit criterion | Result | Evidence |
|---|---|---|
| Identity creation works without a central account server. | Pass | `id_001_*` tests; local key generation and ID derivation; no network dependency in the crates. |
| Signatures verify across Android and Windows. | Partial | Exact cross-platform vectors are committed and verified on Windows. Android runtime verification is deferred to Stage 08 because no device/emulator or Kotlin FFI link exists yet. |
| Changed signed records fail verification. | Pass | `id_002_*`, changed-body, changed-signature, non-canonical, duplicate-key, and identity-mismatch tests. |
| Key-loss limitations are clearly explained. | Pass | `id_005_*`, `id_008_*`, format specification section 10, and user-facing constants. |
| Private keys stay out of logs, analytics, ordinary database queries, and plaintext storage. | Pass | Encrypted at-rest file, redacted `Debug`, no SQLite dependency in `nexus-identity`, and no plaintext key in files or backups. Kotlin/FFI log paths remain Stage 08 evidence. |

## 14. Next stage

Stage 06 is complete. Stage 07 - Publishing, chunk manifests, revisions,
publication IDs, and tombstones - is next and requires explicit user permission.
It will build on the exact-byte envelope, profile, publication, and identity
formats established here.
