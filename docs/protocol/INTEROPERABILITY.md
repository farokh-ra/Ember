# Interoperability and Conformance

**Status:** Self-conformance suite implemented; independent-client and
independent-operator interoperability not yet available
**Protocol:** Ember protocol v1
**Last updated:** 2026-11-26

This document defines what "interoperable" means for Ember protocol v1, what the
repository verifies today, and what it does not verify. It is deliberately
narrower than a claim that independent clients already interoperate.

## 1. What is verified

The repository verifies that one implementation of the protocol is internally
consistent and that the published v1 vectors are exact:

| Layer | What is pinned | Where |
|---|---|---|
| Identity ID | Ed25519 public key and canonical `nx1...` spelling | `crates/nexus-identity/tests/identity.rs` |
| Canonical CBOR | Map order, shortest integers, duplicate rejection, bounds | `crates/nexus-crypto/src/cbor.rs` |
| Signed envelope | Exact body bytes, signature, envelope limits | `crates/nexus-protocol/src/envelope.rs` |
| Profile record | Exact body, signature, envelope SHA-256 | `crates/nexus-protocol/tests/records.rs` |
| Publication record | Exact body, signature, envelope SHA-256, publication ID | `crates/nexus-protocol/tests/records.rs` |
| Tombstone record | Exact body, signature, envelope SHA-256 | `crates/nexus-protocol/tests/stage07.rs` |
| Manifest record | Exact 2.5 MB body, manifest root, content ID | `crates/nexus-protocol/tests/stage07.rs` |
| DHT keys | Domain-separated key derivation and provider keys | `crates/nexus-protocol/src/dht.rs` |
| File wire messages | Request, response, error, and malformed inputs | `crates/nexus-protocol/src/file.rs` |
| Index wire messages | Request, response, error, cursor, and bounds | `crates/nexus-protocol/src/index.rs` |
| Transport | QUIC, TCP + Noise + Yamux, identify, ping, Kademlia, relay, DCUtR | `crates/nexus-net/tests/` |
| Peer-to-peer behavior | Discovery, transfer, replication, search, safety | `crates/nexus-testkit/` and stage tests |

It does not verify that a separately built client, written from this
specification alone, agrees with those vectors. That is the main missing
interoperability evidence.

## 2. Public conformance suite

`crates/nexus-protocol/tests/conformance.rs` is the executable public
conformance suite. It pins the same v1 vectors that appear in
[PROTOCOL_SPEC.md](PROTOCOL_SPEC.md) and adds rejection cases that an
independent implementation must also reject.

Run it with:

```powershell
cargo test -p nexus-protocol --test conformance
```

The suite treats the specification as the authority for expected bytes. A
failure means either the implementation or the published vector changed; both
are protocol review events.

### 2.1 Positive cases

- The committed profile body, signature, and envelope SHA-256 are exact.
- The committed publication body, signature, envelope SHA-256, and publication
  ID are exact.
- The committed tombstone body, signature, and envelope SHA-256 are exact.
- The committed 2.5 MB manifest body and manifest root are exact.
- The identity ID for the committed seed is canonical Bech32m.
- A valid record round-trips without re-encoding its body.
- Unknown minor fields in a supported major schema are preserved.

### 2.2 Negative cases

- Any changed signed field or signature fails verification.
- A signature from one record family cannot be replayed into another.
- A record signed by a different identity is rejected.
- Unknown major schema versions are rejected.
- Non-canonical CBOR, duplicate map keys, non-shortest integers, and trailing
  bytes are rejected.
- Oversized bodies and envelopes are rejected.
- A manifest with the wrong chunk count or chunk size is rejected.
- File and index messages with mixed shapes, unknown shapes, or oversized
  fields are rejected.

## 3. What "independent" would require

The missing evidence is not a unit-test gap. It requires all of the following:

1. A client implementation produced outside this repository by someone who
   worked from the public specification and vectors, not from the Rust source.
2. A record produced by that client and accepted by Ember, plus a record
   produced by Ember and accepted by that client.
3. A file transfer between the two implementations with manifest, chunk, and
   signature verification on both sides.
4. A DHT record exchange through a third-party bootstrap or relay, not only a
   loopback harness.
5. A documented result, including failures, on a named date and version.

No such client or result exists today. No independent operator has published a
bootstrap or relay address for this project. The network has not been exercised
outside Windows loopback with configurable local infrastructure.

## 4. Test harness

The existing harness is deterministic and local:

- `nexus-testkit` builds fixtures, identities, records, and transfer scenarios.
- Stage networking tests run multiple peers in one process over loopback.
- File and index codecs are tested with malformed, truncated, and mutated input.
- The public conformance suite adds a second, documentation-facing pin on the
  exact v1 vectors.

The harness is useful for regression detection and for an external implementer
who wants a fast feedback loop. It is not a substitute for a second
implementation or a real network.

## 5. Compatibility expectations

A v1-conforming client:

- accepts the v1 profile, publication, tombstone, and manifest schemas;
- preserves unknown minor fields in a supported major schema;
- rejects unknown major schemas and unknown wire versions;
- verifies every signature against the claimed identity before display or
  storage;
- re-serves exact signed bytes rather than re-encoding them;
- verifies every manifest and chunk before trusting file bytes;
- treats bootstrap peers, relays, and index providers as optional hints;
- never requires a central account, database, or file host.

A client that cannot meet these rules is not v1-conforming. It may still be
useful, but it must not be described as interoperable with Ember v1.

## 6. Known gaps

| Gap | Status | Impact |
|---|---|---|
| Independent client implementation | Not available | No cross-implementation proof |
| Independent bootstrap operator | Not available | No real-network discovery evidence |
| Independent relay operator | Not available | No third-party NAT-traversal evidence |
| Third-party index provider | Not available | No third-party search evidence |
| Packet capture or wire transcript | Not published | Debugging an external failure is harder |
| Cross-language vector runner | Not available | Vectors are exercised only through Rust |
| Fuzzing at the wire boundary | Partial | Malformed-input coverage is test-based, not coverage-guided |
| Long-running network soak with mixed versions | Not available | Upgrade overlap is unproven at network scale |

These gaps are recorded, not waived. The public launch gate in
[STAGE_20.md](../progress/STAGE_20.md) remains closed while they are open.

## 7. Reporting an interoperability failure

An external implementer should report:

- the specification and vector version used;
- the client name, version, language, and commit;
- the exact bytes or a minimal reproducible message;
- the expected result and the observed result;
- whether the failure is in parsing, verification, transport, or display.

Do not include private keys, personal data, or another person's files. Use the
private channel in [SECURITY.md](../../SECURITY.md) for anything that could be a
security defect.

## 8. References

- [Protocol specification](PROTOCOL_SPEC.md)
- [Versioning and deprecation](VERSIONING_AND_DEPRECATION.md)
- [Signed record formats](../architecture/SIGNED_RECORD_FORMATS.md)
- [File transfer](../architecture/FILE_TRANSFER.md)
- [DHT design](../architecture/DHT_DESIGN.md)
