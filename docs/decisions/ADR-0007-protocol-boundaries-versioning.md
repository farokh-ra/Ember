# ADR-0007 — Protocol boundaries and versioning rules

- **Status:** Accepted
- **Date:** 2026-11-12
- **Deciders:** NEXUS engineering
- **Supersedes:** none
- **Related:** ADR-0003, ADR-0005, ADR-0006, ADR-0010, [Architecture](../ARCHITECTURE.md)

## Context

Identity, profiles, discovery, publication, transfer, replication, and search use different records and failure modes. If one layer parses or mutates another layer's data directly, validation is duplicated, signed bytes can be rewritten, and a malformed message in one domain can corrupt state in another. NEXUS also needs explicit compatibility rules because Android and Windows apps will update at different times, peers will run older versions, and cached records can outlive the app that created them.

## Decision

Use **seven protocol domains** with explicit ownership, immutable signed record bytes, and independent versions.

### Domain ownership

| Domain | Owns | Must not |
|---|---|---|
| Identity | Key generation, identity ID, signatures, backup/restore, key custody. | Touch the network, publication policy, or SQLite record queries. |
| Profile | Signed profile bodies, revisions, display metadata, avatar references. | Publish files, choose download sources, or mutate another identity's profile. |
| Publication | Signed publication bodies, revisions, parent references, tombstones. | Serve bytes, choose replicas, or rewrite signed fields. |
| Discovery | DHT keys, provider records, peer routing, bootstrap and manual peers, freshness metadata. | Trust a record without verification or make availability claims. |
| Transfer | Manifest exchange, chunk requests, verification, pause/resume, source failover. | Mark a file complete before final verification or advertise a partial file. |
| Replication | Local hosting policy, replica verification, serving state, re-verification. | Transfer authorship or alter the original record. |
| Search | Local index, index queries, result verification, coverage and freshness labels. | Mutate signed records, rank by popularity, or require a global index. |

Cross-domain calls go through `nexus-core` services. A lower layer never reaches into another layer's tables or files. For example, `nexus-net` delivers raw bounded record bytes to `nexus-protocol` for validation; it does not insert rows into `nexus-store`.

### Record flow

```text
peer bytes
  -> bounded framing and size check
  -> CBOR parse
  -> schema and field validation
  -> signature verification against the embedded identity ID
  -> canonical-byte check where the record is signed
  -> quarantine or store exact bytes + parsed query fields
```

- Signed bodies are preserved as exact bytes. Re-serving a replica record does not re-encode it.
- Validation happens at the receiving boundary, not later in the UI.
- Unknown fields in a valid newer minor record are preserved but not interpreted. Unknown major versions are rejected.
- A record that fails verification is quarantined or dropped; it is never rendered as valid and never overwrites a valid record.

### Version vectors

| Version | Current v1 value | Compatibility rule |
|---|---|---|
| FFI ABI | `1` | Exact match between bundled library and app. |
| App version | Semantic version | Shown in About and diagnostics. |
| Protocol version | `1` | Major version in protocol IDs and DHT record schema. |
| Publication schema | `1` | Unknown minor fields preserved; unknown major rejected. |
| Profile schema | `1` | Same rule. |
| Tombstone schema | `1` | Same rule. |
| Manifest schema | `1` | Unknown major rejected; exact root check. |
| Database schema | Managed by `nexus-store` | Migrated by the core on startup. |

### Wire protocol rules

- Protocol IDs carry the major version, for example `/nexus/chunk/1`.
- A peer that does not support a requested major version returns a typed `unsupported_version` error. It does not silently downgrade.
- Requests and responses are bounded CBOR messages. File bytes are chunk payloads outside CBOR.
- Every request has a deadline. Every response has a maximum size. Every peer has rate and concurrency limits.
- Retries are bounded and use backoff. A timeout is reported as a partial or failed result, never as success.
- There is no negotiation phase that can grant authority. Bootstrap peers and index providers are not identity or ownership authorities.

### Record version rules

- A publication revision is created by the same identity for the same content ID when metadata changes. It references the previous publication ID in `supersedes`.
- A newer revision does not invalidate the old signature; it supersedes it for display and serving preference. Old records may remain cached and are labelled as superseded.
- A tombstone references a publication ID and remains valid independently of the original record.
- A profile revision is ordered by revision number and signature validity. Equal revisions with different bodies are a visible conflict; the app does not pick a winner by wall-clock time alone.
- Timestamps are informational. Ordering uses revision numbers and verified references. Clock skew is displayed honestly.

### Error taxonomy

| Code | Meaning | Retryable |
|---|---|---|
| `invalid_argument` | Malformed request or field. | No |
| `not_found` | Record, content, or provider not found. | Sometimes |
| `conflict` | Conflicting signed state. | No automatic merge |
| `quota_exceeded` | Managed quota or reserve would be exceeded. | After user action |
| `verification_failed` | Signature, hash, manifest, or content mismatch. | No for the same bytes |
| `unsupported_version` | Unknown major version. | No |
| `network_unavailable` | No usable path or peer. | Yes |
| `timeout` | Bounded operation deadline expired. | Yes |
| `cancelled` | User or lifecycle cancellation. | Yes |
| `io_error` | Local storage or file failure. | Sometimes |
| `internal` | Guarded core fault. | With diagnostics |

Error messages are redacted. Error codes are stable; human-readable text may change.

### No central authority

- No domain may require a company-operated account, database, search service, relay, or file host.
- Bootstrap and index providers are optional infrastructure. They cannot change identity, ownership, or signature validity.
- Any future centralized convenience must be optional, replaceable, and documented in a new ADR; it may not become the only path for a P0 journey.

## Consequences

**Positive**

- Clear ownership prevents accidental coupling and duplicate validation.
- Exact signed bytes make replicas and cross-platform verification reliable.
- Independent versions allow app updates without breaking cached records.
- Error and version rules are testable across both platforms.
- The no-authority rule is explicit and reviewable.

**Negative / trade-offs**

- More boundary code and explicit DTOs than a monolithic core.
- Unknown-field preservation adds storage and validation complexity.
- Revision conflicts are visible to users rather than resolved automatically.
- Protocol major versions require coordinated release planning.
- Timestamps cannot be used as a simple global ordering key.

**Constraints**

- No domain bypasses signature verification for convenience.
- No UI code parses network records directly.
- No record is accepted solely because a bootstrap peer, index provider, or popular peer supplied it.
- No protocol change ships without vectors and a compatibility note.

## Alternatives considered

1. **One monolithic protocol module.** Less code up front, but mixes trust boundaries and makes validation inconsistent. Rejected.
2. **Protobuf schemas with a shared registry.** Good typing, but canonical signed bytes and unknown-field preservation are harder across languages. Rejected for signed records; CBOR is retained.
3. **Trust-on-first-use peer records.** Convenient, but lets a malicious first responder pin false metadata. Rejected.
4. **Last-writer-wins by timestamp.** Simple, but clock skew and multi-device edits silently lose data. Rejected.
5. **Central version negotiation service.** Adds an authority and a network dependency for local-first startup. Rejected.

## Verification

- Stage 06/07 commit cross-platform vectors for every signed record and rejection case.
- Stage 10 tests unknown-major rejection, malformed records, and partial discovery results.
- Stage 13 tests concurrent profile revisions and conflict display.
- Stage 14 tests revision and tombstone behaviour, including replicas serving old records.
- Stage 16 tests unknown index protocol versions and partial coverage.
- Stage 19 fuzzes every decoder and checks that no malformed record reaches the UI as valid.
