# NEXUS Project Handoff

**Current stage:** 04 — UI foundation and design system
**Stage status:** Complete
**Last updated:** 2026-11-13
**Next stage:** 05 — Local database and file-store foundation (waiting for user permission)

## What was implemented

Stage 04 is complete. The shared UI now provides a tested, clickable prototype
for every planned screen without pretending that later-stage features work:

- NEXUS light and dark themes, semantic status colors, typography, spacing,
  shape, and size tokens, plus a custom Compose icon set.
- A reusable component library for cards, status labels, action rows, buttons,
  selections, dialogs, state panels, navigation chrome, brand art, avatars, and
  preview controls.
- Externalized English copy behind `NexusStrings` / `LocalNexusStrings`, ready
  for a resource-backed localization provider.
- Immutable `NexusAppState` with per-section navigation stacks, demo
  onboarding, preview scenario and network state, and versioned persistence
  tokens.
- `NexusAppController` shared by both shells, with theme persistence and
  snapshot/restore support.
- A responsive shell: side rail at 840 dp and above, compact top bar plus bottom
  navigation below it.
- Eleven clickable screens: welcome, identity setup, Explore, Search, profile,
  file details, Publish, Downloads, hosted files, Settings, and network status.
- All five required content states on every data-driven screen, with a visible
  demo note and an action out of each state.
- A persistent, non-dismissible preview banner and visibly simulated content and
  network controls.
- Accessibility support: WCAG AA contrast tests, 48 dp touch targets, visible
  focus rings, roles and selection semantics, live-region state panels,
  text-plus-icon status, Android back, Escape back, and reduced-motion handling
  on Android.
- Android integration with saved state, `BackHandler`, edge-to-edge layout, and
  status-bar icon appearance.
- Desktop integration with a persisted theme, a 1280x720 initial window, and a
  720x560 minimum.
- 65 JVM tests: 44 presentation tests and 21 shared UI tests.
- Stage 04 evidence in `docs/progress/STAGE_04.md`.

All identity, publication, download, hosting, search, and network data is mock
content. No real service, key, or network operation is implemented.

## Files changed

Presentation:

- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/NexusAppState.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/PreviewStates.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/ThemeMode.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/content/ContentState.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoCatalog.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/demo/DemoModels.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/identity/IdentityDraft.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/navigation/AppDestination.kt`
- `shared/presentation/src/commonMain/kotlin/io/nexus/presentation/navigation/NavigationState.kt`
- `shared/presentation/src/jvmTest/**`

Shared UI:

- `shared/ui/src/commonMain/kotlin/io/nexus/ui/NexusApp.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/NexusAppController.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/components/**`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/icons/NexusIcons.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/resources/NexusStrings.kt`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/screens/**`
- `shared/ui/src/commonMain/kotlin/io/nexus/ui/theme/**`
- `shared/ui/src/androidMain/kotlin/io/nexus/ui/theme/**`
- `shared/ui/src/jvmMain/kotlin/io/nexus/ui/theme/**`
- `shared/ui/src/jvmTest/**`

Shells and build:

- `apps/androidApp/build.gradle.kts`
- `apps/androidApp/src/main/kotlin/io/nexus/android/MainActivity.kt`
- `apps/desktopApp/src/main/kotlin/io/nexus/desktop/Main.kt`
- `shared/ui/build.gradle.kts`
- `.editorconfig`, `config/detekt/detekt.yml`
- `scripts/build.ps1`, `.github/workflows/ci.yml`

Documentation:

- `docs/progress/STAGE_04.md`
- `docs/HANDOFF.md`
- `README.md`, `CHANGELOG.md`

## Commands and results

### Full local gate

```text
.\scripts\build.ps1 -WithDependencyScan
```

Result: exit 0, `All requested local checks passed.`

- Gradle `BUILD SUCCESSFUL`; Spotless and detekt passed.
- Android debug APK and Windows desktop app image built.
- Presentation and UI JVM tests passed: 65 tests, 0 failures.
- Rust format, Clippy with warnings denied, and workspace tests passed.
- `cargo deny check` passed all four policies.
- `cargo audit` scanned 34 crate dependencies against 1,490 advisories with no
  vulnerabilities.
- Repository hygiene passed.

### Test breakdown

| Suite | Tests | Failures |
|---|---:|---:|
| `NexusAppStateTest` | 10 | 0 |
| `NavigationStateTest` | 11 | 0 |
| `ContentStateTest` | 4 | 0 |
| `DemoCatalogTest` | 11 | 0 |
| `IdentityDraftTest` | 7 | 0 |
| `StartupPresenterTest` | 1 | 0 |
| `NexusAppControllerTest` | 9 | 0 |
| `StateLabelsTest` | 5 | 0 |
| `ThemeContrastTest` | 7 | 0 |

Presentation subtotal: 44. Shared UI subtotal: 21.

### Artifacts

- Android debug APK:
  `apps/androidApp/build/outputs/apk/debug/androidApp-debug.apk`,
  30,661,291 bytes.
- Windows app image:
  `apps/desktopApp/build/compose/binaries/main/app/NEXUS`, 130,621,045 bytes
  across 200 files.
- Windows launcher: `.../app/NEXUS/NEXUS.exe`, 540,672 bytes.

### Runtime smoke test

The Windows app image launched and rendered the welcome screen in dark theme
with the NEXUS wordmark, preview label, persistent network-inactive banner, demo
badges, and both onboarding actions. It stayed running and closed normally
through the window close request.

No Android emulator or physical device is installed, so the APK was not launched
at runtime.

### Validators

```text
python scripts/check_repo_hygiene.py
python scripts/check_commit_message.py "feat(ui): build design system and screen prototypes"
```

Both pass.

### Clean checkout

After committing Stage 04, run:

```powershell
.\scripts\verify-clean-checkout.ps1
```

The result is recorded in the follow-up evidence commit. The follow-up changes
only documentation, so it does not alter build inputs.

## Errors and failed attempts

1. The Android shell could not resolve `isSystemInDarkTheme` because
   `apps/androidApp` did not declare Compose foundation directly. Added
   `implementation(libs.compose.foundation)`.
2. `StateLabelsTest` failed because offline and degraded shared the same warning
   icon. Offline now uses `NexusIcons.Error`; all network states have distinct
   icons and labels.
3. ktlint rejected `@Composable` function names with uppercase first letters and
   one long demo-fixture line. The `.editorconfig` now ignores function naming
   for `@Composable`, and network snapshots use shared bootstrap-path builders.
4. detekt initially reported Compose length and parameter counts, magic numbers
   in palette/icon/brand art, a misplaced test package, and two early-return
   functions. The detekt policy now excludes Compose functions from complexity
   and naming rules, palette/icon/brand files carry a narrow file-level
   `MagicNumber` suppression, the test package was corrected, and the
   early-return functions have narrow `ReturnCount` suppressions.
5. `RoundedCornerShape(50)` tripped the magic-number rule in chip/status
   components. Both now use `CircleShape`.
6. Desktop window dimensions and the `Dimension(720, 560)` minimum tripped the
   magic-number rule. They are now named constants.
7. The global preview bar and network selector are prototype controls, not
   product settings. They remain visibly labelled as simulated to avoid implying
   that networking works.

## Decisions and unresolved questions

- Stage 04 was explicitly approved by the project owner.
- `shared/ui` owns Compose screens, components, theme, and accessibility;
  `shared/presentation` owns navigation, state, validation, fixtures, and tests.
- Theme persistence uses a shared `ThemeStore` boundary implemented with
  Android `SharedPreferences` and desktop `java.util.prefs.Preferences`.
- Icons are custom Compose vectors in `NexusIcons`; no icon dependency is added.
- Strings are centralized in `NexusStrings` and `EnglishNexusStrings` for a
  future resource-backed provider. Locale-aware formatting remains later-stage
  work.
- The responsive breakpoint is 840 dp: side rail at or above, bottom bar below.
- Navigation state is owned by `NexusAppController` so Android can restore it and
  install `BackHandler`; desktop uses `remember`.
- Persistence tokens are versioned; invalid tokens fall back to a safe
  first-launch state.
- Compose declarative functions are exempt from detekt complexity and naming
  rules, and palette/icon/brand art files use a narrow magic-number suppression.
- No real identity, publication, download, hosting, or network action is
  implemented. Publish and later-stage actions remain disabled or simulated.
- Desktop reduced motion is a no-op in Stage 04; Android reads the system
  animator scale.
- The Windows app image is not an installer. Inno Setup packaging is Stage 19.
- Remote GitHub Actions execution and device-level TalkBack/Narrator/200%
  scaling validation remain environment limitations.

## Exact next action

1. Review the Stage 04 evidence in `docs/progress/STAGE_04.md`.
2. Give explicit approval to start Stage 05.
3. Stage 05 defines the SQLite schema, migrations, local file store, and
   persistence contracts. Do not begin it before that approval.
