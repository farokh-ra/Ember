# ADR-0001 — Kotlin Multiplatform and Compose Multiplatform for shared UI

- **Status:** Accepted
- **Date:** 2026-11-12
- **Deciders:** Ember engineering
- **Supersedes:** none
- **Related:** ADR-0002, ADR-0009, [Architecture](../ARCHITECTURE.md)

## Context

Ember must ship first-class Android and Windows applications with the same product journeys, not a web wrapper and not two unrelated codebases. The roadmap recommends Kotlin + Compose Multiplatform for shared screens, state, workflows, and domain logic, while keeping native platform integration.

The UI must support phone and desktop layouts, dark/light themes, keyboard and screen-reader access, and offline/local-first behaviour. Platform-specific pieces include file pickers, notifications, foreground execution, storage access, window management, and installer integration.

## Decision

Use **Kotlin Multiplatform (KMP)** with **Compose Multiplatform**:

- `shared/commonMain` owns shared Compose screens, navigation, presentation state, view models, domain-facing models, and the Kotlin wrapper around the Rust core.
- `apps/androidApp` is a native Android shell: Activity/Application lifecycle, permissions, notifications, foreground service, Storage Access Framework, and Android accessibility integration.
- `apps/desktopApp` is a native Windows shell: Compose Desktop entry point, window management, file dialogs, notifications, and installer integration.
- Platform-specific behaviour is exposed through narrow `expect`/`actual` interfaces. UI code depends on interfaces, not platform APIs.
- The shared UI never talks to the network, SQLite, or native libraries directly. It calls the Kotlin `NexusCore` API from ADR-0003.
- No web view, embedded browser, or reduced web build is the product UI.

## Consequences

**Positive**

- One implementation of screens, navigation, validation, and presentation logic.
- Android and Windows stay aligned on product behaviour and accessibility semantics.
- Native platform integration remains available where Compose is not the right tool.
- Shared Kotlin tests can cover most presentation logic without a device.

**Negative / trade-offs**

- Compose Multiplatform desktop packaging and accessibility need dedicated testing.
- Platform differences still exist; `expect`/`actual` surfaces must stay small.
- Desktop Compose performance must be measured separately from Android.
- A shared UI increases the cost of platform-specific visual polish; this is accepted for v1.

**Constraints**

- Screens must remain responsive at 360 dp phone width and 1280×720 desktop.
- All strings are externalised; no platform-specific hardcoded copy.
- All network/file/database work is off the UI thread and behind the core API.
- A platform may add native presentation for a platform-only integration, but a core journey may not be omitted.

## Alternatives considered

1. **Separate native Android (Jetpack Compose) and Windows (WinUI/WPF) UIs.** Best platform polish, but duplicates every screen and state machine and risks product drift. Rejected for v1.
2. **Flutter or React Native.** Cross-platform UI, but introduces a third runtime and weakens the Kotlin/Rust boundary and native Windows integration. Rejected.
3. **Web UI wrapped in a desktop/mobile shell.** Violates the first-class-platform rule and weakens offline, accessibility, and lifecycle control. Rejected.
4. **Compose Multiplatform UI with all logic in Kotlin.** Rejected because networking, identity, storage, and transfer belong to the Rust core (ADR-0002).

## Verification

- Stage 03 builds empty Android and Windows shells that launch and share a trivial Compose screen.
- Stage 04 proves navigation, responsive layouts, themes, accessibility, and mock-data states on both platforms.
- Stage 19 runs platform parity, performance, accessibility, and packaging tests.
- A failure of Compose on either platform triggers a documented fallback review: keep the shared domain/API and use a platform-native UI shell; it does not permit dropping a platform.
