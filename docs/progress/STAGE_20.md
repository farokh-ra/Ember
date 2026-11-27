# Stage 20 Evidence: Global Launch and Network Governance

**Status:** Repository deliverables complete; public launch blocked
**Branch:** `feat/20-launch`, stacked on Stage 19 `feat/19-release`
**Pull requests:** #27 (`feat/20-launch` -> `feat/19-release`); #28
(`chore/20-launch-readiness` -> `feat/20-launch`) adds the launch-readiness
follow-up
**Last updated:** 2026-11-27

Stage 20 is the final roadmap stage. It covers public protocol documentation,
community infrastructure operations, governance, security disclosure,
interoperability testing, and post-launch maintenance.

The repository work is complete. The launch itself is not. Ember has no
production signing material, no Android runtime evidence, no independent client
or operator, and no owner approval for a public release. This document records
both the delivered artifacts and the closed launch gate.

## 1. Deliverables

| Roadmap deliverable | Repository result | Status |
|---|---|---|
| Publish the protocol specification and supported client versions | `docs/protocol/PROTOCOL_SPEC.md` | Drafted and vector-checked |
| Publish bootstrap and relay operating instructions | Existing `docs/operations/BOOTSTRAP_NODES.md` and `RELAY_NODES.md`, plus a directory policy | Complete for local/loopback operation |
| Make independent infrastructure operation possible | `docs/operations/INFRASTRUCTURE_DIRECTORY.md` | Policy drafted; no public directory |
| Public list or discovery mechanism for independent operators | Directory format and process defined | Not published |
| Protocol upgrade and deprecation rules | `docs/protocol/VERSIONING_AND_DEPRECATION.md` | Drafted |
| Security disclosure process | `docs/security/DISCLOSURE_AND_ADVISORIES.md`, linked from `SECURITY.md` | Drafted; no advisory exercised |
| Moderation and abuse-handling principles | `docs/governance/MODERATION_PRINCIPLES.md` | Drafted |
| Privacy-preserving, opt-in network health | `docs/operations/NETWORK_HEALTH.md` | Design only; not implemented |
| Interoperability tests with independent clients | `docs/protocol/INTEROPERABILITY.md` and `crates/nexus-protocol/tests/conformance.rs` | Self-conformance complete; independent client unavailable |
| Capacity through community nodes | Operator guidance and directory policy | No community nodes known |
| Backward compatibility and migration policy | Versioning, update/rollback, and known-limitations docs | Drafted |
| Release notes, known limitations, advisories | `KNOWN_LIMITATIONS.md`, `POST_LAUNCH_MAINTENANCE.md`, existing release notes | Known limitations current; no advisory |
| Public Android and Windows releases | Test-signed Stage 19 artifacts only | Not published |
| Governance policy | `docs/governance/GOVERNANCE.md` | Drafted; owner approval pending |
| Post-launch maintenance plan | `docs/release/POST_LAUNCH_MAINTENANCE.md` | Drafted |

## 2. Protocol specification

`docs/protocol/PROTOCOL_SPEC.md` documents protocol v1:

- identity IDs and Ed25519 signing;
- the canonical CBOR profile;
- the signed envelope and all four signed record families;
- DHT key derivation and discovery records;
- `/nexus/file/1` and `/nexus/index/1` request, response, and error shapes;
- transport expectations, limits, and error codes;
- security properties and compatibility references.

The published vectors are the exact constants already committed in
`crates/nexus-protocol/tests/records.rs` and `stage07.rs`. They are not
regenerated from a separate implementation.

## 3. Conformance suite

`crates/nexus-protocol/tests/conformance.rs` adds eight self-conformance tests:

1. canonical identity vector;
2. exact profile, publication, and tombstone vectors;
3. exact manifest vector;
4. round-trip without re-encoding;
5. tampering and domain-replay rejection;
6. unknown-minor preservation and unknown-major rejection;
7. non-canonical CBOR and oversized-record rejection;
8. manifest shape rejection.

Command:

```powershell
cargo test -p nexus-protocol --test conformance
```

Result: 8 passed, 0 failed.

This is not independent-client interoperability. No second implementation was
available. The distinction is recorded in
`docs/protocol/INTEROPERABILITY.md`.

## 4. Governance and policy drafts

The following documents are drafts for project-owner approval. They describe the
rules the project intends to follow; they do not create a governance body, a
security team, or a public infrastructure directory.

- `docs/governance/GOVERNANCE.md`: decision classes, maintainer roles, protocol
  stewardship, release authority, conflicts of interest, and amendments.
- `docs/governance/MODERATION_PRINCIPLES.md`: local-first blocking and reporting,
  project-owned space moderation, operator autonomy, illegal-content handling,
  appeals, and transparency.
- `docs/security/DISCLOSURE_AND_ADVISORIES.md`: intake, severity, timelines,
  embargo, advisory format, safe harbor, and supported versions.
- `docs/operations/INFRASTRUCTURE_DIRECTORY.md`: listing criteria, diversity,
  verification, removal, and the fact that no directory exists yet.
- `docs/operations/NETWORK_HEALTH.md`: local diagnostics today and a
  privacy-preserving opt-in design that is not implemented.
- `docs/release/KNOWN_LIMITATIONS.md`: product limits, evidence gaps, and launch
  blockers.
- `docs/release/POST_LAUNCH_MAINTENANCE.md`: channels, cadence, release
  checklist, security response, rollback, infrastructure operations, and
  post-launch review.
- `docs/architecture/SIGNED_RECORD_FORMATS.md` and
  `docs/security/PRIVACY_REVIEW.md`: cross-links to the public specification and
  the unimplemented health-measurement design.
- `docs/decisions/ADR-0011-public-protocol-and-launch-governance.md` and the ADR
  index: proposed decision record for public protocol publication,
  self-conformance, and the launch gate.

## 5. Verification

| Command | Result |
|---|---|
| `cargo test -p nexus-protocol --test conformance` | 8 passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | 380 passed, 0 failed, 52 suites |
| `cargo deny check` | Passed; advisories, bans, licenses, and sources clean |
| `cargo audit` | No vulnerability; one allowed `paste` unmaintained warning |
| `.\scripts\build.ps1 -WithDependencyScan` | Passed; native Windows/Android artifacts, C harness, Gradle checks with committed locks, Rust gates, dependency scan, SBOM, and hygiene |
| Repository hygiene | Passed; 377 files scanned |

The full build finished in 8 minutes 10 seconds with `BUILD SUCCESSFUL`. The
Rust test count is 380: the previous 372 plus 8 conformance tests. A
documentation stage still had to pass every code gate because it adds an
executable test.

Launch-readiness follow-up:

| Check | Result |
|---|---|
| CycloneDX SBOM | 1,130 components (399 Cargo, 731 Maven), schema 1.5, self-validated |
| Notice bundle | 333 unique licence texts; 46 Cargo and 503 Maven components had no local licence file and remain listed |
| `package-release.ps1 -AllowUnsigned -AllowDirty` | Passed; six artifacts including the SBOM and notice bundle; `verify-release.ps1` passed |
| Release dry run | APK 64,270,553 bytes, AAB 23,276,608 bytes, MSI 74,473,564 bytes, portable ZIP 73,074,450 bytes, SBOM 765,881 bytes, notices 623,568 bytes |
| CI on PR #28 | All seven jobs passed on `4c04d33`: hygiene, title, Rust Linux, Rust Windows, SBOM and locks, Android, and Windows desktop |

The dry run is unsigned and local. It is not a published release and does not
satisfy the signing or device gates.

## 6. Launch gate

The public launch gate remains closed. The following are not satisfied:

1. Production Android keystore and Windows code-signing certificate.
2. Android runtime, device, low-memory, battery, Doze, and poor-network
   evidence.
3. Windows 11 runtime evidence.
4. A reviewed SBOM and notice bundle, and verification metadata exercised on
   the Linux Android release variants as well as the CI task set.
5. An independent client or operator interoperability result.
6. A public infrastructure directory with independent operators.
7. UI wiring from the Kotlin screens to the Rust core for the flows a release
   would claim.
8. Closed alpha or beta feedback with real users.
9. Owner approval of the launch, support policy, and public infrastructure plan.

Stage 20 cannot close these gaps from a repository. It records them honestly
instead of presenting documentation as launch evidence.

## 7. Decisions

- The protocol specification is normative for v1; the Rust implementation is
  the reference implementation, not the specification by itself.
- The conformance suite is self-conformance. It is not described as independent
  interoperability.
- Governance and moderation are local-first. The project does not claim a
  global content authority or a network-wide ban.
- The infrastructure directory is a reviewed hint list, not an authority. No
  directory is published while no independent operator exists.
- Network-health reporting remains unimplemented. The design requires explicit
  opt-in, local preview, aggregation, suppression, and a privacy review.
- No test-signed artifact is described as a public release.
- No launch date is promised.

## 8. Residual risks

- The specification may contain an untested interpretation that only an
  independent client would reveal.
- The governance and security processes may not survive contact with a real
  dispute or a real vulnerability.
- A public directory could become a de facto authority if diversity rules are
  not enforced.
- A future health reporter could become surveillance if the opt-in and
  aggregation rules are weakened.
- The project remains owner-controlled and has a bus-factor risk.
- Android and global-network behavior remain unmeasured.

## 9. Launch-readiness follow-up

A follow-up change on top of Stage 20, open as PR #28, closes the
repository-side supply-chain gap without publishing anything:

| Item | Result |
|---|---|
| Gradle dependency locking | Enabled for every project; lockfiles committed. |
| Gradle verification metadata | SHA-256 metadata committed; CI rejects drift. |
| CycloneDX SBOM | Deterministic generator; CI job uploads the SBOM. |
| Third-party notices | Generated bundle with unique licence texts and a component mapping; stable summary packaged in the Windows app. |
| Community files | Code of conduct, support policy, governance front door, code owners, and release-readiness issue form. |
| Release script | Emits and verifies the SBOM and notice bundle; records supply-chain flags in the manifest. |

This is still not a release. Production signing, device evidence, independent
interoperability, a reviewed notice bundle, Linux Android release-variant
verification, and owner approval remain open. See
[KNOWN_LIMITATIONS.md](../release/KNOWN_LIMITATIONS.md) and
[SBOM_AND_NOTICES.md](../release/SBOM_AND_NOTICES.md).

## 10. Next stage

There is no Stage 21 in the roadmap. The next work is maintenance and launch
readiness, not a new stage:

1. Obtain production signing material and collect Android and Windows 11
   evidence.
2. Review the generated notice bundle, exercise verification metadata on the
   Linux Android release variants, and add provenance attestation.
3. Run a closed alpha or beta and record failures.
4. Recruit an independent client implementer or operator for interoperability
   testing, or explicitly scope a limited preview without that claim.
5. Obtain owner approval for governance, disclosure, directory, and launch
   policies.
6. Re-run the full release gate before any public artifact is published.
