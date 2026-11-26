# Post-Launch Maintenance Plan

**Status:** Proposed for Stage 20; no public launch has occurred
**Applies to:** Official Ember releases and any future public infrastructure
**Last updated:** 2026-11-26

This plan describes how Ember would be maintained after a public release. It is
a draft. The project has not launched, has no production signing material, and
has no independent infrastructure operators. Nothing here should be read as a
service-level commitment.

## 1. Principles

- Security fixes take priority over features.
- Users must be able to tell an official release from a test or community build.
- A release note must state compatibility, known limitations, and rollback.
- No central service is required for the product to keep working.
- Infrastructure is replaceable; losing one operator must not disable the
  network.
- Support claims must match measured evidence.

## 2. Release channels

| Channel | Purpose | Audience |
|---|---|---|
| `dev` | Local and CI builds, not for general use | Contributors |
| `preview` | Signed preview with explicit known limits | Invited testers |
| `stable` | Signed public release | General users |
| `security` | Out-of-band fix for a supported version | All users |

A build must be labelled with its channel. A test-signed or unsigned artifact
must never be presented as `stable`.

## 3. Cadence

Proposed, subject to owner approval:

- **Security fixes:** as needed, with an advisory.
- **Stable patch releases:** every 4–8 weeks when there are user-facing fixes.
- **Minor releases:** every 3–6 months, with a compatibility note.
- **Major protocol releases:** only under
  [VERSIONING_AND_DEPRECATION.md](../protocol/VERSIONING_AND_DEPRECATION.md),
  with a 12-month overlap unless a security emergency requires otherwise.

The project may ship less often. It must not promise a cadence it cannot keep.

## 4. Supported versions

While Ember is `0.1.x`, only the latest published build is supported.

After a public release, the support table will include:

- the current stable release;
- the previous minor release during its overlap window;
- the security-fix window;
- the last version that can read a migrated database;
- the last version that can speak a deprecated protocol major.

A version is marked unsupported only after a public notice. Security fixes may
be backported when feasible; the advisory states exactly which versions are
fixed.

## 5. Release checklist

Before a `stable` or `security` release:

- [ ] `.\scripts\build.ps1 -WithDependencyScan` passes.
- [ ] `.\scripts\verify-clean-checkout.ps1` passes.
- [ ] Release artifacts are built from a clean checkout with production signing.
- [ ] `.\scripts\verify-release.ps1 -RequireSigned` passes.
- [ ] Windows MSI install, upgrade, launch, and uninstall pass.
- [ ] Android APK installs on arm64 and x86_64 targets; the AAB is accepted.
- [ ] Performance and compatibility reports are current.
- [ ] Dependency locking, verification metadata, SBOM, and third-party notices
      are present.
- [ ] The security checklist has no open high-severity item.
- [ ] Release notes state known limitations, rollback, and compatibility.
- [ ] Checksums and the release manifest are published.
- [ ] The project owner approves the release.

A missing item blocks the release. A waiver must be written, scoped, and
approved; it cannot be implied by silence.

## 6. Security response

Security reports follow
[DISCLOSURE_AND_ADVISORIES.md](../security/DISCLOSURE_AND_ADVISORIES.md).

For an active incident:

1. Confirm the report and limit access to the details.
2. Identify affected versions, platforms, and artifacts.
3. Prepare a private fix and regression tests.
4. Decide whether users need an immediate mitigation.
5. Build and sign the fix.
6. Publish the advisory and fixed artifacts together when possible.
7. Update the threat model, tests, and release checklist.
8. Review whether a protocol or governance change is required.

If a signing key is compromised, the project must stop publishing with it,
publish a key-rotation notice, and treat old artifacts as untrusted. A key
compromise is not a normal patch release.

## 7. Rollback and update safety

- Database migrations must leave the previous database recoverable on failure.
- A downgrade may make a newer database unreadable; the update guide says so.
- The Windows MSI supports in-place upgrade and uninstall.
- An Android release must be installable as an upgrade to the previous version
  with the same signing identity.
- A failed rollout must have a documented way to stop distribution and tell
  users what to do.
- Signed records are immutable; a rollback does not rewrite them.

## 8. Infrastructure operations

- Publish at least two independent bootstrap entry points before describing the
  network as generally available.
- Publish relay capacity only when it is reachable and bounded.
- Keep the directory small, reviewed, and diverse.
- Re-verify listings on a published schedule.
- Remove or mark stale listings rather than silently leaving them.
- Do not let one operator become the majority of listed capacity.
- Keep operator guidance current in
  [BOOTSTRAP_NODES.md](../operations/BOOTSTRAP_NODES.md) and
  [RELAY_NODES.md](../operations/RELAY_NODES.md).

No public directory exists today. See
[INFRASTRUCTURE_DIRECTORY.md](../operations/INFRASTRUCTURE_DIRECTORY.md).

## 9. Monitoring

Local diagnostics are available through `network.status`. Remote monitoring is
not implemented. A future opt-in design is documented in
[NETWORK_HEALTH.md](../operations/NETWORK_HEALTH.md).

Until remote monitoring exists, the project must:

- rely on user reports and operator reports;
- state that it has no global uptime visibility;
- avoid claiming network-wide availability or performance;
- publish the limits of its evidence.

## 10. Documentation maintenance

Every release updates:

- [CHANGELOG.md](../../CHANGELOG.md);
- [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md);
- [COMPATIBILITY_MATRIX.md](COMPATIBILITY_MATRIX.md);
- [PERFORMANCE_REPORT.md](PERFORMANCE_REPORT.md) when performance claims change;
- [PROTOCOL_SPEC.md](../protocol/PROTOCOL_SPEC.md) when protocol behavior
  changes;
- [VERSIONING_AND_DEPRECATION.md](../protocol/VERSIONING_AND_DEPRECATION.md)
  when a deprecation is approved;
- the release notes and security checklist.

A documentation change that contradicts the implementation is a release blocker
for the behavior it describes.

## 11. Community and governance

- Publish named maintainer roles before claiming community governance.
- Record significant decisions and conflicts of interest.
- Keep the moderation and infrastructure policies public.
- Do not create a central content authority by accident.
- Review governance after the first public release and at least annually.

See [GOVERNANCE.md](../governance/GOVERNANCE.md) and
[MODERATION_PRINCIPLES.md](../governance/MODERATION_PRINCIPLES.md).

## 12. Capacity and performance

- Expand capacity through community-run nodes and peer-algorithm improvements,
  not mandatory central hosting.
- Measure before tuning chunk size, DHT parameters, or relay limits.
- Publish performance on named hardware and network conditions.
- Do not generalize a Windows loopback result to Android or global networks.
- Revisit quota, retention, and relay limits when real workload data exists.

## 13. Post-launch review

Within 30 days of a public release, review:

- crash and security reports (if any reporting exists);
- install, upgrade, and uninstall failures;
- bootstrap and relay availability;
- protocol errors and version distribution;
- documentation gaps and user confusion;
- whether the launch claims matched the evidence.

The review is published in aggregate. It does not expose individual users or
reporters.

## 14. Current status

| Item | Status |
|---|---|
| Public release | Not published |
| Production signing | Not configured |
| Support table | Development-only policy |
| Named maintainers | Not published |
| Public infrastructure | None |
| Remote monitoring | Not implemented |
| Launch approval | Not requested |

The project is not in post-launch maintenance. This plan exists so the launch
gate can be reviewed against a concrete process rather than an intention.
