# Community Infrastructure Directory Policy

**Status:** Proposed for Stage 20; no public directory is published
**Applies to:** Independently operated bootstrap peers, relays, and index
providers
**Last updated:** 2026-11-26

Ember does not need a company-operated backend. It does need a way for users and
release engineers to find independently operated entry points without turning
one operator, one list, or one repository into an authority over the network.

This document defines the policy for a public infrastructure directory. It is a
draft. Until the project owner approves it and publishes an actual list, Ember
ships no community directory and claims none.

## 1. Principles

- **Optional.** A peer can join through a manually entered address. A directory
  is a convenience, not a requirement.
- **Non-authoritative.** A listing is a hint. It grants no identity, ownership,
  moderation power, signature authority, or protocol privilege.
- **Independent.** Listings should come from different operators, providers,
  regions, and autonomous systems. A single operator must not dominate a list.
- **Removable.** A listed operator can withdraw at any time. The project can
  remove a listing for abuse, staleness, or a security failure without removing
  the operator's ability to run a node.
- **Transparent.** The criteria, current operators, and removal reasons are
  public, except for security details that would harm users.
- **Bounded.** The list is small enough to review and cache. It is not an
  exhaustive census of the network.

## 2. Roles

| Role | What it provides | What it must not claim |
|---|---|---|
| Bootstrap peer | DHT entry point in server mode | Authority over records or identities |
| Relay | Bounded Circuit Relay v2 capacity | Content storage or moderation authority |
| Index provider | Optional signed-record search hints | A complete or authoritative index |
| Directory maintainer | Reviews and publishes listings | Ownership of the network or its content |

A single host may run more than one role. The roles remain logically separate,
and enabling one does not enable another.

## 3. Listing requirements

An operator who wants a public listing supplies:

- the role or roles requested;
- a full multiaddress including `/p2p/<peer-id>` for a bootstrap peer;
- a reachable relay address and advertised relay limits for a relay;
- an index-provider address only if the provider is intentionally public;
- the operator's public contact channel for operational and security issues;
- the hosting provider, region, and country, at the granularity the operator is
  willing to disclose;
- the software version and update policy;
- whether the node is a test, best-effort, or production-intent service;
- an expected retention or shutdown plan for the address.

The project verifies reachability before publishing a listing. Verification is a
point-in-time check, not a guarantee of future availability.

## 4. Diversity requirements

Before a list is published, it should satisfy:

- at least two independent operators for bootstrap service;
- at least two independent operators for relay service if relay fallback is
  described as available;
- no single operator holding a majority of listed entry points;
- more than one hosting provider and region where feasible;
- at least one TCP-reachable and one QUIC-reachable bootstrap address;
- a documented fallback if DNS fails for a named address.

A list that cannot meet these conditions is published with an explicit
"limited diversity" warning or is not published.

## 5. Listing format

A future directory entry contains only operational data:

```text
role: bootstrap
operator: Example Cooperative
contact: ops@example.invalid
address: /dns4/bootstrap.example.invalid/udp/4001/quic-v1/p2p/12D3Koo...
also: /ip4/203.0.113.10/tcp/4001/p2p/12D3Koo...
software: ember 0.1.x
region: Example Region
status: best-effort
verified: 2026-11-26
```

No entry contains user records, content IDs, private keys, account names, or
unbounded free-form notes. Contact details are published only with the
operator's consent.

## 6. Publication and update process

1. The operator opens a private report or contacts the maintainer with the
   required information.
2. The maintainer checks the address from outside the operator's host, confirms
   the peer ID and role, and records the check date.
3. The maintainer reviews diversity, contactability, and any conflict of
   interest.
4. The listing is added to the directory with a verification date.
5. The operator is responsible for reporting address, version, or contact
   changes.
6. The maintainer re-checks listings on a published schedule and removes or
   marks stale entries.

The project does not silently add an operator. A listing is a reviewed public
statement.

## 7. Removal and suspension

A listing may be removed or suspended when:

- the address is unreachable across repeated checks;
- the operator asks to withdraw;
- the node's software is unsupported or has a known critical vulnerability;
- the node is used to inject invalid records, spam, or attacks;
- the operator cannot be contacted for a security or abuse issue;
- the listing no longer meets diversity or disclosure requirements.

Removal is not a takedown of content and does not affect the operator's ability
to run a node or serve peers who already know the address. Except for an active
security emergency, the maintainer states the reason to the operator and records
the decision.

## 8. User-side behavior

- Clients merge compiled, configured, persisted, and manually entered addresses.
- Clients de-duplicate addresses and retry each target independently.
- Losing one listed node does not disable the network.
- A listed address is never treated as a source of truth. Records and files are
  verified regardless of who supplied them.
- Users can remove a persisted address without affecting the published list.

## 9. Privacy and security

- A bootstrap or relay operator can observe peer IDs, addresses, and request
  timing. It cannot read file bytes or private keys.
- Operators must not log content, queries, or personal data beyond what is needed
  to operate the service.
- Operators must publish a security contact and a disclosure expectation.
- The project does not publish an operator's home address, private contact
  details, or infrastructure credentials.
- A directory listing is not an endorsement of an operator's legal compliance
  in every jurisdiction.

## 10. Current status

| Item | Status |
|---|---|
| Public directory published | No |
| Independent bootstrap operators | None known |
| Independent relay operators | None known |
| Independent index providers | None known |
| Directory maintainer assigned | Not yet |
| Owner approval for this policy | Pending |

Until those change, this document is a policy proposal and must not be cited as
evidence that community infrastructure exists. The local and loopback operator
guidance remains in [BOOTSTRAP_NODES.md](BOOTSTRAP_NODES.md) and
[RELAY_NODES.md](RELAY_NODES.md).

## 11. References

- [Bootstrap node operations](BOOTSTRAP_NODES.md)
- [Relay node operations](RELAY_NODES.md)
- [Network health measurement](NETWORK_HEALTH.md)
- [Governance](../governance/GOVERNANCE.md)
- [Moderation principles](../governance/MODERATION_PRINCIPLES.md)
