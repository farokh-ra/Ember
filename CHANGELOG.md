# Changelog

All notable changes to NEXUS are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Repository foundation for Stage 03, including Git hygiene files, editor
  configuration, commit template, and contribution templates.
- Gradle Kotlin Multiplatform and Compose Multiplatform build with shared
  modules for model, core API, core client, presentation, and UI.
- Empty Android application shell with a debug APK build.
- Empty Windows desktop application shell with a Compose desktop application
  image build.
- Cargo workspace with twelve architecture crates and a tested native ABI
  version export.
- Redacted structured logging helpers in Kotlin and Rust.
- Spotless and ktlint formatting, detekt static analysis, Rustfmt, and Clippy.
- Shared JVM tests for redaction, the fake core contract, the core client
  boundary, and startup presentation state.
- CI workflow for repository hygiene, Conventional Commit titles, Rust checks,
  Android builds and tests, a native Windows MSVC Rust build and test job, and
  Windows desktop packaging with artifact upload.
- Dependency review workflow, Dependabot configuration, and release-note
  categories.
- Portable non-admin Windows setup, local build, and clean-checkout verification
  scripts.
- Repository hygiene and commit-message validators.
- Developer setup, development workflow, logging policy, contributing guide, and
  Stage 03 evidence record.
- NEXUS design system with light/dark color schemes, semantic status colors,
  typography, spacing, shape, size, and a custom Compose icon set.
- Reusable component library for navigation chrome, cards, status labels,
  buttons, selections, dialogs, state panels, avatars, brand art, and prototype
  controls.
- Responsive navigation shell with a side rail at 840 dp and above and a compact
  top bar plus bottom navigation below it.
- Eleven clickable screen prototypes: welcome, identity setup, Explore, Search,
  profile, file details, Publish, Downloads, hosted files, Settings, and network
  status.
- Immutable application state with per-section navigation stacks, demo
  onboarding, versioned persistence tokens, and a shared controller.
- Persisted dark, light, and system theme selection on Android and Windows.
- Loading, empty, offline, error, and retry presentations for every data-driven
  screen, plus a visibly simulated preview-state bar and network selector.
- Accessibility support: WCAG AA contrast tests, 48 dp targets, visible focus
  rings, roles and selection semantics, live regions, text-plus-icon status,
  Android back, Escape back, and reduced-motion handling on Android.
- 21 shared UI JVM tests and 44 presentation JVM tests, including palette
  contrast, navigation, state restoration, identity validation, demo search, and
  status-label mapping.
- Stage 04 evidence record.

### Changed

- The Android and Windows shells now host the shared clickable UI prototype
  instead of empty placeholder content.
- The local gate and CI now run `:shared:ui:jvmTest`.
- Compose functions are exempt from detekt complexity and naming rules, and
  palette, icon, and brand-art files use a narrow magic-number suppression.
- ktlint ignores function naming for `@Composable` functions.
- Toolchain pins are recorded for JDK 21.0.13+9, Gradle 9.9.1, Kotlin 2.4.21,
  Compose Multiplatform 1.12.2, Material3 1.9.0, AGP 9.5.1, Rust 1.99.1, and
  Android NDK 29.0.14206865.

### Security

- No API keys, private keys, keystores, or signing material are committed.
- Debug logging is redacted by policy and test coverage.
- CI scans for forbidden files and common credential patterns.

### Known limitations

- Identity, publication, download, hosting, search, and network data in the UI
  are visibly labelled mock content. Real features begin in Stage 05 and later.
- Only the theme choice persists. No real identity or product data is stored.
- Shared UI copy is English-only; locale-aware formatting and translated
  resources are later-stage work.
- 200% text scaling, TalkBack, and Narrator journeys have structural support but
  were not exercised on a device or emulator in this environment.
- Desktop reduced motion is a no-op in Stage 04; Android reads the system
  animator scale.
- The Windows application image is not yet an installer; Inno Setup packaging is
  Stage 19.
- The local non-admin Windows toolchain uses a MinGW GNU Rust host when MSVC C++
  tools are unavailable. CI and release builds use MSVC.
- Android Rust cross-compilation and native library packaging are wired in a
  later stage.
- Gradle configuration cache is disabled because Spotless 8.11.0 cannot
  serialize its line-ending policy under Gradle 9.9.1.
