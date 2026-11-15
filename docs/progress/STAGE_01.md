# Stage 01 — Product definition and requirements

**Current stage:** 01 — Product definition and requirements
**Stage status:** Complete
**Completed on:** 2026-11-12

## Exit criteria check

| Exit criterion | Result | Evidence |
|---|---|---|
| No contradictory product rules remain | Pass | PRD consistency review; P0 out-of-scope scan returned zero hits; manual review fixed node/peer online terminology, reference-counted deletion, tombstone replica behaviour, and missing category. |
| Every first-release feature has a testable acceptance criterion | Pass | 48 P0 feature rows checked; 141 acceptance criteria defined; every P0 row references at least one defined criterion. |
| Platform targets are written down | Pass | Android 9 / API 28 minimum, arm64; Windows 10 22H2 and Windows 11 23H2+ x64; baseline hardware and network environments documented. |
| Deliverables exist and are documented | Pass | PRD, glossary, user flows, feature priority matrix, acceptance criteria, validator, stage record, handoff. Git commit is deferred to Stage 03 because the roadmap assigns repository creation to that stage. |

## Tasks completed

- [x] Read the master roadmap and inspected the workspace (only the roadmap file existed).
- [x] Formalised the product vision and 12 non-negotiable product rules.
- [x] Defined v1 scope, later scope, and explicit exclusions.
- [x] Defined Android and Windows platform targets, architectures, and baseline hardware.
- [x] Defined canonical product vocabulary and eight semantic invariants.
- [x] Defined 139 first-release functional requirements with IDs.
- [x] Defined 10 user journeys with Mermaid diagrams, alternate paths, and error paths.
- [x] Defined data/file limits, categories, quota, duplicate-content, and retention rules.
- [x] Defined accessibility, language, and localisation requirements.
- [x] Defined security, privacy, and safety requirements.
- [x] Defined non-functional and release-engineering requirements.
- [x] Created the feature priority matrix: 48 P0, 17 P1, 6 P2, 11 excluded.
- [x] Created 141 testable acceptance criteria mapped to P0 features.
- [x] Added and passed a documentation validator for IDs, traceability, tables, links, and stub markers.

## Files/modules changed

| File | Lines | SHA-256 |
|---|---:|---|
| `docs/PRODUCT_REQUIREMENTS.md` | 682 | `330644558AD96698C6440666751E513E6097FA3D67E9C9184E5BC8C6DCDFDDB1` |
| `docs/product/GLOSSARY.md` | 191 | `78EAC22FF4B1E06BB2930C13E6DD7D1037B400BAA16B8D1F6033429132880192` |
| `docs/product/USER_FLOWS.md` | 429 | `4F5CE72892D6EA55AAC50FD7CC710080BAC33D3259B601E1C16AE85C70601562` |
| `docs/product/FEATURE_PRIORITY_MATRIX.md` | 147 | `B8DBB6384C1C8A5DDB9C8CE342F328337070CB13937DC71F1D65606914F3A5EA` |
| `docs/product/ACCEPTANCE_CRITERIA.md` | 400 | `2317BCFB3458A8CFCB26C0D0F44B649C2E68653CF28BBAF57417DAF3290C05F4` |
| `scripts/validate_stage01_docs.py` | 286 | `3CF542369B85EFD6395B2D354B2BEDF4928BA25086008B5E90CFFCC8036F70B9` |
| `docs/progress/STAGE_01.md` | this file | updated after validation |
| `docs/HANDOFF.md` | handoff | updated after validation |

## Builds

- **Android:** not applicable — Stage 01 produces no app code. Build setup starts in Stage 03.
- **Windows:** not applicable — Stage 01 produces no app code. Build setup starts in Stage 03.
- **Rust:** not applicable — Stage 01 produces no core code. Build setup starts in Stage 03.

## Tests

### Passed

Command:

```text
python scripts/validate_stage01_docs.py
```

Actual output:

```text
STAGE 01 DOCUMENTATION VALIDATION
  Required documents: 7/7 present
  Acceptance criteria defined: 141 (unique: True)
  Acceptance criteria referenced by PRD/matrix: 141
  Functional requirements defined: 139 (unique: True)
  Functional requirements referenced by matrix/flows: 137
  P0 feature rows checked: 48
  Local markdown links checked: yes
  Markdown tables checked: yes

STAGE 01 VALIDATION: PASS
```

Additional manual checks:

- P0 out-of-scope scan for `private`, `like`, `comment`, `follow`, `mandatory replication`, `remote deletion`, `auto-execute`, `central`: 0 hits.
- Manual consistency review fixed ambiguous node/peer online status, reference-counted deletion, replica behaviour after a tombstone, missing "Other" category, and avatar/manifest record-size boundaries.
- Feature matrix counts verified: 48 P0, 17 P1, 6 P2, 11 Out.

### Failed then fixed

- Initial validator run flagged `DISC-007`, `DISC-008`, and `DL-015` references that had no definitions. These were incorrect range endpoints in the user flows; ranges corrected to `DISC-001–DISC-006` and `DL-001–DL-014`.
- Initial validator had two parsing bugs (P0 table column count and FR table column count). Both were fixed before the final pass.

### Not run

- Android, Windows, and Rust builds; application tests; network tests; performance tests. These belong to later implementation stages and require the Stage 03 toolchain setup.

## Known issues

- The current machine has an Android SDK but no JDK, Gradle, Cargo, or Rust on `PATH`. Stage 03 must install/configure the toolchain before app/core builds.
- The workspace is not yet a Git repository. Stage 03 creates it and will commit the Stage 01 documents.
- Technical choices intentionally deferred: identity ID encoding, canonical serialisation format, chunk size/manifest format, DHT record keys, FFI mechanism, and SQLite column types. These are implementation decisions for Stages 02–07 and do not change product behaviour.
- No app or network functionality exists yet; no screen or runtime claim is implied by the mock-flow documentation.

## Decisions made

- v1 is the first public release; P1 capabilities do not block it.
- v1 downloads from one verified source at a time with sequential failover; simultaneous multi-peer scheduling is later.
- Originals, replicas, downloads, and partials live in the managed store; export is explicit and user-initiated.
- Publishing imports a managed copy; Ember never serves arbitrary external paths and never modifies the external original.
- v1 has no telemetry or crash reporting by default.
- v1 has no automatic key rotation or central recovery; limitations are disclosed.
- Record existence, record verification, and content verification are separate UI states.
- Unpublish is a tombstone, never a remote-delete promise. Independent replicas may continue serving with an unpublished warning.
- Content deletion is reference-counted so shared bytes cannot be removed while a publication or replica still needs them.
- Search/Explore may return partial/cached results and must say so; no central search service is required.

## Next task

**Do not start Stage 02 without user permission.** When approved, Stage 02 will produce `docs/ARCHITECTURE.md`, ADRs, component diagrams, the FFI interface draft, and the dependency/license inventory.

## Evidence

- Final validator output above (PASS).
- File hashes and line counts recorded in the table above.
- Stage 01 deliverables: `docs/PRODUCT_REQUIREMENTS.md`, `docs/product/GLOSSARY.md`, `docs/product/USER_FLOWS.md`, `docs/product/FEATURE_PRIORITY_MATRIX.md`, `docs/product/ACCEPTANCE_CRITERIA.md`.
- No Android/Windows/Rust build output exists at this stage by design.
