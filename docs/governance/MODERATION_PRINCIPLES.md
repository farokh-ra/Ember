# Moderation and Abuse-Handling Principles

**Status:** Proposed for Stage 20; requires project-owner approval
**Applies to:** Project-owned spaces, the infrastructure directory, and product
safety features
**Last updated:** 2026-11-26

Ember has no central moderation authority. Published content lives on users'
devices and can be copied by independent peers. A project that claims to be
decentralized cannot promise a global takedown, and must not pretend otherwise.

This document separates what the project can do from what it cannot do, and
defines the principles for the parts it controls.

## 1. What the project can and cannot do

| Action | Possible | Notes |
|---|---|---|
| Block a peer locally | Yes | Enforced on the local device only |
| Record a local abuse report | Yes | Stays on the device |
| Hide a record in the local UI | Yes | Local policy, not a network deletion |
| Remove a project-owned listing | Yes | Directory entry only |
| Remove content from another peer | No | The project has no authority or access |
| Delete a copy already downloaded | No | The user controls their device |
| Ban an identity network-wide | No | No central identity authority |
| Prevent a blocked peer returning | No | A new identity is cheap |

The product must state these limits before a user publishes. A tombstone is a
signed withdrawal by the original uploader; it does not delete copies held by
others.

## 2. Principles

1. **Local first.** Safety controls run on the user's device and do not depend on
   a remote decision.
2. **No silent censorship.** The project does not maintain a secret global
   blocklist or inject hidden filters into search.
3. **No content scanning.** The core does not upload file bytes, file names, or
   query text for centralized analysis.
4. **Operator autonomy.** A bootstrap, relay, or index operator may refuse
   service under its own policy and local law.
5. **Transparency.** Project-owned removals are recorded with a reason and a
   scope. Privacy and security details may be withheld, but the existence of a
   decision is not hidden.
6. **Proportionality.** A local block, a listing removal, and a security
   advisory are different tools and must not be conflated.
7. **Appeal where possible.** A person affected by a project-owned decision can
   ask for review. There is no appeal for a decision made by an independent
   operator or a local user.
8. **No safety theater.** The project does not claim that a local block removes
   content from the network or that an extension-based warning is malware
   detection.

## 3. Product safety features

- **Local block list.** A user can block a peer ID with a bounded reason and
  note. The block applies to local connections and display. It is not announced
  or gossiped.
- **Local abuse reports.** A user can record a report with a category, bounded
  note, and timestamp. Reports never leave the device.
- **Publication warnings.** The UI warns that publishing is public, that copies
  can persist after the uploader stops serving, and that deletion is not global.
- **File safety notices.** Extension-based classification is advisory. A
  downloaded file is never executed automatically; previews are delegated.
- **No automatic preview execution.** The app does not run downloaded code.

The missing per-report deletion control is a known privacy gap. Reports are
capped and local, but a user cannot remove one report without clearing the local
store. This must be fixed before a production release claims complete local data
control.

## 4. Project-owned spaces

Project-owned spaces include the repository, issue tracker, pull requests,
release channels, and any official chat or forum.

Moderation there may include:

- removing spam, malware links, doxxing, or harassment;
- closing a duplicate or off-topic issue;
- restricting a contributor who repeatedly violates the code of conduct;
- removing a malicious package or release artifact;
- publishing a security advisory.

Moderation does not extend to a user's own device, a peer's copy of a file, or
an independent operator's service.

## 5. Infrastructure directory

The project may remove a bootstrap, relay, or index listing when an operator:

- is unreachable or repeatedly fails verification;
- injects invalid records or attacks peers;
- cannot be contacted about a security issue;
- misrepresents a test node as production;
- asks to withdraw.

Removal affects the listing only. It is not a content takedown, and the operator
may continue to run the node. The reason is shared with the operator and, unless
it would expose a security detail, recorded publicly.

## 6. Illegal content and legal requests

Ember is a peer-to-peer tool. The project does not host user files and cannot
delete copies held by independent peers. If the project receives a legal request
about content it does not control, it should:

1. determine what it actually controls (a repository file, a listing, a release
   artifact, or nothing);
2. respond within the applicable legal process;
3. remove only what it controls;
4. tell the requester, where lawful, that it cannot remove peer-held copies;
5. publish aggregate transparency information when it is safe and lawful.

The project does not volunteer user data to a third party. It has no central
database of users to hand over.

## 7. Abuse reporting

A report about project-owned spaces should include:

- what happened and where;
- a link or minimal reproduction;
- whether a person is at immediate risk;
- whether the report contains personal data that should be minimized.

Do not send private keys, passwords, another person's files, or unredacted
personal data. Security vulnerabilities go through
[DISCLOSURE_AND_ADVISORIES.md](../security/DISCLOSURE_AND_ADVISORIES.md), not a
public issue.

## 8. Appeals and review

A person affected by a project-owned moderation decision may ask for a review by
a maintainer who was not the original decision-maker. If no such maintainer
exists, the project owner reviews the decision and records the conflict. The
review considers:

- whether the action was within the stated policy;
- whether the evidence was sufficient;
- whether a less restrictive action would work;
- whether the decision exposes anyone to harm.

There is no appeal to the project for a local block, an independent operator's
decision, or a copy held by a peer.

## 9. Metrics and transparency

The project may publish aggregate counts of:

- project-owned moderation actions by category;
- infrastructure listings added and removed;
- security advisories by severity.

It must not publish identifying details of reporters, users, or independent
operators without consent. A small count can itself identify a person, so
suppression rules apply.

## 10. Current status

| Item | Status |
|---|---|
| Local block and report | Implemented in the core; UI binding pending |
| Per-report deletion | Not implemented |
| Project-owned moderation log | Not published |
| Infrastructure directory | Not published |
| Public transparency report | None yet |
| Owner approval | Pending |

## 11. References

- [Governance](GOVERNANCE.md)
- [Privacy review](../security/PRIVACY_REVIEW.md)
- [Threat model](../security/THREAT_MODEL.md)
- [Infrastructure directory](../operations/INFRASTRUCTURE_DIRECTORY.md)
- [Known limitations](../release/KNOWN_LIMITATIONS.md)
