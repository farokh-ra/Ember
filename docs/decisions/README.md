# NEXUS Architecture Decision Records

| Field | Value |
|---|---|
| Document ID | NEXUS-ADR-INDEX |
| Version | 1.0.0 |
| Status | Accepted |
| Date | 2026-11-12 |
| Normative source | [Architecture](../ARCHITECTURE.md), [Product requirements](../PRODUCT_REQUIREMENTS.md) |

Architecture Decision Records (ADRs) capture decisions that are expensive to reverse. An ADR is accepted only when its consequences and alternatives are recorded.

| ADR | Title | Status | Area |
|---|---|---|---|
| [ADR-0001](ADR-0001-kotlin-compose-multiplatform.md) | Kotlin Multiplatform and Compose Multiplatform for shared UI | Accepted | UI/platform |
| [ADR-0002](ADR-0002-rust-core-boundary.md) | Rust core owns networking, identity, storage, and transfer | Accepted | Core boundary |
| [ADR-0003](ADR-0003-ffi-c-abi-jni-jna.md) | Stable C ABI with JNI and JNA, versioned CBOR, event queue | Accepted | FFI |
| [ADR-0004](ADR-0004-rust-storage-ownership.md) | Rust owns SQLite and the managed file store | Accepted | Data |
| [ADR-0005](ADR-0005-libp2p-transports-discovery.md) | libp2p transports and discovery stack | Accepted | Networking |
| [ADR-0006](ADR-0006-crypto-and-record-formats.md) | Ed25519, SHA-256, manifest-authenticated chunks, canonical CBOR | Accepted | Crypto/protocol |
| [ADR-0007](ADR-0007-protocol-boundaries-versioning.md) | Protocol boundaries and versioning rules | Accepted | Protocol |
| [ADR-0008](ADR-0008-lifecycle-background-execution.md) | Process, lifecycle, and background execution model | Accepted | Runtime |
| [ADR-0009](ADR-0009-build-packaging-dependency-policy.md) | Build, packaging, and dependency/license policy | Accepted | Build/release |
| [ADR-0010](ADR-0010-search-index-boundary.md) | Search and Explore use a verified local index, never a central service | Accepted | Discovery/search |

## ADR rules

1. One decision per ADR. Do not rewrite history; supersede with a new ADR.
2. An accepted ADR must include context, decision, consequences, alternatives, and verification.
3. A decision that changes product behaviour also requires a PRD change.
4. Platform parity is a review criterion for every ADR.
5. No ADR may introduce a mandatory company-operated backend.
6. Security-sensitive decisions must state their failure mode and trust boundary.
