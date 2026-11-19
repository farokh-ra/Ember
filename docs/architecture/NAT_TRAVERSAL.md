# NAT Traversal and Relay Design

**Status:** Implemented in Stage 11
**Scope:** Direct connection preference, Circuit Relay v2 fallback, DCUtR hole punching, AutoNAT reachability, community relay operation, path reporting, and troubleshooting.

This document records the Stage 11 design. It is implemented by `nexus-net`,
the `network.connect`/`network.connectivity` core methods, and the optional
community relay role. It adds no account service, central directory, or
content proxy.

## 1. Roles and protocols

| Role | Default | Behaviour |
|---|---|---|
| Ordinary node | relay client on | Dials direct first, reserves on connected relays, accepts relayed connections, and tries DCUtR upgrades. |
| Community relay | relay server off | Forwards bounded circuits for peers that cannot connect directly. It does not store, verify, or authorise content. |

Protocols:

- **Circuit Relay v2** (`/libp2p/circuit/relay/0.2.0/hop` and `/stop`) provides
  the relayed fallback path. Ember acts as a relay client by default and as a
  relay server only when explicitly configured.
- **DCUtR** (`/libp2p/dcutr`) attempts a direct upgrade after a relayed
  connection exists. A successful upgrade is reported as a direct path.
- **AutoNAT v1** (`/libp2p/autonat/1.0.0`) asks connected peers to dial back and
  classifies the node as public, private, or unknown.
- Identify advertises `/nexus/0.1.0-stage-11` and `ember/0.1.0`.

Relay serving stays disabled until the node has at least one confirmed external
address. A relay must not advertise the HOP protocol while it is unreachable
from the public network. Loopback listeners are advertised automatically when
relay serving is enabled so single-machine tests and local relays work without
claiming a public address.

## 2. Path selection order

`Network::connect` and the background reconnect loop follow the same order:

1. **Direct.** Dial the full target multiaddress with the configured
   `connect_timeout` (10 s default, 120 s maximum).
2. **Relay.** If the direct dial fails or times out, build one circuit address
   per connected relay and dial them in order until one succeeds. Each attempt
   is bounded by `relay_reservation_timeout` (30 s default, 120 s maximum).
3. **No viable path.** If every relay candidate fails, return
   `NetError::NoViablePath` and report `no_viable_path` in the peer snapshot.

A relay is never used to reach itself, and only currently connected relays are
candidates. A failed direct dial with no connected relay returns the direct
error (`timeout`, `connection_failed`, `wrong_peer_id`, and so on) instead of a
relay error. A relay attempt increments `fallback_attempts`; a successful
relayed connection increments `fallback_successes`.

Background targets dial direct first. When that fails, the driver tries the
relay addresses once, then continues the existing exponential retry instead of
retrying the same relay forever.

## 3. Relay reservations

A node that connects to a relay calls
`swarm.listen_on("/ip4/<relay>/p2p/<relay-id>/p2p-circuit")`. The relay grants
a reservation and the node can then receive inbound circuits.

- A reservation is requested once per relay connection.
- A refused, closed, or timed-out reservation is retried after 5 s.
- Reservation state is bounded by `MAX_RELAYS` (16).
- Reservation and circuit limits are taken from the relay's negotiated
  `Limit`, when the relay reports one.
- Reservation errors are counted separately from circuit errors and never stop
  other relays or the direct path.

## 4. Relay list

Startup merges three sources in this order and removes duplicates:

1. Compile-time `EMBER_RELAY_NODES`.
2. `NetworkConfig::relays` supplied by the embedding deployment.
3. Persisted `network/relays.list`.

Every entry must be a full multiaddress with `/p2p/<peer-id>` and is bounded to
16 entries and 64 KiB. `network.relay_add` and `network.relay_remove` are
atomic and survive a restart. Core configuration also accepts
`relay_fallback` (default `true`) and `relay_server` (default `false`).

A relay is optional infrastructure, not an authority:

- It cannot sign records, own a file, or change a publication.
- It receives no special protocol message beyond Circuit Relay v2.
- Traffic between two peers stays inside the existing Noise-encrypted
  connection. The relay forwards ciphertext and cannot read content.
- The relay can observe that two peer IDs communicate and the circuit limits,
  but not file names, record contents, or search queries.

## 5. NAT reachability

AutoNAT v1 probes connected peers and reports one of:

| Status | Meaning |
|---|---|
| `public` | A peer dialled the node's advertised address successfully. |
| `private` | A peer could not dial the node directly. Relay fallback is required for inbound connections. |
| `unknown` | No conclusive probe has completed yet. |

AutoNAT uses a 15 s probe timeout, a 5 s boot delay, a 60 s retry while
unknown, and a 10 min refresh once established. It uses connected peers and
configured relays as probe servers. AutoNAT can be disabled with
`autonat_enabled: false`.

DCUtR is attempted on relayed connections when `dcutr_enabled` is true. A
successful hole punch changes the reported path to `direct` and increments
`direct_upgrades`. A failed attempt increments `hole_punch_failures` and keeps
the relayed connection usable.

Hole punching is not guaranteed. Symmetric NAT, carrier-grade NAT, and
firewalls that block outbound UDP can defeat both direct dialing and DCUtR; the
relay remains the fallback in those cases. This stage does not claim universal
traversal.

## 6. Connectivity classification

`NetworkStatus::connectivity()` is a single user-visible summary:

| Value | Condition |
|---|---|
| `disabled` | Networking is off. |
| `connecting` | The node is starting. |
| `relay` | At least one data connection is relayed. |
| `direct` | At least one direct data connection exists and none are relayed. |
| `no_peers` | The node is running but has no peer connection. |

Relayed data connections take precedence over the direct connection to relay
infrastructure, so a node that reaches peers only through a relay is not
reported as fully direct. A direct connection to a relay itself is not counted
as peer connectivity for this summary.

`network.connectivity` returns the same classification together with NAT
status, relay counts, reservations, limits, fallback counters, hole-punch
failures, and a short troubleshooting hint. The hint distinguishes:

- networking disabled;
- no configured relay;
- configured relays not connected;
- connected relays with no active reservation;
- a ready relay path with no connected peer.

The demo Network status screen mirrors these states and is labelled simulated;
it is not wired to the core.

## 7. Relay server bounds

A relay server accepts reservations and circuits only within these bounds:

| Item | Default | Maximum |
|---|---|---|
| Active reservations | 128 | 1,024 |
| Active circuits | 16 | 256 |
| Circuits per source peer | 4 | active circuits |
| Reservation duration | 1 h (libp2p default) | not separately configurable |
| Circuit duration | 2 min | 1 h |
| Circuit bytes | 128 KiB | 16 MiB |

The byte limit is the relay's admission limit and is reported through
`Limit::data_in_bytes()`. libp2p does not expose raw relayed byte counters, so
Ember reports the negotiated limit and the number of established/closed
circuits instead of inventing a bandwidth total.

Circuit admission is first-come, first-served within these bounds. A relay
does not prioritise, bill, or identify content. Operators who need stronger
isolation should run separate relay processes or hosts.

## 8. Failure and status reporting

- `network.connect_failed` carries `path`, `relay_peer_id` when one was
  attempted, and a redacted error with `code`, `message`, and `retryable`.
- `NetworkStatus` reports `relays_configured`, `relays_connected`,
  `relay_reservations`, `relay_reservation_failures`,
  `relay_circuits_established`, `relay_data_limit_bytes`,
  `relay_duration_limit_secs`, `fallback_attempts`, `fallback_successes`,
  `direct_connections`, `relayed_connections`, `hole_punch_failures`,
  `direct_upgrades`, and relay-server counters.
- `network.peers` entries include `path` (`direct` or `relay`) and
  `relay_peer_id`.
- The error variants `RelayUnavailable`, `RelayReservationFailed`,
  `HolePunchFailed`, and `NoViablePath` map to the domain code
  `network_unavailable` with stable user-safe messages.

A node with no route reports clearly instead of hanging: the foreground
connect operation ends with `NoViablePath` after the direct and relay deadlines,
and the UI can show the connectivity hint plus the last error.

## 9. Deferred

- UPnP and NAT-PMP port mapping. `libp2p-upnp` is present in the dependency
  graph but is not enabled; explicit listen addresses and relays are used
  instead.
- TURN or any non-libp2p relay. Circuit Relay v2 is the only relay mechanism.
- Relay discovery through the DHT. Relays are compiled, configured, or
  persisted addresses.
- Relay reputation, incentives, billing, and bandwidth accounting beyond the
  negotiated circuit limits.
- Guaranteed traversal for symmetric NAT or CGNAT without a relay.
- Android runtime evidence. The Stage 11 proofs run on Windows loopback through
  the same Rust code that Android loads.

## 10. Verification

- `nexus-net` Stage 11 tests cover reservation caps, direct-path preference,
  failed-direct-to-relay fallback, `NoViablePath`, and relay-list persistence.
- `nexus-core` Stage 11 tests cover relay list persistence, connectivity
  classification, relayed test messages, and an unreachable peer.
- `docs/progress/STAGE_11.md` records commands, counts, and exit criteria.
