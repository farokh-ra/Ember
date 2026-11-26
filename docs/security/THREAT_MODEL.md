# Ember Threat Model

**Status:** Implemented for Stage 18; independent review not yet commissioned
**Last reviewed:** 2026-11-26
**Scope:** Ember `0.1.x` development builds for Android and Windows
**Related requirements:** FR-SEC-01 to FR-SEC-11, SEC-001 to SEC-010

## 1. Purpose

Ember is a local-first, peer-to-peer publishing and discovery application. There
is no central server, no account system, and no central moderation authority.
This document records the assets that matter, the boundaries an attacker can
reach, the threats Ember accepts as in scope, and the controls that currently
exist. It is a working document: a control marked *partial* or *deferred* is not
a claim that the threat is solved.

## 2. Assets

| Asset | Why it matters | Where it lives |
|---|---|---|
| Identity signing key | Anyone holding it can sign records as the user | Encrypted identity vault under the local data root; never exported except through an explicit encrypted backup |
| Device peer key | Identifies the device on the peer network and authenticates transport sessions | Encrypted at `network/peer_key.enc`; separate from the identity key |
| Identity backup and passphrase | Full control of the identity if stolen | Created only on explicit export; passphrase supplied by the user and never stored |
| Local files and download partials | Private user content and metadata | App-managed storage; managed quota with a free-space reserve |
| Local database | Profiles, publications, downloads, block list, abuse reports | SQLite in the local data root |
| Peer graph and network addresses | Can reveal who the user talks to and where they are | In memory and in the local database; shared with peers only as required by the protocol |
| Published records | Public by design once announced | Local database plus the DHT and requesting peers |
| Block list and abuse reports | Private moderation decisions | Local database only |

## 3. Trust boundaries and actors

| Boundary | What crosses it | Default posture |
|---|---|---|
| Host app to Rust core | CBOR requests, config, events, FFI handles | Untrusted input; validated, size-bounded, and method-checked |
| Peer to peer | Signed records, DHT values, file metadata, chunk slices, test messages | Untrusted input; decoded with limits and verified before use |
| Core to local store | SQL, quotas, paths | Trusted code, untrusted values from the network |
| Core to platform file handlers | Downloaded file paths | Delegated preview only; never executed by the core |
| Build to dependency graph | Crates, Gradle modules, native libraries | Scanned in CI; pinned toolchain |

Actors considered: a passive network observer, a malicious peer, a set of
colluding peers (Sybil), a malicious DHT participant, a spam or harassment
sender, a local attacker with access to the device or its backups, and a
compromised dependency.

## 4. Threats and controls

### 4.1 Identity and key theft

| ID | Threat | Impact | Controls | Status |
|---|---|---|---|---|
| T-01 | Identity key extracted from storage | Full impersonation and record forgery | Identity vault encrypted with a key derived from the host-supplied local encryption key; no plaintext key on disk; key material zeroized after use | Implemented |
| T-02 | Device peer key extracted | Session impersonation for this device | Separate encrypted peer key; not part of identity backup or export; not written to events or logs | Implemented |
| T-03 | Backup stolen or passphrase guessed | Full identity takeover | Backup only produced on explicit export, encrypted with an Argon2id-derived key, returned once to the caller and never logged or cached | Implemented |
| T-04 | User loses the identity key or backup | Permanent loss of identity and published history | Startup and identity screens warn about key loss, backup responsibility, and the absence of central recovery; `security.notices` exposes the same warning to the UI | Implemented |
| T-05 | Replay of an old signed record | Stale or revoked data replaces newer data | Monotonic revisions per record kind; equal-revision conflicts remain distinct and verifiable; tombstones bind to the original record | Implemented |
| T-06 | Cross-identity record forgery | One identity pretends to be another | Every signed record carries the signing key and is verified against the claimed identity before storage or display | Implemented |

### 4.2 Fake records, Sybil, and DHT poisoning

| ID | Threat | Impact | Controls | Status |
|---|---|---|---|---|
| T-07 | Forged profile or publication record | Misleading content under a trusted name | Signature, identity, kind, version, size, and cross-field checks before acceptance; invalid records are discarded | Implemented |
| T-08 | Unknown major version or downgrade attempt | Parser confusion or bypass of newer checks | Unknown major record, protocol, and manifest versions are rejected; unknown minor fields are preserved | Implemented |
| T-09 | DHT value flood or oversized value | Memory or storage exhaustion, poisoning | Fixed value-size cap, bounded provider addresses, record kind and key checks, monotonic revisions, and local validation on read | Implemented |
| T-10 | Sybil peers dominate discovery or providers | Eclipse, spam amplification, poor availability | Bounded peer tables and provider lists, per-peer request budgets, one total search deadline, and no popularity or engagement ranking | Partial; Sybil resistance is bounded but not eliminated |
| T-11 | Discovery returns a blocked or malformed peer | Unwanted connection or parser pressure | Discovery results and providers are filtered against the local block list and validated addresses; malformed entries are dropped | Implemented |

### 4.3 Malicious peers and malformed messages

| ID | Threat | Impact | Controls | Status |
|---|---|---|---|---|
| T-12 | Truncated, mutated, or oversized CBOR | Crash, panic, or memory pressure | Every decoder enforces depth, item, map-entry, and byte limits; trailing bytes and duplicate keys are rejected; Stage 18 adds truncation and single-byte mutation sweeps for file messages | Implemented |
| T-13 | Oversized FFI request or response | Buffer overrun or excessive allocation | Explicit request, event, and response caps checked before decode or copy; oversized input returns a stable status and is never partially parsed | Implemented |
| T-14 | Malicious source serves corrupt chunks | Silent file corruption | Every chunk is SHA-256 verified against the signed manifest before it reaches the partial file; corrupt responses are retried from another source and repeated corruption quarantines the source | Implemented |
| T-15 | Peer disappears mid-download | Stalled or failed download | Per-chunk leases, bounded retries, source re-ranking, quarantine, and resumable per-chunk persistence | Implemented |
| T-16 | Request flood or connection churn | CPU, socket, or queue exhaustion | Bounded connection and peer tables, bounded request sizes, per-peer request rate limits for index providers, bounded worker pools, and one in-flight lease per peer | Implemented |
| T-17 | Spam test or file requests from a blocked peer | Repeated unwanted traffic | A blocked peer is refused before handling: inbound requests return `blocked`, inbound connections are closed, and outbound dials fail with `peer_blocked` | Implemented |
| T-18 | Peer sends a valid-looking message under the wrong identity | Session confusion | Transport peer identity is checked against the expected peer ID; mismatches fail with `wrong_peer_id` | Implemented |

### 4.4 Local abuse, spam, and moderation

| ID | Threat | Impact | Controls | Status |
|---|---|---|---|---|
| T-19 | User receives unwanted peer traffic or content | Harassment, spam, scam exposure | Local block list keyed by peer ID, enforced at dial, inbound connection, request, discovery, provider, and download-source layers | Implemented |
| T-20 | Moderation data leaks or is weaponized | Retaliation or loss of trust | Blocking and reporting are local-only; nothing is uploaded, published, or inserted into signed records; listings are marked `local_only` | Implemented |
| T-21 | Block and report tables grow without bound | Disk exhaustion | Store caps (4096 blocks and 4096 reports by default, configurable 1 to 65,536) enforced inside the writer transaction; notes capped at 512 bytes | Implemented |
| T-22 | Malformed peer ID or note injected into moderation tables | Database abuse or UI injection | IDs restricted to 128 ASCII alphanumeric or `- _ . :` characters; unknown reasons and categories rejected; notes bounded on character boundaries | Implemented |
| T-23 | No central authority can remove abusive content | Abuse persists on the network | Explicit non-goal: Ember provides local blocking and reporting, not network-wide takedown. The UI must not imply otherwise | Accepted |

### 4.5 Unsafe files and platform integration

| ID | Threat | Impact | Controls | Status |
|---|---|---|---|---|
| T-24 | Downloaded file executed automatically | Code execution | The core has no execution path; downloads end at a verified file in managed storage; every download event and listing reports `auto_execute: false` | Implemented |
| T-25 | Preview opens a malicious document in a privileged handler | Code execution via the platform | Previews are delegated to the platform viewer or an external sandbox; the core never renders or opens the file itself and reports `preview: "delegated"` | Implemented |
| T-26 | File type is underestimated | User opens an executable or script unknowingly | Advisory classification by extension into `document`, `media`, `archive`, `executable`, `script`, and `unknown`, with a warning flag and message; the UI must display it before delegation | Implemented |
| T-27 | Dangerous file escapes the managed directory | Path traversal or overwrite | Download file names are sanitized and written only under the managed storage root | Implemented |

### 4.6 Privacy and disclosure

| ID | Threat | Impact | Controls | Status |
|---|---|---|---|---|
| T-28 | Logs leak keys, paths, or peer identifiers | Local or shared disclosure | Logging policy forbids secrets; identifiers are debug-only and redacted; release builds do not enable debug logging | Implemented |
| T-29 | Telemetry or crash reporting exfiltrates data | Silent privacy loss | No telemetry or crash reporting is enabled, bundled, or phoned home; no analytics dependency | Implemented |
| T-30 | User does not understand that publishing is public | Unintended disclosure | Publish and profile surfaces warn that signed records are public; `security.notices` repeats the warning; the privacy review documents what is shared | Implemented |
| T-31 | Network metadata reveals social graph or location | Deanonymization | Peer IDs and addresses are shared only as the protocol requires; no contact list, presence, or read receipts; no anonymity guarantee is made | Accepted residual risk |

### 4.7 Local device and supply chain

| ID | Threat | Impact | Controls | Status |
|---|---|---|---|---|
| T-32 | Local attacker edits the database | Tampered state or lost blocks | The database is app-private; signed records are still verified on read; block and report rows are local policy, so local tampering is out of the remote threat model | Accepted |
| T-33 | Compromised dependency or toolchain | Arbitrary code in the build | Pinned toolchain, committed `Cargo.lock`, `cargo deny`, `cargo audit`, dependency review, and license inventory in CI | Implemented |
| T-34 | High-severity dependency advisory | Known exploitable flaw ships | CI fails on advisories; a high-severity finding is a release blocker until fixed or explicitly documented | Implemented process |

## 5. Controls mapped to requirements

| Requirement | Control | Evidence |
|---|---|---|
| FR-SEC-01 | This threat model | `docs/security/THREAT_MODEL.md` |
| FR-SEC-02 | CBOR and protocol bounds, truncation and mutation sweeps, request caps, rate limits | `nexus-protocol`, `nexus-net`, and `nexus-core` Stage 18 tests |
| FR-SEC-03 | Logging policy and redaction helpers | `docs/LOGGING_POLICY.md`; Stage 18 log review |
| FR-SEC-04 | No execution path, delegated previews, safety notices | `nexus-core/src/safety.rs`; `security.notices`; Stage 18 tests |
| FR-SEC-05 | Key-loss and public-visibility notices | Identity and publish surfaces; `security.notices` |
| FR-SEC-06 | Clippy, `cargo deny`, `cargo audit`, dependency review | CI workflows |
| FR-SEC-07 | Public-visibility disclosure | Privacy review and product copy |
| FR-SEC-08 | No telemetry by default or at all | Privacy review; dependency inventory |
| FR-SEC-09 | Version checks and downgrade rejection | Signed record and protocol tests |
| FR-SEC-10 | Verification before storage or display | Record verification suites |
| FR-SEC-11 | Crash reporting absent; consent policy ready if added | Privacy review |

## 6. Explicit non-goals

- Anonymity or censorship resistance against a global observer. Ember does not
  use onion routing, cover traffic, or mix networks.
- Network-wide content removal or a central reputation authority.
- Malware scanning. Ember verifies integrity and warns about file types; it does
  not decide whether content is safe.
- Protection from a compromised operating system, a rooted device, or an
  attacker who can read process memory.
- Hiding that a peer participates in the network from a local network observer.

## 7. Residual risks and release blockers

| Risk | Severity | Disposition |
|---|---|---|
| Sybil peers can still degrade discovery quality | Medium | Bounded and documented; no release blocker for a development build |
| No independent security review has been performed | Medium | Required before a broad public release; tracked in the release checklist |
| Block lists are local, so a blocked peer can reappear through a new identity | Medium | Accepted by design; users can block again |
| Abuse reports cannot yet be deleted from the UI | Low | Retention documented in the privacy review; a deletion control is deferred |
| Android runtime hardening evidence is deferred | Medium | Documented in the Stage 18 evidence record; not claimed as verified |
| Report and block metadata is unencrypted at rest beyond app-private storage | Low | Accepted for the current platform threat model |

A high-severity finding in any control above blocks release until it is fixed or
explicitly accepted with a written rationale and an owner.
