# Ember Protocol Specification v1

**Status:** Public draft for Stage 20 review
**Protocol major version:** 1
**Wire versions:** `/nexus/file/1`, `/nexus/index/1`, `/nexus/kad/1.0.0`
**Record schemas:** profile, publication, tombstone, and manifest schema v1
**Applies to:** Ember `0.1.x`
**Last updated:** 2026-11-26

This document is the public description of the Ember peer protocol. It covers
identity, signed records, discovery records, bounded file and index messages,
transport expectations, limits, and compatibility rules. It is normative for
protocol version 1. Where this document and the implementation disagree, the
implementation is the bug; a protocol change must update both.

The Rust types in `crates/nexus-protocol` and `crates/nexus-crypto` are the
reference implementation. ADR-0005, ADR-0006, and ADR-0007 remain the
architecture decisions behind this specification.

## 1. Design rules

- Devices are peers. There is no required account, central database, file host,
  search service, or moderation authority.
- Signed records are preserved as exact bytes. A replica re-serves the original
  envelope; it never re-encodes a signed body.
- Every received record is validated at the boundary before it is stored,
  indexed, or displayed.
- Every message is bounded and every operation has a deadline. A timeout is a
  partial or failed result, never success.
- Bootstrap peers, relays, and index providers are optional infrastructure.
  They cannot grant identity, ownership, moderation, or signature authority.
- Unknown minor fields in a valid record are preserved. Unknown major versions
  are rejected. There is no silent downgrade.
- File bytes never enter signed records, the DHT control value, or the index
  control envelope.

## 2. Identity

An Ember identity is an Ed25519 key pair. The public identity ID is a canonical
Bech32m string:

```text
identity_id = bech32m("nx", 0xed01 || ed25519_public_key_32_bytes)
```

- Human-readable prefix: `nx`
- Multicodec varint for an Ed25519 public key: `0xed 0x01`
- Payload length: 34 bytes
- Canonical spelling is lowercase; parsing a non-canonical spelling or checksum
  fails
- The private key never leaves the device except in an explicit encrypted backup
- A display name or username is profile metadata, not identity

Committed vector for the all-zero seed used by the test suite:

```text
public key: 3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29
identity ID: nx1a5qnk638hn8tdfpdv23635p2duxhxefjz4m3mcjr5cavqj9p3dva52g6l8d37
```

Signatures are Ed25519 over:

```text
signing_message = domain_tag_bytes || signed_body_bytes
```

The domain tag is part of the signed message. A signature made for one record
family never verifies for another.

## 3. Canonical CBOR

Signed bodies and control messages use a deliberately small canonical CBOR
profile:

- definite lengths only
- shortest integer encodings only
- no tags, floats, undefined values, or indefinite items
- map keys are scalar values ordered by the bytewise lexicographic order of
  their canonical encodings
- duplicate map keys are rejected
- text must be valid UTF-8
- nesting depth, item count, and input length are bounded
- decoding rejects trailing bytes, so the canonical re-encoding must equal the
  input

Unsigned record fields use non-negative integer keys. Unknown keys above the
highest known key are preserved when the record's major schema version is
supported.

## 4. Signed record envelope

A signed record is one canonical CBOR map with exactly two entries:

| Key | Type | Meaning |
|---|---|---|
| 1 | byte string | Exact canonical signed body bytes |
| 2 | byte string | Ed25519 signature over `domain_tag || body` |

Limits:

| Limit | Value |
|---|---|
| Complete signed envelope | 65,536 bytes |
| Signed body | 65,440 bytes |
| Signature | 64 bytes |

The envelope body is opaque to the envelope parser. The record family is
determined by parsing the body and verifying its signature with the embedded
identity ID. A malformed, over-sized, non-canonical, or wrong-domain record is
rejected and is never rendered as valid.

## 5. Signed record families

### 5.1 Profile schema v1

Domain tag: `NEXUS-PROFILE-v1`

| Key | Field | Type | Rules |
|---|---|---|---|
| 1 | schema | unsigned | Must be `1` |
| 2 | identity ID | text | Canonical `nx1...`; signature must verify against it |
| 3 | display name | text | 1-64 Unicode scalar values after normalization |
| 4 | username | text or null | 3-32 Unicode scalar values; not unique, not an account |
| 5 | bio | text | At most 500 Unicode scalar values |
| 6 | avatar content ID | bytes or null | 32 bytes when present |
| 7 | publication refs | array of bytes | At most 64 entries, each 32 bytes |
| 8 | revision | unsigned | At least 1; higher revisions supersede lower ones |
| 9 | updated at | unsigned | Unix milliseconds, informational only |

Text is normalized before signing. A profile with equal revision numbers and
different bodies is a visible conflict; wall-clock time is not used to pick a
winner.

Committed vector:

```text
body:       a901010278406e78316135716e6b363338686e3874646670647632333633357032647578687865666a7a346d336d636a7235636176716a397033647661353267366c38643337036c416461204c6f76656c616365046361646105714c6f63616c2d6669727374206e6f74657306f607800801091b0000018bcfe56800
signature:  c640003c1849c912705ca5e8ac7e4c6af33befbaa87ce536950dbb23bf23f044a0336e0ce38d667ab67d17eaf6bb18850023db4f29af995a1f7cf0d237fb8c09
envelope SHA-256: f2f9d085d8873335772fb436aaa171c35e6a74de4f8dae3d15e3a7fe27680193
```

### 5.2 Publication schema v1

Domain tag: `NEXUS-PUB-v1`

| Key | Field | Type | Rules |
|---|---|---|---|
| 1 | schema | unsigned | Must be `1` |
| 2 | identity ID | text | Canonical `nx1...`; signer must match |
| 3 | content ID | bytes | SHA-256 of the complete file, 32 bytes |
| 4 | size | unsigned | 1 byte through 4 GiB |
| 5 | display filename | text | 1-255 UTF-8 bytes |
| 6 | description | text | At most 4,096 UTF-8 bytes |
| 7 | category | unsigned | `0` documents, `1` images, `2` audio, `3` video, `4` software, `5` data, `6` archives, `7` other |
| 8 | tags | array of text | At most 10 tags, each 1-32 UTF-8 bytes |
| 9 | published at | unsigned | Unix milliseconds, informational only |
| 10 | manifest root | bytes | 32-byte domain-separated manifest root |
| 11 | chunk size | unsigned | Must be `1,048,576` (1 MiB) |
| 12 | revision | unsigned | At least 1 |
| 13 | supersedes | bytes or null | Required for revision > 1; forbidden for revision 1; 32-byte publication ID |
| 14 | parent content ID | bytes or null | Informational; 32 bytes |
| 15 | parent publication ID | bytes or null | Informational; 32 bytes |
| 16 | app version | text | 1-64 UTF-8 bytes, informational |

Publication ID:

```text
publication_id = SHA-256("NEXUS-PUB-ID-v1" || canonical_publication_body)
```

A newer revision keeps the old signature valid and supersedes the old record for
display and serving preference. A replica may continue to hold the old bytes.
Timestamps are not an ordering authority.

Committed vector:

```text
body:       b001010278406e78316135716e6b363338686e3874646670647632333633357032647578687865666a7a346d336d636a7235636176716a397033647661353267366c3864333703582011111111111111111111111111111111111111111111111111111111111111110419100005696e6f7465732e747874066f537461676520303620766563746f7207000882677374616765303666766563746f72091b0000018bcfe568000a582022222222222222222222222222222222222222222222222222222222222222220b1a001000000c010df60ef60ff61065302e312e30
publication ID: 40fcc4cc64e3c49ce8e102cfa34ba4c5b225a637b48098f463d428356b028c21
envelope SHA-256: 24561ecbb4040cd8291d486a84bdc7d3f96cfcc1c8fa0962901afeacfa35f04f
```

### 5.3 Tombstone schema v1

Domain tag: `NEXUS-TOMBSTONE-v1`

| Key | Field | Type | Rules |
|---|---|---|---|
| 1 | schema | unsigned | Must be `1` |
| 2 | identity ID | text | Original uploader; signer must match |
| 3 | publication ID | bytes | Withdrawn publication, 32 bytes |
| 4 | content ID | bytes | Withdrawn content, 32 bytes |
| 5 | revision | unsigned | At least 1 |
| 6 | unpublished at | unsigned | Unix milliseconds, informational only |

A tombstone does not delete copies held by other peers and does not rewrite the
original publication. It is signed by the original uploader and remains valid
independently of the original record.

Committed vector:

```text
body:       a601010278406e78316135716e6b363338686e3874646670647632333633357032647578687865666a7a346d336d636a7235636176716a397033647661353267366c38643337035820444444444444444444444444444444444444444444444444444444444444444444045820111111111111111111111111111111111111111111111111111111111111111110501061b0000018bcfe6eea0
signature:  988cafcacf714ac3dde780c2785af0b66d2c41bc75787e4ca9718a2f55c06e86a1e2784254381374b9e6e5f9ec66d420b7c82e49672606ff528a1f2a7c548f09
envelope SHA-256: c018998945ee4355c349238a849dafbb59ec44aee9027375ec2ded7d4caa0561
```

### 5.4 Manifest schema v1

Domain tag: `NEXUS-MANIFEST-v1`

| Key | Field | Type | Rules |
|---|---|---|---|
| 1 | schema | unsigned | Must be `1` |
| 2 | content ID | bytes | SHA-256 of the complete file, 32 bytes |
| 3 | total size | unsigned | 1 byte through 4 GiB; equals the sum of chunk lengths |
| 4 | chunk size | unsigned | Must be `1,048,576` |
| 5 | chunks | array | At most 4,096 entries; each entry is `[length, sha256]` |

Rules:

- Chunk count must equal `ceil(total_size / 1,048,576)`.
- Every chunk except the last is exactly 1 MiB; the last is 1 byte through
  1 MiB.
- Each entry contains the chunk length and the SHA-256 of the exact chunk bytes.
- The canonical manifest is at most 256 KiB.
- The publication signs only the manifest root, not the whole manifest.

Manifest root:

```text
manifest_root = SHA-256("NEXUS-MANIFEST-v1" || canonical_manifest_body)
```

Committed 2.5 MB vector:

```text
manifest body:  a50101025820910b21fbdf3343e088e1ac6c2aa9aaffad0e893a43ff3b7d282e43f2f4d1e0fa031a002625a0041a001000000583821a0010000058201c59b8670027384143781a8a8bff2f3b44bd8818d0f53b13b064c2375a1afe38821a001000005820d50d98efacff4cc59dedce2c1b0add120d2c6264418cc699883fa1fd389ac7a9821a000625a058203ff806e9f7fdd1004fa33358186c040d7cf5704684cc360d53c7864b83ede395
manifest root:  d99a72330148d149138852e4afebcf486e84ac1853b1da13f937e5485a913a30
content ID:     910b21fbdf3343e088e1ac6c2aa9aaffad0e893a43ff3b7d282e43f2f4d1e0fa
```

## 6. Discovery records

Discovery uses Kademlia over `/nexus/kad/1.0.0`. It is a private Ember DHT, not
the IPFS DHT.

### 6.1 DHT keys

All value keys are 32 bytes and domain-separated:

```text
publication key = SHA-256("NEXUS-DHT-PUB-v1"     || publication_id)
tombstone key   = SHA-256("NEXUS-DHT-TOMB-v1"    || publication_id)
profile key     = SHA-256("NEXUS-DHT-PROFILE-v1" || identity_id_ascii)
index provider key = SHA-256("NEXUS-DHT-INDEX-v1")
content provider key = content_id
```

The content provider key is the raw 32-byte content ID. Provider records are not
hashed value records. A value record is accepted only when its signature
verifies and its identifier derives the DHT key it was stored under.

### 6.2 Values and provider records

- A publication, profile, or tombstone DHT value is one exact signed envelope.
- Maximum DHT value: 65,536 bytes.
- Maximum provider addresses: 8.
- Maximum length of one provider multiaddress: 256 bytes.
- Provider addresses are hints. The downloader verifies the manifest, chunks,
  and signatures before using any bytes.
- A record that fails validation is quarantined or dropped; it never overwrites
  a valid record.

## 7. File protocol `/nexus/file/1`

The file protocol carries metadata and bounded chunk slices. Request and
response bodies are canonical CBOR maps. File bytes never enter the control
envelope or the DHT.

Limits:

| Limit | Value |
|---|---|
| Encoded file message | 512 KiB |
| Chunk slice | 64 KiB |
| Chunk size | 1 MiB |
| Error text | 128 ASCII bytes |

### 7.1 Metadata request

| Key | Field | Type | Value |
|---|---|---|---|
| 1 | wire version | unsigned | `1` |
| 2 | kind | unsigned | `0` metadata |
| 3 | selector | unsigned | `0` content ID, `1` publication ID |
| 4 | selector value | bytes | 32 bytes |

### 7.2 Chunk request

| Key | Field | Type | Rules |
|---|---|---|---|
| 1 | wire version | unsigned | `1` |
| 2 | kind | unsigned | `1` chunk |
| 4 | content ID | bytes | 32 bytes |
| 5 | chunk index | unsigned | Zero-based |
| 6 | offset | unsigned | Byte offset within the 1 MiB chunk |
| 7 | length | unsigned | 1 through 65,536; offset + length within the chunk |

### 7.3 Metadata response

| Key | Field | Type |
|---|---|---|
| 1 | wire version | unsigned `1` |
| 2 | kind | unsigned `0` |
| 3 | manifest | exact canonical manifest bytes |
| 4 | publication | optional exact signed publication bytes |

### 7.4 Chunk response

| Key | Field | Type |
|---|---|---|
| 1 | wire version | unsigned `1` |
| 2 | kind | unsigned `1` |
| 5 | data | 1-65,536 bytes |

### 7.5 Error response

| Key | Field | Type |
|---|---|---|
| 1 | wire version | unsigned `1` |
| 6 | error | ASCII error code text, at most 128 bytes |

Responses must be exactly one of metadata, chunk, or error. Mixed shapes are
rejected. A client assembles one complete 1 MiB chunk in memory and verifies it
against the manifest before writing it to disk.

## 8. Index protocol `/nexus/index/1`

Search is local-first. A peer may opt in as an index provider. Provider
responses are hints: every returned profile or publication is an exact signed
envelope and must be verified again by the requester.

Limits:

| Limit | Value |
|---|---|
| Encoded index message | 1 MiB |
| Query | 128 UTF-8 bytes, no control characters |
| Tag filters | 5, each 1-32 UTF-8 bytes |
| Page size | 1-20 |
| Cursor | 128 bytes |
| Error text | 128 ASCII bytes |

### 8.1 Request

| Key | Field | Type | Rules |
|---|---|---|---|
| 1 | wire version | unsigned | `1` |
| 2 | query | text | Required |
| 3 | kind | unsigned | Optional: `0` profile, `1` publication |
| 4 | category | text | Optional canonical category name |
| 5 | tags | array of text | Optional exact filters; all must match |
| 6 | cursor | text | Optional opaque cursor |
| 7 | limit | unsigned | Required page size |

### 8.2 Response

| Key | Field | Type | Rules |
|---|---|---|---|
| 1 | wire version | unsigned | `1` |
| 2 | records | array | At most 20 records; each `{1: kind, 2: signed envelope}` |
| 3 | next cursor | text | Optional |
| 4 | complete | bool | Required for a non-error response |
| 5 | indexed at | unsigned | Optional Unix milliseconds |
| 6 | error | text | Exclusive error response |

An error response carries only keys 1 and 6. It never carries records, a cursor,
or a completion flag. The protocol never carries unsigned metadata, ranking,
popularity, or a global result set.

## 9. Transport and peer connectivity

Ember uses libp2p with:

- QUIC v1 over UDP
- TCP with Noise authentication and Yamux multiplexing
- Identify and ping for peer metadata and liveness
- AutoNAT for reachability detection
- Circuit Relay v2 for bounded relayed connectivity
- DCUtR for direct-connection upgrades where possible
- Kademlia in client mode by default; server mode only for an entry point

Published addresses must be full multiaddresses including `/p2p/<peer-id>`.
Bootstrap addresses without `/p2p` are rejected. A relay advertises HOP only
after it has a confirmed external address.

Default relay limits are documented in
[Relay Node Operations](../operations/RELAY_NODES.md). Relayed circuits are
bounded control paths, not a file-transfer transport of last resort.

## 10. Errors

Stable codes from ADR-0007:

| Code | Retryable |
|---|---|
| `invalid_argument` | No |
| `not_found` | Sometimes |
| `conflict` | No automatic merge |
| `quota_exceeded` | After user action |
| `verification_failed` | No for the same bytes |
| `unsupported_version` | No |
| `network_unavailable` | Yes |
| `timeout` | Yes |
| `cancelled` | Yes |
| `io_error` | Sometimes |
| `internal` | With diagnostics |

Human-readable text may change. Error text is redacted and bounded. Wire error
responses carry only the stable ASCII code.

## 11. Security properties

- Signatures cover the exact signed body bytes and a record-family tag.
- Identity IDs embed the public key; there is no registry lookup.
- Every peer-supplied record is verified against the claimed identity.
- Signed records are never re-encoded by a relay, replica, or index provider.
- File content is verified chunk by chunk against a signed manifest root.
- All control messages are bounded; malformed input must not panic.
- No record is accepted solely because a bootstrap peer, relay, index provider,
  or popular peer supplied it.
- Downloaded files are never executed automatically.

## 12. Compatibility and conformance

- [Versioning and deprecation](VERSIONING_AND_DEPRECATION.md)
- [Interoperability and conformance](INTEROPERABILITY.md)
- [FFI interface](../architecture/FFI_INTERFACE.md)
- [Protocol boundaries and versioning](../decisions/ADR-0007-protocol-boundaries-versioning.md)
