# Support

Ember is a pre-release, local-first peer-to-peer application. There is no hosted
service, account system, help desk, or service-level agreement.

## Where to ask

| Need | Channel |
|---|---|
| Reproducible bug | [Bug report](https://github.com/farokh-ra/Ember/issues/new?template=bug_report.yml) |
| Scoped feature request | [Feature request](https://github.com/farokh-ra/Ember/issues/new?template=feature_request.yml) |
| Security vulnerability | [Security policy](SECURITY.md); never a public issue |
| Conduct concern | [Code of Conduct](CODE_OF_CONDUCT.md) |
| Release readiness | [Release readiness issue](https://github.com/farokh-ra/Ember/issues/new?template=release_readiness.yml) |
| Documentation | Open an issue with the `documentation` label |

Do not include private keys, recovery phrases, passphrases, tokens, personal file
names, unredacted peer IDs, or full local paths in any public issue.

## What is supported

- The current `main` branch and the latest development line.
- Windows 10 builds produced by this repository.
- Android builds that compile against the pinned SDK, with the runtime evidence
  limits listed in [docs/release/KNOWN_LIMITATIONS.md](docs/release/KNOWN_LIMITATIONS.md).
- Documentation corrections and build failures on the pinned toolchain.

## What is not supported

- Production signing, app-store distribution, or a public release. None exists.
- Android devices, Windows 11, poor networks, or long-running mobile behavior
  that has not been tested. See the known limitations.
- Recovering a lost identity key. There is no central recovery service.
- Removing published content from peers who already copied it.
- Private file sharing, private groups, or anonymity guarantees.
- Individual dependency support. Report upstream issues upstream.

## Response expectations

There is no guaranteed response time. Security reports are prioritised according
to [docs/security/DISCLOSURE_AND_ADVISORIES.md](docs/security/DISCLOSURE_AND_ADVISORIES.md).
Feature requests may be declined to preserve the local-first, no-central-backend
architecture.

## Before opening an issue

1. Read [Known Limitations](docs/release/KNOWN_LIMITATIONS.md).
2. Search existing issues.
3. Reproduce on the latest `main`.
4. Run `.\scripts\build.ps1` and include the failing step and exit code.
5. Redact logs and personal data.

## Commercial and operator support

There is no commercial support offering and no operator SLA. Independent
bootstrap, relay, and index operators are responsible for their own systems. The
project publishes guidance, not a managed service. See
[docs/operations/INFRASTRUCTURE_DIRECTORY.md](docs/operations/INFRASTRUCTURE_DIRECTORY.md).
