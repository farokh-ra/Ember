# Ember Project Handoff

**Current stage:** 11 - NAT traversal, relays and difficult networks
**Stage status:** Complete
**Last updated:** 2026-11-19
**Branch:** `feat/11-nat-traversal`
**Next stage:** 12 - streaming transfer, chunk verification, pause/resume, and process-death recovery (waiting for user permission)

## What was implemented

Stage 11 adds the difficult-network layer required by ADR-0005 and ADR-0007
on top of the Stage 09 transports and the Stage 10 DHT:

- A documented NAT traversal and relay design in
  `docs/architecture/NAT_TRAVERSAL.md`.
- Circuit Relay v2 through the pinned libp2p `0.57.1` `relay` feature. Ember is
  a relay client by default and a bounded relay server only when configured.
- DCUtR hole punching through the `dcutr` feature.
- AutoNAT v1 reachability through the `autonat` feature.
- Direct-first connection fallback to connected relays with bounded deadlines.
  A failed direct dial tries each connected relay in order; exhausted
  candidates return `NoViablePath`.
- Relay reservations with a 30 s default deadline and a 5 s retry after a
  refused, closed, or timed-out reservation.
- Optional community relay nodes with operator guidance in
  `docs/operations/RELAY_NODES.md`.
- Relay lists merged at startup from compile-time `EMBER_RELAY_NODES`, runtime
  configuration, and persisted `network/relays.list`, deduplicated and bounded
  to 16 entries.
- Four new core methods: `network.relay_list` (0x060A),
  `network.relay_add` (0x060B), `network.relay_remove` (0x060C), and
  `network.connectivity` (0x060D).
- NAT, relay, path, fallback, hole-punch, and relay-server fields in
  `network.status`, `network.peers`, `network.connect_*`, and
  `network.peer_event`.
- Connectivity classification `disabled`, `connecting`, `relay`, `direct`, or
  `no_peers`, with a troubleshooting hint that distinguishes missing relays,
  disconnected relays, missing reservations, and a ready-but-idle path.
- A demo-only Network status section for relay usage, NAT reachability, path
  counters, circuit limits, and troubleshooting. It remains visibly simulated.
- 15 new Rust tests, for 229 total.

## Files changed

Network crate:

- `crates/nexus-net/src/behaviour.rs`
- `crates/nexus-net/src/config.rs`
- `crates/nexus-net/src/driver.rs`
- `crates/nexus-net/src/error.rs`
- `crates/nexus-net/src/lib.rs`
- `crates/nexus-net/src/relay.rs` (new)
- `crates/nexus-net/src/shared.rs`
- `crates/nexus-net/src/state.rs`
- `crates/nexus-net/tests/stage09.rs`
- `crates/nexus-net/tests/stage11.rs` (new)

Core crate:

- `crates/nexus-core/src/config.rs`
- `crates/nexus-core/src/engine.rs`
- `crates/nexus-core/src/network.rs`
- `crates/nexus-core/tests/stage09.rs`
- `crates/nexus-core/tests/stage10.rs`
- `crates/nexus-core/tests/stage11.rs` (new)

FFI and workspace:

- `crates/nexus-ffi/src/registry.rs`
- `Cargo.toml`, `Cargo.lock`

Shared Kotlin:

- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoModels.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoCatalog.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/resources/NexusStrings.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/screens/NetworkStatusScreen.kt`

Documentation:

- `docs/architecture/NAT_TRAVERSAL.md` (new)
- `docs/operations/RELAY_NODES.md` (new)
- `docs/architecture/FFI_INTERFACE.md` (version 1.4.0)
- `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md` (version 1.8.0)
- `docs/progress/STAGE_11.md` (new)
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

## Commands and results

### Rust tests

```text
cargo test --workspace -- --test-threads=1
```

Result: PASS, 229 tests, 0 failures.

Stage 11-specific suites:

```text
cargo test -p nexus-net --test stage11 -- --test-threads=1
cargo test -p nexus-core --test stage11 -- --test-threads=1
```

Result: PASS, 5 and 4 tests respectively.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 28 | 0 |
| `nexus-core` Stage 09 tests | 4 | 0 |
| `nexus-core` Stage 10 tests | 2 | 0 |
| `nexus-core` Stage 11 tests | 4 | 0 |
| `nexus-crypto` unit tests | 24 | 0 |
| `nexus-ffi` unit tests | 4 | 0 |
| `nexus-ffi` C ABI tests | 6 | 0 |
| `nexus-identity` unit tests | 9 | 0 |
| `nexus-identity` integration tests | 12 | 0 |
| `nexus-net` unit tests | 22 | 0 |
| `nexus-net` Stage 09 tests | 8 | 0 |
| `nexus-net` Stage 10 tests | 4 | 0 |
| `nexus-net` Stage 11 tests | 5 | 0 |
| `nexus-observability` unit tests | 3 | 0 |
| `nexus-protocol` unit tests | 25 | 0 |
| `nexus-protocol` signed-record tests | 9 | 0 |
| `nexus-protocol` Stage 07 tests | 8 | 0 |
| `nexus-store` unit tests | 17 | 0 |
| `nexus-store` local-import tests | 4 | 0 |
| `nexus-store` Stage 07 tests | 4 | 0 |
| `nexus-store` storage tests | 12 | 0 |
| `nexus-transfer` unit tests | 7 | 0 |
| `nexus-transfer` integration tests | 7 | 0 |
| `nexus-types` unit tests | 1 | 0 |
| **Rust total** | **229** | **0** |

### Kotlin checks

```text
gradlew spotlessCheck detekt :shared:presentation:jvmTest :shared:ui:jvmTest
```

Result: PASS.

### Formatting, linting, and dependency policy

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
cargo audit
```

- Formatting and Clippy with warnings denied: PASS.
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`. One
  unmaintained, non-vulnerable Linux-only `paste` advisory
  (`RUSTSEC-2024-0436`) remains documented in `deny.toml`.
- `cargo audit`: 411 dependencies scanned; no vulnerabilities and one allowed
  warning.
- `Cargo.lock` resolves 411 packages; the dependency inventory is at version
  1.8.0. Stage 11 adds `libp2p-autonat` 0.16.0, `libp2p-dcutr` 0.15.0, and
  `libp2p-relay` 0.22.0. `libp2p-upnp` 0.7.0 is present in the optional graph
  but is not enabled.
- The native artifact build was not re-run because the ABI and exported surface
  did not change. The six C ABI tests still pass in the workspace suite.

## Errors and failed attempts

1. Relay serving requires a confirmed external address. Without one, libp2p
   does not advertise HOP and loopback reservations fail. The driver now
   registers operator `external_addresses` and auto-advertises loopback
   listeners when relay serving is enabled.
2. Clippy rejected the first relay configuration with
   `field_reassign_with_default`, two `collapsible_if` cases, two
   `useless_conversion` cases, and two `too_many_arguments` functions. The
   configuration uses struct initialization, collapsed conditions, direct
   `as_secs()`, and two documented internal allows.
3. DCUtR can promote a circuit to a direct connection immediately after the
   connect event. The core relay test accepts a relay or direct path in the
   snapshot while still proving fallback through the connect event and relay
   status.
4. Deterministic relay-fallback measurements required disabling DCUtR and
   AutoNAT in the test fixtures. Production defaults leave both enabled.
5. `libp2p-relay` does not export its `Limit` type. The driver reads
   `limit.duration()` and `limit.data_in_bytes()` inline.
6. The Stage 09 transport assertion expected exactly `quic` and `tcp`. A relay
   client now also advertises `relay`; the assertion was updated to
   `["quic", "tcp", "relay"]`.
7. `network.connect_failed` initially lost the typed error and emitted only a
   string. The driver now preserves `NetError`, and the core renders the
   `code`/`message`/`retryable` map.

## Decisions and unresolved questions

- Stage 11 was explicitly approved by the project owner.
- Circuit Relay v2 is the relay mechanism. No TURN or custom relay protocol is
  added.
- DCUtR and AutoNAT v1 are enabled by default. Relay serving is off by default.
- Direct dials always run first. Relay fallback is sequential and bounded; the
  node does not race relay and direct dials.
- A relay is optional infrastructure, not an authority. It forwards
  Noise-encrypted bytes and cannot read content.
- The relay list merges compiled, configured, and persisted sources and is
  bounded to 16 entries.
- Connectivity reports `relay` when any data connection is relayed, even if a
  direct connection to relay infrastructure exists.
- UPnP/NAT-PMP port mapping is deferred. `libp2p-upnp` is not enabled.
- Raw relayed byte accounting is not exposed by libp2p. Negotiated circuit
  limits and circuit counters are the supported cost signal.
- The Kotlin troubleshooting UI remains labeled demo data; core wiring is a
  later UI stage.
- The ABI remains 1; the protocol version is `0.1.0-stage-11`.
- Android runtime relay evidence is deferred because no emulator or device is
  available.
- Remote GitHub Actions execution has not run for Stage 11.
- PR #9 (Stage 10) remains open on `main`; Stage 11 is based on the Stage 10
  head and should be rebased after PR #9 merges.

## Exact next action

1. Review `docs/progress/STAGE_11.md`,
   `docs/architecture/NAT_TRAVERSAL.md`,
   `docs/operations/RELAY_NODES.md`, the updated
   `docs/architecture/FFI_INTERFACE.md`, and the `nexus-net`/`nexus-core`
   Stage 11 tests.
2. Give explicit approval to start Stage 12.
3. Stage 12 adds streaming transfer, chunk verification, pause/resume, and
   process-death recovery. Do not begin it before that approval.
