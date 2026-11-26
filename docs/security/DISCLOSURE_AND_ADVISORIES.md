# Security Disclosure and Advisories

**Status:** Proposed for Stage 20; supersedes the short process in `SECURITY.md`
only after owner approval
**Applies to:** Ember `0.1.x` and later official releases
**Last updated:** 2026-11-26

Ember handles identity keys, peer-to-peer connections, and user files. A
security report can affect people who did not choose to be part of the network.
This document defines intake, triage, disclosure, and advisory handling. It does
not create a bug bounty or a contractual service level.

## 1. Reporting a vulnerability

Do not open a public issue with exploit details, private keys, recovery phrases,
tokens, or personal data.

Use GitHub private vulnerability reporting if it is enabled for the repository.
Otherwise, contact the repository owner through the private channel on the
repository profile and include:

- the affected version, commit, or artifact;
- the platform and build type;
- a description of the impact;
- the smallest reproduction you can provide;
- any suggested mitigation;
- whether the issue is already public;
- how you would like to be credited, or that you prefer to remain anonymous.

If you are unsure whether something is in scope, report it privately and let the
maintainer decide.

## 2. Scope

In scope:

- private key, recovery phrase, or backup exposure;
- signature verification bypass or signed-record tampering;
- identity impersonation or protocol downgrade;
- path traversal, unsafe file handling, or deletion of user-published originals;
- remote code execution, memory-safety defects, or unsafe FFI boundaries;
- denial of service with a practical, low-cost trigger;
- signing, update, or packaging weaknesses;
- dependency vulnerabilities that affect a distributed artifact;
- privacy defects that expose data beyond the documented model.

Out of scope:

- missing hardening with no demonstrated impact;
- vulnerabilities in unsupported development snapshots;
- social engineering or physical access;
- reports that require a compromised operating system or a modified app build;
- denial-of-service tests against public infrastructure;
- content disputes, copyright complaints, or moderation disagreements.

## 3. Safe harbor

The project will not pursue legal action against a researcher who:

- tests only their own devices, data, and infrastructure;
- avoids privacy violations, data destruction, and service disruption;
- reports the issue privately and gives a reasonable remediation window;
- does not extort, publish exploit code before disclosure, or access another
  person's data.

This is a good-faith statement, not a legal waiver of third-party rights or
applicable law.

## 4. Intake and acknowledgement

| Step | Target |
|---|---|
| Private report received | Same day when possible |
| Acknowledgement to reporter | Within 3 business days |
| Initial severity assessment | Within 10 business days |
| Fix target for critical or high | Within 90 days, sooner when feasible |
| Fix target for medium or low | Next scheduled release |
| Reporter update | At least every 14 days while open |

These are targets, not guarantees. A difficult fix may take longer; the project
must say so and give a revised estimate rather than go silent.

## 5. Severity

| Severity | Examples | Response |
|---|---|---|
| Critical | Key extraction, unauthenticated remote code execution, signature bypass with network impact | Immediate private fix, advisory, coordinated release |
| High | Local privilege or data loss, practical denial of service, serious privacy exposure | Fix before the next public release; advisory |
| Medium | Limited-impact validation gap, hardening bypass, information leak with conditions | Next scheduled release; advisory if users need to act |
| Low | Defense-in-depth issue, theoretical impact, difficult preconditions | Normal release; advisory optional |

Severity considers impact, exploitability, affected users, and whether a
mitigation exists. A disputed severity is discussed with the reporter; the
project records the final decision and why.

## 6. Embargo and coordination

- The default embargo ends when a fix is available and users have had a
  reasonable update window.
- The project will coordinate a disclosure date with the reporter.
- A critical issue may be disclosed earlier if it is already public or actively
  exploited.
- The project will not quietly close a valid report without telling the
  reporter.
- If the project cannot fix an issue, it will document the limitation and any
  mitigation instead of implying that it is fixed.

## 7. Advisories

Each confirmed high or critical issue receives an advisory containing:

- an identifier and publication date;
- affected versions and platforms;
- a plain-language impact statement;
- the fixed version or mitigation;
- workarounds, if any;
- credit, with consent;
- a CVE identifier when one is assigned;
- links to the fix and tests.

Advisories are published in the repository and linked from release notes. A
security release may be shipped before a full advisory if users need the fix
immediately; the advisory follows as soon as it is safe.

## 8. Supported versions

While Ember is in the `0.1.x` development line, only the latest published
`0.1.x` build is supported. Older development snapshots are unsupported.

After the first signed public release, the project will publish a support table
with at least the current and previous minor release and the security-fix
window. That table is reviewed with every release. A version that is no longer
supported is marked as such; it is not silently abandoned.

## 9. Release signing and verification

A security fix is not complete until:

- the fix is tested and reviewed;
- the release checklist has no open high-severity item;
- the artifact is built from a clean checkout;
- the artifact is signed with production material;
- checksums and the release manifest are published;
- the advisory names the fixed artifact.

A test-signed build is not a security release. It may be used for verification,
but users must not be told that it is publicly trusted.

## 10. No bug bounty

The project does not currently offer a bug bounty or payment for reports. It
will credit reporters who consent. It will not ask a reporter to pay for
disclosure or to accept a confidentiality term that prevents a future advisory.

## 11. Current status

| Item | Status |
|---|---|
| Private reporting channel | GitHub private reporting when enabled; owner profile otherwise |
| Security maintainer named | Not yet published |
| Public advisories | None |
| CVE process | Available through the hosting platform if needed |
| Bug bounty | None |
| Owner approval for this policy | Pending |

## 12. References

- [SECURITY.md](../../SECURITY.md)
- [Threat model](THREAT_MODEL.md)
- [Privacy review](PRIVACY_REVIEW.md)
- [Security release checklist](SECURITY_RELEASE_CHECKLIST.md)
- [Governance](../governance/GOVERNANCE.md)
