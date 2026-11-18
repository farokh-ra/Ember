# Ember Project Handoff

**Current stage:** 10 - Bootstrap, DHT and decentralized peer discovery
**Stage status:** Complete
**Last updated:** 2026-11-18
**Branch:** `feat/10-dht`
**Next stage:** 11 - NAT traversal, relays, and difficult networks (waiting for user permission)

## What was implemented

Stage 10 adds the decentralized discovery layer required by ADR-0005 and
ADR-0007 on top of the Stage 09 transports:

- A documented Kademlia record and routing design in
  `docs/architecture/DHT_DESIGN.md`.
- libp2p Kademlia `/nexus/kad/1.0.0` through the pinned libp2p `0.57.1`
  `kad` feature. Replication factor 8, query parallelism 3, 80 KiB frame cap.
- Client and server DHT modes. Ordinary devices default to `client`; bootstrap
  entry points run `server`.
- Exact signed-envelope DHT values for publications, tombstones, and profiles,
  with key-derived verification and monotonic revision rules.
- Bounded in-memory records and providers: 1,024 records, 64 KiB values,
  20 providers per key, 512 locally advertised provider keys, 24 h TTL, 12 h
  republication.
- Provider discovery for raw 32-byte content IDs.
- Bootstrap entry points merged at startup from compile-time
  `EMBER_BOOTSTRAP_PEERS`, runtime configuration, and persisted
  `network/bootstrap.list`, deduplicated and bounded to 64 entries.
- One new core method, `network.discover` (0x0609), with
  `network.discovery_completed|failed|cancelled` terminal events.
- DHT status fields in `network.status`: mode, routing peers, bootstrap state,
  query, record, provider, and publish counters.
- Inbound record validation through `StoreInserts::FilterBoth`, provider key and
  address validation, stale/conflict rejection, and bounded memory.
- 13 new Rust tests, for 214 total.

The Kotlin product screens still use visibly labelled mock data. Hosting,
downloads, search, relay, hole punching, NAT traversal, and mDNS remain later
stages.

## Files changed

Protocol:

- `crates/nexus-protocol/Cargo.toml`
- `crates/nexus-protocol/src/dht.rs` (new)
- `crates/nexus-protocol/src/error.rs`
- `crates/nexus-protocol/src/lib.rs`

Network crate:

- `crates/nexus-net/src/behaviour.rs`
- `crates/nexus-net/src/bootstrap.rs`
- `crates/nexus-net/src/config.rs`
- `crates/nexus-net/src/driver.rs`
- `crates/nexus-net/src/error.rs`
- `crates/nexus-net/src/lib.rs`
- `crates/nexus-net/src/shared.rs`
- `crates/nexus-net/src/state.rs`
- `crates/nexus-net/tests/stage10.rs` (new)

Core crate:

- `crates/nexus-core/src/config.rs`
- `crates/nexus-core/src/engine.rs`
- `crates/nexus-core/src/lib.rs`
- `crates/nexus-core/src/network.rs`
- `crates/nexus-core/tests/stage09.rs`
- `crates/nexus-core/tests/stage10.rs` (new)

FFI and workspace:

- `crates/nexus-ffi/src/registry.rs`
- `Cargo.toml`, `Cargo.lock`

Documentation:

- `docs/architecture/DHT_DESIGN.md` (new)
- `docs/operations/BOOTSTRAP_NODES.md` (new)
- `docs/architecture/FFI_INTERFACE.md` (version 1.3.0)
- `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md` (version 1.7.0)
- `docs/ARCHITECTURE.md`
- `docs/progress/STAGE_10.md` (new)
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

## Commands and results

### Rust tests

```text
cargo test --workspace -- --test-threads=1
```

Result: PASS, 214 tests, 0 failures.

Stage 10-specific suites:

```text
cargo test -p nexus-net --test stage10 -- --test-threads=1
cargo test -p nexus-core --test stage10 -- --test-threads=1
```

Result: PASS, 4 and 2 tests respectively. The network suite takes about 9 s.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `nexus-core` unit tests | 28 | 0 |
| `nexus-core` Stage 09 tests | 4 | 0 |
| `nexus-core` Stage 10 tests | 2 | 0 |
| `nexus-crypto` unit tests | 24 | 0 |
| `nexus-ffi` unit tests | 4 | 0 |
| `nexus-ffi` C ABI tests | 6 | 0 |
| `nexus-identity` unit tests | 9 | 0 |
| `nexus-identity` integration tests | 12 | 0 |
| `nexus-net` unit tests | 16 | 0 |
| `nexus-net` Stage 09 tests | 8 | 0 |
| `nexus-net` Stage 10 tests | 4 | 0 |
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
| **Rust total** | **214** | **0** |

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
- `cargo audit`: 408 dependencies scanned; no vulnerabilities and one allowed
  warning.
- `Cargo.lock` resolves 408 packages; the dependency inventory is at version
  1.7.0. Kademlia adds `libp2p-kad` 0.49.0 and `uint` 0.10.2 to the Stage 09
  graph.
- The native artifact build was not re-run in Stage 10 because the ABI and
  exported surface did not change. The six C ABI tests still pass in the
  workspace suite.

## Errors and failed attempts

1. Kademlia client mode denies inbound DHT substreams, so two clients could not
   answer each other's routing queries. `dht_mode` now selects client or server;
   entry points run server mode.
2. `MemoryStore::put` rejects values whose length is greater than or equal to
   `max_value_bytes`. The store uses `DHT_MAX_VALUE_BYTES + 1`.
3. Outbound `GET_PROVIDERS` results do not populate the local provider store.
   The driver now accumulates `FoundProviders` peer IDs and resolves addresses
   from provider records or the routing table.
4. Provider records may have no addresses. Empty lists are accepted and resolved
   from routing; non-empty lists stay capped at 8 addresses of 256 bytes.
5. The first core discovery test expected provider counters on the querying
   client. Only server-mode peers receive inbound provider announcements, so it
   now checks the servers.
6. `network.status` does not include a `bootstrap` array. The test now uses
   `network.bootstrap_list` (0x0603).
7. Malformed `network.discover` targets initially became method errors. They now
   return `NEXUS_STATUS_INVALID_ARGUMENT` during dispatch, before an operation or
   event exists.
8. Debug `eprintln!` probes from provider troubleshooting were removed before
   the evidence run.
9. Recheck found a failed `PutRecord` query counted its publish failure twice.
   The duplicate increment is removed and covered by
   `failed_publish_is_counted_once`.

## Decisions and unresolved questions

- Stage 10 was explicitly approved by the project owner.
- The DHT uses libp2p Kademlia `/nexus/kad/1.0.0`, not the public IPFS DHT.
- DHT values are exact signed envelopes. The network layer verifies the
  signature and the key derived from the signed identifier before storing or
  returning a value.
- Revisions are monotonic per key: higher replaces, lower is stale, identical
  bytes at the same revision is idempotent, different bytes at the same revision
  is a conflict.
- The default role is client. Bootstrap nodes run server mode through
  `dht_mode: "server"`.
- `network.discover` is the only core-visible DHT method. Raw publish, lookup,
  start-providing, and find-providers remain network-layer APIs for later
  product stages.
- Bootstrap entry points merge from compiled, configured, and persisted sources.
  A bootstrap peer is an entry point, not an identity or ownership authority.
- In-memory DHT state is not durable product state; TTL and republication are
  the recovery mechanism.
- Provider reputation, availability scoring, and multi-source download policy
  are deferred to Stage 12 and later. Index-provider records are Stage 16.
- There is no global bootstrap-list update service. Distribution remains a
  release, deployment, or operator decision.
- The ABI remains 1; the protocol version is `0.1.0-stage-10`.
- Android runtime DHT evidence is deferred because no emulator or device is
  available. The Android libraries cross-build with the same libp2p graph.
- Remote GitHub Actions execution has not run for Stage 10.

## Exact next action

1. Review `docs/progress/STAGE_10.md`, `docs/architecture/DHT_DESIGN.md`,
   `docs/operations/BOOTSTRAP_NODES.md`, the updated
   `docs/architecture/FFI_INTERFACE.md`, and the `nexus-protocol`/`nexus-net`/
   `nexus-core` Stage 10 tests.
2. Give explicit approval to start Stage 11.
3. Stage 11 adds NAT traversal, relays, and fallback behavior for difficult
   networks. Do not begin it before that approval.
