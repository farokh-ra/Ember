# Known Limitations

**Status:** Public draft for Stage 20
**Applies to:** Ember `0.1.x` development builds
**Last updated:** 2026-11-26

Ember has not launched publicly. This document lists product limits and missing
release evidence so users, reviewers, and operators can judge the project
without relying on marketing claims.

## 1. Product limits

- **Public by design.** Published files and metadata are public. Ember does not
  provide private-file sharing, private groups, or access control after
  publication.
- **No global deletion.** A tombstone withdraws the uploader's record. It does
  not delete copies held by other peers, and the original signed record remains
  valid.
- **No anonymity claim.** Peer IDs, network addresses, timing, and content
  interests may be observable to other participants and to a local network
  observer.
- **Local moderation only.** Blocking and abuse reports stay on the device. A
  blocked peer can return under a new identity.
- **No central account or recovery service.** Losing the identity key is
  permanent unless the user has an encrypted backup. v1 has no automatic key
  rotation or central revocation.
- **No guaranteed availability.** A publisher can stop serving at any time.
  Replication is voluntary and never automatic.
- **No central search.** Distributed search is partial. Index providers are
  optional peers, not authorities, and search never claims complete network
  coverage.
- **Extension-based safety classification.** File-safety notices are advisory,
  not malware detection. Downloaded files are never executed automatically;
  previews are delegated.
- **No telemetry or crash reporting.** The project has no remote visibility into
  crashes or failures. Diagnostics are local unless a user chooses to share
  them.
- **UI prototype status.** Several Kotlin screens still use demo data and are not
  wired to the Rust core for networking, publishing, hosting, or search flows.
- **English-only UI.** Locale-aware formatting and translated resources are not
  complete.
- **No private transfer mode.** All published content is intended to be
  discoverable by peers.

## 2. Platform and runtime evidence gaps

| Area | Status |
|---|---|
| Windows 10 runtime | Exercised on Windows 10 19045 |
| Windows 11 runtime | Not exercised |
| Android runtime networking | Not exercised on a device or emulator |
| Android low-memory and battery behavior | Not measured |
| Android Doze and sleep/wake | Not measured |
| Poor Wi-Fi, mobile, IPv6, and network changes | Not measured |
| Screen readers, 200% text, localization | Not fully exercised |
| Long-running soak on Android | Not measured |
| Windows installer upgrade and uninstall | Exercised with test signing |
| Android APK install and upgrade | Built and signature-verified; not installed on a device |

Windows loopback evidence is real but narrow. It does not prove global-network
behavior, difficult NAT combinations, or mobile reliability.

## 3. Release and supply-chain gaps

- Production Android keystore and Windows code-signing certificate are not
  available. Artifacts are test-signed only.
- Gradle dependency locking and verification metadata are not part of the
  release build.
- A CycloneDX SBOM is not produced.
- Third-party license notices for the bundled JDK and Compose runtime are not
  packaged.
- Reproducibility is claimed for inputs and checksums, not bit-identical output.
- No independent security review has been commissioned.
- No closed alpha or beta with real users has run.
- The Windows MSI does not offer an uninstall keep/remove choice for user data.
- No public release has been signed or published.

These are release blockers, not cosmetic gaps.

## 4. Network and interoperability gaps

- No independent client implementation has been tested.
- No independent bootstrap, relay, or index operator has been verified.
- No public infrastructure directory is published.
- No real-network discovery, transfer, or replication evidence exists outside
  loopback.
- Sybil resistance is partial. Many colluding peers can influence which records
  are reachable.
- DHT records are in-memory hints. A restart loses the local store; records are
  rebuilt by republication and lookup.
- There is no automatic global bootstrap-list update service.
- UPnP and NAT-PMP port mapping are not enabled.
- A relay is a bounded control path, not a file-transfer transport of last
  resort.

## 5. Privacy and operations gaps

- There is no per-report deletion control. Reports are local and capped, but
  removing one requires clearing the local store.
- No network-health reporting exists. A future opt-in design is documented but
  not implemented.
- No public moderation log or transparency report exists.
- No public governance body or named maintainer list exists.
- No public security advisory process has been exercised.
- No operator support commitment or service-level agreement exists.

## 6. What is not claimed

Ember does not claim:

- that it is anonymous, untraceable, or immune to network observation;
- that publishing is reversible network-wide;
- that search covers the whole network;
- that a local block removes content;
- that file-safety notices detect malware;
- that test-signed artifacts are publicly trusted;
- that the network is globally available or independently operated;
- that the protocol is independently implemented;
- that the product is ready for public launch.

## 7. Launch blockers

The public launch gate remains closed until:

1. production release signing is configured and verified;
2. Android runtime, device, and difficult-network evidence is collected;
3. Windows 11 runtime evidence is collected;
4. dependency locking, verification metadata, an SBOM, and third-party notices
   are part of the release;
5. the security release checklist has no open high-severity item;
6. the UI is wired to the core for the flows a release claims;
7. at least one independent client or operator interoperability result exists,
   or the launch is explicitly scoped as a limited preview without that claim;
8. the project owner approves the launch, support policy, and public
   infrastructure plan.

Items 1–7 are evidence or engineering gaps. Item 8 is a decision that cannot be
made by a coding agent.

## 8. References

- [Stage 20 evidence](../progress/STAGE_20.md)
- [Compatibility matrix](COMPATIBILITY_MATRIX.md)
- [Signing and reproducibility](SIGNING_AND_REPRODUCIBILITY.md)
- [Performance report](PERFORMANCE_REPORT.md)
- [Release notes 0.1.0](RELEASE_NOTES_0.1.0.md)
- [Security checklist](../security/SECURITY_RELEASE_CHECKLIST.md)
