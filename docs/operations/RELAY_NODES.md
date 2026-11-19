# Relay Node Operations

**Applies to:** Stage 11 and later
**Audience:** Community operators who publish relay capacity

Relay nodes help peers connect when a direct path is blocked by NAT, a
firewall, or carrier-grade NAT. They forward Circuit Relay v2 traffic. They do
not own identities, approve accounts, store user files, or inspect content.

A relay can run on the same host as a bootstrap node, but the two roles are
independent. Enabling relay serving does not make a node a bootstrap entry
point, and DHT server mode does not enable relay serving.

## 1. What a relay node does

- Accepts libp2p connections over QUIC v1 and TCP + Noise + Yamux.
- Advertises the Circuit Relay v2 HOP protocol after it has a confirmed
  external address.
- Grants bounded reservations and forwards bounded circuits.
- Optionally attempts DCUtR between two relayed peers so they can upgrade to a
  direct connection.
- Advertises its own peer ID through identify.

It does **not**:

- Read, verify, or store user content. Relayed bytes stay inside the peers'
  Noise-encrypted connection.
- Sign records on behalf of users.
- Authorize an identity or publication.
- Provide a stable source of truth.
- Guarantee that every NAT combination can be traversed.

## 2. Running a relay

Relay serving is off by default. A relay deployment enables it explicitly and
must publish a reachable address.

Network layer:

```text
NetworkConfig {
    relay_server_enabled: true,
    external_addresses: vec![
        "/ip4/203.0.113.10/udp/4001/quic-v1".parse()?,
        "/ip4/203.0.113.10/tcp/4001".parse()?,
    ],
    listen: vec![
        "/ip4/0.0.0.0/udp/4001/quic-v1".parse()?,
        "/ip4/0.0.0.0/tcp/4001".parse()?,
    ],
    // Optional: keep the relay connected to bootstrap entry points.
    bootstrap: vec![/* independent entry points */],
    ..NetworkConfig::default()
}
```

Core embedding:

```json
{
  "network_enabled": true,
  "relay_server": true
}
```

`external_addresses` is bounded to 8 entries. Each address must be a full
multiaddress without `/p2p`; libp2p appends the local peer ID. A relay that has
no confirmed external address does not advertise HOP. Loopback listeners are
advertised automatically when relay serving is enabled, which is useful for
tests and single-machine setups but is not public infrastructure.

Recommended baseline:

- Keep both QUIC (UDP) and TCP listeners enabled. A peer may be able to reach
  only one transport.
- Publish the same host and port in `external_addresses` that port forwarding
  or the host firewall exposes.
- Use a stable peer ID. Peers persist the relay's `/p2p/<peer-id>` and treat a
  changed key as a different relay.
- Run at least two relays on different hosts if the deployment depends on
  relay availability.

## 3. Limits and capacity

Every relay enforces the following bounds:

| Item | Default | Configurable maximum |
|---|---|---|
| Active reservations | 128 | 1,024 |
| Active circuits | 16 | 256 |
| Circuits per source peer | 4 | active circuits |
| Circuit duration | 2 min | 1 h |
| Circuit bytes | 128 KiB | 16 MiB |

The reservation lifetime is libp2p's default of 1 h and is renewed by the
client. A relay accepts circuits first-come, first-served within these bounds.

Size a relay from measured circuit concurrency, not from the number of
reservations alone:

- Each active circuit is a bounded stream pair, but both directions share the
  circuit byte limit.
- A 128 KiB limit is enough for discovery and small control messages, not for
  file transfer. Large transfers are expected to use direct connections or be
  chunked by later product stages.
- The relay does not expose raw forwarded-byte counters through libp2p. Ember
  reports the negotiated limit and established/closed circuit counts. Capacity
  planning should use host network counters and the `relay_server` status
  fields.

The relay copies its limits into every granted reservation and circuit. A
client's `network.status` shows `relay_data_limit_bytes` and
`relay_duration_limit_secs` from the most recent grant.

## 4. Firewall and network rules

- Allow inbound UDP and TCP on the advertised listen ports.
- Allow outbound connections to bootstrap peers and to peers that may be
  relayed. A relay with no outbound path can still serve inbound circuits over
  established connections, but it cannot bootstrap or probe reachability.
- Do not place a relay behind a NAT unless the NAT forwards the advertised
  ports and preserves the external address.
- Do not enable relay serving on a node that cannot accept inbound
  connections. The HOP protocol stays unadvertised until an external address is
  confirmed, but a misconfigured published address still produces failed
  circuits.
- Rate-limit at the host or network edge if the default circuit bounds are too
  generous for the available bandwidth.

## 5. Monitoring

Useful fields from `network.status` on the relay:

| Field | Meaning |
|---|---|
| `relay_server_enabled` | Relay serving is configured. |
| `relay_server_reservations` | Active reservations granted. |
| `relay_server_circuits` | Active circuits. |
| `relay_server_circuits_accepted` | Circuits accepted since start. |
| `relay_server_circuits_closed` | Circuits closed since start. |
| `relay_server_denied` | Reservation or circuit requests denied by bounds. |
| `relay_server_max_circuit_bytes` | Configured circuit byte limit. |
| `relay_server_max_circuit_duration_secs` | Configured circuit duration limit. |

A rising `relay_server_denied` counter with full reservations or circuits means
the relay is at capacity. A relay that never leaves `StatusChanged` disabled
has no confirmed external address or has serving turned off.

Clients report:

- `relays_configured`, `relays_connected`, `relay_reservations`;
- `relay_reservation_failures`;
- `relay_circuits_established`;
- `fallback_attempts`, `fallback_successes`;
- `relayed_connections`, `direct_connections`, `direct_upgrades`,
  `hole_punch_failures`.

## 6. Client configuration

Users and deployments can add a relay without rebuilding:

```text
network.relay_add  { "multiaddr": "/ip4/203.0.113.10/tcp/4001/p2p/12D3Koo..." }
network.relay_list {}
network.relay_remove { "multiaddr": "/ip4/203.0.113.10/tcp/4001/p2p/12D3Koo..." }
```

The list is persisted in `network/relays.list`, bounded to 16 entries, and
merged with `EMBER_RELAY_NODES` and `NetworkConfig::relays`. A relay address
must end in `/p2p/<peer-id>`.

## 7. Privacy and abuse

- The relay forwards Noise-encrypted bytes and cannot read file contents,
  record payloads, or search queries.
- The relay can observe peer IDs, circuit timing, circuit counts, and
  negotiated byte limits.
- The relay does not authenticate users or content. Peers still verify every
  signed record and chunk through the normal protocol.
- A relay can refuse service, delay circuits, or disappear. It cannot forge a
  record or change a peer ID without failing authentication.
- Do not log peer addresses or circuit metadata longer than needed for
  operations. Follow the repository logging policy and redact identifiers.

## 8. Failure behaviour

- A refused reservation is retried after 5 s.
- A closed reservation is retried after 5 s while the relay stays connected.
- A relay that disconnects is removed from the connected set; its reservation
  state is discarded.
- A failed circuit falls through to the next connected relay.
- Direct connections are always attempted first. A relay is not used when a
  direct path succeeds.
- A foreground connect with no direct and no relay path returns
  `NoViablePath`; the UI should show the connectivity hint and last error.

## 9. Relationship to bootstrap nodes

| Concern | Bootstrap node | Relay node |
|---|---|---|
| Main purpose | Entry point and DHT routing | Forward circuits when direct fails |
| Default role | DHT `server` | Relay client; serving off |
| Required for basic use | At least one entry point | No; optional fallback |
| Can combine roles | Yes | Yes |
| Trust | Entry point, not authority | Forwarder, not authority |

A small community can publish one host that runs both roles, but independent
hosts and operators reduce correlated failure. Relay distribution follows the
same release, deployment, and operator channels as bootstrap entries.

## 10. Verification

- `nexus-net` Stage 11 tests run a real loopback relay and verify reservations,
  fallback, and the absence of a viable path.
- `nexus-core` Stage 11 tests verify relay list methods, connectivity
  classification, a relayed test message, and an unreachable peer.
- `docs/progress/STAGE_11.md` records the exact commands and results.
