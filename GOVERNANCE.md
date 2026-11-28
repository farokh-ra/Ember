# Ember Governance

Ember is currently owner-led. This file is the repository front door; the
detailed, still-proposed process is in
[docs/governance/GOVERNANCE.md](docs/governance/GOVERNANCE.md).

## Current state

- The project owner makes final decisions on releases, signing authority,
  protocol major versions, infrastructure, and policy.
- Maintainer roles, a public maintainer list, and a community decision process
  are drafted but not approved or staffed.
- No governance body, foundation, or elected council exists.
- No public moderation authority exists. Blocking and reporting are local to each
  device and operator.

## Decision classes

| Class | Examples | Decision |
|---|---|---|
| Routine | Documentation, tests, small fixes | Maintainer review, then merge |
| Feature | New user-visible behavior | Owner or delegated maintainer review |
| Protocol minor | Additive field or message | Protocol review plus versioning policy |
| Protocol major | Changed bytes, signature, or required behavior | Owner decision, ADR, migration note |
| Release | Tag, signed artifact, public launch | Owner decision and a complete release gate |
| Governance | Roles, voting, moderation authority | Owner decision and a public amendment |

## Contributing

Contribution rules, commit conventions, and the definition of done are in
[CONTRIBUTING.md](CONTRIBUTING.md). Participation is covered by the
[Code of Conduct](CODE_OF_CONDUCT.md). Security reports follow
[SECURITY.md](SECURITY.md).

## Amendments

Until a different process is approved, a governance change requires a pull
request that edits this file and
[docs/governance/GOVERNANCE.md](docs/governance/GOVERNANCE.md), an ADR when the
change is expensive to reverse, and explicit project-owner approval. A merged
change does not create a legal entity, a fiduciary duty, or a service commitment.
