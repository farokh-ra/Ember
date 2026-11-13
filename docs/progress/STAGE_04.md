# Stage 04 - UI Foundation and Design System

**Status:** Complete
**Completed:** 2026-11-13
**Progress guide:** 13-18%
**Next stage:** 05 - Local database and file-store foundation (requires user permission)

## 1. Scope

Stage 04 establishes the shared visual language and every clickable prototype
screen. It does not connect real storage, identity, publishing, downloads,
hosting, or networking. The stage covers:

- NEXUS branding, color, typography, spacing, shape, and icon rules.
- A reusable Compose component library for Android and Windows.
- Navigation and immutable application-state patterns.
- Responsive phone and desktop layouts.
- Dark, light, and system themes with persisted selection.
- Loading, empty, offline, error, and retry presentations.
- Accessibility, focus, keyboard, and Android back behavior.
- Eleven clickable screens driven by visibly labelled demo data.

## 2. Deliverables

| Deliverable | Path | Status |
|---|---|---|
| Design system | `shared/ui/src/commonMain/kotlin/io/nexus/ui/theme` | Complete. |
| Icon set | `shared/ui/.../ui/icons/NexusIcons.kt` | Complete; custom `ImageVector`s, no new dependency. |
| Component library | `shared/ui/src/commonMain/kotlin/io/nexus/ui/components` | Complete. |
| Externalized copy | `shared/ui/.../ui/resources/NexusStrings.kt` | Complete; English provider ready for localization. |
| Navigation shell | `shared/ui/.../ui/NexusApp.kt` | Complete; responsive side rail and bottom bar. |
| State and controller | `shared/presentation/...` and `shared/ui/.../NexusAppController.kt` | Complete. |
| Screen prototypes | `shared/ui/src/commonMain/kotlin/io/nexus/ui/screens` | Complete; eleven screens. |
| Android integration | `apps/androidApp/src/main/kotlin/io/nexus/android/MainActivity.kt` | Complete; saved state and back handling. |
| Desktop integration | `apps/desktopApp/src/main/kotlin/io/nexus/desktop/Main.kt` | Complete; persisted theme and window bounds. |
| UI tests | `shared/ui/src/jvmTest` | Complete; 21 tests. |
| Presentation tests | `shared/presentation/src/jvmTest` | Complete; 44 tests. |
| Evidence record | This document | Complete. |

## 3. Architecture and boundaries

Stage 04 keeps platform code out of presentation and screen logic:

- `shared/presentation` owns immutable state, navigation stacks, input
  validation, preview states, content states, demo fixtures, and persistence
  token formatting. It has no Compose or platform dependency and is tested in
  `src/jvmTest`.
- `shared/ui` owns the theme, strings, icons, components, screens, and the
  responsive frame. It depends on `shared/presentation` but not on either shell.
- `apps/androidApp` supplies `SharedPreferences`, activity back dispatch, edge
  to edge insets, and status-bar icon appearance.
- `apps/desktopApp` supplies `java.util.prefs.Preferences`, the window, and the
  minimum window size.
- `NexusAppController` owns the single mutable state holder used by both shells,
  so Android can install `BackHandler` and restore tokens with
  `rememberSaveable`. Desktop uses `remember`.

No new third-party UI, icon, or storage dependency was added.

## 4. Design system

### Branding and color

- The brand mark is a custom Compose canvas node graphic. The adjacent NEXUS
  wordmark carries the accessible name; the mark is decorative.
- `Color.kt` defines light and dark Material 3 schemes plus semantic controlled,
  caution, offline/error, informational, and neutral status colors.
- Primary, surface, container, outline, and semantic pairs are checked by
  `ThemeContrastTest`. Text pairs require at least 4.5:1, non-text accents and
  outlines at least 3:1.
- Status meaning is always paired with a text label and a distinct icon, never
  color alone. Offline, connecting, online, and degraded network states each
  have distinct labels and icons.

### Typography and dimensions

- `Typography.kt` defines one NEXUS type scale used by both platforms.
- `Dimens.kt` defines the shared spacing scale, shape tokens, minimum 48 dp touch
  target, content widths, side-rail width, and state-panel minimum height.
- Screens use the shared spacing and size tokens instead of ad hoc dimensions.
- Content is constrained by `maxContentWidth` and `maxReadingWidth` so desktop
  lines do not become unreadably long.

### Strings and localization readiness

- All user-facing Stage 04 copy lives in `NexusStrings` and is supplied through
  `LocalNexusStrings`.
- Demo filenames, peer labels, and sample values live in the presentation demo
  catalogue and always appear with a demo badge or banner.
- Locale-aware number, date, and file-size formatting is intentionally deferred
  to the localization stage. Stage 04 fixtures use preformatted labels.

### Components

The component package includes:

- focus ring and accessible interactive surfaces;
- buttons, action rows, choice chips, and selections;
- cards for publications, downloads, hosted files, profile summaries, and
  network paths;
- status pills, badges, info banners, and demo labels;
- state panels and the five-state host;
- screen scaffold, section headers, key/value rows, and responsive action
  clusters;
- top bar, side rail, bottom bar, snackbar host, and preview controls;
- dialogs for destructive-action confirmation;
- avatar, brand mark, and the custom icon set.

## 5. Navigation and application state

- `AppDestination` is a sealed type with the eleven screens. `FileDetails`
  carries a demo record identifier.
- `NavigationState` keeps a back stack per section. Selecting a primary section
  resets that section's stack; back pops one screen.
- `NexusAppState` holds navigation, theme mode, preview scenario, preview
  network state, demo mode, and the demo display name. It is immutable and all
  transitions return a new state.
- Persistence tokens are versioned. Malformed or unknown tokens return `null`
  and fall back to a safe first-launch state.
- `NexusAppController` exposes navigation, back, demo onboarding, theme
  selection with persistence, and preview controls.

### Responsive behavior

- The breakpoint is 840 dp.
- At or above 840 dp, the app shows a side rail with primary navigation, profile,
  and network status.
- Below 840 dp, it shows a compact top bar and a bottom navigation bar.
- Welcome and identity setup hide navigation chrome so onboarding stays
  focused.
- Screens use scrollable content, wrapping text, and constrained widths. The
  desktop window opens at 1280x720 dp with a 720x560 minimum.
- Presentation order remains logical in both layouts: banner, navigation,
  screen content, and snackbar.

### Keyboard and back behavior

- Interactive components receive a visible 2 dp focus ring.
- Controls use Material roles and semantics so assistive technology can report
  button, radio, tab, and selection state.
- Escape performs back navigation on desktop when a back action is available.
- Android installs `BackHandler`; back is ignored when the current screen is the
  root of its journey.
- The top bar exposes a labelled back button for discoverability.

## 6. Screen prototypes

| Screen | Prototype behavior | Content states |
|---|---|---|
| Welcome | Explains the product and labels the build as a Stage 04 preview. Opens the demo or identity setup. | Static onboarding. |
| Identity setup | Local-only display name entry with validation and a demo identity action. No key is created. | Form validation. |
| Explore | Demo publication feed, category filters, availability labels, network summary, and Publish entry. | All five. |
| Search | Local demo search over titles, tags, descriptions, and uploaders; filter chips and result counts. | All five. |
| Profile | Demo profile, publication list, key fingerprint, and network summary. | All five. |
| File details | Record metadata, source list, verification labels, replica explanation, and disabled actions. | All five. |
| Publish | Fully visible but disabled form with an explicit "not active in Stage 04" banner and planned steps. | Static disabled prototype. |
| Downloads | Demo download queue with progress, verification, pause/resume, and delete confirmation. | All five. |
| Hosted files | Explicit replicas with verification, peer-served counts, remove confirmation, and uploader attribution. | All five. |
| Settings | Theme selection persists; identity, storage, bandwidth, bootstrap, language, diagnostics, privacy, and about sections. | Theme and static placeholders. |
| Network status | Node state, peer count, bootstrap paths, recent errors, and diagnostics export placeholder. | All five. |

Every data-driven screen shows the preview-state bar. Settings and Publish are
not data-driven screens, but they visibly state which controls are placeholders
or inactive.

## 7. Content states, honest status, and demo labelling

### Five required states

`ContentState` has exactly five cases: `Loading`, `Empty`, `Offline`, `Error`,
and `Ready`. `previewContentState()` maps the global preview control to a state,
and `ContentStateHost` renders each case with:

- a title, explanation, icon, and semantic live region;
- an action out of the state (`Show demo data`, `View cached demo data`, or
  `Retry`);
- a visible note that the state is simulated;
- no raw exception, stack trace, secret, or local path.

Loading never leaves the user without an action. Error copy uses a safe
`UiErrorCode`, not an exception message.

### Preview controls

- The preview bar exposes `Ready`, `Loading`, `Empty`, `Offline`, and `Error`.
- The network selector exposes `Offline`, `Connecting`, `Online`, and
  `Degraded` with distinct labels and icons.
- Both controls explicitly say they change prototype state only and call no
  service.
- The selected state is held in the shared app state, so changing screens keeps
  the demonstration consistent.

### Honest network labeling

- A persistent global banner says the build is a preview and network features
  are inactive. It cannot be dismissed.
- Demo data carries `DEMO DATA` badges and sample identifiers.
- Every network panel shows simulated values and never presents them as live
  peer data.
- Actions that need later stages are disabled or show a labelled demo
  snackbar. Publish is wholly disabled.
- Unpublishing is explained as removing the signed record, not deleting remote
  copies. Hosted replicas identify the original uploader; the replica is never
  described as the uploader.
- Availability uses the canonical vocabulary: `Available now`,
  `Seen recently`, `No known source`, `Record only`, and
  `Unpublished by uploader`, each with a definition.

## 8. Accessibility

Implemented and covered where automation is possible:

- **Contrast:** light and dark text, semantic status, and outline pairs are
  asserted in `ThemeContrastTest` against WCAG 2.2 AA thresholds.
- **Touch targets:** interactive controls use `NexusSizes.minimumTouchTarget`
  (48 dp) through `heightIn` or equivalent.
- **Visible focus:** `nexusFocusRing` draws a 2 dp primary-color ring and is
  applied before the focus target.
- **Semantics:** controls use Material roles, selection state, labels, and
  content descriptions where the visual is not self-describing. State panels
  use a polite live region.
- **Non-color status:** every status includes a text label and a distinct icon.
- **Keyboard and back:** logical composition order, Escape back on desktop, and
  Android `BackHandler`.
- **Reduced motion:** Android reads the system animator duration scale and
  suppresses the loading spinner animation when motion is reduced. Desktop uses
  no non-essential animation in Stage 04.
- **Text scaling:** layouts use wrapping text, scrolling, and width constraints
  rather than fixed text heights. Full 200% scaling, TalkBack, and Narrator
  journeys remain device-level checks because no Android emulator or assistive
  technology runner is available in this environment.

## 9. Theme persistence

- Android `AndroidThemeStore` uses `SharedPreferences`.
- Desktop `PreferencesThemeStore` uses `java.util.prefs.Preferences`.
- Both implementations swallow storage failures and return the system theme
  instead of crashing.
- `NexusAppController.setThemeMode` updates state and saves the preference.
- `ThemeMode.SYSTEM` follows the platform dark setting; `LIGHT` and `DARK`
  override it until changed.
- The in-memory store covers controller tests and previews.

## 10. Commands and results

### Full local gate

```text
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
.\scripts\build.ps1 -WithDependencyScan
```

Result: exit 0, `All requested local checks passed.`

- Gradle `BUILD SUCCESSFUL`; Spotless and detekt passed.
- `:apps:androidApp:assembleDebug` passed.
- `:apps:desktopApp:createDistributable` passed.
- `:shared:presentation:jvmTest` and `:shared:ui:jvmTest` passed.
- `cargo fmt --all -- --check`, Clippy with `-D warnings`, and
  `cargo test --workspace` passed.
- `cargo deny check` passed: `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo audit` scanned 34 crate dependencies against 1,490 advisories with no
  vulnerability reported.
- Repository hygiene passed: 171 files scanned, 7 forbidden names, 10 forbidden
  suffixes, and 7 secret patterns.

### Kotlin tests

Total: 65 tests, 0 failures, 0 skipped.

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

| Artifact | Path | Size |
|---|---|---|
| Android debug APK | `apps/androidApp/build/outputs/apk/debug/androidApp-debug.apk` | 30,661,291 bytes |
| Windows app image | `apps/desktopApp/build/compose/binaries/main/app/NEXUS` | 130,621,045 bytes across 200 files |
| Windows launcher | `.../app/NEXUS/NEXUS.exe` | 540,672 bytes |

### Runtime smoke test

The Windows app image was launched from a clean start, left running for eight
seconds, and captured. It rendered the welcome screen in dark theme with the
NEXUS wordmark, the stage preview label, the persistent preview/network banner,
demo data badges, and both onboarding actions. The process stayed alive and
closed normally through the window close request. A screenshot was captured for
local review but is not committed because it contains the host desktop.

No Android emulator or physical device is installed in this environment, so the
APK was built and tested but not launched at runtime.

### Clean checkout

The Stage 04 commit `2dc053e405b7bb2f8860cec4b2b0dc396da968d3` was cloned into
`C:\Users\Elsa\AppData\Local\Temp\nexus-clean-cb0c27ef`, and the full gate ran
there:

```text
.\scripts\verify-clean-checkout.ps1
```

Result: PASS, exit 0.

- Gradle reported `BUILD SUCCESSFUL in 42s`.
- Android `assembleDebug` and desktop `createDistributable` executed.
- All 65 JVM tests passed. Some Kotlin compile and test tasks were served from
  the local Gradle build cache with matching source inputs; Android packaging,
  desktop runtime-image creation, and the Rust workspace compiled freshly.
- Rust format, Clippy, workspace tests, and repository hygiene passed.
- The temporary clone was removed after the run.

This proves that the committed source builds through the documented setup
without untracked local files. The follow-up evidence commit changes only
documentation and does not alter build inputs.

### Failure evidence

- `StateLabelsTest` first failed because offline and degraded shared the same
  warning icon. Offline now uses a distinct error icon; all 21 UI tests pass.
- ktlint initially rejected composable function names and a long demo-fixture
  line. The repository policy now ignores function naming for `@Composable` and
  the fixture was split into reusable bootstrap-path builders.
- detekt initially reported Compose declarative complexity, magic numbers in
  palette/icon/brand art, one misplaced test package, and two early-return
  functions. Compose functions are excluded from complexity and naming rules,
  palette/icon/brand files use a narrow file-level magic-number suppression,
  the test package was corrected, and the early-return functions carry narrow
  `ReturnCount` suppressions.

## 11. Known limitations

- All identity, publication, download, hosting, search, and network data is
  mock content. No real service is called.
- No real identity is created or persisted. Only the theme preference persists.
- Publish, backup/restore, diagnostics export, and other later-stage actions are
  disabled or simulated.
- The shared UI copy is English-only. Locale-aware formatting and translated
  resources are later-stage work.
- Desktop reduced-motion integration is a no-op; Android reads the animator
  scale. Windows accessibility settings integration is later-stage work.
- 200% text scaling, TalkBack, and Narrator journeys have structural support but
  were not exercised on a device or emulator in this environment.
- The Windows app image is not an installer. Inno Setup packaging is Stage 19.
- GitHub Actions has not run remotely; the workflow runs the same commands that
  pass locally.
- Local Rust uses the GNU/MinGW host because MSVC C++ tools are unavailable.
  CI and release remain MSVC.
- Real screens will be revisited as features land; Stage 04 prototypes are not
  final product behavior.

## 12. Reproducing this stage

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
.\scripts\setup-windows.ps1
. .\scripts\dev-env.ps1
.\scripts\build.ps1 -WithDependencyScan
```

Focused checks:

```powershell
.\gradlew.bat --console=plain spotlessCheck detekt
.\gradlew.bat --console=plain :shared:presentation:jvmTest :shared:ui:jvmTest
.\gradlew.bat --console=plain :apps:androidApp:assembleDebug
.\gradlew.bat --console=plain :apps:desktopApp:createDistributable
```

Launch the desktop image:

```powershell
.\apps\desktopApp\build\compose\binaries\main\app\NEXUS\NEXUS.exe
```

After committing:

```powershell
.\scripts\verify-clean-checkout.ps1
```

The clean-checkout result is recorded in section 10 and in
`docs/HANDOFF.md`.

## 13. Next stage

Stage 05 defines the SQLite schema, migration strategy, local file store, and
persistence contracts. It must not begin until Stage 04 is reviewed and the
project owner explicitly approves it.
