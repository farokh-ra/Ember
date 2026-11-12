# ADR-0006 — Ed25519, SHA-256, manifest-authenticated chunks, canonical CBOR

- **Status:** Accepted
- **Date:** 2026-11-12
- **Deciders:** NEXUS engineering
- **Supersedes:** none
- **Related:** ADR-0002, ADR-0004, ADR-0005, ADR-0007, [Architecture](../ARCHITECTURE.md)

## Context

NEXUS must prove that a publication was created by a specific identity, that downloaded bytes are exactly the published bytes, and that two platforms compute identical identifiers and signatures. The product already fixes two primitives: **Ed25519** for signatures and **SHA-256** for content identity. It leaves the record encoding, chunk structure, identity ID format, key protection, and backup format to architecture.

The design must be unambiguous across Kotlin and Rust, avoid signature malleability and cross-protocol reuse, keep a 4 GiB file within bounded memory, fit a publication record under 64 KiB, and allow replicas to serve the original signed record byte-for-byte.

## Decision

### Primitives

| Purpose | Primitive | Notes |
|---|---|---|
| User signing identity | Ed25519 | 32-byte public key, 64-byte signature, strict verification. |
| Content identity | SHA-256 | Hash of the complete file bytes, exactly as published. |
| Chunk integrity | SHA-256 | One hash per fixed-size chunk. |
| Manifest integrity | SHA-256 | Hash of the canonical manifest bytes with a domain tag. |
| Record signing | Ed25519 | Signature over the domain tag plus canonical body bytes. |
| At-rest key encryption | XChaCha20-Poly1305 | 24-byte random nonce, authenticated header. |
| Backup encryption | XChaCha20-Poly1305 | 24-byte random nonce, authenticated header. |
| Passphrase KDF | Argon2id | Versioned parameters stored in the backup header. |
| Local-secret KDF | HKDF-SHA256 | Derives the at-rest key from the 32-byte LEK. |
| Randomness | OS CSPRNG | `getrandom`/`OsRng`; never a userspace PRNG seeded from time. |

No custom cryptographic construction is introduced. The core uses the RustCrypto primitives and `ed25519-dalek` through vetted crates.

### Identity ID

The public identity ID is a **Bech32m** string with human-readable prefix `nx`, encoding the multicodec value for an Ed25519 public key followed by the 32-byte public key.

- Example shape: `nx1...` (the exact test vector is committed in Stage 06).
- Bech32m provides a checksum and detects transcription errors.
- The identity ID is derived only from the public key. There is no registry, username, or account.
- The private key never leaves the core in plaintext. The UI receives only the identity ID and public metadata.
- This is a cryptographic identity, not a claim about a person's legal name or real-world identity.

Publication, profile, and tombstone records include the uploader's identity ID. A verifier decodes the public key from that ID and verifies the signature. A record whose identity ID does not match its public key, or whose signature fails, is rejected.

### Canonical CBOR

Signed bodies use a deliberately small canonical CBOR profile:

- Definite lengths only. No indefinite strings, arrays, or maps.
- Shortest integer encoding. No tags, floats, NaN, or undefined values.
- Map keys are integers in the signed record schemas. For any map with non-integer keys, keys are sorted by the bytewise lexicographic order of their deterministic encodings.
- Text is valid UTF-8, normalized to NFC where the field is user-visible text.
- Duplicate map keys are rejected.
- Maximum nesting depth and item counts are bounded before allocation.

The canonical encoder is implemented in `nexus-crypto` and covered by RFC 8949 vectors plus property tests. Verification parses the body, re-encodes it canonically, and rejects the record if the bytes differ. This prevents signature malleability through non-canonical encodings.

Signed records use an envelope that preserves the exact signed bytes:

```text
SignedRecord = {
  1: body: bstr,   // canonical CBOR body
  2: sig: bstr     // 64-byte Ed25519 signature
}
```

The signature is over:

```text
domain_tag || body
```

The body is never re-encoded to verify a stored record. Replicas store and serve the exact envelope bytes.

### Domain separation

Every signature and hash has a distinct context string. A signature for one record type cannot be replayed as another.

| Context | Value |
|---|---|
| Publication signature | `NEXUS-PUB-v1` |
| Profile signature | `NEXUS-PROFILE-v1` |
| Tombstone signature | `NEXUS-TOMBSTONE-v1` |
| Publication ID | `NEXUS-PUB-ID-v1` |
| Manifest root | `NEXUS-MANIFEST-v1` |
| Backup encryption AAD | `NEXUS-BACKUP-v1` |
| At-rest key derivation | `NEXUS-IDENTITY-KEY-v1` |
| DHT publication key | `NEXUS-DHT-PUB-v1` |
| DHT tombstone key | `NEXUS-DHT-TOMB-v1` |
| DHT profile key | `NEXUS-DHT-PROFILE-v1` |

The context strings are protocol constants. Changing one is a protocol major-version change.

### Publication record

Publication bodies use integer keys and a v1 schema. The logical fields are:

| Field | Type | Limit / rule |
|---|---|---|
| Schema | uint | `1` |
| Identity ID | text | Valid `nx1...` Bech32m ID |
| Content ID | 32-byte bstr | SHA-256 of complete file bytes |
| Size | uint | 1 byte to 4 GiB |
| Display filename | text | At most 255 UTF-8 bytes; display only |
| Description | text | At most 4,096 UTF-8 bytes |
| Category | uint | Closed v1 category enum |
| Tags | array of text | At most 10 tags, each at most 32 UTF-8 bytes |
| Published at | uint | Unix milliseconds, informational |
| Manifest root | 32-byte bstr | Domain-separated SHA-256 of manifest |
| Chunk size | uint | `1,048,576` bytes in v1 |
| Revision | uint | Starts at 1; increases for metadata changes by the same identity and content |
| Supersedes | 32-byte bstr or null | Previous publication ID for a revision |
| Parent content ID | 32-byte bstr or null | Optional modified-from reference |
| Parent publication ID | 32-byte bstr or null | Optional modified-from reference |
| App version | text | Record-producing app semantic version |

Rules:

- The complete signed envelope must be at most **64 KiB**. A body larger than the limit is rejected before signing.
- A parent reference is informational. It does not imply authorization, endorsement, or authorship of the parent.
- A new revision supersedes the previous record. Peers may still cache and serve the old record; the UI shows the newest verified revision and labels older cached versions as superseded.
- Replicas never rewrite the uploader, signature, content ID, manifest root, or signed fields.

### Publication ID and content ID

- **Content ID** = SHA-256 over the complete file bytes. Identical bytes produce the same content ID on every platform.
- **Publication ID** = SHA-256 of `NEXUS-PUB-ID-v1 || body`. It is unique to the signed publication body and stable across replicas.
- Content ID groups bytes. Publication ID distinguishes uploaders, metadata, and revisions.
- IDs are stored as 32 raw bytes and displayed as lowercase hex. The UI shortens them for display and always offers a copy action.

### Chunk manifest and download verification

v1 uses a flat, manifest-authenticated chunk list rather than a Merkle tree:

- Chunk size is fixed at **1 MiB** (`1,048,576` bytes) for every file. The final chunk may be smaller.
- Maximum file size is 4 GiB, so a manifest has at most 4,096 entries.
- The manifest contains the content ID, total size, chunk size, and an ordered list of `(chunk_size, sha256)` entries.
- Canonical manifest bytes are at most 256 KiB. A manifest larger than the cap is rejected.
- **Manifest root** = SHA-256 of `NEXUS-MANIFEST-v1 || canonical_manifest_bytes`.
- The publication record contains the manifest root and chunk size, not the per-chunk list.
- A download fetches the manifest, checks its length and canonical form, checks the root against the publication record, checks the content ID and size, and then requests chunks by index.
- Every chunk is verified against its manifest hash and expected length before it is written to the verified prefix. Invalid chunks are discarded and retried or failed over.
- After the last chunk, the complete file is hashed with SHA-256 and checked against the content ID. Only then is the file atomically promoted to a verified blob.
- A partial download records only the verified contiguous prefix. On restart, any unverified tail is truncated.

A Merkle tree is not required at the v1 size limit. It can replace the flat manifest in a later protocol major version if chunk counts grow.

### Profiles and tombstones

- A profile is a signed body keyed by identity ID. It carries display name, bio, optional avatar content ID, publication references, revision, and update time. It is at most 64 KiB.
- Usernames are not unique. The UI shows the display name plus a short identity ID and surfaces collisions; the identity ID is the unique key.
- A tombstone is a signed body referencing a publication ID, content ID, identity ID, and unpublish time. It is published under a separate DHT key.
- A replica that sees a valid tombstone marks the publication "unpublished by uploader" but may continue serving the original record and bytes. The tombstone never deletes another peer's copy.
- Concurrent profile revisions from multiple devices are kept as conflicts rather than silently merged. A higher revision with a valid signature is newer; equal revisions with different bodies are a visible conflict.

### Identity at rest

- The identity keypair is stored in `secrets/identity.enc`, not in SQLite.
- The file header contains a magic value, format version, KDF identifier, HKDF salt, and nonce.
- The encryption key is HKDF-SHA256(LEK, salt, `NEXUS-IDENTITY-KEY-v1`). The LEK is supplied by the host from Android Keystore or Windows DPAPI.
- The header is authenticated as additional data. A modified header, wrong LEK, or truncated file fails closed.
- Identity files are written atomically with restrictive permissions. No plaintext key is ever written to a temporary file, log, crash report, or database.

### Identity backup

- Backup is user-initiated and produces a single versioned binary file.
- Header fields: magic, format version, Argon2id version, memory cost, iterations, parallelism, 16-byte salt, 24-byte nonce, and the identity ID.
- Key = Argon2id(NFC(passphrase), salt, memory, iterations, parallelism). The v1 default is 64 MiB, 3 iterations, 1 lane; the exact calibration is verified on the Android baseline in Stage 06.
- Payload = XChaCha20-Poly1305(key, nonce, AAD = header bytes || `NEXUS-BACKUP-v1`, plaintext = keypair + identity metadata).
- A wrong passphrase returns one generic failure. The UI does not reveal whether the passphrase or the file was wrong.
- The backup contains no device peer key, no local database, and no managed files.
- Automatic key rotation and central recovery do not exist in v1. Compromise recovery means creating a new identity and republishing. Old records remain valid under the old key.
- Passphrases are NFC-normalized before KDF input and bounded to 1,024 UTF-8 bytes. The UI enforces a minimum length of 12 characters and warns that there is no recovery.

## Consequences

**Positive**

- One unambiguous byte-level format for signatures, IDs, manifests, and backups.
- Raw signed records can be served by replicas without re-encoding.
- Flat manifests keep the 4 GiB case bounded and simple; a 256 KiB cap is easy to enforce.
- Separate peer and user identities limit network linkability.
- Domain separation prevents cross-record signature reuse.
- Key material is outside SQLite and outside metadata exports.

**Negative / trade-offs**

- Canonical CBOR must be implemented carefully and tested against vectors; libraries do not guarantee canonical output by default.
- A flat manifest repeats 32 bytes per MiB; at 4 GiB it is roughly 160 KiB before overhead, still under the cap.
- Argon2id at 64 MiB is noticeable on low-end Android devices; calibration may lower memory while keeping the format versioned.
- NFC normalization of passphrases is a compatibility decision; it must be identical on both platforms.
- No key rotation means a compromised identity cannot be repaired in place.
- Content ID computation requires reading the whole file before publishing; this is expected and streamed.

**Constraints**

- No SHA-1, MD5, or non-authenticated encryption.
- No record may exceed 64 KiB; no manifest may exceed 256 KiB; no chunk may exceed 1 MiB.
- No plaintext private key at rest, in logs, or in backups.
- No unsigned metadata may be treated as authoritative.
- Any change to a signed field, canonical encoding, domain tag, chunk size, or ID derivation requires a protocol major-version review.

## Alternatives considered

1. **JSON Canonicalization Scheme (JCS).** Human-readable, but larger, more parser edge cases, and a weaker fit for binary hashes. Rejected.
2. **Protocol Buffers for signed records.** Compact and typed, but canonical encoding is not guaranteed across implementations without extra rules, and schema evolution can change bytes. Rejected.
3. **BLAKE3 content IDs.** Faster, but the product fixes SHA-256 for content identity. Rejected.
4. **Merkle tree instead of a flat manifest.** Better for very large files and partial proofs, but unnecessary at 4 GiB and more complex to implement correctly. Deferred to a later protocol major version.
5. **Use the libp2p peer key as the signing identity.** Fewer keys, but links the user identity to network addresses and blocks independent device peer identities. Rejected.
6. **Central key recovery or automatic rotation.** Better user recovery, but violates the no-central-recovery and no-central-backend rules. Rejected.
7. **Store keys in SQLite encrypted with a passphrase entered at every launch.** Simpler platform integration, but worse usability and a larger secret blast radius. Rejected.
8. **DID:key or IPFS CID strings for identity/content IDs.** Standards-based, but adds unrelated semantics and dependencies; Bech32m plus raw SHA-256 is sufficient and self-contained. Rejected.

## Verification

- Stage 06 commits cross-platform vectors for identity IDs, signatures, canonical CBOR, backup round-trip, wrong passphrase, and tampered headers.
- Stage 07 commits vectors for content IDs, chunk manifests, manifest roots, publication IDs, revisions, and tombstone verification.
- Stage 08 proves Android and Windows produce identical bytes and verify each other's signatures.
- Stage 12 corrupts, duplicates, reorders, truncates, and substitutes chunks and proves rejection and safe recovery.
- Stage 19 fuzzes CBOR decoding, record validation, manifest bounds, and backup parsing.
