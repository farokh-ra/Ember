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

### Changed

- Toolchain pins are recorded for JDK 21.0.13+9, Gradle 9.9.1, Kotlin 2.4.21,
  Compose Multiplatform 1.12.2, Material3 1.9.0, AGP 9.5.1, Rust 1.99.1, and
  Android NDK 29.0.14206865.

### Security

- No API keys, private keys, keystores, or signing material are committed.
- Debug logging is redacted by policy and test coverage.
- CI scans for forbidden files and common credential patterns.

### Known limitations

- Product features begin in Stage 04; the current screens are empty shells.
- The Windows application image is not yet an installer; Inno Setup packaging is
  Stage 19.
- The local non-admin Windows toolchain uses a MinGW GNU Rust host when MSVC C++
  tools are unavailable. CI and release builds use MSVC.
- Android Rust cross-compilation and native library packaging are wired in a
  later stage, not Stage 03.
- Gradle configuration cache is disabled because Spotless 8.11.0 cannot
  serialize its line-ending policy under Gradle 9.9.1.
