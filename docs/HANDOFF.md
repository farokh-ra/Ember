# NEXUS Project Handoff

**Current stage:** 06 - Identity and cryptographic foundations
**Stage status:** Complete
**Last updated:** 2026-11-13
**Next stage:** 07 - Publishing, manifests, revisions, and tombstones (waiting for user permission)

## What was implemented

Stage 06 is complete. The Rust core now creates and protects local identities and
verifies exact-byte signed records, while Kotlin remains a demo shell until the
FFI bridge:

- Local Ed25519 key generation from the OS CSPRNG, with no account server.
- Stable public identity IDs in Bech32m with HRP `nx` and the Ed25519 multicodec
  payload.
- Canonical CBOR with shortest integers, sorted scalar map keys, duplicate-key
  rejection, NFC user text, bounded depth/items/input, and decode/re-encode byte
  equality.
- Exact-byte `{1: body bstr, 2: sig bstr}` envelopes with signatures over
  `domain_tag || body`. Stored bodies are never re-encoded for verification.
- Signed profile v1 and publication v1 schemas, unknown minor-field preservation,
  unknown major-version rejection, and stable publication IDs.
- Encrypted `secrets/identity.enc` storage with a host-supplied 32-byte LEK,
  HKDF-SHA256, XChaCha20-Poly1305, an authenticated 68-byte header, atomic
  restrictive writes, and symlink/reparse-point rejection.
- A versioned encrypted backup with Argon2id v1 defaults of 64 MiB, 3 iterations,
  and 1 lane; NFC passphrases; generic wrong-passphrase failure; and no central
  recovery.
- User-facing statements that distinguish a verified signature from real-world
  identity and disclose the absence of v1 rotation or revocation.
- 21 new integration tests plus committed identity, signature, canonical CBOR,
  signed-record, at-rest, and backup vectors.
- Exact v1 format specification in
  `docs/architecture/SIGNED_RECORD_FORMATS.md`.
- Stage 06 evidence in `docs/progress/STAGE_06.md`.

No publishing, chunk manifest, transfer, hosting, DHT, or network operation is
implemented yet. The Kotlin screens continue to use visibly labelled demo data
and do not claim that Stage 06 identity behavior is connected.

## Files changed

Crypto crate:

- `crates/nexus-crypto/Cargo.toml`
- `crates/nexus-crypto/src/lib.rs`
- `crates/nexus-crypto/src/aead.rs`
- `crates/nexus-crypto/src/cbor.rs`
- `crates/nexus-crypto/src/domain.rs`
- `crates/nexus-crypto/src/error.rs`
- `crates/nexus-crypto/src/hash.rs`
- `crates/nexus-crypto/src/identity_id.rs`
- `crates/nexus-crypto/src/kdf.rs`
- `crates/nexus-crypto/src/key.rs`
- `crates/nexus-crypto/src/random.rs`
- `crates/nexus-crypto/src/text.rs`

Identity crate:

- `crates/nexus-identity/Cargo.toml`
- `crates/nexus-identity/src/lib.rs`
- `crates/nexus-identity/src/at_rest.rs`
- `crates/nexus-identity/src/backup.rs`
- `crates/nexus-identity/src/error.rs`
- `crates/nexus-identity/src/identity.rs`
- `crates/nexus-identity/src/local_key.rs`
- `crates/nexus-identity/src/payload.rs`
- `crates/nexus-identity/src/vault.rs`
- `crates/nexus-identity/tests/identity.rs`

Protocol crate:

- `crates/nexus-protocol/Cargo.toml`
- `crates/nexus-protocol/src/lib.rs`
- `crates/nexus-protocol/src/envelope.rs`
- `crates/nexus-protocol/src/error.rs`
- `crates/nexus-protocol/src/fields.rs`
- `crates/nexus-protocol/src/profile.rs`
- `crates/nexus-protocol/src/publication.rs`
- `crates/nexus-protocol/tests/records.rs`

Workspace and documentation:

- `Cargo.toml`, `Cargo.lock`
- `docs/architecture/SIGNED_RECORD_FORMATS.md`
- `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md`
- `docs/progress/STAGE_06.md`
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
- All 65 JVM tests passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace` passed with 98 Rust tests.
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
| `nexus-protocol` unit tests | 10 | 0 |
| `nexus-protocol` integration tests | 9 | 0 |
| `nexus-store` unit and integration tests | 28 | 0 |
| Other Rust workspace tests | 6 | 0 |
| **Rust total** | **98** | **0** |
| **JVM total** | **65** | **0** |

### Validators

```text
python scripts/check_repo_hygiene.py
python scripts/check_commit_message.py "fix(identity): harden vault paths and restore handling"
```

Both pass.

### Clean checkout

After commit `ecbd659` (`fix(identity): harden vault paths and restore
handling`), `scripts/verify-clean-checkout.ps1` cloned `HEAD` into
`C:\Users\Elsa\AppData\Local\Temp\nexus-clean-32257651` and ran the build, tests,
and hygiene gate. It returned exit 0: Gradle built the Android debug APK and
Windows app image, all 65 JVM tests passed, Rust format, Clippy, and all 98
workspace tests passed, and repository hygiene passed. The clone was removed
afterwards.

The clean-checkout script does not run `-WithDependencyScan`; `cargo deny` and
`cargo audit` were verified separately in the full local gate against the same
committed graph. This follow-up evidence commit changes only documentation, so it
does not alter build inputs.

## Errors and failed attempts

1. The deterministic at-rest helper passed fixed-array references where byte
   slices were expected. It now passes the arrays directly to HKDF and AEAD.
2. The publication vector contained the wrong number of repeated content-ID
   bytes. The test now asserts the exact body produced by the encoder.
3. Signing a profile or publication returned the pre-normalization input object
   while the signed body contained NFC text. Signing now parses the signed body
   back into the returned model, so model and bytes cannot diverge.
4. Clippy rejected needless explicit lifetimes and a ten-argument schema
   constructor. Lifetimes were elided; the constructor documents its deliberate
   field-mirroring signature.
5. The first non-canonical-envelope test built invalid bytes. It now constructs a
   real map with key 2 before key 1 and canonical parsing rejects it.
6. The backup and at-rest vector tests initially used incorrect offsets. They now
   compare the exact 44-byte backup prefix and 68-byte at-rest header.
7. Unused direct dependencies were removed so the inventory matches the realized
   graph.
8. A recheck found that vault `load` and `exists` validated only the final
   `identity.enc` component, so a symlinked `secrets` directory could redirect
   reads. The root and `secrets` directory are now checked with symlink metadata,
   Windows reparse attributes, and directory checks before reads or writes.
9. Restore paths copied the seed out of the decoded payload. They now move the
   validated `KeyPair` into the identity.
10. `IdentityId::from_public_key` fell back to the sentinel `nx1invalid` if
    Bech32m encoding failed. The fixed HRP and 34-byte payload cannot fail to
    encode, so the code now asserts that invariant.
11. The format specification overstated that every changed backup header
    returned the generic error. Version, KDF, Argon2-version, and parameter
    failures are now documented as pre-derivation structural errors.

## Decisions and unresolved questions

- Stage 06 was explicitly approved by the project owner.
- ADR-0006 and ADR-0007 are implemented without changing the architecture.
- Canonical CBOR is hand-implemented in `nexus-crypto` rather than delegated to a
  general CBOR library.
- `getrandom` 0.3.4 is used directly instead of adding `rand`.
- `SignedRecord::parse` requires a canonical envelope; verification still uses
  the exact stored body bytes and never re-encodes them.
- Profile and publication bodies must be NFC for user-visible text. Signing
  normalizes first; parsing rejects non-NFC bytes.
- The encrypted payload is a shared versioned fixed binary `NEXKEY01` payload,
  not CBOR.
- The backup format is versioned and stores Argon2id parameters in the header so
  future calibration can change defaults without changing the file version.
- Windows restrictive permissions rely on the app-private parent ACL. Unix uses
  directory `0700` and file `0600`.
- No Android emulator or physical device is available, so Android runtime
  cross-verification is deferred to Stage 08. The committed vectors are the
  byte-level contract.
- Remote GitHub Actions execution has not run for this stage.
- Kotlin/FFI identity integration and platform Keystore/DPAPI adapters remain
  Stage 08 work.

## Exact next action

1. Review the Stage 06 evidence in `docs/progress/STAGE_06.md` and the exact
   formats in `docs/architecture/SIGNED_RECORD_FORMATS.md`.
2. Give explicit approval to start Stage 07.
3. Stage 07 adds canonical chunk manifests, manifest roots, publishing,
   revisions, publication IDs in the data layer, and tombstone verification.
   Do not begin it before that approval.
