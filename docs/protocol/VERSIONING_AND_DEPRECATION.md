# Protocol Versioning and Deprecation Policy

**Status:** Proposed for Stage 20 review
**Applies to:** Ember protocol major version 1 and clients that implement it
**Last updated:** 2026-11-26

This policy defines how the Ember protocol changes, how peers discover versions,
how old clients are supported, and how features are deprecated. It is a public
commitment only after project-owner approval. Until then it describes the rules
the implementation already follows and the process the project intends to use.

## 1. Version vectors

| Component | Current value | Owner | Compatibility rule |
|---|---|---|---|
| Protocol major version | `1` | Protocol | A peer that does not support the requested major returns `unsupported_version` |
| File wire version | `1` | Protocol | Exact match inside `/nexus/file/1` |
| Index wire version | `1` | Protocol | Exact match inside `/nexus/index/1` |
| Kademlia protocol ID | `/nexus/kad/1.0.0` | Network | Major change uses a new protocol ID |
| Profile schema | `1` | Protocol | Unknown minor fields preserved; unknown major rejected |
| Publication schema | `1` | Protocol | Same rule |
| Tombstone schema | `1` | Protocol | Same rule |
| Manifest schema | `1` | Protocol | Same rule; root is signed by the publication |
| FFI ABI | `1` | Platform | Exact match between bundled library and app |
| App version | Semantic version | Release | Shown in diagnostics; not a wire-version authority |
| Database schema | Managed by `nexus-store` | Storage | Migrated on startup; not a peer-visible version |

Wire protocol IDs carry the major version. A record's schema field carries the
major schema version. Adding an optional field is a minor change; changing the
meaning of a field, a domain tag, a key derivation, or a signature input is a
major change.

## 2. Version discovery

Peers and clients can discover versions without a central service:

- Identify advertises the peer's agent and protocol information.
- `system.info` exposes the native ABI version and the build's protocol version.
- A request for an unsupported wire major returns a typed `unsupported_version`
  error or is rejected before a response is produced.
- A record with an unknown major schema version is rejected and never stored as
  valid.
- A record with unknown minor fields is accepted, those fields are preserved,
  and the client does not interpret them.
- There is no negotiation phase that can grant authority or silently downgrade a
  major version.

## 3. Change classes

### 3.1 Editorial

Typos, clarifications, and examples that do not change bytes, limits, or
behavior. They require documentation review but no protocol version change.

### 3.2 Minor

A compatible change such as a new optional field, a new optional error code, or a
new optional capability that old clients can ignore. Requirements:

- Old clients continue to verify and serve old records.
- Unknown fields are preserved.
- New fields do not change the meaning of any existing field.
- Test vectors cover both old and new readers.
- The change is documented in the release notes and protocol changelog.

### 3.3 Major

An incompatible change such as a new signature domain, a changed canonical
encoding, a changed DHT key derivation, a removed required field, or a new
mandatory wire version. Requirements:

- A new protocol ID or schema major version.
- A migration note and an overlap plan.
- New test vectors and rejection cases.
- An ADR or an amendment to ADR-0007.
- Explicit project-owner approval.

### 3.4 Security emergency

A security fix may shorten or bypass the normal deprecation window. The project
must still publish an advisory, the affected versions, the mitigation, and the
compatibility impact as soon as it is safe to do so. Emergency changes still
require new vectors and a review by a second maintainer when one is available.

## 4. Deprecation process

A feature or version is deprecated only when the project can state:

1. What is deprecated and which versions are affected.
2. Why it is being removed and what replaces it.
3. The last version that supports it.
4. The earliest version that may remove it.
5. The migration path for records, configuration, and clients.
6. The expected user impact and the rollback path.

Proposed minimum windows, subject to project-owner approval:

| Change | Minimum notice before removal |
|---|---|
| Optional field or capability | One minor release |
| Wire protocol major | 12 months after a replacement is published |
| Record schema major | 12 months after a replacement is published |
| Security-driven removal | As short as the risk requires, with an advisory |

The project may extend a window. It may shorten one only through the security
emergency process or an explicit owner decision recorded in the release notes.
A deprecated feature remains testable until its removal release.

## 5. Support policy

While Ember is in the `0.1.x` development line:

- Only the latest published `0.1.x` build is supported.
- Older development snapshots are not supported.
- Records created by an older `0.1.x` build remain readable when their schema
  major version is supported.
- No production support window is claimed until the first signed public release.

After a public release, the project will publish a support table with at least
the current release and the previous minor release, plus the security-fix
window. That table is a release artifact and is reviewed with each release.

## 6. Migration rules

### App and database

- The core migrates its local database on startup.
- A failed migration leaves the previous database recoverable; it does not
  silently discard rows.
- Downgrading the app may make a newer database unreadable. The update guide
  must say so and the app must not claim that downgrade is always safe.
- Export, backup, and restore formats are versioned separately from the database.

### Signed records

- A replica re-serves exact stored bytes; it does not migrate a signed body.
- A client that understands a newer minor schema preserves unknown fields but
  does not claim to understand them.
- A client that does not understand a record's major schema rejects it and tells
  the user that the record is unsupported. It does not render a partial record
  as valid.

### Peers and infrastructure

- Bootstrap and relay operators may upgrade independently.
- A peer that cannot speak a requested major is not treated as malicious; the
  caller reports `unsupported_version` honestly.
- The network must remain usable while old and new clients overlap. If an
  overlap is impossible, the release is a coordinated major transition with a
  published cutover date.

## 7. Deprecation registry

| Feature | Deprecated in | Removal not before | Replacement | Status |
|---|---|---|---|---|
| None | - | - | - | - |

This table is intentionally empty for protocol v1. Entries are added when a
deprecation is approved, not when it is merely discussed.

## 8. Review checklist

Before a protocol change ships:

- [ ] The change class is identified.
- [ ] Existing vectors still pass.
- [ ] New vectors cover acceptance and rejection.
- [ ] Unknown-field and unknown-major behavior is tested.
- [ ] The public specification and this policy are updated.
- [ ] Release notes state the compatibility impact.
- [ ] The migration and rollback path is documented.
- [ ] A security review is complete for signature, key, or parsing changes.
- [ ] Project-owner approval is recorded for a major change.

## 9. References

- [Protocol specification](PROTOCOL_SPEC.md)
- [Interoperability and conformance](INTEROPERABILITY.md)
- [ADR-0007: Protocol boundaries and versioning](../decisions/ADR-0007-protocol-boundaries-versioning.md)
- [Update and rollback guide](../release/UPDATE_AND_ROLLBACK.md)
