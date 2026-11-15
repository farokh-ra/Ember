# ADR-0002 — Rust core owns networking, identity, storage, and transfer

- **Status:** Accepted
- **Date:** 2026-11-12
- **Deciders:** Ember engineering
- **Supersedes:** none
- **Related:** ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006

## Context

Ember needs one shared P2P implementation on Android and Windows. The roadmap requires a Rust core using libp2p, with identity, storage, transfer, verification, and replication shared across platforms. Kotlin/Compose should own UI and presentation, not duplicate networking or database logic.

Two runtimes create risks: divergent state, duplicate writers, inconsistent crypto, blocking the UI, and unsafe FFI. The boundary must be small, versioned, and testable.

## Decision

The **Rust core is authoritative** for:

- Identity key generation, signing, verification, and encrypted backup.
- Canonical record and message serialization.
- SQLite metadata and migrations.
- Managed file store, quotas, path safety, and reference counting.
- libp2p node lifecycle, transports, peer identity, connections, bootstrap, DHT, and peer discovery.
- Publication/profile record exchange.
- Hashing, chunk proofs, downloads, pause/resume, replication, and verification.
- Search/index read models derived from verified local records.
- Network diagnostics and redacted logging.

The **Kotlin/Compose layer** owns:

- Screens, navigation, themes, accessibility semantics, and presentation state.
- User interactions and confirmation flows.
- Platform integration: file pickers, streaming file bytes to/from the core, notifications, foreground service, window management, and sharing/export destinations.
- Calling `NexusCore` and rendering events/queries.
- Local ephemeral UI caches that can always be rebuilt from the core.

**Rules**

1. Kotlin must not open the Ember SQLite database or managed store directly.
2. Kotlin must not implement networking, identity, hashing, record parsing, or transfer.
3. Rust must not render UI or make product-policy decisions that belong to the presentation layer; it enforces invariants and returns structured state/errors.
4. All durable state changes go through one core instance.
5. The UI may read cached state offline only through core queries; the core remains the source of truth.
6. No central backend is introduced at the boundary.

## Consequences

**Positive**

- One implementation of security-critical and protocol-critical behaviour.
- Cross-platform vectors can prove identical signatures, content IDs, and records.
- Storage has a single writer; no Kotlin/Rust race.
- UI can be tested against a fake `NexusCore` without networking.
- The core can be fuzzed, benchmarked, and integration-tested independently.

**Negative / trade-offs**

- Every UI operation crosses an FFI boundary; the API must be designed carefully.
- Streaming file import/export needs explicit APIs to avoid loading large files into memory.
- Rust panic/error handling must never cross the ABI unchecked.
- Kotlin cannot use platform database conveniences; it depends on core queries.
- Debugging spans two runtimes.

**Constraints**

- One core instance per application process in v1.
- All long-running work is asynchronous, cancellable, and off the UI thread.
- FFI exposes stable operations, not arbitrary internal modules.
- The core must remain usable offline for local records and downloads.

## Alternatives considered

1. **Kotlin owns networking and SQLite; Rust is a small crypto library.** Easier UI integration, but duplicates platform networking, weakens the shared-core goal, and creates two protocol implementations. Rejected.
2. **Rust core in a separate process/service.** Strong isolation, but adds IPC lifecycle, mobile background complexity, packaging, and attack surface. Rejected for v1; may be revisited for a headless node.
3. **Central backend for identity/discovery/storage.** Violates product rule PR-02. Rejected.
4. **Rust owns UI via a cross-platform framework.** Does not fit Kotlin/Compose and would reduce platform accessibility. Rejected.

## Verification

- Stage 08 proves Android and Windows can initialize, call, and stop the same core API.
- Cross-platform test vectors prove identity, signatures, content IDs, and records.
- Stage 05/12 tests prove Kotlin has no direct database/file-store writes.
- Stage 19 long-session and memory tests prove the boundary does not leak or freeze the UI.
