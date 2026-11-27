# Release engineering

This directory records the Stage 19 release engineering, evidence, and policy for
Ember 0.1.0.

| Document | Contents |
|---|---|
| [PERFORMANCE_REPORT.md](PERFORMANCE_REPORT.md) | Windows startup, memory, idle CPU, artifact sizes, targets, and unmeasured areas. |
| [COMPATIBILITY_MATRIX.md](COMPATIBILITY_MATRIX.md) | Windows, Android, network, storage, and toolchain compatibility with evidence state. |
| [SIGNING_AND_REPRODUCIBILITY.md](SIGNING_AND_REPRODUCIBILITY.md) | Signing inputs, checksums, verification, WiX pin, CI workflow, and reproducibility limits. |
| [UPDATE_AND_ROLLBACK.md](UPDATE_AND_ROLLBACK.md) | Windows and Android upgrade, downgrade, data, and channel policy. |
| [RELEASE_NOTES_0.1.0.md](RELEASE_NOTES_0.1.0.md) | User-facing release notes, install, upgrade, and known limits. |
| [ACCESSIBILITY_AND_LOCALIZATION.md](ACCESSIBILITY_AND_LOCALIZATION.md) | What is tested, what is missing, and the review plan. |
| [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md) | Product limits, platform evidence gaps, and public-launch blockers. |
| [POST_LAUNCH_MAINTENANCE.md](POST_LAUNCH_MAINTENANCE.md) | Channels, cadence, security response, rollback, and post-launch review. |
| [SBOM_AND_NOTICES.md](SBOM_AND_NOTICES.md) | Gradle locks, verification metadata, CycloneDX SBOM, and third-party notice bundle. |

## Scripts

| Script | Purpose |
|---|---|
| `scripts/fetch-wix.ps1` | Downloads the pinned WiX 3 toolset and verifies its SHA-256. |
| `scripts/package-release.ps1` | Builds, signs, checksums, manifests, and verifies APK, AAB, MSI, and portable ZIP. |
| `scripts/verify-release.ps1` | Re-verifies a release directory without rebuilding. |
| `scripts/measure-performance.ps1` | Measures startup, working set, idle CPU, and package sizes. |
| `scripts/build-native.ps1` | Builds and strips the native cores used by both packages. |

## Release gates

A release is publishable only when all of these are true:

1. `.\scripts\build.ps1 -WithDependencyScan` passes.
2. `.\scripts\verify-clean-checkout.ps1` passes.
3. `.\scripts\package-release.ps1 -Version <version>` succeeds with production
   signing configured.
4. `.\scripts\verify-release.ps1 -Directory .\dist\<version> -RequireSigned`
   passes.
5. The Windows MSI installs, upgrades from the previous version, launches, and
   uninstalls.
6. The Android APK installs on arm64 and x86_64 test targets; the AAB is accepted
   by the delivery channel.
7. The performance and compatibility reports are updated with current numbers.
8. The security release checklist has no open high-severity item.

Gates 5 and 6 are not satisfied by Stage 19 for Android because no device or
emulator was available. A production release remains blocked on that evidence
and on production signing material.

Stage 20 adds the public protocol, governance, disclosure, infrastructure,
known-limitations, and maintenance documents, but it does not close those gates.
The current blockers are listed in
[KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md) and
[../progress/STAGE_20.md](../progress/STAGE_20.md). No public release has been
published.

## Crash reporting

Ember 0.1.0 has no crash reporting, telemetry, analytics, advertising, or remote
logging. A crash reporter must not be added without a separate privacy design
that covers:

- explicit opt-in and a visible off switch,
- exactly what is collected and what is redacted,
- local preview before upload,
- retention and deletion,
- the identity and contact of the operator,
- behaviour when the network is unavailable,
- a build without the reporter for reproducible security builds.

Until that exists, crash evidence is collected by the user manually: the app
does not upload anything.
