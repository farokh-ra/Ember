# Stage 11 - NAT Traversal, Relays and Difficult Networks

**Status:** Complete
**Completed:** 2026-11-19
**Progress guide:** 49-55%
**Next stage:** 12 - streaming transfer, chunk verification, pause/resume, and process-death recovery (requires user permission)

## 1. Scope

Stage 11 implements the difficult-network layer required by
[ADR-0005](../decisions/ADR-0005-libp2p-transports-discovery.md) and
[ADR-0007](../decisions/ADR-0007-protocol-boundaries-versioning.md) on top of
the Stage 09 transports and the Stage 10 DHT. It adds no central account
service, content proxy, or identity authority.

The stage covers:

- A documented NAT traversal and relay design in
  [NAT_TRAVERSAL.md](../architecture/NAT_TRAVERSAL.md).
- Circuit Relay v2 through the pinned libp2p `0.57.1` graph and its `relay`
  feature. Ember is a relay client by default and a bounded relay server only
  when configured.
- DCUtR hole punching through the `dcutr` feature.
- AutoNAT v1 reachability through the `autonat` feature.
- Direct-first connection fallback to connected relays with bounded deadlines.
- Optional community relay nodes with operator guidance in
  [RELAY_NODES.md](../operations/RELAY_NODES.md).
- Configurable relay list from compiled, configured, and persisted sources.
- Connectivity classification, relay/path status fields, and troubleshooting
  hints in `network.status` and `network.connectivity`.
- Relay cost visibility through negotiated circuit limits and circuit counters.
- A demo-only troubleshooting section in the shared Network status screen.

Symmetric NAT and restrictive firewalls are handled by failing over to a
reachable relay. The stage does not claim that every NAT combination can be
traversed without a relay.

## 2. Architecture

| Component | Behaviour |
|---|---|
| `relay::client::Behaviour` | Requests reservations and dials relayed circuits. |
| `relay::Behaviour` | Serves HOP circuits when `relay_server_enabled` is true and an external address is confirmed. |
| `dcutr::Behaviour` | Attempts a direct upgrade after a relayed connection exists. |
| `autonat::v1::Behaviour` | Probes connected peers to classify the node as public, private, or unknown. |
| Identify | Advertises `/nexus/0.1.0-stage-11` and `ember/0.1.0`. |

DCUtR and AutoNAT are wrapped in `Toggle` so tests and constrained deployments
can disable them without changing the composed behaviour type. Relay serving is
represented by `relay::Status::Enable`/`Disable` and defaults to disabled.

A relay server does not advertise HOP until it has a confirmed external
address. `external_addresses` is a bounded create-time list of operator-known
addresses; loopback listeners are advertised automatically when relay serving
is enabled so single-machine tests and local relays work without pretending to
be public.

## 3. Fallback order

The foreground `network.connect` operation and the background reconnect loop
use the same order:

1. Direct dial with `connect_timeout` (10 s default, 120 s maximum).
2. Relay dials through connected relays, one at a time, each bounded by
   `relay_reservation_timeout` (30 s default, 120 s maximum).
3. `NoViablePath` when every candidate fails.

A relay is never a candidate for itself. Only connected relays are used. A
failed direct dial with no connected relay returns the direct error; a relay
attempt increments `fallback_attempts`, and a relayed success increments
`fallback_successes`.

Background targets try direct, then relay once, then continue the existing
exponential retry. A failed relay attempt does not disable direct retries.

Reservations are requested once per relay connection. Refused, closed, or
timed-out reservations retry after 5 s. Reservation and circuit limits are read
from the negotiated `Limit` when the relay reports one.

## 4. Relay list and configuration

Startup merges three sources in this order and removes duplicates:

1. Compile-time `EMBER_RELAY_NODES`.
2. `NetworkConfig::relays` supplied by the deployment.
3. Persisted `network/relays.list`.

Every entry must be a full multiaddress with `/p2p/<peer-id>`. The list is
bounded to 16 entries and 64 KiB. Adds and removes through
`network.relay_add` / `network.relay_remove` are atomic and survive a restart.

Core configuration adds:

| Key | Type | Default | Meaning |
|---|---|---|---|
| `relay_fallback` | bool | `true` | Fall back to a connected relay when a direct dial fails. |
| `relay_server` | bool | `false` | Serve bounded circuits for other peers. |

Network configuration adds `external_addresses`, relay client/server toggles,
reservation and circuit limits, AutoNAT/DCUtR toggles, and validation bounds.

## 5. Core integration

Stage 11 adds four methods to the ABI 1 catalog:

| ID | Method | Mode | Request | Result |
|---|---|---|---|---|
| 0x060A | `network.relay_list` | S | none | `{relays: [...]}` |
| 0x060B | `network.relay_add` | S | `multiaddr` | Updated `{relays: [...]}` |
| 0x060C | `network.relay_remove` | S | `multiaddr` | Updated `{relays: [...]}` |
| 0x060D | `network.connectivity` | S | none | Classification, NAT status, relay limits, counters, and `hint` |

Behaviour:

- `relay_add` and `relay_remove` require a running network and a complete
  `/p2p/<peer-id>` address. A missing peer ID is an ABI-level invalid argument;
  removing an unknown address is a domain `not_found`.
- `network.connectivity` returns one of `disabled`, `connecting`, `relay`,
  `direct`, or `no_peers`. Relayed data connections take precedence over the
  direct connection to relay infrastructure.
- `network.status` adds `connectivity`, NAT fields, relay client/fallback
  fields, relay counts, reservation/circuit limits, fallback and hole-punch
  counters, direct/relayed connection counts, and relay-server counters.
- `network.peers` entries add `path` and `relay_peer_id`.
- `network.connect_completed` and `network.connect_failed` add `path` and
  `relay_peer_id`; `network.connect_failed.error` is a map with `code`,
  `message`, and `retryable`.
- New `NetError` variants `RelayUnavailable`, `RelayReservationFailed`,
  `HolePunchFailed`, and `NoViablePath` map to `network_unavailable` with
  stable user-safe messages.

The protocol version string is `0.1.0-stage-11`; the ABI remains 1. The FFI
interface document is at version 1.4.0 with nineteen callable methods.

## 6. Protections and bounds

| Item | Default | Maximum |
|---|---|---|
| Configured relays | 0 | 16 |
| Relay list file | 0 | 64 KiB |
| External addresses | 0 | 8 |
| Relay reservation timeout | 30 s | 120 s |
| Relay server reservations | 128 | 1,024 |
| Relay server circuits | 16 | 256 |
| Relay circuits per peer | 4 | active circuits |
| Relay circuit duration | 2 min | 1 h |
| Relay circuit bytes | 128 KiB | 16 MiB |
| Direct connect timeout | 10 s | 120 s |

- Relay candidates are limited to connected relays and exclude the target.
- A reservation is retried only after a bounded delay.
- The relay server rejects requests beyond its reservation, circuit, duration,
  and byte limits and counts the denial.
- Relay limits are negotiated with the client and reported, not assumed.
- Raw relayed byte totals are not exposed by libp2p; Ember reports the
  negotiated byte limit and circuit counts instead of a fabricated total.
- A relay sees peer IDs, circuit timing, and limits, but not content: the
  end-to-end Noise connection remains intact.

## 7. Connectivity test matrix

| Scenario | Expected result | Evidence |
|---|---|---|
| Direct address reachable | Direct connection, no fallback. | `direct_connections_are_preferred_when_reachable` |
| Direct fails, relay reachable | Relayed connection, fallback counted. | `a_failed_direct_dial_falls_back_to_a_relay`, `a_failed_direct_connect_falls_back_to_the_configured_relay` |
| Direct fails, no relay configured | `NoViablePath`. | `an_unreachable_peer_reports_no_viable_path` |
| Direct fails, relay unreachable | `NoViablePath`, connectivity hint names relay reachability. | `an_unreachable_peer_reports_no_viable_path`, `connectivity_reports_disabled_and_idle_states_with_a_hint` |
| Networking disabled | `disabled`, hint says to enable networking. | `connectivity_reports_disabled_and_idle_states_with_a_hint` |
| Relays connected but no reservation | `no_peers`, hint names the missing reservation. | Hint branch in `network.rs`; status fields in the relay tests |
| Relay list add/remove | Persisted across restart; missing peer ID rejected. | `relay_list_methods_persist_and_reject_missing_peer_ids`, `relay_list_persists_across_restarts` |
| Relay server limits | Negotiated caps reported; defaults bounded. | `relay_reservation_reports_negotiated_caps`, `relay_defaults_are_bounded_and_disabled_only_for_serving` |

## 8. Interface and UI

- The Kotlin Network status screen shows connectivity, NAT reachability, relay
  fallback state, configured relay paths, direct/relayed connection counts,
  fallback counters, circuit limits, and troubleshooting text.
- Every new screen value is demo data from `DemoCatalog`; the existing
  "Simulated network status" notice remains visible. No Kotlin screen is wired
  to the core yet.
- `networkErrorText` maps relay-unavailable and reservation-failed demo errors.
- The screen remains a prototype for Stage 17, not evidence of a live relay UI.

## 9. Verification evidence

### 9.1 `nexus-net` unit tests

6 new tests, 0 failures:

| Test | Covers |
|---|---|
| `relay_defaults_are_bounded_and_disabled_only_for_serving` | Client/fallback/AutoNAT/DCUtR defaults, relay-server default off, missing peer ID, timeout and limit validation. |
| `external_addresses_are_bounded` | Accepted external address and the 8-entry cap. |
| `saves_and_reloads_a_deduplicated_list` | Relay list persistence and duplicate removal. |
| `missing_file_is_an_empty_list` | Missing `network/relays.list` behaviour. |
| `rejects_addresses_without_a_peer_id` | Relay entries require `/p2p/<peer-id>`. |
| `rejects_oversized_files` | 64 KiB relay-list cap. |

### 9.2 `nexus-net` Stage 11 integration tests

5 tests, 0 failures, run with `--test-threads=1`:

| Test | Covers |
|---|---|
| `relay_reservation_reports_negotiated_caps` | A loopback relay server accepts a client reservation and reports the configured circuit byte and duration limits. |
| `direct_connections_are_preferred_when_reachable` | A direct dial succeeds without using the configured relay. |
| `a_failed_direct_dial_falls_back_to_a_relay` | A broken direct address falls back to the loopback relay; the peer path is relayed; a test message crosses the circuit. |
| `an_unreachable_peer_reports_no_viable_path` | A peer behind a closed port and an unreachable relay returns `NoViablePath` and records `no_viable_path`. |
| `relay_list_persists_across_restarts` | Relay add/list/remove survives a network restart. |

### 9.3 `nexus-core` Stage 11 tests

4 integration tests, 0 failures:

| Test | Covers |
|---|---|
| `relay_list_methods_persist_and_reject_missing_peer_ids` | `0x060A`–`0x060C`, persistence, updated lists, invalid address rejection, and removing an unknown entry. |
| `connectivity_reports_disabled_and_idle_states_with_a_hint` | `0x060D` returns `disabled` with networking off, then `no_peers` with a relay hint. |
| `a_failed_direct_connect_falls_back_to_the_configured_relay` | A core connects through a loopback relay, reports the relayed path and relay peer ID, exchanges a test message, and shows relay status counters. |
| `an_unreachable_peer_reports_no_viable_path` | A core reports `network_unavailable` with `net_code=no_viable_path` and a terminal `network.connect_failed` event. |

### 9.4 Workspace totals

`cargo test --workspace -- --test-threads=1` passed with 229 tests and 0
failures:

| Suite | Tests |
|---|---:|
| `nexus-core` unit tests | 28 |
| `nexus-core` Stage 09 tests | 4 |
| `nexus-core` Stage 10 tests | 2 |
| `nexus-core` Stage 11 tests | 4 |
| `nexus-crypto` unit tests | 24 |
| `nexus-ffi` unit tests | 4 |
| `nexus-ffi` C ABI tests | 6 |
| `nexus-identity` unit tests | 9 |
| `nexus-identity` integration tests | 12 |
| `nexus-net` unit tests | 22 |
| `nexus-net` Stage 09 tests | 8 |
| `nexus-net` Stage 10 tests | 4 |
| `nexus-net` Stage 11 tests | 5 |
| `nexus-observability` unit tests | 3 |
| `nexus-protocol` unit tests | 25 |
| `nexus-protocol` signed-record tests | 9 |
| `nexus-protocol` Stage 07 tests | 8 |
| `nexus-store` unit tests | 17 |
| `nexus-store` local-import tests | 4 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-store` storage tests | 12 |
| `nexus-transfer` unit tests | 7 |
| `nexus-transfer` integration tests | 7 |
| `nexus-types` unit tests | 1 |
| **Total** | **229** |

Stage 10 had 214 tests; Stage 11 adds 15: 6 network unit tests, 5 network
Stage 11 tests, and 4 core Stage 11 tests.

### 9.5 Kotlin checks

- `gradlew spotlessCheck detekt :shared:presentation:jvmTest :shared:ui:jvmTest`
  passes.
- The demo catalogue and screen changes add no new Kotlin test cases; the
  existing presentation and UI tests still pass.

### 9.6 Formatting, linting, and dependencies

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo deny check` reports `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo audit` scans 411 dependencies with no vulnerabilities and one allowed
  unmaintained warning (`paste`, RUSTSEC-2024-0436, Linux-only through
  `if-watch`/netlink).
- `Cargo.lock` resolves 411 packages. Stage 11 adds `libp2p-autonat` 0.16.0,
  `libp2p-dcutr` 0.15.0, and `libp2p-relay` 0.22.0. `libp2p-upnp` 0.7.0 is
  present through libp2p's optional graph but is not enabled.
- The dependency and license inventory is at version 1.8.0.

## 10. Exit criteria

| Criterion | Evidence |
|---|---|
| Direct connections work in supported scenarios. | `direct_connections_are_preferred_when_reachable`; the core relay test also starts with a direct-capable stack. |
| Relay fallback works when direct fails and a relay is reachable. | `a_failed_direct_dial_falls_back_to_a_relay`, `a_failed_direct_connect_falls_back_to_the_configured_relay`. |
| The app clearly reports when a connection cannot be established. | `an_unreachable_peer_reports_no_viable_path` in both suites, `NoViablePath` mapping, connectivity hints, and the demo troubleshooting UI. |
| Relay usage is visible and configurable. | Relay list methods, config keys, status counters, negotiated limits, and the demo Network status section. |
| Community relays are optional. | Relay list defaults empty; relay serving defaults off; direct connections work without a relay. |
| Bandwidth and cost are measurable within libp2p's data. | Negotiated circuit byte/duration limits and accepted/closed circuit counters are exposed; raw byte totals are explicitly not claimed. |
| The connectivity matrix is covered. | Section 7 maps each scenario to a test or a documented status branch. |

## 11. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries cross-build with the same libp2p
  relay, DCUtR, and AutoNAT graph.
- The Stage 11 proofs run on Windows loopback through the same Rust code that
  Android will load.
- Android runtime relay behaviour, mobile network switching, and battery impact
  remain deferred.

This is not claimed as Android runtime evidence.

## 12. Failure evidence and fixes

1. Relay serving requires a confirmed external address. Without one, libp2p
   does not advertise HOP and the loopback tests could not reserve. The driver
   now registers operator `external_addresses` and auto-advertises loopback
   listeners when relay serving is enabled.
2. Clippy rejected the first relay configuration with
   `field_reassign_with_default`, two `collapsible_if` cases, two
   `useless_conversion` cases, and two `too_many_arguments` functions. The
   configuration now uses struct initialization, nested conditions are
   collapsed, `as_secs()` is used directly, and the two wide internal
   functions carry a documented allow.
3. DCUtR can promote a circuit to a direct connection immediately after the
   connect event. The core relay test accepts a relay or direct path in the
   snapshot while still requiring the connect event and relay status to prove
   fallback happened.
4. Deterministic relay-fallback measurements required disabling DCUtR and
   AutoNAT in the test fixtures. Production defaults leave both enabled.
5. `libp2p-relay` does not export its `Limit` type. The driver reads
   `limit.duration()` and `limit.data_in_bytes()` inline and stores only the
   two values it reports.
6. The Stage 09 transport assertion expected exactly `quic` and `tcp`. A relay
   client now also advertises the `relay` transport, so the assertion was
   updated to `["quic", "tcp", "relay"]`.
7. `network.connect_failed` originally lost the typed error and emitted only a
   string. The driver now preserves `NetError`, and the core renders the
   `code`/`message`/`retryable` map so `NoViablePath` is distinguishable from a
   direct timeout.

## 13. Decisions and unresolved questions

- Circuit Relay v2 is the relay mechanism. Ember does not add TURN or a custom
  relay protocol.
- DCUtR and AutoNAT v1 are enabled by default for ordinary nodes.
- Relay serving is opt-in and bounded. A relay is optional infrastructure, not
  an authority.
- Direct dials always run first. Relay fallback is sequential and bounded; the
  node does not race relay and direct dials against each other.
- Connectivity reports `relay` when any data connection is relayed, even if a
  direct connection to relay infrastructure exists.
- The relay list merges compiled, configured, and persisted sources and is
  bounded to 16 entries.
- UPnP/NAT-PMP port mapping is deferred. `libp2p-upnp` is not enabled.
- Raw relayed byte accounting is not exposed by libp2p. Negotiated limits and
  circuit counts are the supported cost signal.
- The Kotlin troubleshooting UI remains labeled demo data; core wiring is a
  later UI stage.
- Android runtime relay evidence and remote GitHub Actions execution remain
  outstanding.

## 14. Next stage

Stage 12 adds streaming transfer, chunk verification, pause/resume, and
process-death recovery. It must not begin until Stage 11 is reviewed and
approved by the project owner.
