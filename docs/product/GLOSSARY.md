# Ember Glossary

| Field | Value |
|---|---|
| Document ID | NEXUS-GLOSSARY-001 |
| Version | 1.0.0 |
| Status | Approved — Stage 01 |
| Date | 2026-11-12 |
| Normative source | [Product requirements](../PRODUCT_REQUIREMENTS.md) |

Terms are listed alphabetically. Definitions are normative for product copy, tests, and UI labels. Where a term has a common meaning that would mislead users, the "Not to be confused with" column records the distinction.

---

## A

| Term | Definition | Not to be confused with |
|---|---|---|
| **Acceptance criterion** | A testable pass/fail statement tied to a requirement and an evidence layer. | A goal, a demo, or a subjective impression. |
| **Available / availability** | In a network context, a time-limited observation that at least one source has proved possession of valid content. For the owner, a readable local original is locally usable; network availability additionally requires a serving source. Always shown with freshness. | Record existence, signature validity, local readability, or permanent online status. |
| **Authenticated connection** | A peer connection whose remote peer identity has been cryptographically verified. | An anonymous or unauthenticated transport connection. |
| **Avatar** | An optional public profile image, limited to PNG/JPEG/WebP, ≤2 MiB, ≤1024×1024 px, stored as a separate public content blob referenced by the profile record. | A private device photo, a local file path, or an embedded blob that would exceed the profile record limit. |

## B

| Term | Definition | Not to be confused with |
|---|---|---|
| **Bootstrap peer** | A peer address used to help a new node join the network and find other peers. It is an entry point, not an identity, ownership, or content authority. | A central server, account service, or trusted search provider. |
| **Byte range / chunk** | A bounded portion of a file used for authenticated transfer and verification. | A whole-file guarantee; chunks must be verified and the final file must also be verified. |

## C

| Term | Definition | Not to be confused with |
|---|---|---|
| **Canonical serialisation** | A deterministic encoding of signed fields so that all platforms sign and verify identical bytes. | Any convenient JSON serialisation with unstable key order. |
| **Category** | One of the fixed v1 taxonomy values: Documents, Images, Audio, Video, Software, Data, Archives, Other. | A free-form tag or a file extension. |
| **Content** | The actual bytes identified by a content ID. | The publication record, which is metadata and a signed statement. |
| **Content ID** | A stable identifier derived from SHA-256 over the complete file bytes. Same bytes → same content ID; any byte change → different content ID. | Filename, title, database row ID, or uploader identity. |
| **Content verification** | Confirmation that downloaded or hosted bytes match the content ID and every chunk proof. | Signature verification of a record. |
| **Crash reporting** | Optional, opt-in collection of redacted crash diagnostics. | Default telemetry, analytics, or identity tracking. |

## D

| Term | Definition | Not to be confused with |
|---|---|---|
| **Discoverable / listed** | A signed record was found and can be displayed. It does not imply that content is online or downloadable now. | Available now. |
| **Discovery** | Finding peers, profiles, and publication records through bootstrap, DHT, local cache, and index sources. | Full-text search or a central directory. |
| **Download** | Fetching content from a source into the managed store, with chunk and final verification. | Replication/hosting, which requires an explicit decision to serve the file. |
| **Duplicate content** | The same content ID with one or more publication records, possibly from different identities. The bytes are stored once locally; records remain distinct. | Duplicate publication records or duplicate UI entries. |

## E

| Term | Definition | Not to be confused with |
|---|---|---|
| **Ed25519** | The public-key signature algorithm used for identity and signed records. | Encryption of file content; Ember identity signatures are for authenticity. |
| **Explore** | Browsing categories, recent records, and random records from available verified metadata. | A popularity feed, likes, comments, or follows. |

## F

| Term | Definition | Not to be confused with |
|---|---|---|
| **File page** | The UI that displays a publication's metadata, content ID, signature status, availability, sources, and actions. | A local filesystem property page or a preview that executes content. |
| **First public release (v1)** | The first release meeting the P0 scope and release gates in the PRD. | The complete 100% roadmap, which includes later capabilities. |

## H

| Term | Definition | Not to be confused with |
|---|---|---|
| **Hosting / replicating** | The explicit act of storing a complete verified copy of a publication and optionally serving it to peers. | Downloading for personal use, caching a partial file, or mandatory seeding. |
| **Hosted files** | The user's explicitly selected replicas, with storage and serving status. | All files the user has ever downloaded. |

## I

| Term | Definition | Not to be confused with |
|---|---|---|
| **Identity** | The user's Ed25519 keypair. The public identity ID is derived from the public key. | A username, email address, account, or real-world identity document. |
| **Identity ID** | The documented stable public identifier derived from the public key. | A display name or username label. |
| **Index / index provider** | A source that caches and answers queries over public metadata records. It must not become a hidden central authority. | The DHT itself or a company-owned search database. |

## K

| Term | Definition | Not to be confused with |
|---|---|---|
| **Key loss** | Permanent loss of the private key, and therefore control of the identity, unless the user has an encrypted backup. There is no central reset. | A forgotten username/password that a service can reset. |
| **Key rotation** | Replacing an identity key while preserving a verifiable link. Not implemented in v1; planned later. | Restoring the same identity from a backup. |

## L

| Term | Definition | Not to be confused with |
|---|---|---|
| **Last seen** | The timestamp of the most recent successful observation of a peer or source. | A guarantee that the peer is online now. |
| **Local-first** | The app remains useful for local data and actions without a network. | Offline-only; network features work when connectivity is available. |

## M

| Term | Definition | Not to be confused with |
|---|---|---|
| **Managed file store** | Ember-controlled storage for originals, replicas, completed downloads, and resumable partials, with quotas and generated safe names. | The user's entire disk, arbitrary folders, or SQLite BLOBs. |
| **Modified version** | A distinct file with a different content ID, its own publication record, and an optional parent/source reference. | An edit that preserves the original content ID or original uploader attribution. |
| **Multi-peer download** | Simultaneous chunk scheduling across multiple sources. Later scope; v1 uses one verified source at a time with sequential failover. | Sequential retry from a different source. |

## N

| Term | Definition | Not to be confused with |
|---|---|---|
| **Node** | A running Ember installation participating in the peer network. **Node online** means the local core is running with at least one usable network path; peer count is separate. | A user account, a central server, or a guarantee that peers/files are reachable. |

## O

| Term | Definition | Not to be confused with |
|---|---|---|
| **Offline** | In peer/source context, no successful observation within the last 5 minutes; last-seen time remains visible. In node context, the local core is stopped or has no usable network path. | Deleted, unpublished, or permanently unavailable. |
| **Online** | In peer/source context, an authenticated connection is active or a successful liveness observation occurred within 60 seconds. In node context, the local core is running with at least one usable network path, even if there are zero peers. | Permanently reachable, or a guarantee that a specific file is available. |
| **Original uploader** | The identity that signed the specific publication record being displayed. Attribution is per record. | The replicator, the current source, or a claim about real-world authorship. |

## P

| Term | Definition | Not to be confused with |
|---|---|---|
| **Parent / source reference** | An optional pointer from a modified publication to another content/publication. Informational in v1; it does not prove endorsement. | A signed endorsement by the parent uploader, which v1 does not implement. |
| **Peer** | A running Ember installation that can connect to other installations. | A user account, contact, or follower. |
| **Profile** | A signed public record with display name, optional username label, bio, optional avatar, identity ID, and publication references. | A private account, contact card, or social graph. |
| **Protocol version** | A version number in messages and records that allows compatible evolution and safe rejection of unknown formats. | App marketing version. |
| **Publication record** | A signed statement by an identity that it published a specific content ID with metadata, timestamp, and protocol version. | The file bytes, a local database row without a signature, or a replica's claim. |
| **Publish** | Explicitly create and sign a publication record, then serve the content while online. | Upload to a central server, or silently expose a local folder. |

## R

| Term | Definition | Not to be confused with |
|---|---|---|
| **Record-only** | A known signed record with no currently proven content source. The UI must say the record exists but content is unavailable now. | Deleted or nonexistent. |
| **Relay** | An optional peer that forwards traffic when direct connectivity fails. Later scope; not an authority or file custodian. | A central proxy or mandatory service. |
| **Replica** | A complete, verified, byte-identical copy of a publication. When held by a peer other than the signing uploader, it is an independent replica; a second device of the uploader is still a copy, not a new authorship claim. | A partial download, an unverified cache, or a new publication. |
| **Replication** | The voluntary act of storing and serving a replica. Never mandatory, never automatic, never authorships transfer. | Downloading, caching, or seeding without consent. |
| **Revision / record version** | A monotonically increasing version of a signed record for the same identity/publication. | Content ID; metadata can change without content changing. |

## S

| Term | Definition | Not to be confused with |
|---|---|---|
| **Search** | Querying local and distributed metadata by username, filename, category, or tags. Results may be partial and must say so. | DHT lookup, which finds records/peers by key; or a central search service. |
| **Signed record** | A profile or publication record with a valid signature over canonical fields. | Verified content, or verified real-world identity. |
| **Source** | A peer that claims to serve a content ID and proves possession by serving at least one valid chunk. | Original uploader, replica owner, or a mere DHT record. |
| **Stale** | Cached metadata or an availability observation older than its freshness window. Always labelled; never presented as current. | Invalid signature or deleted content. |
| **Sybil attack** | An attacker creating many peer identities to influence discovery or exhaust resources. Mitigated by validation, diversity, limits, and cautious trust; not fully solved by decentralization. | A single malicious file. |

## T

| Term | Definition | Not to be confused with |
|---|---|---|
| **Tombstone** | A signed unpublish statement marking a publication inactive. It does not delete copies held by others. | Remote deletion, a central takedown, or a guarantee that content is gone. |

## U

| Term | Definition | Not to be confused with |
|---|---|---|
| **Unpublish** | The owner's action to stop serving a publication and sign a tombstone. Copies elsewhere may remain. | Global deletion or revocation of already-distributed bytes. |
| **Username label** | An optional, non-unique human-readable label for discoverability. It is not an identity proof. | A unique account name, verified handle, or identity ID. |

## V

| Term | Definition | Not to be confused with |
|---|---|---|
| **Verified content** | Bytes match the content ID and all chunk proofs. | A valid record signature. |
| **Verified record** | The record signature is valid and signed fields are intact. It does not establish real-world identity, legality, or safety. | Verified content or a safe file. |

## W

| Term | Definition | Not to be confused with |
|---|---|---|
| **Availability observation** | A freshness-limited record of a source successfully serving valid data. | A permanent availability guarantee. |

---

## Canonical UI labels

Use these labels consistently. Do not substitute synonyms that change meaning.

| State | Canonical label | Meaning |
|---|---|---|
| Source observed within 60 s | **Available now** | At least one source is currently reachable and has proved possession. |
| Source observed within 5 min | **Seen recently** | A recent observation exists, but not necessarily live this second. |
| Source observed longer ago | **Last seen {relative time}** | Historical observation only. |
| Record known, no source proof | **Record only — no known source** | The signed record exists; content is not currently available. |
| Cached offline browse | **Offline — cached results** | Results come from local cache and may be stale. |
| Partial distributed search | **Partial results — some sources did not respond** | Search completed with reduced coverage. |
| Signature valid | **Signed record verified** | Cryptographic signature is valid; not a real-world identity claim. |
| Content hash valid | **Content verified** | Bytes match the content ID and chunk proofs. |
| Unpublished by owner | **Unpublished by uploader — copies may still exist** | Tombstone exists; independent copies are not erased. |
| Replica serving | **Hosted by you** / **Available from {n} sources** | Replicator is a source, not the original uploader. |
| Modified file | **Modified version — references {source}** | Distinct content; reference is not an endorsement. |
