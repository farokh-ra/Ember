# NEXUS Development Workflow

This document defines how work moves from an issue to `main` and from `main` to a
release. It is the operational companion to
[CONTRIBUTING.md](../CONTRIBUTING.md) and
[DEVELOPER_SETUP.md](DEVELOPER_SETUP.md).

## 1. Daily loop

```powershell
# 1. Update the trunk
git switch main
git pull --ff-only

# 2. Create a focused branch
git switch -c feat/123-add-empty-state

# 3. Load the pinned toolchain in this shell
. .\scripts\dev-env.ps1

# 4. Work and test incrementally
.\gradlew.bat spotlessApply detekt
.\gradlew.bat :shared:model:jvmTest :shared:core-api:jvmTest
cargo fmt --all
cargo test --workspace

# 5. Run the complete gate before pushing
.\scripts\build.ps1

# 6. Commit with a Conventional Commit title
git add -A
git commit -m "feat(ui): add empty state for the explore screen"

# 7. Push and open a pull request
git push -u origin feat/123-add-empty-state
```

Guidelines:

- Keep changes small enough to review in one sitting.
- Rebase on `main` rather than merging `main` into a feature branch when
  practical.
- Run `spotlessApply` and `cargo fmt` before committing to keep the diff focused.
- Do not commit generated output, caches, `local.properties`, or IDE state.
- Do not mix a dependency upgrade with a feature change.
- If a check fails, fix the cause rather than bypassing the gate.

## 2. Branch conventions

| Branch | Scope |
|---|---|
| `main` | Protected trunk; always builds and passes tests. |
| `feat/<issue>-<slug>` | New behavior. |
| `fix/<issue>-<slug>` | Defect correction. |
| `docs/<issue>-<slug>` | Documentation. |
| `refactor/<issue>-<slug>` | Internal restructuring. |
| `test/<issue>-<slug>` | Tests and fixtures. |
| `ci/<issue>-<slug>` | CI, build, or tooling. |
| `chore/<issue>-<slug>` | Maintenance. |
| `release/<version>` | Release preparation. |

Rules:

- Branch from the latest `main`.
- One issue or one coherent outcome per branch.
- Pull request titles follow Conventional Commits; squash merge uses that title.
- Delete merged branches.
- Never rewrite a shared branch without coordinating with its reviewers.
- Do not push directly to `main` after branch protection is enabled on the
  remote.

## 3. Commit conventions

Format:

```text
<type>(<scope>): <description>
```

The validator enforces the allowed types, lowercase description, no trailing
period, and a 100-character limit:

```powershell
python scripts/check_commit_message.py "fix(core): reject blank peer id"
```

Breaking changes use `!` and a footer:

```text
feat(protocol)!: require signed record version 2

BREAKING CHANGE: peers that only understand record version 1 are rejected.
```

Recommended commit granularity:

1. A build or dependency change that stands alone.
2. A behavior change with its tests.
3. Documentation for the behavior.
4. Formatting-only changes, separate from logic.

## 4. Pull request workflow

1. Open the pull request against `main` using the template.
2. Link the issue with `Refs #<issue>` or `Closes #<issue>`.
3. Describe the change, verification, and compatibility or security impact.
4. Wait for required checks and at least one review.
5. Address review comments with new commits; do not force-push during review
   unless the branch is private to you and reviewers agree.
6. Squash merge with the Conventional Commit pull request title.
7. Confirm `main` is green after merge.

Reviewers verify:

- The change belongs in the selected module or crate.
- Error paths are handled and do not leak secrets.
- Tests cover success, failure, and boundary behavior.
- Accessibility and keyboard behavior are preserved for UI changes.
- Protocol, storage, ABI, and migration effects are documented.
- New dependencies are approved and inventoried.
- Documentation and changelog are updated when needed.

## 5. Quality gates

Run the same gates locally that CI runs:

| Gate | Local command | CI job |
|---|---|---|
| Repository hygiene and secret scan | `python scripts/check_repo_hygiene.py` | `hygiene` |
| Pull request title | `python scripts/check_commit_message.py "<title>"` | `commit-convention` |
| Kotlin formatting | `.\gradlew.bat spotlessCheck` | `android` |
| Kotlin static analysis | `.\gradlew.bat detekt` | `android` |
| Android debug build | `.\gradlew.bat :apps:androidApp:assembleDebug` | `android` |
| Shared Kotlin tests | `.\gradlew.bat :shared:model:jvmTest ...` | `android` |
| Windows desktop image | `.\gradlew.bat :apps:desktopApp:createDistributable` | `desktop` |
| Rust formatting | `cargo fmt --all -- --check` | `rust` |
| Rust lint | `cargo clippy --workspace --all-targets -- -D warnings` | `rust` |
| Rust tests | `cargo test --workspace` | `rust`, `rust-windows` |
| Rust dependency policy | `cargo deny check` | `rust` |
| Rust advisories | `cargo audit` | `rust` |
| Dependency review | GitHub dependency review action | `dependency-review` |

The aggregate local command is:

```powershell
.\scripts\build.ps1
```

Add `-WithDependencyScan` to include `cargo deny check` and `cargo audit`.

### Proving that failures are detected

The exit criteria require CI to detect compilation or test failures. The same
proof can be repeated locally:

1. Introduce a temporary syntax error or failing assertion in a Kotlin or Rust
   source file.
2. Run the corresponding gate, for example:

   ```powershell
   .\gradlew.bat :shared:model:jvmTest
   cargo test --workspace
   ```

3. Confirm the command exits non-zero and reports the failure.
4. Revert the temporary change and confirm the gate passes again.

The exact commands and observed results for Stage 03 are recorded in
[progress/STAGE_03.md](progress/STAGE_03.md).

## 6. CI workflow

`.github/workflows/ci.yml` runs on every push to `main`, every pull request, and
manual dispatch:

| Job | Runner | What it proves |
|---|---|---|
| `hygiene` | Ubuntu | No forbidden files or common credential patterns. |
| `commit-convention` | Ubuntu | Pull request title follows Conventional Commits. |
| `rust` | Ubuntu | Formatting, Clippy, tests, documentation tests, license/advisory policy. |
| `rust-windows` | Windows | Native MSVC workspace build and tests for the release target. |
| `android` | Ubuntu | Formatting, detekt, debug APK, shared JVM tests, artifact upload. |
| `desktop` | Windows | Compose desktop application image and artifact upload. |

The dependency review workflow runs on pull requests and fails on high-severity
dependency changes. Dependabot opens grouped weekly updates for Gradle, Cargo,
and GitHub Actions.

CI uses the MSVC Rust host on Windows and the GNU host on Ubuntu; the local MinGW
fallback is development-only. A CI pass is required before merge. Do not disable
a required check to land a change.

## 7. Dependency updates

Dependabot pull requests are treated like any other change:

1. Review the changelog and license of every updated package.
2. Confirm the pin still matches `docs/architecture/DEPENDENCY_LICENSE_INVENTORY.md`.
3. Run the full build, including `-WithDependencyScan`.
4. Confirm no transitive license or advisory regression.
5. Update the inventory and, if a major version changes, the relevant ADR.
6. Merge only when all gates pass.

Do not accept a major Kotlin, Compose, AGP, Gradle, or Rust upgrade without
checking the compatibility notes and rebuilding both applications. Toolchain
upgrades are separate from feature work.

## 8. Release workflow

Releases are cut from `main` after the stage exit criteria are approved.

1. Confirm all stage records and documentation are current.
2. Update `CHANGELOG.md` and set the version in:
   - `build.gradle.kts` (`allprojects.version`),
   - `apps/androidApp/build.gradle.kts` (`versionName`, `versionCode`),
   - `apps/desktopApp/build.gradle.kts` (`packageVersion`),
   - `Cargo.toml` (`workspace.package.version`).
3. Run `.\scripts\build.ps1 -WithDependencyScan`.
4. Run `.\scripts\verify-clean-checkout.ps1`.
5. Create a `release/<version>` branch, open a pull request, and merge it.
6. Tag `main` with `v<version>`.
7. The release environment supplies signing keys and publishes:
   - the Android APK/AAB,
   - the Windows per-user installer,
   - SHA-256 checksums,
   - the CycloneDX SBOM,
   - third-party notices,
   - generated release notes from `.github/release.yml`.
8. Verify the signatures, checksums, install, upgrade, and uninstall paths.

No signing key, keystore, certificate, or password is committed or attached to a
release note. Release artifacts are never built from an unlocked dependency
graph.

## 9. Hotfixes

For a defect that affects a released version:

1. Branch `fix/<issue>-<slug>` from the release tag or `main`, whichever contains
   the affected code.
2. Make the smallest safe fix and add a regression test.
3. Run the full gate.
4. Merge to `main`, then backport to the release branch if one is maintained.
5. Tag a patch release and update the changelog.

Security fixes follow the same path but are coordinated privately until users
have a reasonable opportunity to update.

## 10. Stage discipline

NEXUS is built stage by stage from
`NEXUS_COMPLETE_DEVELOPMENT_ROADMAP.md`. A stage is complete only when:

- Its deliverables exist and build.
- Its exit criteria have recorded evidence.
- Formatting, linting, static analysis, tests, and hygiene pass.
- Documentation and the handoff are updated.
- The project owner approves the next stage.

Do not start work from a later stage while the current stage is incomplete,
except for a small fix required to keep the current stage buildable. Record such
a fix in the stage file and the handoff.

## 11. Troubleshooting the workflow

| Problem | Resolution |
|---|---|
| `spotlessCheck` fails | Run `.\gradlew.bat spotlessApply`, inspect the diff, and commit formatting separately if needed. |
| detekt fails on a composable name | Compose function naming is configured in `config/detekt/detekt.yml`; do not rename a composable to satisfy a generic rule. |
| Rust Clippy fails | Fix the lint; do not add a blanket `allow`. A narrow, justified allow needs review. |
| Hygiene scan flags a test string | Use an obviously fake, short value that does not match a credential pattern, or restructure the test. Never add a real secret. |
| Clean checkout fails but the working tree passes | The build depends on an untracked or ignored file. Commit the missing source or remove the dependency on local state. |
| CI differs from local | Confirm the pinned versions with `java -version`, `gradle --version`, and `rustc --version --verbose`; CI uses MSVC on Windows and GNU on Ubuntu. |
| A required check is stuck | Re-run the job after confirming the failure is infrastructure-related; do not bypass the check silently. |
