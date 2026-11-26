# Network Health Measurement

**Status:** Design proposal only; no remote measurement is implemented
**Applies to:** Ember `0.1.x` and any future opt-in health reporting
**Last updated:** 2026-11-26

Ember has no telemetry, analytics, or automatic crash reporting. That remains
true after Stage 20. This document defines how the project could measure network
health later without turning participation into surveillance, and what it can
measure locally today.

## 1. Goals

- Detect a network-wide failure that local loopback tests cannot show.
- Measure whether peers can join, discover, publish, download, and replicate.
- Measure whether bootstrap, relay, and index-provider roles are usable.
- Track version and protocol distribution so deprecation windows are realistic.
- Publish aggregate results so operators and users can judge availability.

Non-goals:

- tracking individual users, devices, searches, or downloads;
- building an advertising or engagement profile;
- ranking content or people;
- creating a central account or installation identity;
- uploading file names, content IDs, peer IDs, addresses, or query text.

## 2. What exists today

`network.status` exposes local counters for the running node, including:

| Field | Use |
|---|---|
| `dht_mode` | Confirms client or server operation |
| `dht_routing_peers` | Local routing-table size |
| `dht_bootstrapped` | Whether an initial lookup completed |
| `dht_queries_started` / `dht_queries_failed` | Local query reliability |
| `dht_records_accepted` / `dht_records_rejected` | Local validation outcomes |
| `dht_providers_accepted` / `dht_providers_rejected` | Provider validation outcomes |
| `last_error` | Stable local failure code |

These values stay on the device. They are shown in diagnostics and are not
uploaded. They are enough for a user or operator to troubleshoot a local
deployment, but they cannot show global availability.

## 3. Two-tier design

### Tier 1: local-only diagnostics

Implemented and safe by default:

- counters and stable error codes in `network.status`;
- opt-in debug logging with redaction;
- local export of diagnostics, without keys, paths, file names, or peer
  identifiers;
- operator-side host metrics kept by the operator.

Tier 1 requires no new privacy review because nothing leaves the device.

### Tier 2: opt-in aggregate reporting

Not implemented. It would require all of the following before any code ships:

1. A separate, explicit opt-in that is off by default.
2. A plain-language description of exactly what is sent.
3. A local preview of the exact payload before the first upload.
4. A visible off switch and a way to delete queued data.
5. A privacy review update approved by the project owner.
6. A published retention period and operator identity.
7. A build that can omit the reporter entirely.

An opt-in must not be bundled into onboarding, publishing, or a terms update. A
user who declines must retain full product functionality.

## 4. Proposed metric schema

If Tier 2 is approved, a report should contain coarse, bounded fields:

| Field | Example | Rationale |
|---|---|---|
| schema | `1` | Versioned report format |
| build version | `0.1.0` | Version distribution |
| protocol version | `1` | Compatibility planning |
| platform | `android` or `windows` | Platform reliability |
| platform major | `10` or `14` | Compatibility, not a unique device |
| network class | `wifi`, `ethernet`, `mobile`, `unknown` | Poor-network analysis |
| session bucket | `under_5m`, `5m_1h`, `1h_1d`, `over_1d` | Coarse duration |
| join outcome | `ok`, `timeout`, `unsupported_version`, `other` | Discovery health |
| discovery outcome | `ok`, `partial`, `timeout`, `other` | DHT health |
| transfer outcome | `ok`, `partial`, `failed` | Transfer health |
| relay used | `yes`, `no`, `unknown` | Relay demand |
| error code | stable code only | Diagnosis without free text |
| region bucket | coarse continent or operator-defined region | Regional availability |
| week bucket | ISO week, no timestamp | Trend without precise timing |

The report must not contain:

- identity IDs, peer IDs, device IDs, or installation IDs;
- IP addresses, hostnames, or precise location;
- file names, content IDs, publication IDs, tags, or search queries;
- block lists, abuse reports, or local paths;
- free-form error text or stack traces;
- timestamps finer than the published bucket.

## 5. Aggregation and publication

- Reports are aggregated before publication.
- A bucket with fewer than a published minimum number of reports is suppressed.
- Published output contains counts and rates, not individual reports.
- Raw reports are retained only as long as needed to validate aggregation, then
  deleted on a published schedule.
- The aggregation code and schema are public so independent operators can audit
  the claim.
- The project publishes methodology, limitations, and the date range.

Aggregate numbers are descriptive, not a service-level guarantee. A low sample
count is labelled as such.

## 6. Abuse and manipulation resistance

- Rate-limit reports by network and coarse session, without a persistent user
  ID.
- Discard reports with unknown schema, out-of-range values, or impossible
  combinations.
- Treat a sudden spike as suspicious until corroborated.
- Publish confidence intervals or sample sizes.
- Never let a report change a user's local policy, block list, or content view.
- Never use health data to rank or recommend content.

No measurement design eliminates Sybil manipulation. The project must not claim
that aggregate health data is tamper-proof.

## 7. Operator-side measurement

Community bootstrap and relay operators can publish their own availability and
capacity data. The project may link to it, but it does not become the authority
for that operator's numbers. Operator dashboards should state:

- the measurement window;
- whether the node was reachable from outside its host;
- the software version;
- known maintenance windows;
- whether the data is best-effort or audited.

## 8. Launch gate

Tier 2 is not a launch requirement. The launch gate requires only that the
project is honest about what it does not measure. Before any Tier 2 code ships:

- [ ] Privacy review updated and approved.
- [ ] Owner approval recorded.
- [ ] Payload preview and off switch implemented.
- [ ] Retention and deletion implemented.
- [ ] Aggregation minimum and suppression rules implemented.
- [ ] Reporter-free build verified.
- [ ] Public methodology published.

Until then, the only network-health evidence is local and loopback testing.
The known gaps are recorded in
[KNOWN_LIMITATIONS.md](../release/KNOWN_LIMITATIONS.md).

## 9. References

- [Privacy review](../security/PRIVACY_REVIEW.md)
- [Threat model](../security/THREAT_MODEL.md)
- [Bootstrap node operations](BOOTSTRAP_NODES.md)
- [Relay node operations](RELAY_NODES.md)
- [Infrastructure directory policy](INFRASTRUCTURE_DIRECTORY.md)
