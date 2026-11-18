# Bootstrap Node Operations

**Applies to:** Stage 10 and later
**Audience:** Community operators and release engineers who publish entry points

Bootstrap nodes help new peers find the network. They are ordinary
authenticated peers running in DHT server mode. They do not own identities,
approve accounts, store user files, or decide who may join.

## 1. What a bootstrap node does

- Accepts libp2p connections over QUIC v1 and TCP + Noise + Yamux.
- Speaks `/nexus/kad/1.0.0` in Kademlia server mode.
- Answers routing queries and stores a bounded set of signed records and
  provider advertisements.
- Advertises its own peer ID through identify.
- Forwards nothing. There is no proxy or central account service.

It does **not**:

- Sign records on behalf of users.
- Verify or host user files.
- Authorize an identity or a publication.
- Provide a stable source of truth. DHT records are hints that clients verify.

## 2. Running a server

A bootstrap node is a normal deployment with DHT server mode enabled.

Network layer:

```text
NetworkConfig {
    dht_mode: DhtMode::Server,
    listen: vec![
        "/ip4/0.0.0.0/udp/4001/quic-v1",
        "/ip4/0.0.0.0/tcp/4001",
    ],
    bootstrap: vec![/* other independent entry points */],
    ..
}
```

Core embedding:

```json
{
  "network_enabled": true,
  "dht_mode": "server"
}
```

The default is `dht_mode: "client"`. A client still queries and stores records
routed to it, but it does not accept inbound Kademlia requests. Only a server
can act as an entry point for other peers.

Recommended baseline:

- Keep both QUIC (UDP) and TCP listeners enabled. TCP is the fallback for
  networks that block UDP.
- Use a stable peer key at `network/peer_key.enc`. The peer ID is derived from
  it, so replacing the data root changes the address operators publish.
- Publish the full `/p2p/<peer-id>` multiaddress, for example
  `/dns4/bootstrap.example.org/udp/4001/quic-v1/p2p/12D3Koo...`.
- Keep time and the host clock correct; certificates and QUIC handshakes depend
  on it.
- Monitor process liveness, listener sockets, `network.status` counters, and
  disk space for local logs. DHT records are in memory and bounded.

## 3. Independent operators and diversity

Users should not depend on one operator, one host, or one network path.

- Publish at least two entry points run by different operators.
- Prefer different hosting providers, regions, and autonomous systems.
- Publish one DNS name and, where possible, one IP-based address so a DNS
  outage is not a single point of failure.
- Keep entry points on different software update schedules.
- Never publish a bootstrap address without `/p2p`. The client rejects it.

The client merges entry points from all sources and retries each target
independently. Losing one bootstrap node does not disable the network.

## 4. Distributing and updating addresses

Stage 10 supports four entry-point sources:

| Source | Use |
|---|---|
| Compile-time `EMBER_BOOTSTRAP_PEERS` | Release builds that ship community entry points. Semicolon, comma, or newline separated full multiaddresses. |
| `NetworkConfig::bootstrap` | Deployments that inject addresses from their own configuration system. |
| `network/bootstrap.list` | Per-installation persistence, managed through `network.bootstrap_add` and `network.bootstrap_remove`. |
| Manual `network.connect` | User-entered peer address for a one-off connection, without persisting it as bootstrap. |

Startup merges all sources, removes duplicates, and keeps the order above.
Changing a compiled list requires a new release; the configured and persisted
lists can change without one.

Recommended update process:

1. Stand up the new node and confirm it is reachable from outside the host.
2. Add its `/p2p` address to the operator's published list and to at least one
   release or deployment configuration.
3. Keep the previous node running during the overlap.
4. Remove the old address only after clients have had time to update.

There is no automatic global bootstrap-list update service in Stage 10. Every
list is an operator or release decision, which keeps the network from acquiring
a central directory.

## 5. Firewall and network checklist

- Allow inbound UDP and TCP on the published ports.
- Allow outbound UDP and TCP for peer connections and DNS.
- Do not place a TCP-only load balancer in front of QUIC. A TCP proxy terminates
  the transport and breaks peer authentication.
- If using NAT, forward the published ports and confirm that the advertised
  address is the external one. Addresses reported by peers are observations, not
  proof of reachability.
- Restrict shell and management access separately. The peer protocol has no
  administrative surface.

## 6. Monitoring and failure handling

Useful signals from `network.status`:

| Field | Meaning |
|---|---|
| `dht_mode` | Should be `server` on an entry point. |
| `dht_routing_peers` | Peers currently in the local routing table. |
| `dht_bootstrapped` | A Kademlia bootstrap query has completed. |
| `dht_queries_started` / `dht_queries_failed` | Query volume and failures. |
| `dht_records_accepted` / `dht_records_rejected` | Inbound record validation. |
| `dht_providers_accepted` / `dht_providers_rejected` | Inbound provider validation. |
| `last_error` | Stable code for the most recent network failure. |

Expected behavior:

- A restart loses the in-memory DHT store. Clients republish and look up again.
- A bootstrap node can be offline for maintenance; other entry points and
  already-connected peers remain usable.
- Malformed records and provider announcements are rejected and counted; they
  do not crash the node.
- A full store evicts or refuses new records within the configured bounds; it
  does not grow without limit.

Alert on:

- Listener loss or repeated start failure.
- A sustained rise in `dht_queries_failed` or rejection counters.
- Process restarts more often than the update schedule.
- Disk exhaustion from local logs (record storage is memory-only).

## 7. Security and privacy expectations

- Run the node on a dedicated host or account. Do not co-locate it with user
  data or identity backups.
- The node observes peer IDs, addresses, and queried DHT keys. It does not
  receive file bytes, private keys, or account credentials.
- Traffic is authenticated and encrypted by the transport. Stage 10 has no
  relay or anonymous routing.
- Do not log raw peer addresses or query keys beyond the documented redacted
  counters. See [LOGGING_POLICY.md](../LOGGING_POLICY.md).
- Keep the peer key backed up separately from user identity material. Losing it
  changes the node's peer ID and invalidates published addresses.
- A bootstrap node is not a certificate authority. Clients verify every signed
  record and every content chunk themselves.

## 8. Related documents

- [DHT design](../architecture/DHT_DESIGN.md)
- [Architecture, section 9](../ARCHITECTURE.md)
- [ADR-0005: libp2p transports and discovery](../decisions/ADR-0005-libp2p-transports-discovery.md)
