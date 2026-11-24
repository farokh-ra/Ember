# Stage 09 - Peer Identity, Transports, and Direct Connections

**Status:** Complete
**Completed:** 2026-11-16
**Progress guide:** 43-47%
**Next stage:** 10 - DHT publish and lookup (requires user permission)

## 1. Scope

Stage 09 implements the transport and connection layer defined by
[ADR-0005](../decisions/ADR-0005-libp2p-transports-discovery.md),
[ADR-0007](../decisions/ADR-0007-protocol-boundaries-versioning.md), and
[ADR-0008](../decisions/ADR-0008-lifecycle-background-execution.md), without
changing the Stage 05-08 storage, identity, record, or ABI contracts.

The stage covers:

- A device peer key independent of the user signing identity.
- libp2p `0.57.1` with QUIC v1 and TCP + Noise + Yamux.
- Authenticated direct connections and wrong-identity rejection.
- A bounded `/nexus/test/1` request/response exchange used to prove
  end-to-end connectivity.
- Network and peer state with honest observed/confirmed address reporting.
- Persisted bootstrap and manual peer addresses.
- Bounded connect, request, identify, ping, idle, and shutdown timeouts.
- Exponential reconnect backoff and network-change handling.
- Connection, stream, message, and per-peer request limits.
- Eight `network.*` core methods and the matching event family.
- Local loopback integration tests for QUIC, TCP, reconnection, mismatch
  rejection, persistence, and repeated start/stop.

DHT routing, record provider lookups, relay, hole punching, mDNS, downloads,
and the remaining product methods remain later-stage work.

## 2. Architecture

The new responsibilities are split as follows:

| Layer | Responsibility |
|---|---|
| `nexus-net` | Peer key, libp2p swarm, transports, behaviour, codec, bootstrap persistence, driver loop, bounded synchronous API, network state. |
| `nexus-protocol` | Bounded canonical-CBOR `/nexus/test/1` request and response schema. |
| `nexus-crypto` | `DomainTag::PeerKey` separation and the AEAD/HKDF primitives used by the peer-key file. |
| `nexus-core` | Lifecycle integration, method dispatch, domain-error mapping, CBOR status/peer encoders, async workers, events. |
| `nexus-ffi` | Status mapping for new core errors; no ABI shape change. |

`nexus-net` owns one OS thread with a current-thread Tokio runtime and the
libp2p swarm. It exposes a synchronous, bounded API: every call either
completes or returns a typed `NetError`. The core never links libp2p directly;
it holds an `Arc<Network>` and submits work to its existing worker pool.

## 3. Peer identity and key separation

- Each installation generates an Ed25519 device key and derives its libp2p
  `PeerId` from it. The peer ID identifies the device, not the person.
- The user signing identity from Stage 06 is untouched. Publications, profiles,
  tombstones, and backups keep using that identity.
- The device key is stored at `network/peer_key.enc`. The file is
  XChaCha20-Poly1305 encrypted with a key derived from the local encryption key
  through HKDF-SHA256 and `DomainTag::PeerKey` (`NEXUS-PEER-KEY-v1`).
- The file format is versioned (`NEXPEER1`, version 1, KDF id 1, 32-byte salt,
  24-byte nonce, 32-byte seed, 16-byte tag). A wrong local secret, tampering,
  truncation, unknown magic, unknown version, or unknown KDF all fail without
  overwriting the existing file.
- Identity backup and restore never read, copy, or replace the peer key. A
  restored user identity keeps the device's existing peer ID unless the app
  data is removed. A test asserts that identity files and peer-key files are
  independent.

This separation limits linkability between a user identity and a device's
network address. It does not provide anonymity: peers still observe the
addresses of peers they connect to.

## 4. Transports and authentication

The swarm builder composes:

| Layer | Configuration |
|---|---|
| QUIC v1 | `/ip4/0.0.0.0/udp/0/quic-v1`, preferred; 60 s idle timeout, keep-alive at one third. |
| TCP | `/ip4/0.0.0.0/tcp/0` with Noise XX and Yamux, fallback. |
| DNS | Enabled for bootstrap and user-entered DNS multiaddresses. |
| Identify | `/nexus/0.1.0-stage-09`, agent `ember/0.1.0`, 300 s interval, listen-address pushes. |
| Ping | 20 s interval, 10 s timeout, used for RTT and liveness. |
| Request/response | `/nexus/test/1`, 8 s default request timeout, 32 concurrent streams. |
| Connection limits | 64 established, 2 per peer, 16 pending inbound. |

Both transports authenticate the remote peer before any application protocol
runs. A dial that presents a different key than the `/p2p/<peer-id>` in the
target address fails at the swarm boundary with `wrong_peer_id`; the mismatched
peer is never recorded as connected and the local node stays usable. Calling
`connect` again with the correct peer ID succeeds.

mDNS is disabled in v1. Discovery uses the persisted bootstrap/manual list and
explicit connects only.

## 5. Network and peer state

Aggregate network states:

| State | Meaning |
|---|---|
| `disabled` | `network_enabled` is false; no transport was started. |
| `starting` | Listeners and bootstrap dials are being established. |
| `online` | At least one peer is connected. |
| `degraded` | Listeners are up, but no peer is connected. |
| `unavailable` | No usable listener or the driver failed to start. |
| `stopping` | Shutdown is in progress. |

Peer states are `connecting`, `connected`, `disconnected`, and `failed`.

`network.status` reports three address classes separately:

- `listen_addresses`: local listeners the node actually bound.
- `observed_addresses`: addresses peers or libp2p candidates reported for this
  node.
- `confirmed_addresses`: addresses libp2p confirmed end to end.

The core never turns an observed address into a public-reachability claim. In
the Stage 09 loopback tests `confirmed_addresses` is empty, and that is the
correct result: a loopback listener is not publicly reachable.

## 6. Connections, bootstrap, and reconnection

- `bootstrap.list` persists one multiaddress per line in
  `network/bootstrap.list`, bounded to 64 KiB and 64 entries. Every entry must
  include `/p2p/<peer-id>`; a malformed file cannot redirect a dial to an
  unauthenticated address.
- The persisted list is merged with the configured list before listeners start.
  Adds and removes are atomic and survive a restart.
- A configured bootstrap target is dialed at startup. Failures schedule an
  exponential retry from 1 s up to 60 s per target. Losing one target never
  disables the others.
- `network.reconnect` re-dials one known peer or every bootstrap target.
  `network.connect` dials an explicit multiaddr. Both use
  `PeerCondition::Always` so a second transport connection can be added to an
  already-connected peer.
- A network-path change (Wi-Fi switch, sleep/wake) resets every target's
  backoff to zero and triggers a fresh dial on the next driver tick.
  `Network::notify_network_change` is synchronous: it returns once the reset is
  applied.
- `Network::shutdown` sends a shutdown command, waits for the driver to exit,
  and joins the thread. It is idempotent and runs from `Drop`.

Example: a peer that goes offline for several retry cycles reconnects after it
restarts on the same address. A second test adds five failed bootstrap dials,
restores the peer, reports a network change, and asserts a prompt reconnect
instead of waiting for the accumulated backoff.

## 7. Timeouts, limits, and bounded work

| Item | Default | Bound |
|---|---|---|
| Direct dial | 10 s | 120 s configurable |
| Request/response | 8 s (+1 s driver grace) | 120 s configurable |
| Identify | 300 s interval | fixed |
| Ping | 20 s interval, 10 s timeout | fixed |
| Idle connection | 60 s | 120 s configurable |
| Driver startup | 5 s | fixed |
| Established connections | 64 | 512 |
| Established per peer | 2 | 512 |
| Pending inbound | 16 | 512 |
| Concurrent request streams | 32 | 512 |
| Bootstrap entries | 64 | 64 |
| Listen addresses | 8 | 8 |
| Inbound test requests | 60 per peer per minute | 512 |
| Test payload | 4 KiB | 4 KiB |
| Test frame | 8 KiB | 8 KiB |

The command queue holds 64 commands. A full queue returns `busy` instead of
blocking. Per-peer rate windows are capped at 256 entries; expired windows are
dropped. Inbound requests over the limit receive an error response and increment
`requests_rejected`.

## 8. Protocol schema

`/nexus/test/1` exists only to prove an authenticated, end-to-end application
exchange. It is not a product method.

- Frames are a four-byte big-endian length followed by a canonical CBOR body.
  Empty, truncated, and oversized frames are rejected before allocation.
- Request: `{v: 1, payload: bytes}`.
- Response: `{v: 1, payload: bytes}` or `{v: 1, error: text}`.
- Payload maximum is 4 KiB; the whole message maximum is 8 KiB.
- The codec uses `nexus-crypto` canonical CBOR, not serde or the
  request-response CBOR helper.

## 9. Core integration

Stage 09 adds eight methods to the ABI 1 catalog:

| ID | Method | Mode | Request | Result |
|---|---|---|---|---|
| 0x0601 | `network.status` | S | none | State, peer ID, transports, address classes, counts, counters, last error. |
| 0x0602 | `network.peers` | S | `cursor`, `limit` | Bounded peer page with `next_cursor` and `total`. |
| 0x0603 | `network.bootstrap_list` | S | none | `{bootstrap: [...]}`. |
| 0x0604 | `network.bootstrap_add` | S | `multiaddr` | Updated list. |
| 0x0605 | `network.bootstrap_remove` | S | `multiaddr` | Updated list. |
| 0x0606 | `network.reconnect` | A | `peer_id?` | `network.reconnect_completed\|failed\|cancelled`. |
| 0x0607 | `network.connect` | A | `multiaddr` | `network.connect_completed\|failed\|cancelled`. |
| 0x0608 | `network.send_test` | A | `peer_id`, `payload` | `network.test_message\|failed\|cancelled`. |

Behaviour:

- `start` starts the network after identity and storage work. A network start
  failure never fails `start`: the core remains usable offline, emits
  `network.state` with `state=unavailable`, and exposes the stable failure code
  through `network.status.last_error`.
- `stop` shuts the network down before the event queue closes.
- Async methods validate and return `{op_id, accepted}`; the worker runs one
  bounded operation and emits exactly one terminal event. Terminal events carry
  `op_id`.
- Operational failures such as `disabled`, `unavailable`, `timeout`,
  `connection_failed`, `wrong_peer_id`, `not_found`, `not_connected`, and
  `rejected` are domain errors with a `net_code` detail. Malformed parameters,
  such as a missing `multiaddr` field or a non-bytes payload, remain ABI-level
  `NEXUS_STATUS_INVALID_ARGUMENT`.
- `network.status` is callable even without a running node. It returns
  `disabled` when networking is off, otherwise `unavailable` with `last_error`.
- `system.health`, `system.snapshot`, and `system.info` capabilities include the
  network state and methods.

Event additions:

| Event | Class | Data |
|---|---|---|
| `network.state` | Durable | `state`, `peer_count`, `transports`, optional `reason` |
| `network.peer_event` | Durable | `peer_id`, `event`, optional `direction`, `transport`, `reason` |
| `network.reconnect_completed` | Terminal | `peer_id` (null for all targets) |
| `network.reconnect_failed` | Terminal | `peer_id`, `error` |
| `network.reconnect_cancelled` | Terminal | `peer_id` |
| `network.connect_completed` | Terminal | `peer_id`, `transport`, `remote_address`, `established_ms`, `multiaddr` |
| `network.connect_failed` | Terminal | `multiaddr`, `error` |
| `network.connect_cancelled` | Terminal | `multiaddr` |
| `network.test_message` | Terminal | `peer_id`, `payload`, `round_trip_ms` |
| `network.test_failed` | Terminal | `peer_id`, `error` |
| `network.test_cancelled` | Terminal | `peer_id` |

The protocol version string is `0.1.0-stage-09`; the ABI remains 1.

## 10. Verification evidence

### 10.1 `nexus-net` unit tests

14 tests, 0 failures:

| Area | Tests |
|---|---|
| Bootstrap persistence | 4 |
| Configuration and backoff | 4 |
| Peer key | 4 |
| Driver address labeling | 1 |
| State wire names | 1 |

The peer-key tests cover creation, reload, wrong-secret rejection without
overwrite, tampering, unknown formats, and independence from identity files.
The bootstrap tests cover save/reload deduplication, missing files, missing
peer IDs, and oversized files/lists. The configuration tests cover default QUIC
and TCP listeners, bounds, `/p2p` requirements, and capped exponential delay.

### 10.2 `nexus-net` Stage 09 integration tests

8 tests, 0 failures, run with `--test-threads=1`:

| Test | Covers |
|---|---|
| `two_nodes_exchange_a_test_message_over_quic_and_tcp` | QUIC message exchange, TCP/Noise/Yamux exchange on the same peer, transport labels, peer counts. |
| `a_mismatched_peer_identity_is_rejected` | Dialing peer B while claiming peer C fails with `wrong_peer_id`; B is never connected. |
| `unreachable_peers_fail_without_stopping_the_node` | A closed port returns a bounded failure; the node still connects and exchanges messages. |
| `reconnect_backoff_recovers_after_a_peer_restarts` | A bootstrap peer restarts on the same port and is re-dialed. |
| `a_network_change_resets_backoff_and_reconnects_promptly` | After five failed dials, a network-change report resets backoff and reconnects within 2 s. |
| `bootstrap_peers_persist_across_restarts` | Add, restart, reload, remove, and missing-entry failure. |
| `start_and_stop_is_repeatable` | 100 start/stop cycles with a stable peer ID. |
| `network_config_rejects_invalid_addresses_and_bounds` | Missing `/p2p`, invalid limits, and bad listen addresses fail before sockets. |

Every listener is loopback-only. No test depends on an external service or
claims public reachability.

### 10.3 `nexus-core` Stage 09 integration tests

4 tests, 0 failures, run with `--test-threads=1`:

| Test | Covers |
|---|---|
| `two_cores_exchange_a_test_message_over_quic_and_tcp` | Two full cores connect over QUIC and TCP, exchange payloads, report `online`, and return network sections in `system.health`/`system.snapshot`. |
| `a_mismatched_peer_identity_fails_without_stopping_the_core` | `connect_failed` carries `verification_failed` + `net_code=wrong_peer_id`; a later correct dial still succeeds. |
| `a_network_start_failure_leaves_the_core_usable` | A corrupt peer key leaves `system.health.state=running`, `network.state=unavailable`, `last_error=peer_key_invalid`; network methods return `network_unavailable`. |
| `network_config_failures_return_domain_errors` | Bootstrap without `/p2p` and oversized test payloads fail with stable domain errors. |

### 10.4 Workspace totals

`cargo test --workspace` passed with 201 tests and 0 failures:

| Suite | Tests |
|---|---:|
| `nexus-core` unit tests | 27 |
| `nexus-core` Stage 09 tests | 4 |
| `nexus-crypto` unit tests | 24 |
| `nexus-ffi` unit tests | 4 |
| `nexus-ffi` C ABI tests | 6 |
| `nexus-identity` unit tests | 9 |
| `nexus-identity` integration tests | 12 |
| `nexus-net` unit tests | 14 |
| `nexus-net` Stage 09 tests | 8 |
| `nexus-observability` unit tests | 3 |
| `nexus-protocol` unit tests | 21 |
| `nexus-protocol` signed-record tests | 9 |
| `nexus-protocol` Stage 07 tests | 8 |
| `nexus-store` unit tests | 17 |
| `nexus-store` local-import tests | 4 |
| `nexus-store` Stage 07 tests | 4 |
| `nexus-store` storage tests | 12 |
| `nexus-transfer` unit tests | 7 |
| `nexus-transfer` integration tests | 7 |
| `nexus-types` unit tests | 1 |
| **Total** | **201** |

The earlier Stage 08 suite had 166 tests; Stage 09 adds 35: 14 network unit
tests, 8 network integration tests, 4 core integration tests, 4 core network
unit tests, and 5 protocol wire tests.

### 10.5 Formatting, linting, and build

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `scripts/build-native.ps1 -Target All -Configuration Release` builds the
  Windows DLL and both Android `.so` files; required exports are unchanged.
- The C ABI contract tests still pass, so the new core errors did not change the
  exported surface.

## 11. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries are cross-built and export-checked.
- The Stage 09 network proofs run on Windows loopback through the same Rust
  code that Android will load.
- Android runtime execution, on-device QUIC/TCP behavior, Wi-Fi switch and
  sleep/wake handling, and battery impact remain deferred.
- `Network::notify_network_change` is ready for the platform lifecycle hook, but
  the Android lifecycle binding and its runtime evidence are later work.

This is not claimed as Android runtime evidence. The remaining risk is
platform-specific networking and lifecycle behavior, not a second protocol
implementation.

## 12. Failure evidence and fixes

1. Adding a second transport connection to an already-connected peer initially
   did nothing. `DialOpts` needed `PeerCondition::Always`; the default condition
   skips peers the swarm already considers connected.
2. `request_response::Behaviour::add_address` recorded an address for the
   protocol but did not give the swarm a dial target. Bootstrap dials now call
   `Swarm::add_peer_address` before `dial`.
3. `multiaddr` 0.19 changed `Protocol::P2p` to carry a parsed `PeerId` directly;
   the Stage 09 parser uses the typed value instead of constructing a
   multihash manually.
4. Core networking failures were initially surfaced as `CoreError`, which the
   FFI mapped to `NEXUS_STATUS_INTERNAL`. Operational failures now become
   domain errors with `net_code`, while malformed field values stay ABI-level
   `INVALID_ARGUMENT`. This keeps ABI statuses for boundary faults only.
5. A network start failure initially propagated out of `Engine::start`. It is
   now recorded and swallowed so the core stays usable offline, as ADR-0005 and
   ADR-0008 require.
6. `Network::notify_network_change` was fire-and-forget with no test. It now
   waits for the driver to apply the reset, and a loopback test proves a prompt
   reconnect after accumulated backoff.

## 13. Decisions and unresolved questions

- libp2p is pinned exactly at `0.57.1`; `tokio` is `1.54.1`, `futures` is
  `0.3.34`.
- QUIC is preferred, TCP + Noise + Yamux is the fallback, DNS resolution is
  enabled, and mDNS is disabled in v1.
- The device peer key is separate from the user signing identity and is stored
  encrypted under the local encryption key with `DomainTag::PeerKey`.
- Status reports observed and confirmed addresses separately and never claims
  public reachability.
- A network failure never fails core start.
- Bootstrap and manual peers persist in `network/bootstrap.list`; every entry
  must include `/p2p`.
- Reconnect backoff is 1 s to 60 s by default and resets on a reported network
  change.
- `network.*` operational failures are domain errors; malformed arguments are
  ABI-level.
- DHT routing, relay, hole punching, mDNS, and multi-source transfers are out of
  scope for Stage 09.
- Android runtime networking evidence and remote GitHub Actions execution
  remain outstanding.

## 14. Next stage

Stage 10 adds Kademlia-based peer routing and bounded record publication and
lookup on top of the Stage 09 transports. It must not begin until Stage 09 is
reviewed and approved by the project owner.
