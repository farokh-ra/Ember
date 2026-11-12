# NEXUS Feature Priority Matrix

| Field | Value |
|---|---|
| Document ID | NEXUS-PRIORITY-001 |
| Version | 1.0.0 |
| Status | Approved — Stage 01 |
| Date | 2026-11-12 |
| Normative source | [Product requirements](../PRODUCT_REQUIREMENTS.md) |

## Priority definitions

| Priority | Meaning | Release rule |
|---|---|---|
| **P0** | Required for the first public release (v1). | Cannot ship v1 without it. Every P0 row has at least one acceptance criterion. |
| **P1** | Planned for a later global-network release. | Must not delay P0. May be implemented earlier only after P0 quality gates pass. |
| **P2** | Optional or opportunistic. | May be added if it does not compromise P0 correctness, security, or accessibility. |
| **Out** | Excluded because it contradicts product rules or is explicitly outside the product. | Must not be implied by UI or documentation. |

A priority change requires updates to this matrix, the PRD, acceptance criteria, and the stage handoff.

---

## P0 — First public release

| ID | Capability | Roadmap stage | Requirements | Acceptance criteria |
|---|---|---|---|---|
| F-01 | Android application build, install, launch | 03, 19 | FR-REL-01, FR-REL-02, FR-UX-01 | PLAT-001, REL-001, REL-002 |
| F-02 | Windows application build, install, launch, uninstall | 03, 19 | FR-REL-01, FR-REL-02, FR-UX-01 | PLAT-002, REL-001, REL-002 |
| F-03 | Shared Rust core and Kotlin FFI on both platforms | 02, 08 | FR-ID-01, FR-NET-05, FR-NFR-10 | PLAT-003, PLAT-004, NET-006, NFR-010 |
| F-04 | First-launch onboarding and public-risk disclosure | 04 | FR-ONB-01–FR-ONB-04 | ONB-001–ONB-004 |
| F-05 | Local Ed25519 identity and stable identity ID | 06 | FR-ID-01, FR-ID-02, FR-ID-05, FR-ID-06 | ID-001–ID-003, ID-008 |
| F-06 | Encrypted identity backup, restore, and limitations | 06 | FR-ID-03, FR-ID-04, FR-ID-07, FR-ID-08 | ID-004–ID-008, PLAT-003 |
| F-07 | Signed public profile with collisions and conflicts | 13 | FR-PROF-01–FR-PROF-07 | PROF-001–PROF-007 |
| F-08 | SQLite schema, migrations, persistence | 05 | FR-DATA-01, FR-DATA-02, FR-DATA-07 | DATA-001–DATA-004, DATA-008 |
| F-09 | Managed file store, quotas, cleanup safety | 05 | FR-DATA-03–FR-DATA-08 | DATA-003–DATA-008, DL-012 |
| F-10 | SHA-256 content ID, chunk manifest, verification | 07 | FR-HASH-01–FR-HASH-05 | HASH-001–HASH-005 |
| F-11 | Signed publication record and canonical fields | 07 | FR-PUB-01, FR-PUB-02 | PUB-001, PUB-002 |
| F-12 | Publish a file explicitly and serve while online | 14 | FR-PUB-03–FR-PUB-05 | PUB-003–PUB-005 |
| F-13 | File page with attribution and honest availability | 14 | FR-PUB-06, FR-PUB-07 | PUB-006, PUB-007, UX-004 |
| F-14 | Modified file as distinct content with source reference | 07, 14 | FR-PUB-09 | PUB-009 |
| F-15 | Duplicate content and multiple publication records | 07, 14 | FR-PUB-10 | PUB-010 |
| F-16 | Unpublish and tombstone without deletion promise | 14 | FR-PUB-08, FR-PUB-13 | PUB-008, PUB-013 |
| F-17 | Replica preserves signed uploader attribution | 15 | FR-PUB-11, FR-REP-05 | PUB-011, REP-006 |
| F-18 | Peer identity, encrypted connections, disconnect handling | 09 | FR-NET-01–FR-NET-10 | NET-001–NET-003, NET-005–NET-007 |
| F-19 | Configurable bootstrap peers and manual peer entry | 10 | FR-BOOT-01–FR-BOOT-04 | BOOT-001–BOOT-004 |
| F-20 | DHT peer and record discovery with safe validation | 10 | FR-DISC-01–FR-DISC-05 | DISC-001–DISC-004 |
| F-21 | Explore categories, recent, and random discovery | 16 | FR-DISC-06–FR-DISC-08 | DISC-005, DISC-006 |
| F-22 | Search profiles/files with partial-result honesty | 16 | FR-SEARCH-01–FR-SEARCH-08 | SEARCH-001–SEARCH-006 |
| F-23 | Direct download with chunk and final verification | 12 | FR-DL-01, FR-DL-02, FR-DL-11, FR-DL-15 | DL-001, DL-002, DL-011, DL-014 |
| F-24 | Pause, resume, retry, cancel, restart recovery | 12 | FR-DL-03–FR-DL-06 | DL-003–DL-006 |
| F-25 | Safe paths, disk-space checks, bounded memory | 12 | FR-DL-07–FR-DL-09, FR-DL-13 | DL-007–DL-009, DL-013 |
| F-26 | Download progress UI, managed library, explicit export | 12 | FR-DL-10, FR-DL-12 | DL-010, DL-012 |
| F-27 | Explicit voluntary replication with quotas | 15 | FR-REP-01–FR-REP-10 | REP-001–REP-008 |
| F-28 | Hosted-files screen and verification timestamps | 15 | FR-REP-06 | REP-003 |
| F-29 | Settings for identity, storage, bandwidth, bootstrap, theme, language, diagnostics | 04, 19 | FR-UX-05 | UX-005 |
| F-30 | Network status and redacted diagnostics | 09, 11 | FR-NET-03, FR-UX-06 | NET-005, UX-006 |
| F-31 | UI foundation, navigation, responsive layouts, honest states | 04 | FR-UX-01–FR-UX-04, FR-UX-07–FR-UX-10 | UX-001–UX-004, UX-007–UX-010 |
| F-32 | Accessibility to WCAG 2.2 AA principles | 04, 19 | FR-A11Y-01–FR-A11Y-05 | A11Y-001–A11Y-006 |
| F-33 | Localisation-ready English UI | 04, 19 | FR-L10N-01–FR-L10N-03 | L10N-001–L10N-004 |
| F-34 | Security, privacy, and abuse-resistance baseline | 18 | FR-SEC-01–FR-SEC-11 | SEC-001–SEC-010 |
| F-35 | Versioned protocol and record validation | 02, 07, 18 | FR-SEC-09, FR-SEC-10, FR-PUB-01, FR-PUB-02 | SEC-010, PUB-001, PUB-002 |
| F-36 | Performance, memory, battery, and durability targets | 19 | FR-NFR-01–FR-NFR-10 | NFR-001–NFR-010 |
| F-37 | Signed release artifacts, installers, checksums, release docs | 19 | FR-REL-01–FR-REL-06 | REL-001–REL-006 |
| F-38 | Local metadata backup/export/restore | 05 | FR-DATA-06 | DATA-007 |
| F-39 | Safe previews and no auto-execution | 14, 18 | FR-PUB-12, FR-SEC-04, FR-DL-12 | PUB-012, SEC-005, DL-012 |
| F-40 | Cross-platform identity, signature, content-ID consistency | 06, 07, 19 | FR-ID-07, FR-HASH-04, FR-NFR-10 | PLAT-003, PLAT-004, PLAT-005, NFR-010 |
| F-41 | Background and transfer lifecycle handling | 09, 12 | FR-NET-04, FR-DL-13 | NET-004, DL-004, DL-013 |
| F-42 | Public-visibility and anonymity-limits disclosure | 06, 18 | FR-SEC-07, FR-ONB-01 | SEC-008, ONB-001 |
| F-43 | Community bootstrap/relay operations guidance | 10, 20 | FR-BOOT-05 | REL-006 |
| F-44 | Protocol, governance, and known-limitations documentation | 20 | FR-REL-06 | REL-006 |
| F-45 | Local-first offline behaviour | 04, 05 | FR-ONB-03, FR-NFR-08 | ONB-003, NFR-009 |
| F-46 | Actionable error handling and press diagnostics | 04, 18 | FR-UX-08, FR-NET-03 | UX-008, NET-005 |
| F-47 | Data durability across restart/update | 05, 19 | FR-DATA-07, FR-REL-02 | DATA-001, DATA-003, REL-002 |
| F-48 | Release quality gate: no blockers, full test evidence | 19 | FR-REL-03–FR-REL-05 | REL-003–REL-005, NFR-005 |

**P0 rule:** The matrix is not a schedule. Stages may implement P0 capabilities in dependency order, but v1 cannot ship with any P0 row untested. A P1 feature may not be used to justify delaying a P0 row.

---

## P1 — Later global-network release

| ID | Capability | Roadmap stage | Why later |
|---|---|---|---|
| F-101 | Multi-peer chunk downloads, source scoring, cross-peer resume | 17 | v1 downloads from one verified source at a time with sequential failover; multi-source scheduling needs adversarial testing. |
| F-102 | NAT traversal, hole punching, relay fallback, relay status UI | 11 | v1 reports direct-connection failure honestly; optional community relays need operational guidance and bandwidth accounting. |
| F-103 | Larger-scale distributed indexing and advanced Explore ranking | 16 | v1 provides basic search/explore; scale needs index-provider discovery, freshness, and abuse testing. |
| F-104 | Battery-aware and metered-network controls | 15 | v1 respects platform network signals and user bandwidth settings; deeper controls need device testing. |
| F-105 | Additional languages and complete RTL localisation | 19 | v1 is English, localisation-ready; translation and review follow. |
| F-106 | Key rotation, multiple local identities, device management | 06, 20 | v1 documents limitations; rotation needs signed continuity records and migration rules. |
| F-107 | Local blocking, community reporting, moderation tooling | 18 | v1 has safe defaults and no central authority; community mechanisms need governance design. |
| F-108 | Windows ARM64 build | 19 | x64 is v1 baseline; ARM64 needs toolchain and device coverage. |
| F-109 | Broader safe preview formats | 14 | v1 previews only formats that can be handled safely. |
| F-110 | Opt-in crash reporting and privacy-preserving network health metrics | 19, 20 | v1 has no telemetry by default; opt-in design needs consent and redaction review. |
| F-111 | Independent client interoperability test suite | 20 | Follows a published protocol and stable release. |
| F-112 | Protocol governance and upgrade tooling | 20 | Follows v1 protocol documentation and community operation. |
| F-113 | Advanced Sybil/spam resistance and peer diversity | 18 | v1 has validation, limits, and cautious trust; advanced mechanisms need network data. |
| F-114 | Automatic bootstrap list updates from multiple independent sources | 10 | v1 supports user-configurable lists; automatic updates need authenticity and anti-censorship design. |
| F-115 | Android share sheet and Windows shell integration | 19 | Convenience, not release-blocking. |
| F-116 | Portable Windows package | 19 | Installer is v1; portable packaging is optional. |
| F-117 | Relay/operator tooling and cost measurement | 20 | Depends on relay protocol and community operations. |

---

## P2 — Optional / opportunistic

| ID | Capability | Note |
|---|---|---|
| F-201 | Independent web client | Not a reduced wrapper of the native apps; only if protocol support is mature. |
| F-202 | Media streaming while downloading | Must not bypass verification or safe-preview rules. |
| F-203 | Multiple local profiles in one installation | Identity model complexity; not needed for v1. |
| F-204 | Bandwidth scheduling and time-of-day limits | Convenience after basic limits. |
| F-205 | Tablet/TV-optimised layouts | Responsive phone/desktop layouts are v1. |
| F-206 | Headless/CLI node | Useful for community infrastructure, later. |

---

## Out — Excluded

| ID | Excluded capability | Reason |
|---|---|---|
| F-901 | Private/secret file sharing | Violates PR-01 (all published files public). |
| F-902 | Likes, comments, follows, social feeds | Violates PR-08. |
| F-903 | Company accounts, cloud storage, mandatory backend | Violates PR-02. |
| F-904 | Mandatory or automatic replication | Violates PR-03. |
| F-905 | Guaranteed remote deletion | Violates PR-10; technically impossible across independent peers. |
| F-906 | Real-world identity verification/KYC | NEXUS provides cryptographic identity only; signatures are not legal identity proof. |
| F-907 | Guaranteed anonymity | P2P peers can observe network addresses; the product must not promise anonymity. |
| F-908 | Automatic execution/opening of downloads | Violates PR-11 and security requirements. |
| F-909 | Centralised search, central moderation authority, central file host | Violates PR-02; community/local mechanisms are the direction. |
| F-910 | Cryptocurrency tokens, payments, financial incentives | Outside product scope; adds legal and abuse risk. |
| F-911 | DRM or built-in copyright enforcement | Outside app scope; legal obligations remain with users/operators. |

---

## Release gates

| Gate | Requirement | Evidence |
|---|---|---|
| G1 Capability | All P0 rows implemented and mapped to passing acceptance criteria. | Test report, traceability check. |
| G2 Security | Threat model, malformed-message tests, redaction review, dependency/static analysis. | SEC evidence, no unmitigated high severity. |
| G3 Quality | Performance, accessibility, localisation, compatibility, long-session and interruption tests. | NFR/A11Y/L10N reports. |
| G4 Packaging | Signed Android and Windows artifacts, checksums, install/update/uninstall tests. | REL evidence. |
| G5 Documentation | Protocol, bootstrap/relay operations, known limitations, security disclosure, governance. | Published docs. |
| G6 Beta | Closed alpha/beta on real devices and network environments with no release-blocking issues. | Beta report. |

A gate may be marked "blocked" only with a written issue and owner. It cannot be waived silently.
