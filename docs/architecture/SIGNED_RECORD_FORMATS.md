# Ember Signed Record and Identity Formats v1

| Field | Value |
|---|---|
| Document ID | NEXUS-ARCH-RECORDS-001 |
| Version | 1.1.0 |
| Status | Stage 07 implementation reference |
| Date | 2026-11-14 |
| Normative sources | [Public protocol specification](../protocol/PROTOCOL_SPEC.md), [ADR-0006](../decisions/ADR-0006-crypto-and-record-formats.md), [ADR-0007](../decisions/ADR-0007-protocol-boundaries-versioning.md) |
| Code | `nexus-crypto`, `nexus-identity`, `nexus-protocol`, `nexus-transfer`, `nexus-store` |

This document defines the exact v1 bytes for identity IDs, canonical CBOR,
signed record envelopes, profile, publication, chunk-manifest, and tombstone
bodies, identity-at-rest storage, and encrypted identity backups. It also records
the committed cross-platform vectors. The public, implementation-independent
summary is [PROTOCOL_SPEC.md](../protocol/PROTOCOL_SPEC.md); this document keeps
the deeper storage and backup detail. Changing a byte layout, a domain tag, a
field meaning, or a canonical-encoding rule requires a protocol major-version
review.

## 1. Ownership and trust

- `nexus-crypto` owns canonical CBOR, Ed25519, SHA-256, Bech32m, AEAD, and KDF
  primitives. It has no network, SQLite, filesystem, or UI dependency.
- `nexus-identity` owns local key generation, encrypted at-rest storage, backup,
  restore, and user-facing key-loss statements. It does not store keys in SQLite.
- `nexus-protocol` owns signed-record schemas, exact-byte envelopes, profile,
  publication, manifest, and tombstone verification, and publication IDs.
- `nexus-transfer` owns streaming content hashing, manifest construction, chunk
  verification, and whole-file verification with bounded memory.
- `nexus-store` owns immutable storage of exact signed publication and tombstone
  bytes, content-addressed manifest files, and reconciliation. It never rewrites
  signed bytes or reassigns uploader attribution.
- Kotlin and the FFI layer never parse a signed record as authoritative and never
  receive a plaintext private key.
- Only verified record bytes may become authoritative local metadata. Derived
  display columns, search entries, and cached summaries are never authoritative
  over the signed bytes.

## 2. Identity ID

An identity ID is a **Bech32m** string:

```text
HRP      = "nx"
payload  = 0xed 0x01 || 32-byte Ed25519 public key
total    = 34 payload bytes
```

`0xed 0x01` is the unsigned varint multicodec value for an Ed25519 public key.
The canonical form is lowercase. Parsing rejects a wrong HRP, a wrong checksum
variant, uppercase or mixed-case spelling, a wrong payload length, and a wrong
multicodec. The public key is recovered only from the ID; there is no registry,
username, account server, or central assignment.

Committed vector for the 32-zero-byte Ed25519 seed:

```text
public key = 3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29
identity   = nx1a5qnk638hn8tdfpdv23635p2duxhxefjz4m3mcjr5cavqj9p3dva52g6l8d37
```

A signed record includes the identity ID as text. Verification decodes the
embedded public key, verifies the signature with that key, and rejects the record
if the signing key does not match the ID.

## 3. Canonical CBOR profile

Signed bodies and envelopes use a deliberately small CBOR profile:

1. Definite lengths only. Indefinite strings, arrays, and maps are rejected.
2. Shortest integer encoding only. A non-shortest encoding is an error.
3. No tags, floats, undefined values, or reserved simple values.
4. Map keys are scalar values. Array and map keys are rejected.
5. Map entries are sorted by the bytewise lexicographic order of their canonical
   key encodings. Duplicate keys are rejected.
6. Text is valid UTF-8. User-visible text in profile and publication bodies must
   already be NFC; signing normalizes it first and parsing rejects non-NFC bytes.
7. Signed-record bounds are depth 16, 512 parsed values including keys, and
   65,536 input bytes. The encoder enforces the same bounds.
8. The decoder re-encodes the parsed value and requires byte equality with the
   input. Any non-canonical encoding is rejected.

Example canonical vectors:

```text
0                         -> 00
23                        -> 17
24                        -> 1818
100                       -> 1864
{"a": 1, "b": 2}          -> a2616101616202
{1: 1, 2: "nx"}           -> a2010102626e78
```

## 4. Signed envelope

```text
SignedRecord = {
  1: body: bstr,   // canonical CBOR body, exact bytes
  2: sig:  bstr    // 64-byte Ed25519 signature
}
```

- The complete canonical envelope is at most 65,536 bytes.
- The body is non-empty and at most 65,440 bytes, leaving envelope headroom.
- The envelope must contain exactly keys 1 and 2. Extra fields are rejected.
- The signature covers `domain_tag || body`, using the exact stored body bytes.
- Verification never re-encodes a stored body. Replicas store and serve the exact
  envelope bytes from `SignedRecord::raw`.
- Parsing the envelope requires canonical CBOR. Parsing a profile, publication,
  manifest, or tombstone additionally requires a canonical body and validates
  every schema field.
- `ProfileV1::parse_body`, `PublicationV1::parse_body`, `ManifestV1::parse_body`,
  and `TombstoneV1::parse_body` validate shape and canonical encoding only. They
  do not authenticate a record. Only the signed-record wrappers establish that
  the body was signed by the identity it names.
- A manifest is authenticated indirectly: the publication signs its
  domain-separated root.

Domain tags:

| Context | Bytes |
|---|---|
| Publication signature | `NEXUS-PUB-v1` |
| Profile signature | `NEXUS-PROFILE-v1` |
| Tombstone signature | `NEXUS-TOMBSTONE-v1` |
| Publication ID | `NEXUS-PUB-ID-v1` |
| Manifest root | `NEXUS-MANIFEST-v1` |
| Backup AAD | `NEXUS-BACKUP-v1` |
| At-rest key derivation | `NEXUS-IDENTITY-KEY-v1` |
| DHT publication key | `NEXUS-DHT-PUB-v1` |
| DHT tombstone key | `NEXUS-DHT-TOMB-v1` |
| DHT profile key | `NEXUS-DHT-PROFILE-v1` |

The DHT values are reserved protocol constants; Stage 07 does not perform network
lookups.

## 5. Profile body v1

The body is a CBOR map with integer keys 1-9. All are required.

| Key | Type | Rule |
|---:|---|---|
| 1 | uint | Schema major version, `1`. |
| 2 | text | Canonical `nx1...` identity ID. |
| 3 | text | Display name, 1-64 Unicode scalar values. |
| 4 | text or null | Non-unique username label, 3-32 scalar values. |
| 5 | text | Bio, 0-500 scalar values. |
| 6 | bstr(32) or null | Optional avatar content ID. |
| 7 | array of bstr(32) | Publication references, at most 64. |
| 8 | uint | Revision, at least 1. |
| 9 | uint | Informational update time, Unix milliseconds. |

Unknown integer keys above 9 are preserved as long as the schema major version
remains 1. They are re-encoded canonically when the profile is signed again. A
schema value other than 1 is rejected.

## 6. Publication body v1

The body is a CBOR map with integer keys 1-16. All are required.

| Key | Type | Rule |
|---:|---|---|
| 1 | uint | Schema major version, `1`. |
| 2 | text | Canonical `nx1...` identity ID. |
| 3 | bstr(32) | SHA-256 of the complete file bytes. |
| 4 | uint | File size, 1 byte to 4 GiB. |
| 5 | text | Display filename, non-empty, at most 255 UTF-8 bytes. |
| 6 | text | Description, at most 4,096 UTF-8 bytes. |
| 7 | uint | Closed category code 0-7. |
| 8 | array of text | At most 10 tags, each non-empty and at most 32 UTF-8 bytes. |
| 9 | uint | Informational publication time, Unix milliseconds. |
| 10 | bstr(32) | Domain-separated chunk-manifest root. |
| 11 | uint | Chunk size, exactly `1,048,576`. |
| 12 | uint | Revision, at least 1. |
| 13 | bstr(32) or null | Previous publication ID for a revision. |
| 14 | bstr(32) or null | Optional modified-from content ID, informational only. |
| 15 | bstr(32) or null | Optional modified-from publication ID, informational only. |
| 16 | text | Record-producing app version, non-empty, at most 64 bytes. |

Category codes: `0` documents, `1` images, `2` audio, `3` video, `4` software,
`5` data, `6` archives, `7` other. Unknown codes are rejected.

Unknown integer keys above 16 are preserved. A schema value other than 1 is
rejected. Parent references never imply authorization, endorsement, or authorship.

Publication ID:

```text
publication_id = SHA-256("NEXUS-PUB-ID-v1" || body)
```

It is computed over the exact canonical body bytes and is stable across replicas.

## 7. Chunk manifest v1

A manifest is not separately signed. The publication record signs the
domain-separated root of the canonical manifest body, so a replica that forwards
the manifest cannot alter chunk lengths, chunk hashes, the content ID, or the
total size without changing the signed root.

The manifest body is a CBOR map with integer keys 1-5. All are required.

| Key | Type | Rule |
|---:|---|---|
| 1 | uint | Schema major version, `1`. |
| 2 | bstr(32) | SHA-256 of the complete file bytes; the content ID. |
| 3 | uint | Complete file size, 1 byte to 4 GiB. |
| 4 | uint | Chunk size, exactly `1,048,576`. |
| 5 | array | Ordered chunk entries, 1 to 4,096 items. |

Each chunk entry is a two-item array `[length: uint, sha256: bstr(32)]`. Schema
rules:

1. Every chunk except the last is exactly `1,048,576` bytes.
2. The final chunk is 1 to `1,048,576` bytes.
3. Chunk lengths sum exactly to the total size.
4. The entry count is exactly `ceil(total_size / chunk_size)`.
5. The canonical manifest body is at most 262,144 bytes.
6. Empty files are not publishable in v1.
7. Manifest decoding uses depth 8, 20,000 parsed values, and the 262,144-byte
   input cap. Unknown integer keys above 5 are preserved; a schema value other
   than 1 is rejected.

```text
manifest_root = SHA-256("NEXUS-MANIFEST-v1" || canonical_manifest_body)
```

The root is domain-separated from the raw content ID. A publication record must
have the same content ID, total size, chunk size, and manifest root as the
manifest it references. Verification computes the root over the exact stored
manifest bytes and never re-encodes them first.

Manifests contain no file bytes. They are stored as exact canonical bytes at
`store/manifests/<root-hex>.cbor`; the filename is the trusted root, and a file
whose bytes do not hash to that root is corrupt and quarantined.

## 8. Tombstone body v1

A tombstone withdraws a publication without deleting any stored file or
rewriting the original publication record. The original uploader signs it. A
replica may relay and store a tombstone but cannot alter uploader attribution.

The body is a CBOR map with integer keys 1-6. All are required.

| Key | Type | Rule |
|---:|---|---|
| 1 | uint | Schema major version, `1`. |
| 2 | text | Canonical `nx1...` identity ID of the original uploader. |
| 3 | bstr(32) | Publication ID being withdrawn. |
| 4 | bstr(32) | Content ID of the withdrawn publication. |
| 5 | uint | Tombstone revision, at least 1. |
| 6 | uint | Informational unpublish time, Unix milliseconds. |

Unknown integer keys above 6 are preserved. A schema value other than 1 is
rejected. The envelope and signature rules are the same as for other signed
records, with domain tag `NEXUS-TOMBSTONE-v1`.

Tombstone rules:

1. A tombstone must reference the identity, publication ID, and content ID of
   the signed publication it withdraws. Any mismatch is rejected.
2. A higher tombstone revision supersedes a lower revision. Revisions may skip
   numbers because a peer may observe only part of the history.
3. A successor must keep the same identity, publication ID, and content ID.
4. Equal revisions with different signed bytes are conflicts. They remain
   distinct records; no silent merge or discard is permitted.
5. A tombstone may arrive before its publication row and remains authoritative
   on its own. Storage must preserve its original signed attribution.
6. A tombstone does not delete replicas, does not prove deletion from other
   peers, and does not revoke the cryptographic validity of the publication
   signature. It records the original uploader's withdrawal verdict.

## 9. Identity at rest

The identity keypair is stored only in:

```text
<app-private-root>/secrets/identity.enc
```

It is never stored in SQLite, metadata exports, logs, diagnostics, or crash
reports. The platform shell supplies a 32-byte local encryption key (LEK) from
Android Keystore or Windows DPAPI. The Rust type `LocalEncryptionKey` is
zeroized on drop and redacted in `Debug` output.

The file header is exactly 68 bytes:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | Magic `NEXIDNT1` |
| 8 | 2 | Format version, little-endian u16, `1` |
| 10 | 1 | KDF identifier, `1` = HKDF-SHA256 |
| 11 | 1 | Flags, `0` |
| 12 | 32 | HKDF salt |
| 44 | 24 | XChaCha20-Poly1305 nonce |

The derived key is:

```text
key = HKDF-SHA256(LEK, salt, "NEXUS-IDENTITY-KEY-v1")
```

The header is authenticated as additional data. Ciphertext is the 84-byte
plaintext payload plus the 16-byte Poly1305 tag, so a valid file is 168 bytes.
A changed header, wrong LEK, truncated file, or unknown version fails closed.

Plaintext payload layout (encrypted in both at-rest files and backups):

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | Magic `NEXKEY01` |
| 8 | 2 | Payload version, little-endian u16, `1` |
| 10 | 1 | Key type, `1` = Ed25519 |
| 11 | 1 | Flags, `0` |
| 12 | 8 | Creation time, little-endian u64 Unix milliseconds |
| 20 | 32 | Ed25519 public key |
| 52 | 32 | Ed25519 secret seed |

Writes are atomic: a new file is created in the `secrets` directory, flushed,
and renamed over the destination. Temporary files are removed on failure. Unix
uses mode `0600` for the file and `0700` for the directory. On Windows the file
inherits the app-private parent directory ACL. Symbolic links, junctions, and
other reparse points on the root, the `secrets` directory, or the identity file
are rejected before any read or write.

## 10. Encrypted backup v1

A backup is one user-selected binary file. It contains only the identity keypair
and creation time: no device peer key, no SQLite database, and no managed files.

Header length is `69 + N`, where `N` is the identity ID text length:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | Magic `NEXBKUP1` |
| 8 | 2 | Format version, little-endian u16, `1` |
| 10 | 1 | KDF identifier, `1` = Argon2id |
| 11 | 1 | Flags, `0` |
| 12 | 4 | Argon2 version, little-endian u32, `0x13` |
| 16 | 4 | Memory cost in KiB, little-endian u32 |
| 20 | 4 | Iterations, little-endian u32 |
| 24 | 4 | Parallelism, little-endian u32 |
| 28 | 16 | Argon2 salt |
| 44 | 24 | XChaCha20-Poly1305 nonce |
| 68 | 1 | Identity ID text length `N` |
| 69 | N | Canonical `nx1...` identity ID |

v1 defaults are 65,536 KiB (64 MiB), 3 iterations, and 1 lane. Parsing bounds
untrusted values to 1,048,576 KiB, 16 iterations, and 16 lanes before allocating
memory.

```text
key = Argon2id(NFC(passphrase), salt, memory, iterations, parallelism)
AAD = header_bytes || "NEXUS-BACKUP-v1"
plaintext = 84-byte NEXKEY01 payload
ciphertext = XChaCha20-Poly1305 ciphertext || 16-byte tag
```

Passphrases are NFC-normalized, must not be empty, are limited to 1,024 UTF-8
bytes, and new backups require at least 12 Unicode scalar values. A wrong
passphrase, changed authenticated header, truncated ciphertext, or changed
ciphertext returns one generic error:
`backup could not be opened; the passphrase or file is invalid`. The UI does not
reveal which part failed, and a failed restore never replaces the live identity
file. Structural damage and an unsupported format version, KDF, or Argon2
parameter set are rejected before key derivation as `MalformedBackup` or
`UnsupportedBackupVersion`; those checks reveal nothing about the passphrase.

A valid backup of the zero-seed vector is 233 bytes with:

```text
salt       = 16 bytes of 0x33
nonce      = 24 bytes of 0x44
created_at = 1700000000000
length     = 233
SHA-256    = d3cf4d6ba0e22e5e8c26276629c8c3f77899d05d1b1656eb78a3cb493c3febee
```

## 11. Versioning and unknown fields

- Unknown fields in a valid minor record are preserved. They are carried through
  parse and re-sign, and are never treated as authoritative display data.
- Unknown major schema versions are rejected.
- Unknown envelope fields are rejected.
- A record with a changed signed field, changed signature, non-canonical body,
  duplicate key, non-NFC user text, out-of-range value, or identity/key mismatch
  is rejected before storage.
- Equal revisions with different signed bodies are conflicts. They remain
  distinct records; no silent merge or discard is permitted.
- A newer verified revision may supersede an older one. Old cached records remain
  valid and are labelled as superseded by higher layers.

## 12. Key lifecycle and recovery limits

- v1 has no automatic key rotation and no central revocation or recovery.
- Losing the identity file and every backup loses the identity. There is no
  password reset, account recovery, or server-held escrow.
- A compromised key cannot be repaired in place. The user must create a new
  identity and republish. Records signed by the old key remain cryptographically
  valid under that old key and must not be presented as belonging to the new
  identity.
- A backup restores the same identity ID and signing capability on any platform
  that implements this specification.
- The UI must say **"Signed record verified"**, not "identity verified". A valid
  signature proves control of the identity key. It does not prove a real name,
  legal authorship, or that content is legal.

## 13. Committed cross-platform vectors

All vectors use synthetic material. The seed is 32 zero bytes unless stated.

### Ed25519 and identity

```text
message          = "NEXUS-PUB-v1" || "nexus-stage-06"
signature        = 3fe32d30a11f1d3e0c9da80cb9f82d84b5fb517a1d5fddf952736fe785cdb779ea6db58a773b9d2bc92d527f97f69d81278dcfb636a3ee55c0368fa03f019e00
publication hash = c5109ddb870d0325411cee8a934308fde01b3995fbeb1d67f2db5b5fdac625f1
```

### Profile v1

```text
display name = "Ada Lovelace"
username     = "ada"
bio          = "Local-first notes"
revision     = 1
updated_at   = 1700000000000
body SHA-256 = d23d436c02be3ef14da6c2a1188294c38c63e5f4097a5b642a13f7f8126017fb
signature    = c640003c1849c912705ca5e8ac7e4c6af33befbaa87ce536950dbb23bf23f044a0336e0ce38d667ab67d17eaf6bb18850023db4f29af995a1f7cf0d237fb8c09
envelope SHA-256 = f2f9d085d8873335772fb436aaa171c35e6a74de4f8dae3d15e3a7fe27680193
```

The committed test asserts the full body hex, the exact signature, and the exact
envelope hash.

### Publication v1

```text
content ID    = 32 bytes of 0x11
size          = 4096
filename      = "notes.txt"
description   = "Stage 06 vector"
category      = 0 (documents)
manifest root = 32 bytes of 0x22
revision      = 1
published_at  = 1700000000000
app version   = "0.1.0"
tags          = ["stage06", "vector"]
signature     = 6e4459e8cc18e2ee0e8c95840a1899a3c7005b882f8a5c44b832c7a0156e9ac5f0173295e3e4a7c539d231cbd5f725e3c132eb9401a3d71be4097dba03cf990f
envelope SHA-256 = 24561ecbb4040cd8291d486a84bdc7d3f96cfcc1c8fa0962901afeacfa35f04f
publication ID = 40fcc4cc64e3c49ce8e102cfa34ba4c5b225a637b48098f463d428356b028c21
```

The committed tests assert the full canonical body hex, the exact signature, the
exact envelope hash, and the publication ID.

### Chunk manifest v1

The vector is 2,500,000 bytes generated by `(i * 31 + 7) % 251`, independently
of the Rust implementation. It produces three chunks: 1,048,576, 1,048,576, and
402,848 bytes.

```text
content ID     = 910b21fbdf3343e088e1ac6c2aa9aaffad0e893a43ff3b7d282e43f2f4d1e0fa
chunk 0 SHA-256 = 1c59b8670027384143781a8a8bff2f3b44bd8818d0f53b13b064c2375a1afe38
chunk 1 SHA-256 = d50d98efacff4cc59dedce2c1b0add120d2c6264418cc699883fa1fd389ac7a9
chunk 2 SHA-256 = 3ff806e9f7fdd1004fa33358186c040d7cf5704684cc360d53c7864b83ede395
manifest root  = d99a72330148d149138852e4afebcf486e84ac1853b1da13f937e5485a913a30
body length    = 172 bytes
one-byte change content ID = 3441cd7ebe54222fcc3f01bffc3194f41a3a7a237b09fedc8d8910d983e920b8
```

Canonical manifest body hex:

```text
a50101025820910b21fbdf3343e088e1ac6c2aa9aaffad0e893a43ff3b7d282e43f2f4d1e0fa
031a002625a0041a001000000583821a0010000058201c59b8670027384143781a8a8bff2f3b
44bd8818d0f53b13b064c2375a1afe38821a001000005820d50d98efacff4cc59dedce2c1b0a
dd120d2c6264418cc699883fa1fd389ac7a9821a000625a058203ff806e9f7fdd1004fa33358
186c040d7cf5704684cc360d53c7864b83ede395
```

### Tombstone v1

```text
identity ID      = nx1a5qnk638hn8tdfpdv23635p2duxhxefjz4m3mcjr5cavqj9p3dva52g6l8d37
publication ID   = 32 bytes of 0x44
content ID       = 32 bytes of 0x11
revision         = 1
unpublished_at   = 1700000100000
body length      = 152 bytes
signature        = 988cafcacf714ac3dde780c2785af0b66d2c41bc75787e4ca9718a2f55c06e86a1e2784254381374b9e6e5f9ec66d420b7c82e49672606ff528a1f2a7c548f09
envelope SHA-256 = c018998945ee4355c349238a849dafbb59ec44aee9027375ec2ded7d4caa0561
```

Canonical tombstone body hex:

```text
a601010278406e78316135716e6b363338686e387464667064763233363335703264757868786566
6a7a346d336d636a7235636176716a397033647661353267366c3864333703582044444444444444
44444444444444444444444444444444444444444444444444045820111111111111111111111111
11111111111111111111111111111111111111110501061b0000018bcfe6eea0
```

### Deterministic at-rest file

```text
LEK        = 32 bytes of 0x55
salt       = 32 bytes of 0x66
nonce      = 24 bytes of 0x77
created_at = 1700000000000
length     = 168
SHA-256    = fcad93c2d40b8ec483c1c7c164e33f9228e40e0c21f5f147f09e4ff3f5cde84b
```

### Rejection vectors

The tests also commit and exercise:

- wrong passphrase returns the single generic backup error;
- tampered, truncated, and unsupported-version backups do not replace identity;
- changed signed fields and changed signatures fail verification;
- profile domain replay fails;
- identity ID and signing key mismatch fails;
- non-canonical map order, duplicate keys, non-shortest integers, trailing bytes,
  and a non-canonical envelope fail;
- non-NFC user text fails parsing;
- depth, item, and 64 KiB bounds fail;
- unknown minor fields are preserved and unknown major versions are rejected;
- a one-byte change produces a different content ID and manifest root;
- corrupted, reordered, truncated, and over-long chunks and whole files are
  rejected;
- manifest bytes that do not hash to the signed root are rejected;
- a manifest whose content ID, size, chunk size, or root differs from its signed
  publication is rejected;
- a changed tombstone body, changed signature, or publication-domain replay
  fails verification;
- a tombstone that changes identity, publication ID, or content ID relative to
  the signed publication is rejected;
- a lower or equal tombstone revision cannot replace a higher revision, and
  equal revisions with different bytes are conflicts.

## 14. Cross-platform conformance

Android and Windows must reproduce:

1. the same public key and `nx1...` ID for the same seed;
2. the same canonical body bytes and Ed25519 signature;
3. the same envelope bytes and publication ID;
4. the same content ID, chunk hashes, canonical manifest bytes, and manifest root
   for the committed 2.5 MB fixture;
5. the same tombstone body, signature, and envelope hash;
6. the same backup header and derived bytes for fixed salt, nonce, passphrase,
   parameters, and seed;
7. the same generic failure for a wrong passphrase or tampered file, and the same
   rejection of corrupted chunks or invalid signatures.

The Rust tests in `nexus-crypto`, `nexus-identity`, `nexus-protocol`,
`nexus-transfer`, and `nexus-store` are the executable reference. Stage 07 runs
them on Windows. Android runtime cross-verification is deferred to Stage 08
because the Kotlin shells do not link the Rust core yet and no Android device or
emulator is available locally. The committed vectors are the byte-level contract
for that later check.
