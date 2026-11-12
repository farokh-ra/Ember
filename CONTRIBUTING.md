# Contributing to NEXUS

NEXUS is built in reviewed stages. Contributions must respect the architecture
boundaries, the pinned toolchain, the license policy, and the local-first
privacy model.

Read these first:

- [README.md](README.md)
- [docs/DEVELOPER_SETUP.md](docs/DEVELOPER_SETUP.md)
- [docs/DEVELOPMENT_WORKFLOW.md](docs/DEVELOPMENT_WORKFLOW.md)
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/LOGGING_POLICY.md](docs/LOGGING_POLICY.md)
- [docs/decisions](docs/decisions/README.md)

## Before you start

1. Install the pinned toolchain with `.\scripts\setup-windows.ps1`.
2. Load it into the current shell with `. .\scripts\dev-env.ps1`.
3. Run `.\scripts\build.ps1` and confirm the baseline passes.
4. Open or claim an issue before large changes. Small documentation and test
   fixes may go straight to a pull request.

Do not upgrade Kotlin, Compose, Gradle, AGP, Rust, the NDK, or a runtime
dependency as part of an unrelated change. Dependency updates are reviewed
separately because they affect reproducibility, licenses, and native builds.

## Branch model

The repository uses trunk-based development with short-lived branches.

| Branch | Use |
|---|---|
| `main` | Always buildable and reviewable. Protected; no direct pushes once remotes are configured. |
| `feat/<issue>-<slug>` | New user-visible behavior. |
| `fix/<issue>-<slug>` | Defect correction. |
| `docs/<issue>-<slug>` | Documentation only. |
| `refactor/<issue>-<slug>` | Internal change with no behavior change. |
| `test/<issue>-<slug>` | Test-only change. |
| `ci/<issue>-<slug>` | Workflow, build, or tooling change. |
| `chore/<issue>-<slug>` | Maintenance that fits no other type. |
| `release/<version>` | Release preparation only; merged back to `main`. |

Rules:

- Branch from the latest `main`.
- Keep a branch focused on one issue or one coherent change.
- Rebase or merge `main` before requesting review; resolve conflicts locally.
- Delete the branch after merge.
- Never force-push a branch that someone else is reviewing without warning.

## Commit and pull request titles

Commits and pull request titles follow [Conventional Commits
1.0.0](https://www.conventionalcommits.org/en/v1.0.0/):

```text
<type>(<scope>): <description>
```

The description is lowercase, has no trailing period, and the full title is at
most 100 characters.

Allowed types:

| Type | Meaning |
|---|---|
| `feat` | New user-visible capability. |
| `fix` | Defect correction. |
| `docs` | Documentation only. |
| `test` | Tests or fixtures only. |
| `refactor` | Internal restructuring without behavior change. |
| `perf` | Performance improvement. |
| `build` | Build system, dependency pins, or packaging. |
| `ci` | Continuous integration or automation. |
| `chore` | Maintenance that fits no other type. |
| `revert` | Reverts a previous commit. |
| `style` | Formatting only, no logic change. |

Suggested scopes: `android`, `desktop`, `shared`, `model`, `core-api`,
`core-client`, `presentation`, `ui`, `core`, `ffi`, `net`, `store`, `transfer`,
`identity`, `crypto`, `protocol`, `search`, `docs`, `build`, `ci`, `release`.

Examples:

```text
feat(android): add foreground service notification
fix(core): reject blank peer identifiers
docs(setup): document the MinGW fallback
build(deps): pin Compose Material3 to 1.9.0
```

Use `!` after the type or scope and a `BREAKING CHANGE:` footer for incompatible
protocol, storage, ABI, or configuration changes:

```text
feat(protocol)!: require signed record version 2
```

Validate a title locally:

```powershell
python scripts/check_commit_message.py "feat(ui): add empty state"
```

CI validates the pull request title. The merge strategy is squash merge, so the
pull request title becomes the commit on `main`.

## Pull requests

Every pull request must:

- Link its issue with `Refs #<issue>` or `Closes #<issue>`.
- Explain what changed, why, user-visible impact, and compatibility or security
  consequences.
- Include tests for new behavior and regression tests for fixes.
- Update documentation when behavior, commands, configuration, or architecture
  changes.
- Pass all local quality gates.
- Contain no secrets, keystores, signing material, generated build output, or
  machine-specific paths.
- Keep unrelated formatting or refactoring out of the diff.

Use the repository pull request template. Reviewers check architecture
boundaries, error handling, accessibility, privacy, license compatibility, and
the definition of done below.

## Issues

Use the provided bug report or feature request form. Include the commit or
version, platform, reproduction steps, expected and actual behavior, and
redacted logs. Never include private keys, recovery phrases, tokens, peer
addresses, personal file names, or unredacted user paths.

Security vulnerabilities must not be filed publicly. Follow
[SECURITY.md](SECURITY.md): use GitHub private vulnerability reporting when it is
enabled for the repository, or contact the repository owner through the private
channel listed on the repository profile. Include the affected version, impact,
reproduction, and any known mitigation.

## Coding standards

### Kotlin and Compose

- Kotlin 2.4.21, JVM target 21, official Kotlin code style.
- Four-space indentation, 120-character line limit.
- Public declarations use explicit visibility and KDoc where the contract is not
  obvious.
- Keep composables in `shared/ui`; keep product state and validation in
  `shared/presentation`; keep durable work out of both.
- Add tests under `src/jvmTest` until Android host tests or common tests are
  enabled by a later change.
- Run `.\gradlew.bat spotlessApply detekt` before committing.

### Rust

- Rust 1.99.1, edition 2024, four-space indentation, 120-character line limit.
- Workspace lints deny `unsafe_code` and Clippy warnings. `nexus-ffi` is the only
  crate allowed to opt out for the C ABI surface.
- Public items have doc comments. Fallible APIs return typed errors.
- Keep crate responsibilities as defined in `docs/ARCHITECTURE.md`; do not add a
  dependency from a lower layer to a higher layer.
- Add unit tests in the crate and integration tests in `nexus-testkit` when the
  behavior crosses crates.
- Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test --workspace`.

### Logging and privacy

- Never log secret material, tokens, private keys, recovery phrases, passphrases,
  signing material, or full user file paths.
- Use the redaction helpers in `shared/model` and `nexus-observability`.
- Debug logging is opt-in; info-level logs must be safe to attach to a bug report.
- Follow [docs/LOGGING_POLICY.md](docs/LOGGING_POLICY.md).

## Dependency policy

- Allowed licenses are defined in
  [ADR-0009](docs/decisions/ADR-0009-build-packaging-dependency-policy.md).
- New dependencies require a reason, license check, platform check, owner, and an
  entry in
  [DEPENDENCY_LICENSE_INVENTORY.md](docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md).
- No dynamic versions, version ranges, snapshots, or unapproved Git sources.
- Do not add analytics, crash reporting, advertising, or account SDKs.
- Run `cargo deny check` and `cargo audit` when Rust dependencies change.
- Gradle dependency verification metadata is introduced before signed releases;
  until then, review changes to `gradle/libs.versions.toml` and lockfiles
  explicitly.

## Release conventions

- Versions follow semantic versioning: `MAJOR.MINOR.PATCH`, with optional
  pre-release suffixes such as `-alpha.1`.
- Release tags use `v<version>`, for example `v0.1.0`.
- `CHANGELOG.md` is updated before tagging.
- Release notes are generated from `.github/release.yml` and reviewed manually.
- Android APK/AAB and Windows installer artifacts are built by CI, signed with
  release-environment keys, and published with SHA-256 checksums and an SBOM.
- Private signing keys, keystores, certificates, and passwords are never
  committed. They are supplied only through the release environment.
- A release requires a clean checkout, all CI gates green, a completed
  `docs/progress/STAGE_*.md` record for the covered stage, and project-owner
  approval.

## Definition of done

A change is done when:

- The behavior is implemented within the correct module or crate.
- New and changed behavior has tests, and the full suite passes.
- Formatting, linting, static analysis, and repository hygiene pass.
- Documentation, changelog, and architecture records are updated as needed.
- No secrets or local machine paths are present.
- The pull request title follows Conventional Commits and links the issue.
- The change is reviewed and merged to `main`.
