# Ember Governance

**Status:** Proposed for Stage 20; requires project-owner approval
**Applies to:** Protocol stewardship, releases, infrastructure listings, and
security response
**Last updated:** 2026-11-26

Ember is a peer-to-peer application with no central account system and no
central content authority. Its governance must not recreate, by accident, the
central control the product is designed to avoid.

This document describes how decisions are made today and how the project
intends to make them as contributors and independent operators join. It is a
draft, not a claim that a community governance structure already exists.

The repository front door is [GOVERNANCE.md](../../GOVERNANCE.md). Participation
is covered by the [Code of Conduct](../../CODE_OF_CONDUCT.md); support scope is
in [SUPPORT.md](../../SUPPORT.md); security reports follow
[SECURITY.md](../../SECURITY.md).

## 1. Scope

Governance covers:

- the protocol specification and compatibility policy;
- the reference implementation and release process;
- security response and advisories;
- the public infrastructure directory;
- documentation, trademarks, and project identity;
- moderation principles applied to project-owned spaces.

It does not give the project authority over:

- content published by users on their own devices;
- copies held by independent peers;
- infrastructure run by independent operators;
- a user's local block list or abuse reports;
- another operator's relationship with its users or jurisdiction.

## 2. Current decision structure

| Decision | Who decides | Record |
|---|---|---|
| Day-to-day code review | Maintainers | Pull request review |
| Protocol editorial change | Maintainers | Documentation change plus review |
| Protocol minor change | Maintainers, with public notice | Specification diff and tests |
| Protocol major change | Project owner, with public rationale | ADR, migration plan, new vectors |
| Release approval | Project owner | Release checklist and signed artifacts |
| Public infrastructure listing | Directory maintainer, with owner-approved policy | Directory entry and verification date |
| Security advisory | Security maintainer, with owner notification | Advisory and affected-version table |
| Moderation of project-owned spaces | Maintainers | Moderation log and stated reason |
| Trademark or license change | Project owner | Public notice |

The project owner is the final decision-maker while the project is
owner-funded. This is a transitional structure, not a permanent claim of
legitimacy. A future foundation, cooperative, or maintainer council requires an
explicit public decision and an amendment to this document.

## 3. Maintainer roles

- **Project owner:** accountable for release, legal, and major protocol
  decisions; holds signing and repository administration.
- **Protocol maintainer:** owns the specification, vectors, compatibility
  review, and ADR updates.
- **Security maintainer:** owns intake, triage, advisories, and the security
  release checklist.
- **Release maintainer:** owns build reproducibility, signing workflow,
  artifacts, and release notes.
- **Infrastructure maintainer:** owns the directory policy, operator
  verification, and removal decisions.
- **Community maintainer:** owns contribution guidelines, project spaces, and
  moderation process.

One person may hold more than one role while the project is small. A role must
be documented, and a decision that creates a conflict of interest must be
reviewed by someone else when possible. If no second reviewer exists, the
decision is recorded as single-reviewer with the reason.

## 4. Decision classes

### 4.1 Routine

Documentation, tests, refactors, bug fixes, and dependency updates that do not
change protocol bytes, security posture, or release behavior. One maintainer
review is sufficient.

### 4.2 Significant

Changes to storage schema, wire limits, error codes, security controls, release
gates, privacy behavior, or infrastructure listings. Requires:

- a written rationale;
- tests or evidence;
- review by a maintainer who did not author the change;
- a changelog entry;
- an update to affected public documentation.

### 4.3 Major

A protocol major change, a license change, a signing-authority change, a
governance change, or a public-launch decision. Requires:

- an ADR or formal proposal;
- a public comment period of at least 14 days unless a security emergency
  requires faster action;
- project-owner approval;
- a migration and rollback plan;
- an announcement in release notes.

## 5. Protocol stewardship

The protocol is a public contract once a release claims v1 conformance. Changes
follow [VERSIONING_AND_DEPRECATION.md](../protocol/VERSIONING_AND_DEPRECATION.md):

- editorial changes do not alter bytes or behavior;
- minor changes preserve old readers and vectors;
- major changes require a new version, overlap plan, and owner approval;
- security emergencies may shorten notice but still require an advisory and a
  recorded rationale.

The reference implementation is not the specification by itself. A behavior
that is not documented and tested is not a stable promise.

## 6. Release authority

A release is approved only when the gates in
[POST_LAUNCH_MAINTENANCE.md](../release/POST_LAUNCH_MAINTENANCE.md) and
[SECURITY_RELEASE_CHECKLIST.md](../security/SECURITY_RELEASE_CHECKLIST.md) are
satisfied. The project owner approves the release and controls signing material.
No contributor may publish an artifact as an official Ember release without that
approval.

Test-signed artifacts, local builds, and community builds must be labelled so
users cannot mistake them for an official signed release.

## 7. Infrastructure and independence

- The public directory is non-authoritative and opt-in.
- Listing criteria and removal reasons are public.
- The project does not compel an operator to host content or retain data.
- The project does not grant an operator special protocol rights.
- A removed operator may continue to run a node; removal only affects the
  listing.
- The project should avoid operating the majority of public bootstrap or relay
  capacity once community capacity exists.

See [INFRASTRUCTURE_DIRECTORY.md](../operations/INFRASTRUCTURE_DIRECTORY.md).

## 8. Security and disclosure

Security reports are handled under
[DISCLOSURE_AND_ADVISORIES.md](../security/DISCLOSURE_AND_ADVISORIES.md). The
project commits to:

- private intake and acknowledgement;
- a documented severity and fix target;
- coordinated disclosure;
- an advisory for every confirmed high-severity issue;
- credit only with the reporter's consent;
- no retaliation for good-faith research.

A security emergency may bypass the normal comment period. The project still
publishes what happened, what was affected, and what users should do.

## 9. Moderation

Moderation is local by default. The project does not run a global content
authority and cannot remove copies held by independent peers. Project-owned
spaces and the public directory are moderated under
[MODERATION_PRINCIPLES.md](MODERATION_PRINCIPLES.md).

## 10. Conflicts of interest

A maintainer must disclose a financial, employment, or personal interest that
could affect a decision about infrastructure, a vendor, a protocol change, or a
security report. When a conflict exists:

- the interested person does not make the final decision alone;
- the conflict is recorded in the decision record;
- an outside reviewer is used when available.

## 11. Transparency and records

Public records include:

- accepted ADRs and specification changes;
- release notes and known limitations;
- security advisories;
- infrastructure listing criteria and verification dates;
- governance amendments.

Private records include:

- security reports before disclosure;
- personal contact details;
- signing material and infrastructure credentials;
- moderation reports that contain personal data.

## 12. Amendments

This document changes through a significant decision. A major governance change
requires a public proposal, a comment period, and project-owner approval. The
change is recorded in the changelog and the previous version remains available
in git history.

## 13. Current status

| Item | Status |
|---|---|
| Project owner | Repository owner |
| Named maintainers | Not yet published |
| Public comment process | Not yet exercised |
| Foundation or council | Not established |
| Governance approval | Pending |
| Public launch decision | Not made |

The project has not launched publicly. This document describes intended process
and must not be read as evidence that a community governance body exists.

## 14. References

- [Moderation principles](MODERATION_PRINCIPLES.md)
- [Protocol versioning](../protocol/VERSIONING_AND_DEPRECATION.md)
- [Disclosure and advisories](../security/DISCLOSURE_AND_ADVISORIES.md)
- [Infrastructure directory](../operations/INFRASTRUCTURE_DIRECTORY.md)
- [Release maintenance](../release/POST_LAUNCH_MAINTENANCE.md)
