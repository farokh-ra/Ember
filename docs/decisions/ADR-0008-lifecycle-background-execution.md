# ADR-0008 — Process, lifecycle, and background execution model

- **Status:** Accepted
- **Date:** 2026-11-12
- **Deciders:** Ember engineering
- **Supersedes:** none
- **Related:** ADR-0001, ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0009

## Context

Ember performs long-running network and file work, but it is not a system daemon. Android restricts background execution and can stop a process at any time. Windows users expect a window to close predictably and may not want a background peer running. The lifecycle design must keep the UI responsive, preserve resumable downloads, avoid false completion, protect keys, and tell the truth about whether the node is online.

The core is a native library inside the app process in v1. A separate service process would add IPC, packaging, and mobile lifecycle complexity that is not justified for the first release.

## Decision

Use **one core instance inside one application process**, owned by a `CoreHost` in the shared Kotlin layer.

### Core states

```text
Created -> Starting -> Running -> Stopping -> Stopped -> Destroyed
                  \-> Failed
```

- `start` and `stop` are idempotent. `destroy` releases the registry handle after `stop`.
- The core refuses to start twice on the same handle. A second process using the same data directory is rejected by a store lock or platform single-instance guard.
- Starting opens the database, runs migrations, unlocks the identity, recovers download state, and starts networking if enabled. Offline startup reaches a usable local screen without waiting for network timeouts.
- Stopping stops accepting new operations, pauses active transfers, persists verified progress, stops libp2p, flushes SQLite, and zeroizes key material.
- A failed start leaves the previous database intact and reports an actionable error. It does not partially delete data.

### Thread ownership

| Thread / pool | Owner | Work |
|---|---|---|
| UI main thread | Compose | Rendering, input, accessibility. No native calls. |
| Kotlin IO dispatcher | Shared core client | FFI calls, import/export chunking, file picker handoff. |
| Event pump | Shared core client | One blocking `next_event` consumer; publishes to `SharedFlow`. |
| Tokio runtime | `nexus-core`/`nexus-net` | Async I/O, timers, libp2p, request/response. |
| SQLite writer | `nexus-store` | One serialized writer connection. |
| SQLite readers | `nexus-store` | Bounded read pool for queries and search. |
| Hashing pool | `nexus-transfer` | At most two concurrent CPU-bound hash jobs, bounded buffers. |

No native callback enters the JVM. No database or network work runs on the UI thread. No unbounded thread creation per operation.

### Android lifecycle

- `NexusApplication` creates the `CoreHost` and starts the core when an identity exists. Onboarding runs before networking if no identity exists.
- The activity binds to the host; configuration changes do not restart the core.
- Active downloads and explicit background hosting use a **foreground service** with a visible notification. The service type is `dataSync`; Android may impose a time limit on long `dataSync` sessions. When the OS stops the service, transfers pause and remain resumable; the app never claims completion.
- The service is started only from a user action or an already-visible active transfer. The app does not start a background service silently at boot.
- If the user leaves the app with no active transfer and no background hosting, the host stops the core after a short grace period. Published files are served only while the node is online.
- The app requests notification permission contextually. Denying notifications does not block identity, profile, publishing, browsing, or settings. A foreground service notification remains visible in system UI where required.
- The app does not request battery-optimization exemption and does not hold wake locks for idle networking.
- Process death loses in-memory state. On next launch, the database and verified partial prefixes are recovered. The UI shows paused downloads, not completed ones.

### Windows lifecycle

- The desktop app owns the core for the lifetime of the JVM process.
- Closing the window while a transfer is active or the user has enabled background hosting hides the app to the system tray and keeps the core running with a visible tray icon and menu.
- Closing the window with no active work exits the process and stops the core. "Exit" in the tray always stops the core.
- The app does not install a login auto-start entry in v1. Background serving is an explicit session choice, not an always-on service.
- Sleep, resume, network changes, and VPN changes are observed by the core. Connections reconnect or report offline; transfers pause and resume from the verified prefix.
- A second launch detects the single-instance guard and reports that Ember is already running; it does not open a second core against the same data.

### Network and transfer lifecycle

- Network state is separate from core state: `Stopped`, `Starting`, `Online`, `Degraded`, `Offline`.
- `Online` means the core is running and at least one usable path or peer is available. `Degraded` means partial connectivity or bounded lookup failures. `Offline` means no usable path. The UI must not display `Online` when the core is stopped.
- Active transfers pause on network loss, sleep, app backgrounding without a foreground service, or quota pressure. They remain resumable.
- On startup, downloads in `Transferring` or `Verifying` are moved to `Paused` after recovery, never to `Completed`.
- Replica verification runs on startup and before serving after a detected local change, and at least every 7 days. A failed verification stops advertising and serving.

### Updates

- Android updates preserve app-private data and identity. The new core runs migrations on first start.
- Windows upgrades preserve `%LOCALAPPDATA%\Ember\data` and the sealed local secret. The installer stops the running app before replacing binaries.
- If a migration or ABI check fails after an update, the app shows a recovery screen with backup/restore and diagnostics. It does not silently reset the identity.
- Downgrades are not supported. A newer database schema is not opened by an older core.

### Core faults

- Rust panics are caught at the FFI boundary and converted to internal errors where possible.
- If the process aborts, the next start performs SQLite WAL recovery, truncates partials to the verified prefix, reconciles the store, and reports the interrupted operations.
- Diagnostics include a redacted crash marker and last operation state, never key material or file paths.

## Consequences

**Positive**

- Predictable lifecycle on both platforms with one core instance.
- Transfers and replicas are honest about foreground/background limitations.
- Crash recovery is designed around durable verified state.
- No always-on daemon, wake lock, or hidden background seeding.
- The core can be tested through 100 start/stop cycles without platform UI.

**Negative / trade-offs**

- Android OEM battery policies may still stop the process; resumability is the mitigation.
- Long `dataSync` sessions may hit OS limits; users may need to resume.
- Windows tray behaviour adds platform code and accessibility testing.
- Published files are not served when the app is fully closed.
- Process death can lose recent unverified progress, which is discarded safely.

**Constraints**

- No background seeding the user did not explicitly enable.
- No claim of completion or availability without a live, verified state.
- No foreground service started at boot.
- No plaintext key material in memory after stop where it can be zeroized.
- No second core against the same store.

## Alternatives considered

1. **Separate native service process.** Strong isolation and independent lifetime, but adds IPC, packaging, lifecycle, and attack surface. Rejected for v1.
2. **Android WorkManager for transfers.** Good for deferrable work, but unsuitable for long, user-visible, bidirectional P2P transfers and foreground progress. Rejected.
3. **Always-on background node.** Better availability, but violates user control, battery expectations, and mobile platform rules. Rejected.
4. **Keep the core running after the Windows window closes without a tray indicator.** Hidden background activity is dishonest. Rejected.
5. **Restart the core on every activity recreation.** Simpler lifecycle code, but loses connections and transfer continuity. Rejected.
6. **Run one core per screen or per operation.** Causes multiple writers and resource leaks. Rejected.

## Verification

- Stage 08 proves 100 create/start/stop/destroy cycles, idempotent stop, ABI mismatch handling, and no orphaned threads.
- Stage 09 proves sleep/wake, Wi-Fi change, and network-drop recovery.
- Stage 12 proves process-death recovery from the verified prefix and no false completion.
- Stage 17 proves Android foreground-service behaviour, notification denial, and Windows tray/exit behaviour.
- Stage 19 runs a 24-hour session and measures idle battery, memory, and background chatter.
