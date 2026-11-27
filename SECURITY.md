# Security Policy

Ember handles local identity keys, peer-to-peer connections, and user files. We
take reports seriously and ask that you avoid public disclosure until a fix is
available.

## Supported versions

| Version | Supported |
|---|---|
| `0.1.x` development builds | Yes |
| Older development snapshots | No |

Ember has not published a production release yet. This table will be updated
when the first signed release is available.

## Reporting a vulnerability

Do not open a public issue with exploit details, private keys, recovery phrases,
tokens, or personal data.

Use GitHub private vulnerability reporting if it is enabled for the repository.
Otherwise, contact the repository owner through the private channel listed on the
repository profile and include:

- the affected version or commit;
- the platform and build type;
- a description of the impact;
- the smallest reproduction you can provide;
- any suggested mitigation;
- whether the issue is already public.

You will receive an acknowledgement when the report is reviewed. Please allow
time for a fix and coordinated disclosure before publishing details.

The detailed intake, severity, embargo, advisory, and safe-harbor process is in
[docs/security/DISCLOSURE_AND_ADVISORIES.md](docs/security/DISCLOSURE_AND_ADVISORIES.md).
That document is a Stage 20 proposal and does not create a bug bounty or a
service-level guarantee.

## Scope

Examples of in-scope issues:

- private key or recovery phrase exposure through logs, storage, exports, or the
  user interface;
- signature verification bypass or record tampering;
- path traversal, unsafe file handling, or deletion of user-published originals;
- peer impersonation or protocol downgrade;
- remote code execution, memory-safety defects, or unsafe FFI boundaries;
- dependency vulnerabilities that affect a distributed artifact;
- signing, update, or packaging weaknesses.

Out of scope:

- missing hardening that has no demonstrated impact;
- vulnerabilities in unsupported development snapshots;
- social engineering or physical access attacks;
- reports that require a compromised operating system or a modified app build.

## Safe research

- Test only against your own devices and data.
- Do not access, modify, or exfiltrate another person's files or identity.
- Do not run denial-of-service tests against public infrastructure.
- Do not include real secrets or personal data in a report.
- Follow responsible disclosure and give the maintainers a reasonable window to
  respond.

## Security practices in this repository

- No API keys, private keys, keystores, or signing material are committed.
- CI runs repository hygiene and secret-pattern checks.
- Debug logging is redacted by policy and test coverage.
- Rust dependency licenses and advisories are checked with `cargo-deny` and
  `cargo-audit`.
- Release artifacts are built from a clean checkout with pinned tools and are
  signed with release-environment keys.
- Incoming protocol messages and metadata are schema-validated and size-bounded;
  malformed input is rejected without panicking.
- Signed records are verified against the claimed identity before storage or
  display.
- Downloaded files are never executed automatically, and previews are delegated
  to platform handlers or a sandbox.
- Users can block peers and record local abuse reports. These controls are local
  policy: nothing is uploaded, announced, or published.
- No telemetry or automatic crash reporting is enabled.

The current threat model, privacy review, and release gate are documented in:

- [docs/security/THREAT_MODEL.md](docs/security/THREAT_MODEL.md)
- [docs/security/PRIVACY_REVIEW.md](docs/security/PRIVACY_REVIEW.md)
- [docs/security/SECURITY_RELEASE_CHECKLIST.md](docs/security/SECURITY_RELEASE_CHECKLIST.md)

A known high-severity issue blocks release until it is fixed or explicitly
accepted in writing by the project owner.

See [docs/LOGGING_POLICY.md](docs/LOGGING_POLICY.md) for the logging rules,
[CONTRIBUTING.md](CONTRIBUTING.md) for the contribution process,
[SUPPORT.md](SUPPORT.md) for non-security support, and
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community standards.
