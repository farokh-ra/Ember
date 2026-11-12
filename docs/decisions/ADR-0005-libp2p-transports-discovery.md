# ADR-0005 — libp2p transports and discovery stack

- **Status:** Accepted
- **Date:** 2026-11-12
- **Deciders:** NEXUS engineering
- **Supersedes:** none
- **Related:** ADR-0002, ADR-0006, ADR-0007, ADR-0008, ADR-0010

## Context

NEXUS must connect Android and Windows peers directly, authenticate peer identities, discover peers and records without a central account service, and remain usable when some sources are offline. The roadmap selects Rust and libp2p. The networking design must be realistic for mobile networks and NAT, must not silently depend on a company endpoint, and must leave room for relay support in a later stage.

The application also has a user signing identity that must not become a permanent network tracking identifier. A user may restore the same identity on two devices, and each device should be able to appear as a separate peer.

## Decision

Use **rust-libp2p 0.57.x** as the transport and peer-routing foundation, pinned exactly in the dependency inventory.

### Transports

| Priority | Transport | Purpose |
|---|---|---|
| 1 | QUIC v1 over UDP | Preferred encrypted, multiplexed transport with lower handshake latency. |
| 2 | TCP + Noise + Yamux | Fallback where UDP or QUIC is blocked, filtered, or unavailable on a platform. |
| 3 | DNS address resolution | Resolve bootstrap and user-entered DNS multiaddresses. |

- Both transports use authenticated peer identities. An unauthenticated or identity-mismatched connection is rejected by libp2p before application protocols run.
- The node advertises only addresses it can actually listen on or observe through a supported address-discovery mechanism. It never claims to be publicly reachable without evidence.
- Connection establishment, identify, ping, and request timeouts are bounded. A failed attempt produces a visible offline or degraded state, not a fabricated online state.
- Relay, hole punching, and advanced NAT traversal are **not part of v1**. They remain isolated behind the transport layer for Stage 11.

### Peer identity

- Each installation generates a **device peer key** independent of the user's signing identity. The peer ID identifies the device on the network, not the person.
- The peer key is stored encrypted under the host-provided local secret. Restoring a user identity backup does not copy or overwrite the device peer key.
- Publication, profile, and tombstone records are signed by the user identity and verified independently of the transport peer ID. A replica is allowed to serve a record signed by someone else.
- This separation limits linkability between a user identity and a device's network address. It does not provide anonymity; peers can still observe addresses of peers they connect to.

### Discovery and records

Use **Kademlia DHT** for peer routing and bounded record lookup, with these rules:

- Application nodes default to Kademlia client mode. Community bootstrap nodes may run the same core in server mode as a separate headless build; the app does not become a public DHT router by default.
- v1 ships with a release-configured list of at least three independently operated bootstrap addresses, plus user-added and manually entered peers. The list is compiled into the release and documented in release notes; there is no runtime company-owned bootstrap service.
- Losing one bootstrap peer never disables the remaining paths. The node retries with backoff and reports partial connectivity.
- DHT keys are domain-separated hashes. Provider records use the raw content ID; value records use the prefixes defined in ADR-0006 and the protocol boundary document.
- DHT values are capped at 64 KiB. Records larger than the cap are rejected before storage. Manifests are fetched through the transfer protocol, not stored in one DHT value.
- Publication and provider records are republished while the node is serving, with a 24-hour record lifetime and bounded retry. Lookups have a total deadline and never hang indefinitely.
- Invalid signatures, unknown major versions, oversized values, and malformed CBOR are rejected or quarantined before they can affect valid results.
- DHT results are hints, not proof. A publication record is accepted only after signature and schema verification. A source is accepted only after it serves a valid chunk.

### Local and manual discovery

- v1 supports manual peer entry and configured bootstrap paths for local and internet connections.
- mDNS is **not enabled in v1**. Android multicast permissions, battery cost, and unintended LAN presence disclosure are not justified for the first release. It remains a candidate for a later opt-in setting.
- A local connection without internet works through manual peer entry. The UI reports a clear failure when no path is available.

### Protocol boundaries

The network layer exposes request/response protocols and record exchange, but does not own publication, storage, search, or replication policy. Initial protocol IDs:

| Protocol ID | Purpose |
|---|---|
| `/nexus/record/1` | Fetch a signed publication, profile, or tombstone record by key. |
| `/nexus/manifest/1` | Fetch the bounded chunk manifest for a content ID. |
| `/nexus/chunk/1` | Fetch one verified chunk by content ID and index. |
| `/nexus/index/1` | Reserved for Stage 16 distributed index queries. |

The major version is part of the protocol ID. Unknown major versions are rejected with a version error. The wire schemas and limits are owned by `nexus-protocol`.

### Resource limits

- Maximum inbound and outbound connections, streams per connection, request size, response size, and requests per peer per minute are configurable and enforced.
- One download uses one source at a time in v1. Sequential failover is allowed; simultaneous multi-source scheduling is later scope.
- Bandwidth limits are enforced at the transfer layer, not by the operating system.
- Idle nodes do not hold wake locks and do not perform unbounded background chatter.

## Consequences

**Positive**

- A mature, audited P2P stack provides authenticated transports, multiplexing, DHT, and request/response primitives.
- Android and Windows use the same networking code and wire protocol.
- Separate peer and user identities reduce linkability and support multi-device identities.
- A clear v1 transport subset keeps NAT failure honest and leaves relay work isolated.
- Static, multi-operator bootstrap defaults avoid a mandatory company service.

**Negative / trade-offs**

- libp2p APIs change between releases; pinning and cross-platform verification are mandatory.
- QUIC on Android has platform and network variability; TCP fallback must be tested.
- Client-only app nodes rely on community bootstrap nodes for DHT routing in v1.
- Without relay or mDNS, some local and restrictive networks cannot connect in v1.
- Separate peer identities mean users cannot infer "same device" from a peer ID; the UI must present devices and identities distinctly.

**Constraints**

- No mandatory central rendezvous, account, or search service.
- No default relay dependency.
- No application data is sent before transport authentication.
- Every inbound record and message is untrusted until verified.
- Transport changes that alter reachability or privacy require an ADR update.

## Alternatives considered

1. **Custom UDP/TCP protocol.** Maximum control, but duplicates encryption, multiplexing, NAT, and peer routing, with a large security surface. Rejected.
2. **WebRTC data channels.** Good browser NAT traversal, but a heavy stack, weak fit for a Rust core and background Android service, and no native DHT. Rejected.
3. **IPFS/Kubo as the network layer.** Adds a large external runtime, pinning and gateway assumptions, and a different content model. Rejected.
4. **Central rendezvous and search API.** Simplest discovery, but violates the no-central-backend rule. Rejected.
5. **Use the user identity key as the libp2p peer key.** Simplifies key management, but permanently links the user identity to network addresses and device churn. Rejected.
6. **Enable mDNS by default for local discovery.** Convenient for demos, but leaks presence on LANs and adds Android multicast complexity. Deferred.
7. **Run every app node as a DHT server.** Improves routing, but increases mobile battery, attack surface, and inbound connection load. Rejected; headless community nodes can serve.

## Verification

- Stage 09 proves two devices connect over QUIC and TCP, exchange an authenticated test message, reject a mismatched peer identity, and handle network changes and 100 start/stop cycles.
- Stage 10 proves DHT publish/lookup, multi-bootstrap failover, bounded lookup timeouts, malformed-record rejection, churn, and operation without a central account service.
- Stage 11 adds relay and difficult-network evidence; it must not change the v1 protocol silently.
- Stage 19 measures idle battery, connection limits, and long-running stability on both platforms.
