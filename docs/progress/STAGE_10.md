# Stage 10 - Bootstrap, DHT and Decentralized Peer Discovery

**Status:** Complete
**Completed:** 2026-11-18
**Progress guide:** 43-49%
**Next stage:** 11 - NAT traversal, relays, and difficult networks (requires user permission)

## 1. Scope

Stage 10 implements the decentralized discovery layer required by
[ADR-0005](../decisions/ADR-0005-libp2p-transports-discovery.md) and
[ADR-0007](../decisions/ADR-0007-protocol-boundaries-versioning.md) on top of
the Stage 09 transports. It does not introduce a central account service,
directory, or identity authority.

The stage covers:

- A documented Kademlia record and routing design in
  [DHT_DESIGN.md](../architecture/DHT_DESIGN.md).
- libp2p Kademlia through the pinned `0.57.1` graph and its `kad` feature,
  protocol `/nexus/kad/1.0.0`.
- Client and server DHT modes. Ordinary devices default to client; bootstrap
  nodes run server mode.
- Exact signed-envelope DHT values for publications, profiles, and tombstones,
  with key-derived verification and monotonic revisions.
- Bounded in-memory value and provider records with TTL and republish behavior.
- Provider discovery for content IDs.
- Multiple independent bootstrap entry points from compiled, configured, and
  persisted sources.
- Bounded `network.discover` peer routing with one terminal event per operation.
- Protections against malformed records, oversized values, stale or conflicting
  revisions, unbounded provider address lists, and a bounded form of DHT
  poisoning.
- Loopback churn, bootstrap-loss, and fresh-join tests.

Relay, hole punching, NAT traversal, mDNS, download scheduling, provider
reputation, and index-provider records remain later-stage work.

## 2. Architecture

The new responsibilities are split as follows:

| Layer | Responsibility |
|---|---|
| `nexus-protocol` | DHT key derivation, signed-envelope verification, record kind and revision extraction, update classification, provider key and address validation. |
| `nexus-net` | Kademlia behaviour and bounded memory store, DHT driver commands, bootstrap merge, lookup deadlines, provider accumulation, inbound validation, status counters. |
| `nexus-core` | `network.discover` dispatch, configuration, worker execution, terminal events, domain-error mapping, DHT status fields. |
| `nexus-ffi` | Config field plumbing and stable status mapping; no ABI shape change. |

`nexus-net` still owns the single swarm thread. DHT calls use the same bounded
command channel and one-shot reply pattern as Stage 09. `nexus-core` never links
libp2p directly; it submits one discovery query to its worker pool and emits a
terminal event.

## 3. Keys, values, and update rules

| Record | DHT key | Value |
|---|---|---|
| Publication | `sha256("NEXUS-DHT-PUB-v1" + publication_id)` | Exact signed publication envelope |
| Tombstone | `sha256("NEXUS-DHT-TOMB-v1" + publication_id)` | Exact signed tombstone envelope |
| Profile | `sha256("NEXUS-DHT-PROFILE-v1" + identity_id)` | Exact signed profile envelope |
| Content provider | Raw 32-byte content ID | Provider peer ID plus optional addresses |

Rules:

- A value is accepted only when it parses as one of the supported signed
  envelopes, its signature verifies, and the key derived from its signed
  identifier equals the DHT key it was stored under.
- The maximum value is 64 KiB, matching the signed-envelope cap.
- Provider addresses are advisory and may be empty. A non-empty list is capped
  at 8 addresses of at most 256 bytes each.
- A lookup returns hints. The core must verify record bytes again before using
  them as product state.
- Revisions are monotonic per key. A higher revision replaces the stored record;
  a lower revision is rejected as stale; the same revision with identical bytes
  is an idempotent success; the same revision with different bytes is rejected
  as a conflict.
- The store is in memory and bounded. It is not durable product state and is
  rebuilt by publication and lookup after a restart.

## 4. Routing and discovery

- The protocol is `/nexus/kad/1.0.0`, private to Ember and separate from the
  public IPFS DHT.
- Replication factor is 8 and query parallelism is 3. Kademlia's disjoint query
  paths reduce the chance that one malicious node controls a lookup.
- `network.discover` runs `GetClosestPeers` for an optional 32-byte target.
  Omitted targets sample a random key so repeated calls explore the network
  instead of probing one point.
- One query is bounded by `dht_query_timeout`: 30 s default, 120 s maximum.
- A result contains at most 32 peers, sorted and deduplicated by peer ID, with
  at most 8 addresses each.
- A fresh node bootstraps when routing peers appear, retries at most every 10 s
  while unbootstrapped, and rebootstraps every 5 minutes while online.
- A failed or timed-out query returns `timeout`, increments
  `dht_queries_failed`, and leaves the node and other peers usable.
- Provider lookups accumulate `FoundProviders` results across query steps.
  Addresses resolve from provider records or the routing table at the end of
  the query, because outbound `GET_PROVIDERS` results are not inserted into the
  local store by libp2p.

## 5. Bootstrap entry points

Startup merges three sources in this order and removes duplicates:

1. Compile-time `EMBER_BOOTSTRAP_PEERS`.
2. `NetworkConfig::bootstrap` supplied by the embedding deployment.
3. Persisted `network/bootstrap.list`.

Every entry must be a full multiaddress with `/p2p/<peer-id>`. The list is
bounded to 64 entries and 64 KiB. Adds and removes through
`network.bootstrap_add` / `network.bootstrap_remove` are atomic and survive a
restart. Each target has its own exponential retry from 1 s to 60 s, so losing
one entry point does not disable the others.

A bootstrap peer is an entry point, not an authority:

- It cannot vouch for an identity.
- It cannot own a file or override a signed record.
- It cannot change the DHT key a record was published under.
- It receives no special protocol message or administrative surface.

[BOOTSTRAP_NODES.md](../operations/BOOTSTRAP_NODES.md) documents operator
setup, distribution, updates, diversity, firewall rules, and monitoring.

## 6. Protections and bounds

| Item | Default | Maximum |
|---|---|---|
| Stored value records | 1,024 | 8,192 |
| Value size | 64 KiB | 64 KiB |
| Providers per key | 20 | 128 |
| Locally advertised provider keys | 512 | 4,096 |
| Record and provider TTL | 24 h | 7 days |
| Query timeout | 30 s | 120 s |
| Kademlia frame | 80 KiB | 80 KiB |
| Discovered peers per result | 32 | 32 |
| Addresses per peer | 8 | 8 |
| Bootstrap entries | 64 | 64 |

- `StoreInserts::FilterBoth` sends every inbound record and provider through
  the validator before libp2p stores it.
- Values are capped before CBOR decoding and use the existing bounded canonical
  decoder.
- Provider announcements with an invalid key or oversized address list are
  rejected and counted.
- The store is bounded; a full store refuses new entries instead of growing
  without limit.
- The driver keeps at most 32 discovered peers per result and at most 256
  per-peer rate windows for the existing request/response protocol.

These controls reject malformed values, rollback attempts, same-revision
conflicts, forged ownership, and malformed provider records. They do not make
the DHT Byzantine-fault-tolerant: an attacker can still run many peers and
influence which records are reachable. Product code must verify records and
chunks, use multiple sources, and treat availability as probabilistic.

## 7. Core integration

Stage 10 adds one method to the ABI 1 catalog:

| ID | Method | Mode | Request | Result |
|---|---|---|---|---|
| 0x0609 | `network.discover` | A | `target?` (32-byte key) | `{op_id, accepted}`, then one `network.discovery_*` event |

Behaviour:

- `target` must be exactly 32 bytes when present. A wrong type or length is an
  ABI-level `NEXUS_STATUS_INVALID_ARGUMENT` before an operation starts.
- The worker runs one bounded query and emits exactly one terminal event with
  `op_id`.
- `network.discovery_completed` carries `target` (null when sampled), `peers`,
  `routing_peers`, and `elapsed_ms`.
- `network.discovery_failed` carries `target` and a redacted `error` with a
  stable `net_code`.
- `network.discovery_cancelled` carries `target`.
- `dht_mode` is a create-time config key: `client` (default) or `server`.
- `network.status` adds `dht_mode`, `dht_routing_peers`, `dht_bootstrapped`,
  `dht_queries_started`, `dht_queries_failed`, `dht_records_accepted`,
  `dht_records_rejected`, `dht_providers_accepted`, `dht_providers_rejected`,
  `dht_publish_successes`, and `dht_publish_failures`.
- `system.health`, `system.snapshot`, and `system.info` include the extended
  network state and the `network.discover` capability.

The protocol version string is `0.1.0-stage-10`; the ABI remains 1. The FFI
interface document is at version 1.3.0 with fifteen callable methods.

## 8. Peer diversity

Diversity is addressed at the entry-point and query level:

- Releases may bake in community entry points, deployments may configure their
  own, and users may persist additional peers. No source is required.
- Operators are instructed to publish at least two entry points on different
  hosts, regions, and software schedules.
- Each bootstrap target retries independently and never replaces another.
- Queries use Kademlia disjoint paths and bounded parallelism rather than a
  single designated peer.
- Discovery returns a bounded set of candidate peers; later stages choose
  multiple sources for a download.

Global Sybil resistance and availability scoring are not claimed. They are
deferred with provider reputation and multi-source download policy.

## 9. Verification evidence

### 9.1 `nexus-protocol` DHT unit tests

4 tests, 0 failures:

| Test | Covers |
|---|---|
| `signed_records_derive_kind_and_key` | Publication, tombstone, and profile key derivation and kind detection. |
| `rejects_wrong_keys_bad_signatures_and_malformed_values` | Wrong key, changed bytes, bad signature, malformed CBOR, and oversized values. |
| `revision_is_exposed_and_updates_are_monotonic` | Revision extraction, replacement, stale, idempotent, and conflict decisions. |
| `provider_keys_use_raw_content_ids` | Raw 32-byte provider keys and address list validation. |

### 9.2 `nexus-net` Stage 10 integration tests

4 tests, 0 failures, run with `--test-threads=1` in about 9 s:

| Test | Covers |
|---|---|
| `fresh_node_joins_and_exchanges_dht_records` | Two server entry points; a fresh client joins, discovers peers, publishes and looks up a signed profile, discovers a provider, rejects invalid keys and values, and validates provider announcements. |
| `losing_one_bootstrap_peer_keeps_the_network_reachable` | A node joins through two entry points; after one shuts down, discovery still succeeds through the survivor. |
| `node_churn_does_not_disable_discovery` | Three client join/leave cycles against a server; the server stays online and a later node still joins and discovers peers. |
| `failed_publish_is_counted_once` | A publish with an empty routing table fails as `Rejected` and increments the query-failure and publish-failure counters exactly once. |

Every listener is loopback-only. No test depends on an external service,
central account server, or public bootstrap operator.

### 9.3 `nexus-core` Stage 10 tests

2 integration tests and 1 unit test, 0 failures:

| Test | Covers |
|---|---|
| `discovery_joins_through_bootstrap_and_reports_dht_status` | Two cores; a bootstraps through b, `network.discover` completes with the bootstrap peer, targeted discovery works, and status reports mode, routing, counters, and the persisted bootstrap list. |
| `discovery_rejects_malformed_targets_without_starting_an_operation` | Short and non-byte targets fail as `InvalidArgument` and do not increment `dht_queries_started`. |
| `discovery_failure_emits_a_terminal_event` | A stopped network produces exactly one `network.discovery_failed` event with `network_unavailable` and `net_code=not_running`. |

### 9.4 Workspace totals

`cargo test --workspace -- --test-threads=1` passed with 214 tests and 0
failures:

| Suite | Tests |
|---|---:|
| `nexus-core` unit tests | 28 |
| `nexus-core` Stage 09 tests | 4 |
| `nexus-core` Stage 10 tests | 2 |
| `nexus-crypto` unit tests | 24 |
| `nexus-ffi` unit tests | 4 |
| `nexus-ffi` C ABI tests | 6 |
| `nexus-identity` unit tests | 9 |
| `nexus-identity` integration tests | 12 |
| `nexus-net` unit tests | 16 |
| `nexus-net` Stage 09 tests | 8 |
| `nexus-net` Stage 10 tests | 4 |
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
| **Total** | **214** |

Stage 09 had 201 tests; Stage 10 adds 13: 4 protocol DHT unit tests, 2 network
unit tests, 4 network Stage 10 tests, 1 core DHT unit test, and 2 core Stage 10
tests.

### 9.5 Formatting, linting, and dependencies

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo deny check` reports `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo audit` scans 408 dependencies with no vulnerability and one allowed
  unmaintained warning (`paste`, RUSTSEC-2024-0436, Linux-only through
  `if-watch`/netlink).
- The dependency and license inventory is at version 1.7.0. `Cargo.lock`
  resolves 408 packages; Kademlia adds `libp2p-kad` 0.49.0 and `uint` 0.10.2 to
  the Stage 09 graph.

## 10. Exit criteria

| Criterion | Evidence |
|---|---|
| A fresh install joins through at least one bootstrap path. | `fresh_node_joins_and_exchanges_dht_records`, `discovery_joins_through_bootstrap_and_reports_dht_status`. |
| Peers discover each other without a central account service. | DHT peer and provider discovery in both Stage 10 suites; no account or directory service exists in the protocol. |
| Losing one bootstrap peer does not disable the network. | `losing_one_bootstrap_peer_keeps_the_network_reachable`, plus independent per-target retry and merged entry-point sources. |
| Malformed records and resource exhaustion are bounded. | Protocol unit tests, `StoreInserts::FilterBoth`, value/address caps, bounded store, and TTLs. |
| Bootstrap peers are entry points, not authorities. | No authority message exists; records verify by signature and derived key; documented in DHT and operations guides. |
| Churn is tested. | `node_churn_does_not_disable_discovery`. |

## 11. Deferred Android runtime evidence

No Android emulator or physical device is available in this environment.
Therefore:

- The Android arm64-v8a and x86_64 libraries cross-build with the same libp2p
  and Kademlia graph.
- The Stage 10 DHT proofs run on Windows loopback through the same Rust code
  that Android will load.
- Android runtime DHT behavior, mobile network switching, and battery impact
  remain deferred.

This is not claimed as Android runtime evidence.

## 12. Failure evidence and fixes

1. Kademlia defaults to client mode, and a client denies inbound DHT
   substreams. Two clients could not answer each other's routing queries, so
   the tests initially timed out. The design now exposes `dht_mode`, uses
   server mode for entry points, and keeps client mode for ordinary devices.
2. `MemoryStore::put` rejects values whose length is greater than or equal to
   `max_value_bytes`. The store is configured with `DHT_MAX_VALUE_BYTES + 1` so
   a maximum-size signed envelope is still accepted.
3. An outbound `GET_PROVIDERS` query returns `FoundProviders` events but does
   not populate the local provider store. Reading only the local store at the
   end of the query lost remote providers. The driver now accumulates peer IDs
   across query steps and resolves addresses from stored provider records or
   the routing table.
4. A provider can announce no addresses. Requiring one address made valid
   records fail; empty lists are now accepted and resolved from routing, while
   non-empty lists remain capped.
5. The first core test expected provider counters on the querying client. Only
   server-mode bootstrap peers receive inbound provider announcements, so the
   assertion now waits for either server's accepted or rejected counter to
   advance. This reflects real client/server behavior instead of weakening the
   check.
6. The core test expected a `bootstrap` array inside `network.status`.
   Bootstrap entries are exposed by `network.bootstrap_list` (0x0603); status
   reports DHT state and counters. The test now checks the correct method.
7. `network.discover` initially returned a bare method error for a malformed
   target. It now validates in dispatch and returns
   `NEXUS_STATUS_INVALID_ARGUMENT` before creating an operation, so no
   operation or event is leaked.
8. Debug `eprintln!` probes from provider troubleshooting were removed before
   the evidence run.
9. Recheck found that a failed `PutRecord` query counted the publish failure in
   both `fail_pending` and the error branch, doubling `dht_publish_failures`.
   The duplicate increment is removed and `failed_publish_is_counted_once`
   guards the behavior.

## 13. Decisions and unresolved questions

- The DHT uses libp2p Kademlia `/nexus/kad/1.0.0`, not the public IPFS DHT.
- DHT values are exact signed envelopes. Raw publication bytes are never
  re-encoded before publication or verification.
- The default role is client. Bootstrap nodes run server mode through
  `dht_mode: "server"`.
- `network.discover` is the only core-visible DHT method. Raw publish, lookup,
  and provider methods stay in `nexus-net` for later product stages.
- Bootstrap sources merge at startup in compiled, configured, persisted order,
  deduplicated.
- In-memory DHT records are hints, not durable state. TTL and republication are
  the recovery mechanism.
- Provider reputation, availability scoring, and multi-source downloads are
  deferred to Stage 12 and later.
- Index-provider records are reserved for Stage 16.
- Relay, hole punching, and NAT traversal are out of scope for Stage 10.
- There is no global bootstrap-list update service. Distribution remains a
  release, deployment, or operator decision.
- Android runtime DHT evidence and remote GitHub Actions execution remain
  outstanding.

## 14. Next stage

Stage 11 adds NAT traversal, relays, and fallback behavior for difficult
networks on top of the Stage 10 DHT. It must not begin until Stage 10 is
reviewed and approved by the project owner.
