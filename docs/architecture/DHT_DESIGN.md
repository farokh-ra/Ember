# DHT Design

**Status:** Implemented in Stage 10; signed profile publication added in Stage 13; signed file publication added in Stage 14
**Scope:** Kademlia peer routing, signed record publication and lookup, provider records, bootstrap entry points, and bounded discovery.

This document records the Stage 10 design and the Stage 13 and Stage 14 signed
record extensions. It is implemented by `nexus-protocol::dht`, `nexus-net`, the
`network.discover` core method, the `profile.*` core methods, and the
`publication.*` core methods. It does not introduce a new authority, account
service, or mutable global namespace.

## 1. Roles and protocol

| Role | Default | Behaviour |
|---|---|---|
| Application node | `client` | Queries the DHT, stores records routed to it, and answers requests only when promoted to server mode. |
| Bootstrap node | `server` | Participates in routing and storage for the public network. |

- The Kademlia protocol ID is `/nexus/kad/1.0.0`. It is separate from the
  public IPFS DHT and from the Stage 09 `/nexus/test/1` transport check.
- Identify advertises `/nexus/0.1.0-stage-12` and `ember/0.1.0`.
- Replication factor is 8 and query parallelism is 3. Kademlia's disjoint
  query paths reduce the chance that one malicious node controls a lookup.
- The frame cap is 80 KiB, leaving headroom over the 64 KiB signed-envelope
  cap for protocol framing and provider metadata.
- The create-time `dht_mode` config key selects `client` or `server`. The
  default is `client`. A bootstrap node is an ordinary server-mode peer; it
  receives no special protocol authority.

## 2. Keys and values

| Record | DHT key | Value | Lifetime |
|---|---|---|---|
| Publication | `sha256("NEXUS-DHT-PUB-v1" + publication_id)` | Exact signed publication envelope, at most 64 KiB | 24 h; republished while local |
| Tombstone | `sha256("NEXUS-DHT-TOMB-v1" + publication_id)` | Exact signed tombstone envelope, at most 64 KiB | 24 h; republished while local |
| Profile | `sha256("NEXUS-DHT-PROFILE-v1" + identity_id)` | Exact signed profile envelope, at most 64 KiB | 24 h; republished while local |
| Content provider | Raw 32-byte content ID | Provider peer ID plus optional addresses | 24 h; republished while serving |
| Index provider | Reserved | Reserved | Stage 16 |

Key derivation lives in `nexus-protocol::dht` and uses the existing
domain-separation tags. The network layer never accepts a caller-supplied value
under a mismatched key.

DHT values are hints:

- A publication, profile, or tombstone value is accepted only when the exact
  envelope parses, its signature verifies, and the key derived from its signed
  identifier equals the DHT key it was stored under.
- Provider identity is the authoritative part of a provider record. Addresses
  are advisory routing hints and may be empty. A non-empty list is capped at 8
  addresses of at most 256 bytes each.
- A lookup never turns a returned record into trusted product state. The core
  verifies record bytes again before indexing or presenting them.

## 3. Update rules

Records are immutable signed envelopes. Revisions are monotonic per DHT key:

| Condition | Decision |
|---|---|
| No stored record | Store |
| Incoming revision greater than stored | Replace |
| Incoming revision lower than stored | Reject as stale |
| Same revision, byte-identical | Idempotent success |
| Same revision, different bytes | Reject as conflict |
| Derived key differs | Reject as conflict |

This prevents rollback with an older signed record and prevents a peer from
shadowing a record with divergent bytes at the same revision. The counter
`dht_records_accepted` counts stores and idempotent matches;
`dht_records_rejected` counts malformed, stale, and conflicting records.

Provider records follow the same principle at the identity level: a provider is
added by peer ID, addresses are replaced by validated hints, and a malformed
key or oversized address list is rejected before it reaches the store.

### 3.1 Profile records

Stage 13 publishes profiles through the same signed-record path.

- The key is `sha256("NEXUS-DHT-PROFILE-v1" + identity_id)` and the value is the
  exact signed profile envelope using the profile domain tag.
- A profile revision starts at 1 and increases by one for every field change.
  Republishing the existing signed bytes keeps the revision unchanged.
- The core stores the signed bytes locally before announcing them. A local row
  is `draft`, `pending`, or `published`; a verified cache from another identity
  is `remote`.
- A lookup verifies the signature, the requested identity ID, and the derived
  key before caching. A newer revision replaces the cached row, an older
  revision is reported as stale without rolling back, and a same-revision
  record with different bytes is retained in `profile_conflicts` without
  replacing the accepted row.
- A cached record served after a network miss or failure is labeled stale.
  Cached rows whose last successful check is older than seven days are also
  presented as stale.
- Usernames are labels, not reservations. Collision detection compares labels
  case-insensitively and returns each distinct identity ID. Two identities may
  publish the same label without either record being invalid.

### 3.2 Publication and tombstone records

Stage 14 publishes file metadata and withdrawals through the same signed-record
path.

- The publication key is `sha256("NEXUS-DHT-PUB-v1" + publication_id)` and the
  value is the exact signed publication envelope. The publication ID is the
  SHA-256 of that signed record.
- The tombstone key is `sha256("NEXUS-DHT-TOMB-v1" + publication_id)` and the
  value is the exact signed tombstone envelope. The tombstone binds the owner
  identity, publication ID, content ID, and revision.
- A publication revision starts at 1. Stage 14 creates revision 1 only and
  treats the signed bytes as immutable; editing and revision chains are later
  work.
- The core stores the exact signed record locally before announcing it. It also
  announces a content provider record so a lookup can find the publishing peer
  by content ID. Provider identity is authoritative; addresses remain hints.
- `publication.lookup` verifies the record signature, the requested publication
  ID, and the derived key. It then looks up the tombstone key and verifies the
  tombstone against the publication before caching either record. A malformed
  or mismatched tombstone fails the lookup instead of being cached.
- A verified tombstone forces `availability = record_only`. A provider hit
  yields `available_now`; a recent earlier hit without a current provider yields
  `seen_recently`; a miss with no source yields `no_known_source`.
- `publication.unpublish` announces the tombstone and withdraws the provider
  record unless another local publication, an active replica, or a retained
  download still justifies serving the same content.
- DHT records have a 24 h TTL. Stage 14 announces on publish and unpublish but
  does not run a periodic re-announcement worker, so long-term discoverability
  depends on the hosting and republication work in later stages.

## 4. Routing and lookups

- `network.discover` runs `GetClosestPeers` for an optional 32-byte target.
  Without a target the driver samples a random key so repeated calls explore
  instead of probing one point.
- One query is bounded by `dht_query_timeout` (30 s default, 120 s maximum).
- A discovery result contains at most 32 peers, each with at most 8 addresses.
  Peers are sorted and deduplicated by peer ID.
- `routing_peers` reports the number of peers currently in the local K-buckets.
- The driver runs a Kademlia bootstrap when routing peers appear, retries at
  most every 10 s while unbootstrapped, and rebootstrap every 5 minutes.
- Provider lookups accumulate `FoundProviders` results across query steps and
  resolve addresses from provider records or the local routing table at the end
  of the query. Reading only the local store would miss remote providers because
  libp2p does not emit inbound provider announcements for outbound
  `GET_PROVIDERS` responses.
- A timed-out or failed query returns `timeout` and increments
  `dht_queries_failed`. It does not stop the node or invalidate other peers.

## 5. Bootstrap entry points

Bootstrap addresses are merged at startup in this order, deduplicated:

1. The compile-time `EMBER_BOOTSTRAP_PEERS` list.
2. The configured `NetworkConfig::bootstrap` list.
3. The persisted `network/bootstrap.list`.

Rules:

- Every entry must be a full multiaddress with `/p2p/<peer-id>`.
- At most 64 entries and 64 KiB of persisted text.
- A bootstrap peer is an entry point, not an authority. It cannot vouch for an
  identity, own a file, or override a signed record.
- Losing one bootstrap peer never disables the others. Each target has its own
  exponential retry from 1 s to 60 s.
- Adding or removing a bootstrap peer through `network.bootstrap_add` /
  `network.bootstrap_remove` updates the persisted list atomically and survives
  a restart.
- Distribution is deliberately simple: a release may bake in community entry
  points, an operator may configure addresses at build or deployment time, and
  a user or installer may add a persisted address. No central list service is
  required. See [BOOTSTRAP_NODES.md](../operations/BOOTSTRAP_NODES.md).

## 6. Resource bounds and abuse resistance

| Item | Default | Maximum |
|---|---|---|
| Stored value records | 1,024 | 8,192 |
| Value size | 64 KiB | 64 KiB |
| Providers per key | 20 | 128 |
| Locally advertised provider keys | 512 | 4,096 |
| Record/provider TTL | 24 h | 7 days |
| Query timeout | 30 s | 120 s |
| Discovered peers per result | 32 | 32 |
| Addresses per peer | 8 | 8 |
| Bootstrap entries | 64 | 64 |
| Kademlia frame | 80 KiB | 80 KiB |

- The store is in-memory and bounded. It is rebuilt by republication and
  lookups after a restart; DHT state is not durable product state.
- `StoreInserts::FilterBoth` routes every inbound record and provider through
  the validator before libp2p stores it.
- Record values are capped before CBOR decoding, then parsed with the existing
  bounded canonical-CBOR decoder.
- Provider records without addresses are allowed, but non-empty lists are
  capped. This lets a lookup use the authenticated peer ID and resolve addresses
  from routing.
- The driver keeps at most 32 discovered peers per result and 256 per-peer rate
  windows for the existing request/response protocol.

A basic poisoning attempt is limited to publishing a newer signed record under
a key the attacker can sign for. Because DHT keys are derived from signed
identifiers, an attacker cannot move a publication to another key, forge a
record for another identity, roll back a revision, or introduce a provider with
a malformed key. Sybil resistance is not global: an attacker can still run many
peers and influence which records are reachable. Product code must therefore
verify records and chunks, use multiple sources, and treat availability as
probabilistic.

## 7. Privacy and observability

- DHT queries reveal the queried key to peers that participate in the lookup.
  The sampled-target mode avoids repeatedly exposing one exact interest.
- Provider records reveal which peer serves a content ID, not the file bytes or
  the user identity behind the device peer key.
- Status counters expose counts and stable error codes, not raw addresses of
  unrelated peers or key material.
- The core does not expose raw DHT publish/lookup methods. `profile.publish`,
  `profile.lookup`, `publication.create`, `publication.unpublish`, and
  `publication.lookup` publish and retrieve signed envelopes only; arbitrary DHT
  keys and values remain network-layer APIs.

## 8. Deferred

- Durable DHT caching and record persistence across restarts.
- Periodic republication of local records before their 24 h TTL expires.
- Provider reputation, availability scoring, and multi-source download policy
  (Stage 15 and later).
- Index-provider records (Stage 16).
- Relay, hole punching, and NAT traversal (Stage 11).
- A public bootstrap list distribution service. Stage 10 documents manual,
  configured, compiled, and persisted entry points only.

## 9. Verification

Stage 10 adds:

- `nexus-protocol` DHT unit tests for key derivation, envelope verification,
  update classification, and provider validation.
- `nexus-net` Stage 10 tests for fresh-node join, record publication and lookup,
  provider discovery, bootstrap loss, and churn.
- `nexus-core` Stage 10 tests for `network.discover` completion, invalid
  targets, DHT status fields, and a terminal failure event.

Stage 13 adds:

- `nexus-store` tests for the local profile lifecycle, expected-revision
  enforcement, remote cache outcomes, conflict retention, and case-insensitive
  username-label queries.
- `nexus-core` profile worker tests for publish success, deferred publication,
  verified caching, collisions, stale caches, revision rollback, and identity
  mismatch rejection.
- `nexus-core/tests/stage13.rs`, where two real engines publish and retrieve
  signed profiles through loopback DHT servers, observe a newer revision, and
  serve a stale-labeled cache after both servers stop.

Stage 14 adds:

- `nexus-store` tests for publication metadata and tags, exact remote caching,
  cache conflicts, the serving gate, tombstone state, and manifest persistence.
- `nexus-core` publication worker tests for record and provider announcement,
  deferred publication, tombstone announcement and provider withdrawal, verified
  caching, and publication ID mismatch rejection.
- `nexus-core/tests/stage14.rs`, where a publisher imports, signs, announces,
  serves, and withdraws a file while a second engine discovers the record,
  verifies it, downloads the exact bytes, and observes the signed tombstone.

All network tests bind loopback addresses and use ordinary peers. No test
depends on a central account service or an external bootstrap operator.
