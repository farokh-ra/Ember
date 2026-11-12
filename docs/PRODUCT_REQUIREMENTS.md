# NEXUS — Product Requirements Document

| Field | Value |
|---|---|
| Document ID | NEXUS-PRD-001 |
| Version | 1.0.0 |
| Status | Approved — Stage 01 complete |
| Date | 2026-11-12 |
| Owner | NEXUS product and engineering |
| Scope | First public release (v1) and product-wide rules |
| Normative language | **MUST**, **MUST NOT**, **SHOULD**, **MAY** as defined in RFC 2119 |

This document turns the NEXUS roadmap vision into a formal, testable product definition. It is the authority for product behaviour. Technical choices that do not change product behaviour are recorded in the architecture decision records produced in Stage 02.

Related documents:

- [Glossary](product/GLOSSARY.md)
- [User flows](product/USER_FLOWS.md)
- [Feature priority matrix](product/FEATURE_PRIORITY_MATRIX.md)
- [Acceptance criteria](product/ACCEPTANCE_CRITERIA.md)
- [Stage 01 record](progress/STAGE_01.md)

---

## 1. Purpose

NEXUS is a decentralized application for discovering people and publicly shared files, publishing files from a user's own device, and downloading files directly from other participating devices.

There is no company-owned central server, central cloud storage, or mandatory cloud account. Devices participate as peers. Users may voluntarily keep verified copies of public files so those files remain available to others.

The first release is a real Android and Windows product, not a prototype, web wrapper, or two-device demo. Early releases may have few peers, but every architectural and product decision must remain valid for a global network.

---

## 2. Product definition

### 2.1 Core experience

A user can:

1. Create and control a NEXUS cryptographic identity.
2. Create one public profile containing their published files.
3. Browse other users' public profiles.
4. Publish public files with a description, category, and tags.
5. Explore categories and discover recent or random public files.
6. Search for users and files.
7. Open a file page showing details, signature status, and current availability.
8. Download a file directly from an available source peer.
9. Resume an interrupted download.
10. Voluntarily host selected public files for the network.
11. See which files they currently host.
12. Preserve original-uploader attribution while hosting a replica.
13. Publish a modified file as a distinct file with an optional source reference.
14. Use the same NEXUS network from Android and Windows.

### 2.2 Product principles

1. **Local-first.** Local identity, profile drafts, publication records, downloads, settings, and replication choices remain usable without a network.
2. **No central dependency.** No company-operated backend is required for identity, profiles, discovery, storage, or downloads.
3. **Explicit user control.** Publishing, replication, storage limits, bandwidth limits, and background activity are user-controlled.
4. **Verify before trust.** Filenames, peer claims, and metadata are not proof of content authenticity.
5. **Signed authorship.** A valid signature proves control of a key at signing time, not a real-world identity or legal authorship.
6. **Content-addressed files.** File identity is derived from content bytes, not from filename or uploader.
7. **Resilient transfers.** Downloads support chunk verification, retry, pause, resume, and safe completion.
8. **Honest availability.** A discoverable record does not imply that content is online or downloadable now.
9. **Versioned protocols.** Network messages and signed records are versioned and validated.
10. **Observable performance.** Battery, memory, CPU, bandwidth, startup, and transfer reliability are measured on real devices.
11. **Privacy-aware metadata.** Public content is public, but the app minimises unnecessary device, path, and location exposure.
12. **Secure defaults.** The app exposes only explicitly published or explicitly replicated files.

---

## 3. Non-negotiable product rules

These rules override any conflicting feature request. A change requires an explicit product decision and a roadmap amendment.

| Rule | Statement |
|---|---|
| PR-01 | All published files and profiles are public. NEXUS has no private or secret file-sharing mode. |
| PR-02 | NEXUS must not require a company-owned central server, central database, central file host, or mandatory cloud account. |
| PR-03 | Replication is never mandatory or automatic. The user chooses every hosted file. |
| PR-04 | Replication does not transfer authorship. A replica always serves the original signed publication record. |
| PR-05 | Original publication records are signed by the publishing identity. Originator attribution is verifiable, not editable text. |
| PR-06 | A modified file receives its own content identity and its own signed publication record. It may reference a source file but must not impersonate it. |
| PR-07 | Downloaded content is verified against its published content identity. Corrupt content is never presented as complete or valid. |
| PR-08 | There are no likes, comments, follows, or social graph features. Discovery is by profile, file record, category, tags, and search. |
| PR-09 | Android and Windows are first-class platforms. Neither is a reduced web wrapper. Core journeys have platform parity. |
| PR-10 | NEXUS must not promise remote deletion. Unpublishing stops the user's service and publishes a tombstone; copies held by others may remain. |
| PR-11 | NEXUS never automatically executes or opens downloaded files. Previews are limited to safe formats and safe platform handlers. |
| PR-12 | The architecture is designed for a global network. Two-device convenience must not become a protocol limitation. |

---

## 4. Scope

### 4.1 In scope for the first public release (v1)

- Android application and Windows desktop application.
- Local cryptographic identity creation, encrypted backup, and restore.
- Signed public profile records.
- Signed public publication records.
- Local SQLite metadata and a managed file store.
- Peer identity, encrypted authenticated connections, and basic peer discovery.
- Configurable bootstrap peers and manual peer entry.
- Direct file publishing and serving while online.
- Direct downloading with per-chunk verification and final verification.
- Pause, resume, retry, cancel, and restart recovery.
- Explicit voluntary replication and a hosted-files screen.
- Profile pages, file pages, category browsing, recent files, and basic search.
- Clear record, signature, online, offline, last-seen, and availability status.
- Safe error handling, diagnostics, and redacted logs.
- Accessible, localisation-ready UI in English.
- Signed release artifacts with published checksums.

### 4.2 Explicitly out of scope

These are not v1 features and must not be implied by the UI:

- Private, secret, or invitation-only file sharing.
- Likes, comments, follows, follower counts, or social feeds.
- Company-owned accounts, password resets, cloud storage, or mandatory services.
- Centralised search, centralised content moderation, or a company-operated file host.
- Mandatory replication or background seeding the user did not select.
- Remote deletion or guaranteed erasure from other peers.
- Automatic execution, automatic opening, or silent installation of downloaded files.
- Real-world identity verification, KYC, or claims that a key belongs to a named person.
- Guaranteed anonymity. Peers can observe network addresses of peers they connect to.
- Guaranteed availability. No source online means no download at that moment.
- Built-in copyright enforcement, DRM, or a central takedown authority.
- Cryptocurrency tokens, payments, or financial incentives.

### 4.3 Later scope

The following capabilities are planned but are not release-blocking for v1. They are tracked in the feature priority matrix.

- Multi-peer chunk downloads, source scoring, and cross-peer resume.
- Relay fallback, advanced NAT traversal, and difficult-network diagnostics.
- Larger-scale distributed indexing and richer Explore ranking.
- Additional languages and full RTL localisation.
- Battery-aware and metered-network controls.
- Local blocking, community reporting, and stronger abuse resistance.
- Key rotation, multiple local identities, and richer device management.
- Windows ARM64 builds and additional packaging formats.
- Independent client interoperability and governance tooling.

---

## 5. Target platforms and environments

### 5.1 Platform support matrix

| Target | Minimum | Recommended | Architecture | Notes |
|---|---|---|---|---|
| Android | Android 9 (API 28) | Android 12 or newer | `arm64-v8a` required; `x86_64` for emulator testing | Target the latest stable Android API required by Google Play at release. Phones are the baseline; tablets use responsive layouts. |
| Windows | Windows 10 22H2 (build 19045) | Windows 11 23H2 or newer | x64 | Windows 10 is best-effort after vendor end of support; Windows 11 is the supported baseline. No 32-bit build. ARM64 is later scope. |

### 5.2 Baseline hardware

| Platform | Baseline used for acceptance and performance tests |
|---|---|
| Android | 4-core device, 2 GB RAM, 360 dp wide, Android 9+, at least 2 GB free storage for testing |
| Windows | 2-core x64 machine, 4 GB RAM, 1280×720 display, at least 2 GB free storage for testing |

### 5.3 Platform parity

Both platforms MUST provide every P0 journey in this document. Platform-specific presentation is allowed for file pickers, notifications, sharing, background execution, and keyboard behaviour. A platform MUST NOT omit identity, profile, publishing, discovery, search, download, replication, settings, or diagnostics.

### 5.4 Network environments

- IPv4 and IPv6 must be supported where the platform and network provide them.
- The app must remain usable offline and must report network state honestly.
- Direct connectivity is not guaranteed on every network. v1 must handle failure gracefully; relay fallback is later scope.
- Local-network connection without internet access must be possible through manual peer entry and/or local discovery where the platform permits.

---

## 6. Product vocabulary and semantic invariants

The full definitions are in the [Glossary](product/GLOSSARY.md). The following terms are normative for requirements and tests.

| Term | Normative meaning |
|---|---|
| Peer / node | A running NEXUS installation participating in the network. |
| Identity | An Ed25519 keypair controlled by the user. The public identity ID is derived from the public key. |
| Profile | A signed public record containing a display name, optional username label, bio, optional avatar, and references to the identity's publications. |
| Publication record | A signed statement by an identity that it published a specific content hash with specific metadata, timestamp, and protocol version. |
| Content ID | A stable identifier derived from the SHA-256 hash of the complete file bytes. |
| Source | A peer that claims to serve a content ID and can prove possession by serving at least one valid chunk. |
| Replica | A complete, verified byte-identical copy of a publication. When held by a peer other than the signing uploader, it is an independent replica; a second device of the uploader is still a copy, not a new authorship claim. |
| Original uploader | The identity that signed the specific publication record being displayed. Attribution is per publication record, not a global claim about the content. |
| Modified version | A distinct file with a different content ID, its own publication record, and an optional reference to a parent content/publication. |
| Available | In a network context, at least one source has recently proved possession of valid content. For the owner, a readable local original is locally usable; "network available" additionally requires a serving source. Availability is time-limited and must be shown with freshness. |
| Peer online | An authenticated connection to that peer is active, or a successful liveness observation occurred within 60 seconds. |
| Peer offline | No successful observation of that peer within the last 5 minutes. The last-seen time is still shown. |
| Node online | The local core is running and has at least one usable network path. Peer count is shown separately; zero peers does not imply file availability. |
| Node offline | The local core is stopped or no usable network path exists. Local data remains usable. |
| Last seen | The timestamp of the most recent successful observation of a peer or source. |
| Verified record | The record's signature is valid and its signed fields are intact. This does not prove real-world identity. |
| Verified content | Downloaded bytes match the published content ID and every chunk proof. |
| Listed / discoverable | A signed record was found. This does not imply that content is online now. |
| Stale | Cached metadata or an availability observation older than its freshness window. Stale data is labelled and never presented as current. |
| Tombstone | A signed unpublish statement marking a publication inactive. It does not delete copies held by others. |
| Explore | Browsing categories, recent records, and random records. It is not a popularity feed. |
| Search | Querying metadata by username, filename, category, or tags through local and distributed sources. Results may be partial. |
| Bootstrap peer | A peer address used to join the network. It is an entry point, not an authority for identity or ownership. |
| Relay | An optional peer that forwards traffic when direct connectivity fails. Relay support is later scope. |
| Hosting / replicating | The explicit act of storing and serving a verified complete copy of a publication. |

### 6.1 Semantic invariants

These distinctions are release-blocking. The UI and API must never collapse them.

| ID | Invariant |
|---|---|
| SI-01 | A publication record can exist while no source is online. The UI must say "record exists, unavailable now", never "deleted" or "offline file exists". |
| SI-02 | A valid signature proves key control, not real-world identity, authorship, legality, or safety. |
| SI-03 | Record verification and content verification are separate states and must be labelled separately. |
| SI-04 | Replication preserves attribution and never makes the replicator the original uploader. |
| SI-05 | A parent/source reference on a modified file is informational. It does not prove endorsement by the parent uploader unless a separate signed endorsement exists. v1 has no endorsement record. |
| SI-06 | Publishing and unpublishing are not global delete operations. Copies held by independent peers may persist. |
| SI-07 | Downloading a file does not make the downloader a host. Hosting requires an explicit replication action. |
| SI-08 | Online status is not permanent. It is an observation with a timestamp and freshness window. |

---

## 7. Users and jobs

NEXUS has no account roles. One person may act in all roles at different times.

| Role | Job to be done |
|---|---|
| Publisher | Create an identity, set up a public profile, publish files, manage publication status, and know what is public. |
| Explorer | Find profiles and files by category, recency, randomness, or search; judge signature and availability honestly. |
| Downloader | Obtain a verified file, survive interruption, and avoid unsafe or corrupt content. |
| Replicator | Voluntarily keep selected public files available and see the storage and bandwidth impact. |
| Community operator | Run optional bootstrap infrastructure and understand that it is not an identity or ownership authority. |

---

## 8. User journeys

Detailed flows, alternate paths, and error handling are in [User flows](product/USER_FLOWS.md).

| Journey | Goal | Primary acceptance criteria |
|---|---|---|
| J-01 First launch | Understand the product, public visibility, key responsibility, and network limits before creating an identity. | ONB-001–ONB-004 |
| J-02 Identity creation and backup | Create a local identity and leave with a usable encrypted backup. | ID-001–ID-008 |
| J-03 Profile setup | Publish a signed public profile and understand username collisions. | PROF-001–PROF-007 |
| J-04 Publish a file | Select a file, add metadata, accept public visibility, sign a record, and serve while online. | PUB-001–PUB-013 |
| J-05 Explore | Browse categories, recent records, and random records with honest freshness. | DISC-005, DISC-006, UX-004 |
| J-06 Search | Find profiles and files across available sources and understand partial results. | SEARCH-001–SEARCH-006 |
| J-07 Download | Download, verify, pause, resume, and safely complete a file. | DL-001–DL-014 |
| J-08 Replicate | Explicitly host a verified file and manage the hosted copy. | REP-001–REP-008 |
| J-09 Unpublish | Stop serving a publication, publish a tombstone, and understand residual copies. | PUB-008, UX-007 |
| J-10 Network status and recovery | Understand connectivity, configure bootstrap peers, and recover from failures. | NET-003–NET-006, BOOT-001–BOOT-004 |

---

## 9. First-release functional requirements (P0)

All requirements in this section are required for v1. Each requirement references one or more testable acceptance criteria in [Acceptance criteria](product/ACCEPTANCE_CRITERIA.md). Requirements marked **MUST** are release-blocking.

### 9.1 Onboarding

| ID | Requirement | Acceptance |
|---|---|---|
| FR-ONB-01 | Before identity creation, the app MUST explain that published files and profiles are public, that there is no central account or password reset, that content is only downloadable while a source is online, and that losing the identity key loses the identity. The user MUST explicitly acknowledge this. | ONB-001 |
| FR-ONB-02 | The user MUST be able to create a new identity or restore an existing encrypted backup. No email, phone number, account, or central registration is required. | ONB-002 |
| FR-ONB-03 | After onboarding, the app MUST remain usable offline for local profile editing, viewing cached content, managing downloads, and changing settings. Network-dependent screens must show an offline state. | ONB-003, UX-003 |
| FR-ONB-04 | Permissions MUST be requested contextually and optionally where the platform allows. Denying notifications or optional permissions MUST NOT block browsing, identity, profile, publishing, or settings. | ONB-004 |
| FR-ONB-05 | Mock or sample data used in prototypes MUST be visibly labelled. A screen MUST NOT imply that networking, publishing, or discovery works when it is mocked. | ONB-004, UX-008 |

### 9.2 Identity and cryptography

| ID | Requirement | Acceptance |
|---|---|---|
| FR-ID-01 | The app MUST generate an Ed25519 keypair locally and derive a documented, stable public identity ID from the public key. Identity creation MUST work without a network. | ID-001, ID-007 |
| FR-ID-02 | The app MUST sign and verify profile and publication records. Any change to a signed field MUST cause verification to fail. | ID-002 |
| FR-ID-03 | The app MUST provide a user-initiated encrypted backup and restore format with a versioned header, integrity protection, and wrong-passphrase failure. There is no central recovery. | ID-004, ID-006 |
| FR-ID-04 | The UI MUST explain that key loss means identity loss and that no password reset exists. It MUST NOT offer a fake recovery path. | ID-005 |
| FR-ID-05 | Private key material MUST NOT appear in logs, diagnostics, analytics, crash reports, ordinary database queries, or unencrypted exports. Keys MUST be encrypted at rest and protected with platform-appropriate storage where available. | ID-003, SEC-004 |
| FR-ID-06 | The UI MUST distinguish cryptographic identity from real-world identity. It MUST NOT claim that a signature proves a person's name, legal authorship, or content legality. | ID-008 |
| FR-ID-07 | Restoring the same backup on Android and Windows MUST produce the same identity ID and valid signatures. Concurrent records from multiple devices MUST be surfaced as conflicts rather than silently merged. | ID-007, PLAT-003 |
| FR-ID-08 | v1 MUST disclose that automatic key rotation and central revocation are not available. Compromise recovery requires creating a new identity and republishing; old signed records remain valid under the old key. | ID-008, SEC-006 |

### 9.3 Profiles

| ID | Requirement | Acceptance |
|---|---|---|
| FR-PROF-01 | A profile MUST contain a display name (1–64 characters), an optional username label (3–32 characters), a bio (0–500 characters), an optional avatar, the public identity ID, and references to published records. It MUST NOT contain local paths, precise location, device serial numbers, or private contact details by default. | PROF-001, PROF-005 |
| FR-PROF-02 | Profiles MUST be signed, versioned, timestamped, and published through the decentralized record system. A newer valid version supersedes an older version; older cached versions MUST NOT be shown as current. | PROF-002, PROF-003 |
| FR-PROF-03 | Username labels are discoverability labels, not unique reservations. Two identities MAY use the same username. The UI MUST show the identity ID and MUST NOT imply that a username proves identity. | PROF-004 |
| FR-PROF-04 | Conflicting records with the same version but different signed content MUST be shown as a conflict and MUST NOT be silently resolved. Records with invalid signatures MUST be rejected. | PROF-007 |
| FR-PROF-05 | A profile page MUST show the display name, username label, bio, avatar, identity ID, signature status, last update, and the identity's published files with record and availability status separated. | PROF-005 |
| FR-PROF-06 | Avatar upload MUST accept PNG, JPEG, or WebP up to 2 MiB and 1024×1024 pixels. Larger or unsupported images MUST be rejected with a clear error. The avatar MUST be stored as a separate public content blob referenced by the profile record, not embedded in the 64 KiB profile record. | PROF-006 |
| FR-PROF-07 | Profile edits MUST be savable locally while offline and publishable later. An unpublished draft MUST NOT be advertised as public. | PROF-001, PROF-002 |

### 9.4 Local data and file store

| ID | Requirement | Acceptance |
|---|---|---|
| FR-DATA-01 | The app MUST persist identity references, profiles, publication records, file metadata, downloads, replicas, peer observations, and settings in SQLite with a documented schema and migration history. | DATA-001 |
| FR-DATA-02 | Migrations MUST work from a clean database and from every previously shipped schema version. Failed migrations MUST roll back and report an actionable error without losing existing data. | DATA-002 |
| FR-DATA-03 | File bytes MUST be stored in a managed file store, not as large SQLite BLOBs. Stored names MUST be generated safely from content identity; user filenames are display metadata only. Publishing MUST import a managed copy; NEXUS MUST NOT serve arbitrary external paths, and the external original MUST NOT be modified or deleted. | DATA-003, DATA-004 |
| FR-DATA-04 | The managed store MUST enforce the storage quota, minimum free-space reserve, and file-size limits from section 10. Partial downloads MUST count toward the managed quota. | DATA-005 |
| FR-DATA-05 | Cleanup and deletion MUST be reference-counted: bytes still referenced by a published original or active replica MUST NOT be deleted when a download record is removed. User-published originals and active replicas MUST NOT be deleted without explicit user confirmation. Incomplete downloads may be cleaned only according to a user-visible retention policy. | DATA-006 |
| FR-DATA-06 | The app MUST provide a metadata backup/export and restore path for recoverable local data. Corrupt or incompatible archives MUST be rejected without overwriting current data. | DATA-007 |
| FR-DATA-07 | Data MUST survive normal restarts and app updates. Unsafe filenames, reserved Windows names, control characters, and filename collisions MUST be handled without escaping the store root. | DATA-001, DATA-004, DATA-008 |
| FR-DATA-08 | Downloads and replicas MUST live in the managed store until the user explicitly exports a copy. Exporting a copy MUST use a platform file picker or share mechanism and MUST NOT overwrite an existing file silently. | DATA-005, DL-012 |

### 9.5 File identity and integrity

| ID | Requirement | Acceptance |
|---|---|---|
| FR-HASH-01 | The content ID MUST be derived from SHA-256 over the complete file bytes. The same bytes MUST produce the same content ID on Android and Windows; a one-byte change MUST produce a different ID. | HASH-001 |
| FR-HASH-02 | Large files MUST be divided into authenticated chunks with per-chunk hashes and a documented manifest or Merkle structure. A corrupted or missing chunk MUST fail verification. | HASH-002 |
| FR-HASH-03 | Hashing, serving, and verifying MUST stream data and MUST NOT load the whole file into memory. | HASH-003 |
| FR-HASH-04 | Cross-platform test vectors for content IDs, chunk proofs, canonical serialisation, and signatures MUST be committed and MUST pass on both platforms. | HASH-004, PLAT-004 |
| FR-HASH-05 | A download or replica MUST be fully verified before it is marked complete, advertised, or served as valid. Failed verification MUST NOT present the file as usable. | HASH-005 |

### 9.6 Publication records

| ID | Requirement | Acceptance |
|---|---|---|
| FR-PUB-01 | A publication record MUST contain a protocol version, record version, content ID, size, display filename, description, category, tags, uploader identity ID, publication timestamp, authenticated chunk-manifest root/reference, optional parent reference, and signature. Signed fields MUST use canonical serialisation. The record MUST stay within the 64 KiB record limit and MUST NOT embed a large per-chunk list. | PUB-001, PUB-002 |
| FR-PUB-02 | Any change to a signed publication field MUST fail signature verification. Unknown record versions or algorithms MUST be rejected or quarantined, not guessed. | PUB-002 |
| FR-PUB-03 | Publishing MUST require explicit file selection, metadata entry, a public-visibility warning, and a confirmation. The app MUST NOT publish a file silently or by default. | PUB-003 |
| FR-PUB-04 | Publishing MUST enforce the file-size, description, tag, filename, and category limits in section 10. A file above 4 GiB MUST be rejected before hashing begins with a clear explanation. | PUB-004 |
| FR-PUB-05 | A publication created while offline MUST be stored locally as a signed record and clearly marked "not yet discoverable". It MUST be advertised when the node is online and publishing is enabled. | PUB-005 |
| FR-PUB-06 | A file page MUST show the display filename, content ID, size, description, category, tags, uploader identity, signature status, publication time, record version, availability, last-seen time, and known sources. | PUB-006 |
| FR-PUB-07 | The UI MUST separate "record exists" from "content is available now". If no source is online, it MUST say the record is known but unavailable now, not that the file was deleted. | PUB-007 |
| FR-PUB-08 | The owner MUST be able to unpublish: stop serving, sign a tombstone, and see a warning that copies held by others may remain. The UI MUST NOT promise global deletion. | PUB-008 |
| FR-PUB-09 | A modified file MUST have a different content ID and its own signed publication record. It MAY reference a parent content/publication. The UI MUST state that the reference is not an endorsement. | PUB-009 |
| FR-PUB-10 | Multiple identities MAY publish the same content ID as separate publication records. Local storage MUST deduplicate the bytes, while the UI MUST keep publication attributions distinct. | PUB-010 |
| FR-PUB-11 | A replica MUST serve the original signed publication record unchanged. A replica MUST NOT be able to alter the uploader, signature, content ID, or signed metadata. | PUB-011 |
| FR-PUB-12 | Previews MUST be limited to safe formats. Downloaded files MUST NOT be executed or opened automatically. Unsupported formats MUST show metadata and an explicit external-open action with a warning. | PUB-012 |
| FR-PUB-13 | A tombstoned publication that still has independent sources MUST be labelled "unpublished by uploader; copies may still exist" wherever it is downloaded or displayed. | PUB-013 |

### 9.7 Peer networking and connections

| ID | Requirement | Acceptance |
|---|---|---|
| FR-NET-01 | The app MUST use libp2p peer identities and encrypted, authenticated connections. Unauthenticated or identity-mismatched peers MUST be rejected. | NET-001 |
| FR-NET-02 | Connection establishment MUST have bounded timeouts, retry/backoff, and a visible failure state. Failures MUST NOT crash the app or block the UI. | NET-003 |
| FR-NET-03 | The app MUST expose a connection-state model (stopped, starting, online, degraded, offline) and network diagnostics without exposing private keys, passphrases, or local paths. | NET-005 |
| FR-NET-04 | The app MUST handle network changes, sleep/wake, and process restart without corruption. Active transfers MUST pause and be resumable. | NET-004, DL-004 |
| FR-NET-05 | Long-running network operations MUST NOT block the UI thread and MUST support cancellation. | NET-006 |
| FR-NET-06 | Repeated node start/stop cycles MUST not leak unbounded memory, crash, or leave orphaned connections. | NET-006 |
| FR-NET-07 | The network status shown to the user MUST match the actual node state; the app MUST NOT show "online" when the core is stopped or unreachable. | NET-005 |
| FR-NET-08 | Two devices MUST be able to connect and exchange a small authenticated test message. A local-network connection MUST be possible without internet access through manual peer entry and/or local discovery where supported. | NET-001, NET-002 |
| FR-NET-09 | Peer disconnects and malformed peer behaviour MUST be handled with bounded errors, retries, and connection/request limits. | NET-007 |
| FR-NET-10 | The app MUST enforce connection, request, message-size, and rate limits to resist resource exhaustion. | NET-007, SEC-003 |

### 9.8 Bootstrap and decentralized discovery

| ID | Requirement | Acceptance |
|---|---|---|
| FR-BOOT-01 | A fresh installation MUST be able to join through at least one configured bootstrap path and discover another peer without a central account service. | BOOT-001 |
| FR-BOOT-02 | The user MUST be able to add, remove, inspect, and persist multiple bootstrap addresses and manually entered peer addresses. Invalid addresses MUST be rejected with a clear error. | BOOT-002, BOOT-004 |
| FR-BOOT-03 | The loss of one bootstrap peer MUST NOT automatically disable the network or discard other configured paths. The app MUST retry and use remaining paths. | BOOT-003 |
| FR-BOOT-04 | The default configuration MUST NOT depend on a single company-owned endpoint. Public-release defaults MUST offer multiple independently operated options or a documented local/manual path. | BOOT-004 |
| FR-BOOT-05 | Bootstrap nodes MUST be documented as entry points, not identity, ownership, or content authorities. Community operation instructions MUST be published for release. | REL-006 |
| FR-DISC-01 | The app MUST use the chosen DHT design to publish and look up peer and publication records. Lookups MUST have timeouts and bounded retries. | DISC-001 |
| FR-DISC-02 | Discovery MUST report partial, empty, or timed-out results honestly. It MUST NOT hang indefinitely or fabricate results. | DISC-002 |
| FR-DISC-03 | Malformed, oversized, invalid-signature, or unknown-version records MUST be rejected or quarantined without affecting valid results or crashing the app. | DISC-003 |
| FR-DISC-04 | Peer churn (join, leave, return) MUST be handled; stale peers MUST be pruned and valid cached records retained with freshness labels. | DISC-004 |
| FR-DISC-05 | Discovery MUST NOT require a central account, database, or search service. | DISC-004 |
| FR-DISC-06 | Explore MUST provide categories, recent records, and random discovery from discovered verified records. It MUST NOT use likes, comments, follows, or popularity, and MUST NOT imply that discovered content is currently available. | DISC-005 |
| FR-DISC-07 | Explore and file lists MUST show freshness/coverage status and MUST label cached offline results. Loss of one index or peer MUST NOT prevent remaining sources from being queried. | DISC-006 |
| FR-DISC-08 | Duplicate records MUST be deduplicated by record identity/content while conflicting metadata MUST remain visible as a conflict. | DISC-006 |

### 9.9 Search

| ID | Requirement | Acceptance |
|---|---|---|
| FR-SEARCH-01 | Search MUST support username/display name, filename, category, and tags. Search input MUST be bounded to 128 characters. | SEARCH-001 |
| FR-SEARCH-02 | Search MUST query local indexed metadata and available distributed sources. It MUST NOT rely on a hidden company-owned search database. | SEARCH-002 |
| FR-SEARCH-03 | Results MUST be checked against signed records where available. Invalid or unverifiable records MUST be excluded or clearly marked; malicious metadata MUST be rendered safely. | SEARCH-003 |
| FR-SEARCH-04 | Search MUST show partial results after a bounded timeout (target 10 seconds), indicate that results may be incomplete, and support pagination without duplicates. | SEARCH-004 |
| FR-SEARCH-05 | Search MUST support sorting/filtering by relevance, recency, and observed availability, and MUST deduplicate results while surfacing conflicts. | SEARCH-005 |
| FR-SEARCH-06 | Empty, offline, stale, and error states MUST be distinguishable and actionable. Cached results MUST show their last refresh time. | SEARCH-006 |
| FR-SEARCH-07 | Search MUST continue when some index providers or peers disappear, and MUST label reduced coverage. | SEARCH-002 |
| FR-SEARCH-08 | Oversized, malformed, or abusive queries MUST be rejected or rate-limited without crashing or blocking the UI. | SEARCH-006 |

### 9.10 Downloads

| ID | Requirement | Acceptance |
|---|---|---|
| FR-DL-01 | v1 MUST implement a request/response protocol for metadata and chunks and download from one verified source at a time. Sequential failover to another source is allowed; simultaneous multi-source scheduling is later scope. | DL-001 |
| FR-DL-02 | Every received chunk MUST be verified against the authenticated chunk structure before it is accepted. Invalid chunks MUST be rejected and retried or reported. | DL-002 |
| FR-DL-03 | Downloads MUST support pause, resume, retry, and cancel. Pause/resume MUST continue from the last verified offset, not restart from zero. | DL-003 |
| FR-DL-04 | Download state MUST survive app restart. After process death, a resumable download MUST be restorable with its verified progress intact. | DL-004 |
| FR-DL-05 | Cancel MUST ask whether to keep a resumable partial or delete it, unless the user has set a remembered preference. Automatic cleanup MUST follow a user-visible retention policy. | DL-005 |
| FR-DL-06 | A source disconnect mid-transfer MUST pause or fail over without corrupting the partial file. The UI MUST show what happened and offer retry. | DL-006 |
| FR-DL-07 | The app MUST check quota and free space before starting and during transfer. Insufficient space MUST stop safely with a clear error and intact existing data. | DL-007 |
| FR-DL-08 | Filenames and paths from peers MUST never escape the managed store. Unsafe names MUST be sanitised for display/export, and existing files MUST NOT be overwritten silently. | DL-008 |
| FR-DL-09 | Transfer memory MUST remain bounded for the maximum supported file. Downloads MUST stream chunks and final hashing. | DL-009 |
| FR-DL-10 | The download UI MUST show bytes transferred, total size when known, speed, source peer, state, and actionable errors. Updates MUST NOT freeze the UI. | DL-010 |
| FR-DL-11 | On completion, the app MUST verify the complete content ID before marking the file complete. A mismatch MUST keep the file unusable, display an error, and offer retry/delete. | DL-011 |
| FR-DL-12 | Completed downloads MUST remain in the managed library until the user explicitly exports or deletes them. Export MUST use a platform-safe picker/share flow and MUST NOT auto-open the file. | DL-012 |
| FR-DL-13 | Active transfers MUST use platform-supported foreground mechanisms where available. If the OS stops the app, the transfer MUST pause and remain resumable; the app MUST NOT claim completion. | DL-013 |
| FR-DL-14 | Concurrent downloads MUST respect the configured limit and remain stable under pause/resume/retry. | DL-013 |
| FR-DL-15 | A download of a tombstoned publication MUST remain possible only while a source serves it, and MUST display the unpublished-status warning. | DL-014 |

### 9.11 Voluntary replication

| ID | Requirement | Acceptance |
|---|---|---|
| FR-REP-01 | Hosting MUST require an explicit user action for each file. NEXUS MUST NOT silently replicate or seed content the user did not publish or explicitly choose to host. | REP-001 |
| FR-REP-02 | Before advertising a replica, the app MUST download or verify the complete file against its content ID and chunk structure. Incomplete or corrupt content MUST NOT be advertised. | REP-002 |
| FR-REP-03 | The user MUST be able to set storage and bandwidth limits. The app MUST enforce them and explain when a quota blocks hosting. | REP-004 |
| FR-REP-04 | The user MUST be able to pause or remove a replica. Removing MUST stop serving, require confirmation, and delete local bytes only after confirmation. | REP-005 |
| FR-REP-05 | A replica MUST preserve the original signed publication record and MUST be shown as a source, not as the original uploader. | REP-006 |
| FR-REP-06 | The hosted-files screen MUST list each replica's size, publication, verification state, last verification time, storage use, and serving state. | REP-003 |
| FR-REP-07 | Removing a replica MUST NOT alter the original uploader's record or other peers' copies. | REP-007 |
| FR-REP-08 | A replica MUST be re-verified on startup and before serving after a detected local change, and at least every 7 days. A failed verification MUST stop advertising and serving. | REP-008 |
| FR-REP-09 | The app MUST never advertise a partial download as a replica. Downloading a file does not create a hosting obligation. | REP-001, REP-002 |
| FR-REP-10 | Battery-aware and metered-network replication controls are later scope. v1 MUST surface platform network-availability and metered signals, apply the user's configured hosting/serving preference, and MUST NOT silently start a large background replication on a metered network. | REP-004 |

### 9.12 UX, settings, and honest states

| ID | Requirement | Acceptance |
|---|---|---|
| FR-UX-01 | The app MUST provide the screens listed in the roadmap: welcome, identity setup, Explore, Search, profile, file details, Publish, Downloads, Hosted files, Settings, and Network status. Navigation MUST support back/return and state restoration. | UX-001 |
| FR-UX-02 | The app MUST use a shared design system for typography, spacing, colour, icons, controls, and dark/light themes. The theme choice MUST persist. | UX-002 |
| FR-UX-03 | Every data-driven screen MUST implement loading, empty, offline, error, and retry states. | UX-003 |
| FR-UX-04 | Status vocabulary MUST be consistent and honest: "Available now", "Seen recently", "Last seen", "No known source", and "Record only" must not be used interchangeably. | UX-004 |
| FR-UX-05 | Settings MUST include identity backup/restore, storage usage and quota, bandwidth limits, bootstrap peers, theme, language, diagnostics, privacy, and about/version information. | UX-005 |
| FR-UX-06 | Network status MUST show node state, connectivity, peer count, configured bootstrap paths, and recent errors, with a redacted diagnostics export. | UX-006 |
| FR-UX-07 | Destructive actions (unpublish, remove replica, delete download, reset identity) MUST require confirmation and explain consequences, including irreversible ones. | UX-007 |
| FR-UX-08 | Errors MUST be actionable and MUST NOT expose raw stack traces, secrets, or local paths. Mock/demo data MUST be labelled as such. | UX-008 |
| FR-UX-09 | Windows MUST support keyboard navigation and visible focus. Android MUST support standard back navigation and accessible touch targets. | UX-009 |
| FR-UX-10 | The app MUST provide a global network indicator that distinguishes offline, connecting, online, and degraded states without implying that content is available. | UX-006, UX-010 |

### 9.13 Accessibility

| ID | Requirement | Acceptance |
|---|---|---|
| FR-A11Y-01 | The UI MUST target WCAG 2.2 AA principles adapted to native applications: contrast, focus visibility, non-colour status, text scaling, and operable controls. | A11Y-001 |
| FR-A11Y-02 | TalkBack on Android and Narrator on Windows MUST be able to complete onboarding, publishing, search, download, replication, and settings journeys with meaningful labels and order. | A11Y-002, A11Y-003 |
| FR-A11Y-03 | Touch targets MUST be at least 48×48 dp; keyboard focus order MUST follow visual order; focus MUST be visible. | A11Y-004 |
| FR-A11Y-04 | Text MUST remain usable at 200% font scale; reduced-motion settings MUST be respected; status MUST NOT rely on colour alone. | A11Y-005, A11Y-006 |
| FR-A11Y-05 | Essential actions MUST NOT be time-limited without a warning and an extension option. | A11Y-006 |

### 9.14 Language and localisation

| ID | Requirement | Acceptance |
|---|---|---|
| FR-L10N-01 | All user-facing strings MUST be externalised into resources. English (US) is the complete v1 language. No concatenated translated fragments. | L10N-001 |
| FR-L10N-02 | Dates, times, numbers, file sizes, and relative times MUST use locale-aware formatting. Layouts MUST be RTL-ready and verified with a pseudo-locale. | L10N-002, L10N-003 |
| FR-L10N-03 | The app MUST follow the OS language by default and allow an in-app override that persists. Unsupported languages MUST fall back to English. | L10N-004 |

### 9.15 Security, privacy, and safety

| ID | Requirement | Acceptance |
|---|---|---|
| FR-SEC-01 | A threat model MUST cover identity/key theft, fake records, Sybil behaviour, DHT poisoning, malicious peers, spam, and resource exhaustion. | SEC-001 |
| FR-SEC-02 | All incoming messages and metadata MUST be schema-validated, size-bounded, and rate-limited. Malformed input MUST NOT crash the app or corrupt local state. | SEC-002, SEC-003 |
| FR-SEC-03 | Logs and diagnostics MUST redact private keys, passphrases, identity backup contents, local paths, and unnecessary network identifiers. | SEC-004 |
| FR-SEC-04 | Downloaded files MUST never be auto-executed. Previews MUST be sandboxed or delegated to safe platform handlers. | SEC-005 |
| FR-SEC-05 | The app MUST warn about key loss, backup responsibility, public visibility, and the lack of central recovery. | SEC-006 |
| FR-SEC-06 | Dependencies MUST be scanned for vulnerabilities and static analysis MUST be part of CI. High-severity findings MUST be fixed or explicitly release-blocked. | SEC-007 |
| FR-SEC-07 | The app MUST disclose that published profiles/files are public, that peers may see network addresses, and that NEXUS does not guarantee anonymity. | SEC-008 |
| FR-SEC-08 | No telemetry or crash reporting is enabled by default. Any collection MUST be opt-in, minimised, documented, and separable from core functionality. | SEC-009 |
| FR-SEC-09 | Protocol and record versions MUST be checked. Unknown or downgrade attempts MUST be rejected or quarantined safely. | SEC-010 |
| FR-SEC-10 | Signed records MUST be verified before storage or display as valid. A malicious record MUST NOT overwrite a valid record or impersonate another identity. | SEC-010 |
| FR-SEC-11 | Crash reporting, if enabled, MUST exclude secrets and local paths and MUST respect consent. | SEC-009 |

### 9.16 Non-functional requirements

| ID | Requirement | Acceptance |
|---|---|---|
| FR-NFR-01 | Cold start to interactive MUST be ≤5 s p95 on the Android baseline and ≤3 s p95 on the Windows baseline. | NFR-001 |
| FR-NFR-02 | Idle memory MUST be ≤250 MB on Android baseline and ≤300 MB on Windows baseline. Active transfer overhead MUST stay ≤128 MiB above idle per download. | NFR-002, NFR-003 |
| FR-NFR-03 | No main-thread work may block UI interaction for more than 200 ms. Hashing, network, database-heavy work, and file I/O MUST be off the UI thread. | NFR-004 |
| FR-NFR-04 | Crash-free sessions in beta MUST be ≥99.5%. A 24-hour session and 100 core start/stop cycles MUST not leak or crash. | NFR-005 |
| FR-NFR-05 | Typical local database queries MUST have p95 ≤100 ms with 10,000 publication records. Search first local results SHOULD appear within 500 ms. | NFR-006 |
| FR-NFR-06 | Distributed search MUST show partial results within 10 s when sources are slow or missing, with an honest coverage indicator. | NFR-007 |
| FR-NFR-07 | Idle operation MUST not hold wake locks or perform unbounded background network chatter. Battery and background behaviour MUST be measured and documented. | NFR-008 |
| FR-NFR-08 | Offline startup MUST reach a usable local screen without waiting for network timeouts. | NFR-009 |
| FR-NFR-09 | The architecture MUST be load-tested with documented peer/record counts; no claim of internet-scale capacity may be made without measurements. | NFR-010 |
| FR-NFR-10 | Both platforms MUST pass the same content-ID, signature, record, and protocol compatibility vectors. | NFR-010, PLAT-004 |

### 9.17 Release engineering

| ID | Requirement | Acceptance |
|---|---|---|
| FR-REL-01 | Release artifacts MUST be signed: Android APK/AAB and Windows installer/executable. SHA-256 checksums and release notes MUST be published. | REL-001 |
| FR-REL-02 | Install, update, and uninstall MUST work on both platforms. The Windows installer MUST support per-user installation without administrator rights by default. Updates MUST preserve user data and identity unless the user explicitly resets. | REL-002 |
| FR-REL-03 | A release MUST have no known release-blocking crash, data-loss bug, or unmitigated high-severity security issue. | REL-003 |
| FR-REL-04 | Performance, compatibility, accessibility, and localisation reports MUST be produced for each release candidate. | REL-004 |
| FR-REL-05 | Reproducible build documentation MUST be validated by a clean-checkout build. | REL-005 |
| FR-REL-06 | Protocol, bootstrap operations, relay operations where supported, security disclosure, governance, and known-limitations documentation MUST be published with the release. | REL-006 |

---

## 10. Data and file policy (v1 parameters)

These values are normative and testable. Technical encoding details are decided in Stages 05–07.

| Parameter | v1 value |
|---|---|
| Minimum file size | 1 byte (empty files rejected) |
| Maximum published file size | 4 GiB = 4,294,967,296 bytes |
| Default managed storage quota | 10 GiB |
| Quota configurable range | 1–500 GiB, additionally bounded by free space |
| Minimum free-space reserve | 1 GiB after any write; warn below 2 GiB |
| Managed store contents | Published originals, replicas, completed downloads, and resumable partial downloads |
| Quota accounting | All managed-store bytes, including partial downloads |
| Export behaviour | Explicit user action; platform picker/share; never silent overwrite |
| Maximum display filename | 255 UTF-8 bytes, 1–255 bytes; control characters rejected/sanitised |
| Stored filename | Generated by content ID and safe internal ID; never a user path |
| Description | 0–2,000 Unicode characters |
| Tags | 0–10 tags; each 1–32 characters after trim; case-insensitive dedupe |
| Categories | Documents, Images, Audio, Video, Software, Data, Archives, Other (exactly one) |
| Display name | 1–64 characters |
| Username label | Optional; 3–32 characters; lowercase letters, digits, `_`, `-`, `.`; must start and end alphanumeric; collisions allowed |
| Bio | 0–500 characters |
| Avatar | PNG/JPEG/WebP; ≤2 MiB; ≤1024×1024 px |
| Search query | ≤128 characters |
| Search distributed timeout | 10 s before partial-result display |
| Online freshness | Liveness within 60 s |
| Offline threshold | No successful observation for 5 min |
| Cached metadata freshness label | After 24 h since last refresh |
| Replica re-verification | On startup, after detected local change, and at least every 7 days |
| Default concurrent downloads | 2; configurable 1–4 |
| Default concurrent uploads | 4; configurable 1–8 |
| Bandwidth defaults | Unlimited unless user sets upload/download limits |
| Network timeouts | Direct connection 15 s; bootstrap attempt 30 s; bounded retries |
| Protocol record maximum | 64 KiB per signed profile/publication record; larger records rejected |
| Diagnostics retention | Local, user-clearable; redacted; no secrets |

### 10.1 File categories

| Category | Intended content |
|---|---|
| Documents | Text, PDF, office documents, e-books |
| Images | Photos, graphics, diagrams |
| Audio | Music, speech, sound recordings |
| Video | Films, clips, recordings |
| Software | Applications, installers, scripts, source archives |
| Data | Datasets, databases, structured exports |
| Archives | Compressed bundles and backups |
| Other | Content that does not fit the other categories; still subject to all safety rules |

### 10.2 Duplicate content and modified files

- Content identity is global to the bytes. Local storage keeps one verified copy per content ID, and deduplicated bytes count once toward storage quota.
- Each identity may have its own signed publication record for the same content ID.
- The same identity republishing the same content ID creates a new record version for metadata changes; the previous version is superseded but may remain cached by peers.
- A modified file has a different content ID and a new publication record. The optional parent reference identifies a content ID and/or publication record; it does not imply authorisation or endorsement.
- The UI groups sources by content ID but MUST keep uploader attribution and record versions distinct.

### 10.3 Deletion, unpublishing, and retention

- Unpublish stops local serving and publishes a signed tombstone. It does not erase copies held by others.
- Removing a replica stops serving and deletes the local replica after confirmation; the publication record remains.
- Deleting a download removes the local reference only; it does not affect the network and MUST NOT delete shared bytes still referenced by a publication or replica.
- Shared bytes are reference-counted: they are removed only when no publication, replica, or retained download references them.
- Resetting identity is destructive and requires explicit confirmation and a backup warning.
- The app MUST NOT silently delete published originals or active replicas to satisfy a quota. It may refuse new work and explain what to remove.
- Profile publication references are a convenience index; the signed publication record and its tombstone are authoritative. A profile update may remove tombstoned references, but an old profile cached by a peer is not proof that a file is still active.
- A replica that learns of a valid tombstone MUST display "unpublished by uploader". It MAY continue serving as an independent copy; the UI MUST NOT imply uploader endorsement, and the uploader cannot force the replica to delete or stop.

---

## 11. UX honesty rules

1. Never show "online" or "available" based only on a cached metadata record.
2. Always show last-seen time for a source that is not currently observed.
3. Never show "deleted" for a file whose record is known but whose sources are offline.
4. Never show a single "verified" badge that conflates signature verification and content verification.
5. Never imply that a username is unique or that a signature proves real-world identity.
6. Never imply that unpublishing deletes copies from the network.
7. Never imply that a replica is the original uploader.
8. Never promise that search results are complete; show coverage/freshness and partial-result state.
9. Never show sample/mock network content without a visible demo label.
10. Never claim a transfer is complete before full content verification.

---

## 12. Accessibility, language, and localisation requirements

- Target WCAG 2.2 AA principles for native UI. Minimum text contrast 4.5:1; non-text UI contrast 3:1.
- Android: TalkBack labels, content descriptions, focus order, 48×48 dp touch targets, font scaling to 200%.
- Windows: Narrator names/states, keyboard-only operation, visible focus, logical tab order, high-contrast support.
- Status must be conveyed by text/icon as well as colour.
- Respect reduced-motion settings; avoid essential animations.
- English (US) is the only complete v1 language. All strings externalised.
- RTL-ready layouts; locale-aware dates, numbers, and file sizes; in-app language override.
- No text baked into images for essential information.
- Error messages must be readable by screen readers and include an action.

---

## 13. Security, privacy, and safety requirements

- Cryptographic identity: Ed25519. Content identity: SHA-256. Signed records use canonical serialisation.
- Private keys are generated locally, encrypted at rest, and never logged or exported unencrypted.
- All peer messages and records are validated, bounded, and rate-limited before use.
- The app never auto-executes downloads and never silently opens files.
- Public visibility is disclosed before publishing; profile and published files are public.
- Peers can observe network addresses of peers they connect to. NEXUS does not promise anonymity.
- No telemetry by default; no central analytics dependency; crash reporting optional and redacted.
- Dependency scanning, static analysis, and a threat model are required before release.
- Local blocking/community reporting are later scope, but the protocol must not assume a central moderation authority.
- Legal and copyright obligations remain the responsibility of users and operators; NEXUS does not implement central enforcement.

---

## 14. Definition of first-release complete

v1 is complete only when all of the following are true:

1. Android and Windows applications install and run on the target matrix.
2. Both use the same Rust core and compatible protocol versions for identity, records, discovery, and transfer.
3. A user can create, back up, restore, and use an identity without a company account.
4. A user can publish and retrieve a signed profile.
5. A user can publish a file and another peer can discover, download, and verify it.
6. Pause/resume/retry/cancel and restart recovery pass integration tests.
7. A user can explicitly replicate a file, see it in hosted files, and serve it to another peer with attribution preserved.
8. Search and Explore work from decentralized sources and honestly report partial/stale/offline results.
9. Unpublish and tombstone behaviour is implemented without a false deletion promise.
10. Accessibility, localisation, security, performance, and compatibility checks meet the release gates.
11. Signed artifacts, checksums, release notes, protocol documentation, and known limitations are published.
12. No known release-blocking crash, data-loss bug, or unmitigated high-severity security issue remains.

---

## 15. Feature priority summary

The full matrix is in [Feature priority matrix](product/FEATURE_PRIORITY_MATRIX.md).

- **P0 — first public release:** required. Every P0 row has at least one acceptance criterion in this document's traceability chain.
- **P1 — later global-network release:** planned after v1; not release-blocking for v1.
- **P2 — optional/opportunistic:** may be added if it does not compromise P0 quality.
- **Out — excluded:** contradicts product rules or is explicitly out of scope.

A feature cannot be moved from P1 to P0 without updating this PRD, the matrix, acceptance criteria, and the roadmap handoff.

---

## 16. Assumptions, dependencies, and risks

| Item | Assumption or risk | Response |
|---|---|---|
| Toolchain | Stage 03 must install or configure a JDK, Gradle, Rust, and Android SDK components; the current machine has an Android SDK but no Java or Rust on PATH. | Verify in Stage 03 and document exact versions; do not block Stage 01. |
| libp2p APIs | Transport, DHT, and relay APIs vary by version and platform. | Pin versions in Stage 02; prove on both platforms in Stages 08–11; keep fallbacks. |
| NAT | Direct connections are not guaranteed. | v1 reports failure honestly; relays are P1. |
| Availability | No source online means no download. | Show record-vs-availability distinction; support voluntary replicas. |
| Search completeness | Distributed search cannot guarantee complete results. | Partial/freshness indicators; no false claims. |
| Storage | Mobile quotas and OS restrictions vary. | Managed store, quotas, foreground transfer, resumable state. |
| Key loss | No central recovery exists. | Mandatory backup warnings; encrypted export; clear limitations. |
| Security | P2P exposes peers to malicious input and network addresses. | Validation, limits, threat model, no anonymity claims, safe defaults. |
| Legal | Decentralized operation does not remove legal obligations. | Publish governance/principles; no central enforcement claim. |
| Scale | Decentralization alone does not guarantee millions of users. | Stage 19 load tests and staged adoption; no unsupported claims. |

---

## 17. Consistency review

The following checks were performed for Stage 01:

- All published files are public; no private-sharing feature appears in scope.
- No company-owned backend is required by any P0 requirement.
- Replication is explicit and never transfers attribution.
- Record existence and content availability are separate in requirements, flows, and UX rules.
- Signature verification and content verification are separate.
- Modified files have distinct content IDs and records; parent references do not imply endorsement.
- Unpublish never promises remote deletion.
- Android and Windows are both first-class and share the same core/protocol requirements.
- File-size, quota, metadata, and avatar limits are internally consistent: a maximum 4 GiB file fits within the default 10 GiB quota when free space permits.
- Every P0 capability in the feature matrix maps to at least one acceptance criterion.
- All acceptance-criteria references resolve to defined IDs (validated by `scripts/validate_stage01_docs.py`).
- Out-of-scope social and centralised features do not appear as P0 requirements.
- UI honesty rules do not contradict any functional requirement.

Open product questions intentionally deferred to Stage 02 or later technical stages are limited to implementation choices, not product behaviour: final identity ID encoding, canonical serialisation format, chunk size, DHT record key scheme, FFI mechanism details, and database column types. Each is owned by a documented ADR.
