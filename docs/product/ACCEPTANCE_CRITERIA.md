# Ember Acceptance Criteria

| Field | Value |
|---|---|
| Document ID | NEXUS-AC-001 |
| Version | 1.0.0 |
| Status | Approved — Stage 01 |
| Date | 2026-11-12 |
| Normative source | [Product requirements](../PRODUCT_REQUIREMENTS.md) |
| Traceability | [Feature priority matrix](FEATURE_PRIORITY_MATRIX.md) |

Each criterion is a pass/fail statement. A criterion is satisfied only when the stated evidence exists. Test-layer tags are guidance; the actual test plan is produced with each implementation stage.

## Test layers

| Tag | Layer | Typical evidence |
|---|---|---|
| U | Unit | Rust/Kotlin test output, deterministic vectors |
| I | Integration | Kotlin↔Rust, database, file-store, protocol tests |
| N | Network | Two-or-more node tests, churn, interruption, adversarial peers |
| S | Security/adversarial | Fuzz/malformed input, log scan, threat-model review |
| P | Performance | Benchmarks on baseline hardware, memory/CPU/battery traces |
| M | Manual/compatibility | Device/OS matrix, accessibility and UX walkthroughs |

---

## PLAT — Platform and cross-platform consistency

**PLAT-001 — Android install and launch** *(M, I)* — Given a clean Android 9 (API 28) or newer arm64 device/emulator, when the signed APK is installed and launched (and the signed AAB is generated for Play delivery), then onboarding appears within the 5 s p95 target, no account or network is required, and the app remains usable offline.

**PLAT-002 — Windows install, launch, and uninstall** *(M)* — Given a clean Windows 10 22H2 or Windows 11 x64 machine, when the per-user installer is run without administrator rights and the app is launched, then onboarding appears within the 3 s p95 target; uninstall removes the application and asks explicitly what to do with user data.

**PLAT-003 — Same identity across Android and Windows** *(I, M)* — Given one encrypted identity backup, when it is restored on Android and Windows, then both installations show the same identity ID, produce signatures that verify on the other platform, and surface conflicting record versions instead of silently merging them.

**PLAT-004 — Cross-platform content and record vectors** *(I)* — Given identical file bytes and signed record fields, when processed on Android and Windows, then the content ID, canonical signed bytes, chunk proofs, and signature verification results are identical.

**PLAT-005 — P0 platform parity** *(M)* — Given the P0 capability list, when the manual parity matrix is executed, then every P0 journey works on both platforms; only documented platform-specific presentation differences are allowed.

---

## ONB — Onboarding

**ONB-001 — Public-risk disclosure and acknowledgement** *(M)* — Given a fresh installation, when the welcome screen is shown, then it explains public visibility of profiles/files, the lack of a central account or password reset, the need for an online source, and key-loss consequences, and it requires explicit acknowledgement before identity creation.

**ONB-002 — Create or restore without an account** *(M, I)* — Given the welcome screen, when the user creates a new identity or restores a backup, then no email, phone number, account, or central registration is requested; cancelling leaves no partial identity behind.

**ONB-003 — Offline usability** *(I, M)* — Given no network connection after onboarding, when the app starts, then local screens and cached content are usable, network-dependent screens show an offline state, and no screen hangs waiting for a timeout.

**ONB-004 — Optional permissions and honest mock states** *(M)* — Given the user denies notifications or another optional permission, when they continue, then identity, profile, browsing, publishing, and settings remain available; any mock/sample data is visibly labelled and does not imply that networking works.

---

## ID — Identity and cryptography

**ID-001 — Local identity creation and stable ID** *(U, I)* — Given no local identity, when the user creates one, then an Ed25519 keypair is generated locally, a documented stable public identity ID is derived, no network is required, and the same ID is shown after restart.

**ID-002 — Signature verification and tamper rejection** *(U, I)* — Given a signed test message or record, when it is verified unmodified, then verification succeeds; when any signed field is changed, then verification fails.

**ID-003 — Private-key protection** *(S, I)* — Given a created identity, when the database, logs, diagnostics export, crash export, and ordinary queries are inspected, then no plaintext private key or passphrase is present, and key material is encrypted at rest with platform-appropriate protection.

**ID-004 — Encrypted backup and restore** *(I)* — Given an identity, when the user exports an encrypted backup with a passphrase and restores it, then the same identity ID is recovered and signing works; a wrong passphrase fails without revealing which part was wrong.

**ID-005 — Key-loss warning and no fake recovery** *(M)* — Given the identity or backup screens, when the user looks for recovery, then the UI clearly states that losing the key loses the identity, that no central reset exists, and it offers no fake recovery action.

**ID-006 — Backup format version and tamper detection** *(I, S)* — Given an identity backup, when a valid version is restored, then it succeeds; when the version is unsupported, the file is truncated, or integrity checks fail, then it is rejected without overwriting current identity data.

**ID-007 — Multi-device restore and conflict handling** *(I, M)* — Given the same identity restored on two devices, when both sign records, then both signatures verify; records with conflicting versions or timestamps are shown as conflicts, and no record is silently discarded.

**ID-008 — Cryptographic versus real-world identity and key limits** *(M)* — Given any signature status or identity-security screen, when it is displayed, then it says "Signed record verified" rather than "identity verified", makes no claim about a person's real name, legal authorship, or content legality, and explains that v1 has no automatic key rotation or central revocation.

---

## PROF — Profiles

**PROF-001 — Profile editing, limits, and offline draft** *(I, M)* — Given an identity, when the user enters valid and invalid profile values, then the limits in section 10 are enforced with inline errors; a valid edit can be saved as a local draft while offline, and the draft is not advertised as public.

**PROF-002 — Signed profile publish and retrieve** *(I, N)* — Given a valid profile draft, when the user publishes it online, then a signed profile record is retrievable by another peer and its signature verifies; when published offline, it is stored and marked "not yet discoverable" until online.

**PROF-003 — Profile update, stale cache, and supersession** *(I)* — Given a published profile, when the user publishes a newer version, then the newer version is shown as current; an older cached version is not presented as current, and cache freshness is labelled when applicable.

**PROF-004 — Username collision transparency** *(M, I)* — Given two identities using the same username label, when both profiles are displayed, then each shows a distinct identity ID and the UI explains that usernames are labels, not unique reservations or identity proof.

**PROF-005 — Profile page content and privacy** *(M)* — Given a profile page, when it loads, then it shows display name, username label, bio, avatar, identity ID, signature status, last update, and published files with record and availability status separated; it does not reveal local paths, device serial numbers, or precise location.

**PROF-006 — Avatar limits and storage** *(I, M)* — Given an avatar of PNG/JPEG/WebP at or below 2 MiB and 1024×1024 px, when uploaded, then it is accepted and stored as a separate public content blob referenced by the profile record; larger images or unsupported formats are rejected with a clear error while preserving the rest of the draft.

**PROF-007 — Malformed and conflicting profiles** *(N, S)* — Given a malformed, oversized, invalid-signature, or same-version conflicting profile record, when it is received, then invalid records are rejected, conflicts are shown without silent resolution, and the app remains stable.

---

## DATA — Local database and file store

**DATA-001 — Schema creation and restart persistence** *(I)* — Given a clean installation, when the database is initialized, then all required entities are created; after a normal restart, identity references, profiles, publications, downloads, replicas, peer observations, and settings are preserved.

**DATA-002 — Migrations and rollback** *(I)* — Given a database created from a clean install and from every previously shipped schema version, when migrations run, then data is preserved; when a migration is forced to fail, then it rolls back and reports an actionable error without data loss.

**DATA-003 — File bytes outside SQLite and managed-copy import** *(I)* — Given a large published or downloaded file, when stored, then its bytes live in the managed file store, not as a SQLite BLOB; database size remains bounded, the content survives restart, and publishing imports a managed copy without modifying or deleting the external original.

**DATA-004 — Path safety and filename collisions** *(U, I)* — Given filenames containing path traversal, absolute paths, Windows reserved names, control characters, or duplicates, when content is stored or exported, then nothing escapes the managed root, existing files are not silently overwritten, and a safe display/export name is used.

**DATA-005 — Quota and free-space enforcement** *(I)* — Given the configured quota and free-space reserve, when publishing, downloading, or replicating would exceed them, then the operation is blocked safely with a clear explanation; partial downloads count toward the quota and existing data remains intact.

**DATA-006 — Cleanup, deletion, and reference counting** *(I, M)* — Given quota pressure, cleanup, or deletion of a download whose bytes are also referenced by a publication or replica, when deletion runs, then shared bytes are not removed while another reference exists, published originals and active replicas are never deleted without explicit confirmation, and incomplete downloads are cleaned only under a user-visible retention policy.

**DATA-007 — Metadata backup, export, and restore** *(I)* — Given local metadata, when the user exports and restores it on a clean installation, then records and settings are restored; a corrupt or incompatible archive is rejected without overwriting current data.

**DATA-008 — Filename limits and safe fallback** *(U, I)* — Given filenames at and beyond 255 UTF-8 bytes, empty names, or names with forbidden characters, when handled, then limits are enforced, a safe fallback is shown or stored, and the app does not crash.

---

## HASH — File identity and integrity

**HASH-001 — SHA-256 content identity** *(U)* — Given known test vectors and arbitrary file bytes, when hashed, then the expected SHA-256 value is produced; identical bytes produce identical content IDs and a one-byte change produces a different content ID.

**HASH-002 — Chunk authentication** *(U, I)* — Given a chunk manifest or Merkle structure, when all chunks are valid, then verification succeeds; when a chunk is corrupted, missing, duplicated, or out of order, then verification fails and the chunk is rejected.

**HASH-003 — Streaming and bounded memory** *(P, I)* — Given the maximum supported file size, when hashing, serving, downloading, or verifying, then data is streamed with memory bounded per NFR-003, the whole file is never loaded into RAM, and cancellation releases resources.

**HASH-004 — Cross-platform canonical vectors** *(I)* — Given committed vectors for content IDs, chunk proofs, canonical record bytes, and signatures, when the suite runs on Android and Windows, then all results match and every signature verifies.

**HASH-005 — Final verification gate** *(I, N)* — Given a completed download or replica candidate, when the full content ID or chunk structure fails verification, then the file is not marked complete, not advertised, and not served as valid; the user sees an error and retry/delete actions.

---

## PUB — Publication records and publishing

**PUB-001 — Publication record fields, size, and versioning** *(U)* — Given a publication, when its record is serialized and signed, then it contains the required fields from FR-PUB-01, uses canonical serialisation, includes a protocol and record version, stays within the 64 KiB record limit without embedding a large chunk list, and an unknown version is rejected or quarantined rather than guessed.

**PUB-002 — Tamper rejection and record precedence** *(U, S)* — Given a valid signed publication record, when any signed field is changed or a malicious peer tries to replace a valid record with an invalid one, then verification fails and the valid record is not overwritten.

**PUB-003 — Explicit publish workflow** *(M, I)* — Given a selected file and metadata, when the user publishes, then explicit file selection, a public-visibility warning, and a confirmation are required; cancelling before signing creates no publication record and exposes no file.

**PUB-004 — Publication limits** *(I)* — Given files of 1 byte, exactly 4 GiB, 4 GiB + 1 byte, and metadata at/over each limit, when publishing, then valid inputs are accepted and invalid inputs are rejected with a clear error; an oversized file is rejected before hashing begins.

**PUB-005 — Offline publish and later advertisement** *(I, N)* — Given no network, when a valid publication is signed, then it is stored locally and marked "not yet discoverable"; when the node later comes online with publishing enabled, then the record is advertised and retrievable by a peer.

**PUB-006 — File page completeness** *(M)* — Given a file page, when it loads, then it shows display filename, content ID, size, description, category, tags, uploader identity, signature status, publication time, record version, availability, last-seen time, and known sources; absent fields are labelled rather than fabricated.

**PUB-007 — Record existence versus availability** *(M, I)* — Given a known publication with no currently reachable source, when the file page is shown, then it says "Record only — no known source" (or the canonical equivalent) and never says the file was deleted or is available.

**PUB-008 — Unpublish and tombstone** *(I, M)* — Given the user's own publication, when they unpublish, then the app stops serving, signs a tombstone, marks the record inactive, and warns that copies held by others may remain; it does not promise global deletion.

**PUB-009 — Modified file as distinct publication** *(I, M)* — Given a file derived from another file, when published, then it has a different content ID and its own signed record; an optional parent reference is displayed as informational and explicitly not as an endorsement.

**PUB-010 — Duplicate content and distinct publications** *(I)* — Given the same content ID published by two identities, when both records are stored, then the bytes are deduplicated locally, each publication keeps its own attribution, and the UI does not merge the uploaders into one claim.

**PUB-011 — Replica preserves signed record** *(I, N)* — Given a replica, when it is advertised, served, or displayed, then the original signed publication record is unchanged, the uploader attribution is preserved, and the replica cannot alter signed fields.

**PUB-012 — Safe previews and no auto-execution** *(M, S)* — Given a downloaded file, when the user views details or a preview, then executable and unsupported content is never run or auto-opened; only safe formats are previewed, and external opening requires an explicit warning/action.

**PUB-013 — Tombstoned download warning** *(I, M)* — Given a tombstoned publication that still has an independent source, when it is displayed or downloaded, then the UI labels it "Unpublished by uploader — copies may still exist" and does not imply that the download is endorsed or permanent.

---

## NET — Peer networking and connections

**NET-001 — Authenticated peer connection** *(N)* — Given two devices on the same local network, when they connect, then peer identities are authenticated, unauthenticated peers are rejected, and a small test message is exchanged successfully.

**NET-002 — Local/manual connection without internet** *(N)* — Given no internet access, when the user connects through manual peer entry or supported local discovery, then a direct local connection works; if no path is available, the UI reports a clear failure instead of pretending to be online.

**NET-003 — Timeout, failure, and retry** *(N)* — Given an unreachable peer or bootstrap address, when a connection is attempted, then it fails within the configured timeout, shows an actionable error, keeps the UI responsive, and can be retried without restarting the app.

**NET-004 — Network change and sleep/wake** *(N, M)* — Given an active connection or transfer, when Wi-Fi changes, the device sleeps/wakes, or the network drops, then the app reconnects or reports offline, pauses active transfers, preserves resumable state, and does not corrupt data.

**NET-005 — Diagnostics and redaction** *(S, M)* — Given the network diagnostics screen/export, when it is viewed or exported, then it shows node state, peers, transports, and recent errors, but contains no private keys, passphrases, local paths, or unnecessary network identifiers.

**NET-006 — Responsiveness and lifecycle stability** *(P, I)* — Given long-running network operations and 100 start/stop cycles, when they run, then the UI remains responsive, cancellation works, memory stays bounded, and no orphaned connections or crashes remain.

**NET-007 — Disconnect, malformed peers, and limits** *(N, S)* — Given a peer that disconnects mid-operation or sends malformed/oversized/abusive traffic, when the app handles it, then errors are bounded and retryable, connection/request/rate limits are enforced, and the app does not crash.

---

## BOOT — Bootstrap and manual peer configuration

**BOOT-001 — Fresh join through bootstrap** *(N)* — Given a fresh installation and at least one configured bootstrap path, when the node starts, then it joins and discovers another peer without a central account service.

**BOOT-002 — Configurable peer paths** *(I, M)* — Given the settings screen, when the user adds, removes, inspects, and persists multiple bootstrap or manual peer addresses, then valid entries survive restart and invalid entries are rejected with a specific error while the existing list remains intact.

**BOOT-003 — Bootstrap loss resilience** *(N)* — Given multiple configured bootstrap peers, when one is offline, then the node uses the remaining paths, retries appropriately, and does not declare the whole network unavailable solely because that peer is missing.

**BOOT-004 — No single company dependency** *(M, N)* — Given the public-release default configuration or a manual/offline setup, when inspected, then no single company-owned endpoint is required; a local or manual path works, and documentation states that bootstrap peers are entry points, not identity or ownership authorities.

---

## DISC — Decentralized discovery and Explore

**DISC-001 — DHT publish and lookup** *(N)* — Given a running node, when a peer or publication record is published to the chosen DHT and looked up from another node, then it is found within the configured timeout.

**DISC-002 — Bounded lookup and partial results** *(N)* — Given slow, missing, or partitioned sources, when a lookup or discovery request runs, then it returns partial, empty, or timeout status within the bound, never hangs indefinitely, and never fabricates results.

**DISC-003 — Malformed and invalid records** *(N, S)* — Given malformed, oversized, invalid-signature, or unknown-version records, when received, then they are rejected or quarantined, valid results are unaffected, and the app remains stable.

**DISC-004 — Churn and no central service** *(N)* — Given peers joining, leaving, and returning, when discovery runs, then stale peers are pruned, valid cached records carry freshness labels, and discovery works without a central account service.

**DISC-005 — Explore without social signals** *(M, I)* — Given discovered verified records, when Explore loads, then it shows categories, recent records, and random records; it does not imply that discovered content is currently available and contains no likes, comments, follows, follower counts, or popularity ranking.

**DISC-006 — Freshness, offline, provider loss, and deduplication** *(N, M)* — Given offline mode, partial providers, duplicate records, or conflicting metadata, when Explore or a file list renders, then cached/partial/coverage states are labelled honestly, duplicates are deduplicated, and conflicts remain visible.

---

## SEARCH — Search

**SEARCH-001 — Search fields and input bound** *(I)* — Given indexed records, when the user searches by username/display name, filename, category, or tag, then matching verified results are returned; a query longer than 128 characters is rejected with guidance.

**SEARCH-002 — No hidden central search and provider loss** *(N)* — Given no company search service and one or more unavailable index providers, when the user searches, then local and remaining distributed sources are queried, results are shown with reduced-coverage labelling, and search does not fail completely.

**SEARCH-003 — Signature checking and hostile metadata** *(N, S)* — Given invalid-signature records or metadata containing markup, scripts, oversized fields, or control characters, when results are rendered, then invalid records are excluded or marked unverified, hostile content is safely sanitized or truncated, and the app does not crash.

**SEARCH-004 — Partial results, timeout, and pagination** *(N)* — Given slow distributed sources, when a search runs, then partial results appear within 10 seconds with an explicit "may be incomplete" indicator, and pagination does not duplicate or skip results.

**SEARCH-005 — Deduplication, conflicts, and sorting** *(I)* — Given duplicate records and conflicting metadata, when results are merged, sorted, or filtered by relevance, recency, or observed availability, then duplicate record identities are collapsed, conflicts are surfaced, and the sort order is explainable.

**SEARCH-006 — Empty, offline, stale, and abusive states** *(M, I)* — Given an empty query result, offline mode, stale cache, or oversized/malformed query, when the UI responds, then each state is distinct and actionable, cached results show their last refresh time, and abusive input is rejected or rate-limited without blocking the UI.

---

## DL — Downloads

**DL-001 — Direct download and verification** *(N, I)* — Given a valid publication and a reachable source, when the user downloads, then the file completes, its content ID verifies, and it is marked complete in the managed library.

**DL-002 — Corrupt chunk and malicious source** *(N, S)* — Given a source that sends a corrupted or invalid chunk, when the downloader processes it, then the chunk is rejected, retried or the source is marked unreliable, and the final file is either valid or explicitly failed—never silently corrupted.

**DL-003 — Pause and resume** *(I, N)* — Given an active download, when the user pauses and later resumes, then transfer continues from the last verified offset rather than restarting from zero.

**DL-004 — Restart recovery** *(I, N)* — Given an interrupted download, when the app process is killed and restarted, then the download state and verified offset are restored and the user can resume.

**DL-005 — Cancel and partial retention** *(I, M)* — Given an active or paused download, when the user cancels, then they are asked whether to keep a resumable partial or delete it; the chosen action is applied safely and follows the visible retention policy.

**DL-006 — Source disconnect and failover** *(N)* — Given an active download, when the source disconnects mid-transfer, then the partial file remains intact, the app pauses or fails over to another source, and retry works without corruption.

**DL-007 — Disk space and quota exhaustion** *(I)* — Given insufficient free space or quota before or during a transfer, when the condition is detected, then the download stops safely, existing files remain intact, and the error explains how to free space or change the limit.

**DL-008 — Path traversal and overwrite safety** *(I, S)* — Given a peer-supplied filename containing traversal, absolute paths, reserved names, or an existing filename, when downloading or exporting, then nothing escapes the managed root, no existing file is silently overwritten, and a safe name is used.

**DL-009 — Bounded memory for large files** *(P)* — Given a download at the maximum supported file size, when transfer and final hashing run, then memory stays within 128 MiB above idle per download and the whole file is never loaded into RAM.

**DL-010 — Download progress UI** *(M, P)* — Given an active download, when the UI is shown, then it displays bytes transferred, total size when known, speed, source peer, state, and actionable errors; updates do not freeze or stutter the UI.

**DL-011 — Final verification failure** *(I)* — Given a completed transfer whose full content ID or chunk structure does not match, when verification fails, then the file is not marked complete, is not presented as usable, and the user is offered retry or delete.

**DL-012 — Managed library and explicit export** *(M, I)* — Given a completed download, when the user wants it outside Ember, then the file remains in the managed library until an explicit export/share action uses a platform picker and never auto-opens the file or silently overwrites an existing one.

**DL-013 — Background lifecycle and concurrency** *(M, I)* — Given active transfers, backgrounding, process death, and the configured concurrent limit, when these conditions occur, then platform-supported foreground mechanisms are used where available, transfers pause resumably otherwise, the queue respects the limit, and no completion is falsely reported.

**DL-014 — Tombstoned publication download** *(N, M)* — Given a tombstoned publication with an independent source, when the user downloads it, then the download is allowed only while a source serves valid content and the unpublished-status warning is shown.

---

## REP — Voluntary replication

**REP-001 — Explicit hosting only** *(I, M)* — Given any downloaded or published file, when the user has not chosen to host it, then no replica is created, no data is retained for seeding, and no advertisement occurs; only an explicit host action creates a replica.

**REP-002 — Verify before advertising** *(I, N)* — Given a host request for an incomplete or corrupt local copy, when replication starts, then the app downloads or verifies the complete file first and does not advertise or serve it until verification succeeds.

**REP-003 — Hosted-files visibility** *(M)* — Given at least one replica, when the hosted-files screen loads, then it shows the publication, size, verification state, last verification time, serving state, and storage use for each replica.

**REP-004 — Quota, bandwidth, and network-signal enforcement** *(I)* — Given configured storage and bandwidth limits, a metered or unavailable network signal, and the user's hosting preference, when hosting or serving would exceed a limit, conflict with that preference, or start silently on a metered network, then the limit/preference is applied, the signal is surfaced, and the user sees a clear explanation rather than silent eviction or unexpected background replication.

**REP-005 — Pause and remove replica** *(I, M)* — Given an active replica, when the user pauses it, then serving stops while bytes remain; when the user removes it, then serving stops immediately and local bytes are deleted only after explicit confirmation.

**REP-006 — Serve with preserved attribution** *(N)* — Given an active replica, when another peer downloads from it, then the original signed uploader attribution is preserved and the replica is shown as a source, not as the original uploader.

**REP-007 — Removal does not affect the original** *(I, N)* — Given a replica is removed, when the operation completes, then the original publication record and other peers' copies are unaffected.

**REP-008 — Re-verification and stop-on-corruption** *(I, N)* — Given a replica whose local bytes changed or became corrupt, when the app re-verifies on startup, after the change, or at the 7-day interval, then it stops advertising and serving on failure and resumes only after a valid re-verification.

---

## UX — Product experience

**UX-001 — Screens, navigation, and state restoration** *(M)* — Given the app, when the user navigates welcome, identity, Explore, Search, profile, file details, Publish, Downloads, Hosted files, Settings, and Network status, then every screen is reachable, back/return works, and navigation state is restored appropriately.

**UX-002 — Responsive layouts and themes** *(M)* — Given a 360 dp phone and a 1280×720 desktop window, when each main screen is shown, then critical controls are not clipped, layouts remain usable, and dark/light theme choice persists across restarts.

**UX-003 — Loading, empty, offline, error, and retry states** *(M, I)* — Given each data-driven screen, when it is loading, empty, offline, or in error, then the correct state and a retry path are shown; no blank screen or infinite spinner appears.

**UX-004 — Honest status vocabulary** *(M)* — Given any record/source status, when it is displayed, then the canonical vocabulary in the glossary is used consistently; "available now", "seen recently", "last seen", "no known source", and "record only" are never used interchangeably.

**UX-005 — Settings completeness** *(M)* — Given Settings, when opened, then it provides identity backup/restore, storage usage/quota, bandwidth limits, bootstrap peers, theme, language, diagnostics, privacy, and about/version information.

**UX-006 — Network status and global indicator** *(M, I)* — Given network state changes, when the user views the global indicator or Network status, then offline, connecting, online, and degraded states are distinct, and the screen shows node state, peer count, bootstrap paths, recent errors, and a redacted diagnostics export.

**UX-007 — Destructive-action confirmation** *(M)* — Given unpublish, remove replica, delete download, or reset identity, when triggered, then a confirmation explains the consequences, including irreversible or network-wide limits, and cancel leaves state unchanged.

**UX-008 — Actionable errors and safe mock labelling** *(M, S)* — Given an error or mock/demo state, when it is displayed, then the message is actionable and contains no raw stack traces, secrets, or local paths; mock data is visibly labelled as not live.

**UX-009 — Keyboard and back navigation** *(M)* — Given Windows keyboard-only use or Android back navigation, when the user moves through the main flows, then focus order is logical, focus is visible, and back/return does not trap or lose the user.

**UX-010 — Global state versus file availability** *(M, I)* — Given the global network indicator and a specific file, when both are displayed, then the global state (offline/connecting/online/degraded) is shown separately from that file's availability and last-seen status.

---

## A11Y — Accessibility

**A11Y-001 — Contrast and WCAG principles** *(M)* — Given the light and dark themes, when audited against WCAG 2.2 AA principles, then text contrast is at least 4.5:1, non-text UI contrast at least 3:1, and focus indicators are visible.

**A11Y-002 — TalkBack journey** *(M)* — Given TalkBack on Android, when a user completes onboarding, publishing, search, download, replication, and settings journeys, then labels, roles, states, and focus order are meaningful and no essential action is unreachable.

**A11Y-003 — Narrator and keyboard journey** *(M)* — Given Narrator on Windows and keyboard-only input, when the same main journeys are completed, then all controls are operable, named, and ordered correctly.

**A11Y-004 — Touch targets and focus order** *(M)* — Given interactive controls, when measured and traversed, then touch targets are at least 48×48 dp, focus follows visual order, and focus is never hidden.

**A11Y-005 — Text scaling and reduced motion** *(M)* — Given 200% font scaling and the platform reduced-motion setting, when the main journeys are used, then text remains readable and operable, and non-essential motion is reduced or removed.

**A11Y-006 — Non-colour status and timeouts** *(M)* — Given status indicators and any time-limited action, when displayed, then status is conveyed by text/icon as well as colour, and a warning plus extension is available before an essential timeout.

---

## L10N — Language and localisation

**L10N-001 — Externalised strings and English completeness** *(I, M)* — Given the UI source, when strings are scanned, then all user-facing strings are externalised, English (US) resources are complete, and there are no concatenated translated fragments or broken plural/placeholder cases.

**L10N-002 — Locale-aware formatting** *(I, M)* — Given a changed locale, when dates, times, numbers, file sizes, and relative times are displayed, then they use locale-aware formatting and remain unambiguous.

**L10N-003 — RTL readiness** *(M)* — Given a pseudo-RTL locale, when the main screens are shown, then layouts mirror appropriately, text is not clipped, and essential controls remain reachable.

**L10N-004 — Language override and fallback** *(M, I)* — Given an in-app language override, when the app restarts, then the choice persists; an unsupported or incomplete locale falls back to English without breaking layout or navigation.

---

## SEC — Security, privacy, and safety

**SEC-001 — Threat model** *(M)* — Given the security documentation, when reviewed, then it covers identity/key theft, fake records, Sybil behaviour, DHT poisoning, malicious peers, spam, and resource exhaustion, with mitigations and residual risks mapped to requirements.

**SEC-002 — Malformed-message resistance** *(S, I)* — Given malformed, fuzzed, oversized, or hostile network messages and metadata, when processed, then schema validation and bounds are enforced, the app does not crash or corrupt local state, and valid operations continue.

**SEC-003 — Resource limits** *(S, I)* — Given connection, request, message-size, storage, and bandwidth limits, when a peer or user exceeds them, then limits are enforced with bounded errors and the UI remains usable.

**SEC-004 — Log and diagnostics redaction** *(S)* — Given logs, diagnostics, crash exports, and error reports, when scanned, then they contain no private keys, passphrases, identity backup contents, local paths, or unnecessary network identifiers.

**SEC-005 — No auto-execution** *(S, M)* — Given any downloaded file, when it completes or is previewed, then it is never executed or opened automatically; previews are limited to safe formats and external opening is an explicit user action with a warning.

**SEC-006 — Key-backup and recovery warning** *(M)* — Given identity creation, backup, and settings, when the user views key management, then the lack of central recovery and the consequence of key loss are clearly stated, and a backup action is available.

**SEC-007 — Dependency and static analysis** *(S)* — Given the dependency and static-analysis reports, when a release candidate is evaluated, then there are no unresolved high-severity findings unless explicitly blocked from release with an owner and mitigation.

**SEC-008 — Public visibility and anonymity disclosure** *(M)* — Given first publish/profile use, when the user proceeds, then the UI discloses that profiles/files are public, peers may observe network addresses, and Ember does not guarantee anonymity.

**SEC-009 — No default telemetry or crash reporting** *(S, M)* — Given a default installation, when network traffic and settings are inspected, then no telemetry or crash reporting is enabled; any collection is opt-in, documented, minimised, and excludes secrets and local paths.

**SEC-010 — Protocol/version validation and record integrity** *(S, I)* — Given unknown, downgraded, or malicious protocol/record versions, when received, then they are rejected or quarantined, signed records are verified before trust, and a malicious record cannot overwrite a valid one or impersonate another identity.

---

## NFR — Performance, memory, battery, and durability

**NFR-001 — Cold start** *(P)* — Given the baseline Android and Windows devices, when the app cold-starts, then time to interactive is ≤5 s p95 on Android and ≤3 s p95 on Windows.

**NFR-002 — Idle memory** *(P)* — Given the app idle after onboarding, when measured, then resident memory is ≤250 MB on the Android baseline and ≤300 MB on the Windows baseline.

**NFR-003 — Active transfer memory** *(P)* — Given active downloads, when measured, then transfer overhead is ≤128 MiB above idle per download and remains bounded for the maximum file size.

**NFR-004 — UI thread responsiveness** *(P)* — Given hashing, network, database, and file operations, when profiled, then no main-thread block exceeds 200 ms and the UI remains responsive.

**NFR-005 — Crash-free sessions and long-running stability** *(P, M)* — Given beta usage, a 24-hour session, and 100 core start/stop cycles, when measured, then crash-free sessions are ≥99.5% and there is no unbounded memory growth, orphaned process, or crash.

**NFR-006 — Database and local search performance** *(P)* — Given 10,000 publication records, when typical queries and local search run, then p95 query time is ≤100 ms and first local search results appear within 500 ms.

**NFR-007 — Distributed search freshness** *(P, N)* — Given slow or missing distributed sources, when a search runs, then partial results and a coverage indicator appear within 10 s.

**NFR-008 — Battery and background behaviour** *(P, M)* — Given idle and background states, when measured, then the app holds no unnecessary wake locks, performs no unbounded network chatter, and its battery/background behaviour is documented.

**NFR-009 — Offline startup** *(M, I)* — Given no network, when the app starts, then it reaches a usable local screen without waiting for network timeouts.

**NFR-010 — Load and compatibility evidence** *(P, I)* — Given documented load tests and cross-platform vectors, when run, then peer/record counts, resource use, and compatibility results are recorded; no internet-scale capacity claim is made without measurements.

---

## REL — Release engineering and documentation

**REL-001 — Signed artifacts and checksums** *(M)* — Given a release candidate, when Android and Windows artifacts are produced, then signatures verify, SHA-256 checksums are published, and release notes identify the protocol/app versions.

**REL-002 — Install, update, and uninstall** *(M)* — Given an existing installation, when the user updates, then data and identity are preserved unless an explicit reset is chosen; uninstall behaviour is documented and tested on both platforms.

**REL-003 — No release blockers** *(M)* — Given the full release test matrix, when the candidate is evaluated, then there is no known release-blocking crash, data-loss bug, or unmitigated high-severity security issue.

**REL-004 — Release evidence reports** *(M)* — Given a release candidate, when release evidence is assembled, then performance, compatibility, accessibility, and localisation reports exist and are linked from the release notes.

**REL-005 — Reproducible build** *(M, I)* — Given a clean checkout, when the documented build steps are followed, then the build completes and reproducibility is verified or documented with known nondeterminism.

**REL-006 — Protocol, operations, and governance documentation** *(M)* — Given a public release, when documentation is checked, then protocol versions, bootstrap/relay operation, security disclosure, governance/moderation principles, and known limitations are published.

---

## Traceability and evidence rules

1. Every P0 feature in the [feature priority matrix](FEATURE_PRIORITY_MATRIX.md) references at least one criterion in this document.
2. Every criterion ID referenced by the PRD or matrix must be defined exactly once here; this is checked by `scripts/validate_stage01_docs.py`.
3. A criterion is not "passed" because code exists. Evidence must identify the build, command, device/OS, and result.
4. Failed, blocked, or not-run criteria must be recorded in the stage handoff. A stage cannot be marked complete while a P0 criterion is not run or failing.
5. Test evidence must not contain private keys, passphrases, or personal data.
6. Manual evidence must include the device/OS matrix and a screenshot or signed-off checklist where applicable.
7. Network evidence must record topology, peer roles, and whether a relay was used. v1 does not claim relay support.
8. Performance evidence must record hardware, build type, dataset size, and percentile method.
9. Security evidence must record the tool/version and whether findings were fixed, mitigated, or explicitly blocked.
10. Any criterion changed after approval requires a PRD/matrix update and a new document version.
