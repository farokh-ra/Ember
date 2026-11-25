# Stage 02 — Architecture and Technical Decision Records

**Status:** Complete
**Completed:** 2026-11-12
**Progress guide:** 4–9%
**Next stage:** 03 — Repository, build system and developer workflow (requires user permission)

## 1. Scope

Stage 02 defines the architecture for the first Ember release and records the decisions that are expensive to reverse. It produces design documents only; no application, Rust, or network code is created in this stage.

The stage covers:

- Kotlin Multiplatform and Compose Multiplatform for shared UI and presentation.
- Rust as the shared P2P, identity, storage, and transfer core.
- The Kotlin-to-Rust FFI approach for Android and Windows.
- Core start, stop, update, monitoring, and failure behaviour.
- Process, thread, and lifecycle ownership.
- SQLite metadata versus managed file-byte storage.
- Protocol boundaries between identity, profiles, discovery, transfer, replication, and search.
- Architecture decision records for major choices.
- Fallback choices if a library does not work on both target platforms.
- A direct dependency and license inventory.

## 2. Deliverables

| Deliverable | Path | Status |
|---|---|---|
| Architecture document | `docs/ARCHITECTURE.md` | Complete |
| ADR index and rules | `docs/decisions/README.md` | Complete |
| ADR-0001 shared UI | `docs/decisions/ADR-0001-kotlin-compose-multiplatform.md` | Accepted |
| ADR-0002 Rust core boundary | `docs/decisions/ADR-0002-rust-core-boundary.md` | Accepted |
| ADR-0003 FFI | `docs/decisions/ADR-0003-ffi-c-abi-jni-jna.md` | Accepted |
| ADR-0004 storage ownership | `docs/decisions/ADR-0004-rust-storage-ownership.md` | Accepted |
| ADR-0005 networking | `docs/decisions/ADR-0005-libp2p-transports-discovery.md` | Accepted |
| ADR-0006 crypto and records | `docs/decisions/ADR-0006-crypto-and-record-formats.md` | Accepted |
| ADR-0007 protocol boundaries | `docs/decisions/ADR-0007-protocol-boundaries-versioning.md` | Accepted |
| ADR-0008 lifecycle | `docs/decisions/ADR-0008-lifecycle-background-execution.md` | Accepted |
| ADR-0009 build and licensing | `docs/decisions/ADR-0009-build-packaging-dependency-policy.md` | Accepted |
| ADR-0010 search boundary | `docs/decisions/ADR-0010-search-index-boundary.md` | Accepted |
| Component diagrams | `docs/architecture/COMPONENT_DIAGRAMS.md` | Complete |
| FFI interface draft | `docs/architecture/FFI_INTERFACE.md` | Complete |
| C ABI header draft | `docs/architecture/nexus_core.h` | Complete |
| Dependency and license inventory | `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md` | Complete |
| Stage validator | `scripts/validate_stage02_docs.py` | Complete |
| Stage record | `docs/progress/STAGE_02.md` | Complete |
| Handoff | `docs/HANDOFF.md` | Updated |

## 3. Key decisions

| Area | Decision |
|---|---|
| Shared UI | Kotlin Multiplatform with Compose Multiplatform; native Android and Windows shells. |
| Core boundary | Rust owns identity, protocol, SQLite, managed store, networking, transfer, replication, and search. Kotlin owns UI and platform integration. |
| FFI | One versioned C ABI; JNI on Android, JNA on Windows; CBOR envelopes; event queue; no callbacks. |
| Storage | Rust is the only writer; SQLite metadata plus a content-addressed managed store; encrypted keys outside SQLite. |
| Networking | libp2p 0.57 with QUIC and TCP + Noise + Yamux; Kademlia DHT; multi-operator bootstrap; no relay or mDNS in v1. |
| Identity | Ed25519 user identity; separate device peer identity; Bech32m `nx1...` identity IDs. |
| Records | Canonical CBOR bodies with exact signed bytes; domain-separated signatures; 64 KiB record cap. |
| Integrity | SHA-256 content IDs; 1 MiB chunks; flat manifest with a domain-separated root; 4 GiB max file. |
| Lifecycle | One core per process; user-started Android foreground service; Windows tray; no always-on daemon. |
| Packaging | Gradle, Kotlin, Compose, Rust; Android APK/AAB; Windows app image with an Inno Setup per-user installer. |
| Search | Verified local FTS5 index plus optional bounded peer index providers; no central search service. |
| Dependencies | Pin direct versions; lockfiles; cargo-deny, cargo-audit, Gradle license report, CycloneDX SBOM. |

## 4. Exit-criteria evidence

| Exit criterion | Evidence |
|---|---|
| Every major component has one responsibility. | `ARCHITECTURE.md` section 4 assigns a single responsibility and explicit exclusions to seven Kotlin modules and twelve Rust crates. The validator checks both tables. |
| Android and Windows can use the same Rust core API. | `FFI_INTERFACE.md` defines one C ABI and one method catalog; JNI and JNA are adapters. The validator checks the catalog and header. Runtime proof is scheduled for Stage 08. |
| No core feature requires a centralized company backend. | `ARCHITECTURE.md` section 3, ADR-0005, ADR-0009, and ADR-0010 define optional community bootstrap and index peers, no account service, no central search, no default telemetry, and no mandatory update service. |

The second and third criteria are design-complete here and receive runtime evidence in Stages 08–10, as the roadmap schedules implementation after this stage.

## 5. Toolchain research

Versions were checked against the registries on 2026-11-12 and pinned for Stage 03. The exact patch versions and transitive graphs are locked in Stage 03.

| Tool | Pin |
|---|---|
| Kotlin | 2.4.21 |
| Compose Multiplatform | 1.12.2 |
| Android Gradle Plugin | 9.5.2 |
| Gradle | 9.9.2 |
| Rust | 1.99.1 |
| Android NDK | 29.0.14206865 |
| JDK | Temurin 21 LTS |
| Android compile and target SDK | 37 |
| Android minimum SDK | 28 |
| Inno Setup | 6.4.3 |
| libp2p | 0.57.1 |
| rusqlite | 0.42.0 |
| JNA | 5.19.1 |

Current machine inspection:

- Android SDK exists with platforms 34, 36, 36.1, and 37.0; build-tools 34.0.0 and 36.0.0; no NDK.
- Visual Studio 18 directory exists, but `cl`, `link`, `cmake`, `ninja`, and `msbuild` are not on `PATH`, and no Windows SDK include directory was found.
- Java, Gradle, Cargo, and Rust are not on `PATH`.
- These gaps are assigned to Stage 03. Stage 02 does not claim a build.

## 6. Validation

Command:

```text
python scripts/validate_stage02_docs.py
```

Actual result:

```text
STAGE 02 ARCHITECTURE VALIDATION
  Required documents: 18/18 present
  ADRs checked: 10 (accepted: 10)
  Kotlin modules: 7
  Rust components: 12
  FFI methods: 46
  FFI C functions in header: 17
  Rust dependencies inventoried: 26
  Kotlin and JVM dependencies inventoried: 14
  Local markdown links checked: yes
  Markdown tables checked: yes

STAGE 02 VALIDATION: PASS
```

The Stage 01 validator is also re-run because Stage 02 adds documents under `docs/` and changes links.

## 7. Errors and corrections

1. The Maven Central search endpoint timed out during version research. Direct repository metadata was used instead; the pinned versions reflect the repository metadata at the time of research.
2. One crate's current version metadata did not expose a usable license value. That crate was removed from the direct inventory rather than recorded with an unknown license.
3. The ADR-0006 title initially said "Merkle chunks" while the accepted design uses a flat manifest-authenticated chunk list. The title was corrected to match the decision.
4. The Stage 02 validator was written to catch missing ADR sections, duplicate FFI method IDs, disallowed licenses, missing header functions, and table-width errors before the stage was marked complete.

## 8. Known limitations and deferred decisions

- No build or runtime test exists yet. Stage 03 installs the missing toolchain and proves a clean-checkout build.
- Exact bootstrap operator addresses are release configuration for Stage 10.
- Index provider discovery and shard layout are designed in Stage 16.
- Exact SQLite tables and indexes are designed in Stage 05.
- Argon2id parameters are calibrated on baseline devices in Stage 06.
- Android OEM foreground-service behaviour and Windows tray behaviour are tested in Stage 17/19.
- Signing certificates and release credentials are release operations for Stage 19.
- Relay and advanced NAT traversal are Stage 11 work and do not change the v1 protocol silently.

## 9. Next stage

Stage 03 — Repository, build system and developer workflow — begins only after explicit user permission. It will:

1. Initialize Git and the module structure.
2. Install or configure the pinned JDK, Gradle, Rust, NDK, and Windows build tools.
3. Create buildable empty Android and Windows applications and a Rust library.
4. Add formatting, linting, static analysis, CI, dependency locking, and vulnerability scanning.
5. Prove a clean checkout can be built with documented steps.
