# Ember Project Handoff

**Current stage:** 20 - global launch and ongoing network governance
**Stage status:** Repository deliverables complete; public launch blocked
**Last updated:** 2026-11-26
**Branch:** `feat/20-launch`, stacked on Stage 19 `feat/19-release`
**Pull requests:** #28 (`chore/20-launch-readiness` -> `feat/20-launch`), #27
(`feat/20-launch` -> `feat/19-release`), and #26 (`feat/19-release` -> `main`)
are open and unmerged
**Next stage:** none in the roadmap; launch readiness and maintenance remain

## What Stage 20 adds

Stage 20 publishes the protocol and the operating policy that a decentralized
network needs, without pretending that a public launch has happened:

- A public protocol v1 specification covering identity, canonical CBOR, the
  signed envelope, profile, publication, tombstone, and manifest records, DHT
  keys, `/nexus/file/1`, `/nexus/index/1`, transport expectations, limits,
  error codes, and security properties.
- A versioning and deprecation policy with version vectors, change classes,
  support windows, migration rules, and a review checklist.
- An interoperability document and an eight-test public conformance suite that
  pins the same vectors as the specification. This is self-conformance, not
  independent-client evidence.
- A community infrastructure directory policy for independent bootstrap, relay,
  and index operators. No directory is published.
- A privacy-preserving network-health design. Local diagnostics exist; remote
  opt-in reporting is not implemented.
- Governance, moderation, security disclosure, known-limitations, and
  post-launch maintenance documents. They are drafts for owner approval.

The stage does **not** publish a release, create public infrastructure, or
satisfy the production launch gate.

## Launch-readiness follow-up

A stacked change on `chore/20-launch-readiness`, open as PR #28, closes the
repository-side supply-chain gap and fills the community files. It still does
not publish a release.

- Gradle dependency locking is enabled for every project; one `gradle.lockfile`
  per project and `settings-gradle.lockfile` are committed.
- `gradle/verification-metadata.xml` records SHA-256 checksums for the resolved
  plugin and Maven graph.
- `scripts/generate_sbom.py` emits a deterministic CycloneDX 1.5 SBOM and an
  optional third-party notice bundle.
- `scripts/update-dependency-locks.ps1` regenerates the lock and verification
  state after an intentional dependency change.
- `package-release.ps1` emits the SBOM and notice bundle, records both in the
  manifest, and `verify-release.ps1` rejects a missing or malformed bundle.
- The Windows app image includes `LICENSE.md` and `THIRD_PARTY_NOTICES.md`.
- `CODE_OF_CONDUCT.md`, `SUPPORT.md`, `GOVERNANCE.md`, `.github/CODEOWNERS`, and a
  release-readiness issue form are added.
- CI gains a supply-chain job and a lock-drift check.

See [SBOM_AND_NOTICES.md](release/SBOM_AND_NOTICES.md) and
[STAGE_20.md](progress/STAGE_20.md). The launch gate remains closed on signing,
device evidence, independent interoperability, a reviewed notice bundle, and
owner approval.

## Files changed

Protocol and conformance:

- `docs/protocol/PROTOCOL_SPEC.md` (new)
- `docs/protocol/VERSIONING_AND_DEPRECATION.md` (new)
- `docs/protocol/INTEROPERABILITY.md` (new)
- `crates/nexus-protocol/tests/conformance.rs` (new, 8 tests)

Operations:

- `docs/operations/INFRASTRUCTURE_DIRECTORY.md` (new)
- `docs/operations/NETWORK_HEALTH.md` (new)
- `docs/operations/BOOTSTRAP_NODES.md`
- `docs/operations/RELAY_NODES.md`

Governance and security:

- `docs/governance/GOVERNANCE.md` (new)
- `docs/governance/MODERATION_PRINCIPLES.md` (new)
- `docs/security/DISCLOSURE_AND_ADVISORIES.md` (new)
- `docs/decisions/ADR-0011-public-protocol-and-launch-governance.md` (new)
- `SECURITY.md`

Release and project state:

- `docs/release/KNOWN_LIMITATIONS.md` (new)
- `docs/release/POST_LAUNCH_MAINTENANCE.md` (new)
- `docs/release/SBOM_AND_NOTICES.md` (new)
- `docs/release/README.md`
- `docs/architecture/SIGNED_RECORD_FORMATS.md`
- `docs/security/PRIVACY_REVIEW.md`
- `docs/progress/STAGE_20.md` (new)
- `docs/HANDOFF.md`
- `README.md`
- `CHANGELOG.md`

Supply chain and community files:

- `gradle/verification-metadata.xml` (new)
- `gradle.lockfile`, `settings-gradle.lockfile`, and one `gradle.lockfile` per
  project (new)
- `scripts/generate_sbom.py` (new)
- `scripts/update-dependency-locks.ps1` (new)
- `THIRD_PARTY_NOTICES.md` (new)
- `CODE_OF_CONDUCT.md`, `SUPPORT.md`, `GOVERNANCE.md` (new)
- `.github/CODEOWNERS` and `.github/ISSUE_TEMPLATE/release_readiness.yml` (new)
- `.github/workflows/ci.yml`, `.github/pull_request_template.md`,
  `.github/ISSUE_TEMPLATE/config.yml`
- `scripts/package-release.ps1`, `scripts/verify-release.ps1`,
  `scripts/build.ps1`, `build.gradle.kts`, `apps/desktopApp/build.gradle.kts`
- `CONTRIBUTING.md`, `SECURITY.md`

## Commands and results

| Command | Result |
|---|---|
| `cargo test -p nexus-protocol --test conformance` | 8 passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | 380 passed, 0 failed, 52 suites |
| `cargo deny check` | Passed |
| `cargo audit` | No vulnerability; one allowed `paste` unmaintained warning |
| `.\scripts\build.ps1 -WithDependencyScan` | Passed; native Windows/Android artifacts, C harness, Gradle checks with committed locks, Rust gates, dependency scan, SBOM, and hygiene |
| CycloneDX SBOM | 1,130 components (399 Cargo, 731 Maven) |
| `package-release.ps1 -AllowUnsigned -AllowDirty` | Passed; SBOM and notice bundle emitted and verified |
| Repository hygiene | Passed; 377 files scanned |

The full build finished in 8 minutes 10 seconds with `BUILD SUCCESSFUL`. The
unsigned dry run is local evidence, not a published release.

## Launch blockers

The public launch gate remains closed on:

1. production Android keystore and Windows code-signing certificate;
2. Android runtime, device, low-memory, battery, Doze, and poor-network evidence;
3. Windows 11 runtime evidence;
4. a reviewed SBOM and notice bundle, plus verification metadata exercised on
   every CI platform;
5. independent client or operator interoperability evidence;
6. a public infrastructure directory with independent operators;
7. UI wiring from the Kotlin screens to the Rust core for the flows a release
   would claim;
8. closed alpha or beta feedback;
9. project-owner approval of launch, support, governance, and infrastructure
   policy.

A coding agent cannot close items 1, 5, 6, 8, or 9.

## Stage 19 summary

Stage 19 built the release pipeline and produced test-signed artifacts:

- per-user Windows MSI with a stable upgrade UUID and no elevation;
- portable Windows ZIP;
- signed Android APK and AAB with environment-driven signing;
- checksums, release manifest, packaging, verification, and performance
  scripts;
- a tag/manual release workflow;
- release, performance, compatibility, signing, reproducibility,
  update/rollback, and accessibility documents.

Verified evidence:

| Item | Result |
|---|---|
| APK | 64,282,841 bytes, SHA-256 `fb020582...df86c` |
| AAB | 23,285,459 bytes, SHA-256 `15b21e02...5c91` |
| MSI | 74,465,280 bytes, SHA-256 `2669b08b...256e63` |
| Portable ZIP | 73,063,852 bytes, SHA-256 `1f52e12a...30655f` |
| MSI install / upgrade / uninstall | Passed |
| Installed launch | 2,887 ms |
| Portable launch | 2,890 ms |
| Median startup | 2,805 ms |
| Working set | 207.1 MiB |
| Idle CPU | 0.021% |
| 180-second growth | 0.9 MiB |

The artifacts are test-signed. The self-signed Windows certificate reports
`UnknownError` trust, and the Android signature is valid but not publicly
trusted. See [STAGE_19.md](progress/STAGE_19.md) for the full record.

## Decisions

- Protocol v1 is documented publicly; the Rust implementation is the reference
  implementation, not the specification by itself.
- The conformance suite is self-conformance and is not described as independent
  interoperability.
- Governance and moderation are local-first. The project claims no global
  content authority and no network-wide ban.
- The infrastructure directory is a reviewed hint list, not an authority. It is
  not published while no independent operator exists.
- Network health remains local-only. Any future reporting needs explicit
  opt-in, preview, aggregation, suppression, and a privacy review.
- Test-signed artifacts are never described as a public release.
- No launch date is promised.

## Exact next action

1. Keep PR #26 open; do not merge without project-owner approval.
2. Keep PR #27 open against `feat/19-release`; do not merge it without
   approval.
3. Keep PR #28 open against `feat/20-launch`; do not merge it without approval.
4. Obtain production signing material and collect Android and Windows 11
   evidence before any public release.
5. Run a closed alpha or beta and record failures.
6. Recruit an independent client implementer or operator, or explicitly scope a
   limited preview without an interoperability claim.
7. Obtain owner approval for governance, disclosure, directory, and launch
   policy.
8. Review and approve the generated third-party notice bundle.
9. Re-run the full release gate before publishing any artifact.
