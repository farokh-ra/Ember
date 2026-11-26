# Accessibility and localization review - Ember 0.1.0

**Reviewed:** 2026-11-26
**Scope:** Shared Compose Multiplatform UI and the Windows/Android application
shells
**State:** Baseline accessibility review complete; localization is not
implemented; no assisted-technology testing was possible.

## 1. What already exists

The Stage 04 design system and shell provide:

- WCAG 2.2 AA contrast assertions for text, semantic status, and outline pairs in
  `ThemeContrastTest` (7 tests).
- A 48 dp minimum interactive target through the shared dimensions.
- A visible 2 dp focus ring applied before the focus target.
- Logical composition order, keyboard activation of navigation and action rows,
  Escape back on desktop, and Android back handling.
- Roles and selection semantics on navigation, buttons, and selectable rows.
- Text plus icon for status, not colour alone.
- Live-region semantics for changing status text.
- Reduced-motion handling on Android through the system animator duration scale.

These are verified by 21 shared UI tests and 44 presentation tests from Stage 04.
They are component-level and shell-level checks, not a full assistive-technology
audit.

## 2. Findings

| Area | Finding | Severity |
|---|---|---|
| Screen readers | No NVDA, Narrator, TalkBack, or VoiceOver session was run. Semantics are asserted only through Compose tests. | High for release evidence |
| Localization | All copy is in one English `NexusStrings` provider. No translated resources, locale selection, or fallback test exist. | Medium |
| String formatting | Some labels are preformatted English (counts, dates, relative times). These do not follow locale plural or date rules. | Medium |
| Text expansion | No pseudo-localization or long-string layout test exists. German and Finnish expansions may truncate compact labels. | Medium |
| Right-to-left | No RTL layout test. Compose mirrors many layouts automatically, but custom drawing and icon direction are unverified. | Medium |
| Desktop accessibility | Windows high-contrast mode, Narrator, and system text scaling are not integrated or tested. Reduced motion is a no-op on desktop. | High for Windows release |
| Dynamic text size | Compose uses sp for text; the 48 dp targets are fixed. Large system font settings were not exercised. | Medium |
| Keyboard | Tab order and Escape are tested. Full keyboard traversal of dialogs, menus, and scroll areas was not. | Low-medium |
| Focus after navigation | Focus restoration after screen changes is not asserted. | Low-medium |
| Error announcements | Errors are rendered as status text; live-region behaviour under rapid updates was not tested with a screen reader. | Medium |
| Media and charts | No charts or video are shipped in 0.1.0. File type is conveyed by text and icon. | Not applicable |
| Android permissions | Network permissions are declared. No accessibility-specific permissions are used. | Not applicable |

## 3. Localization plan

The string provider is a good first step but is not a localization system. A
future localization stage should:

1. Move copy to Compose Multiplatform resources or a keyed catalogue with
   `en` as the source locale.
2. Use ICU-style plurals, number, date, and relative-time formatting instead of
   preformatted literals.
3. Add a pseudo-localization build that expands strings by 30-40% and inserts
   accented characters, then run layout screenshots.
4. Add RTL layout tests with Arabic or Hebrew copy, including icon mirroring.
5. Have a native speaker review each locale; machine translation alone is not
   release evidence.
6. Keep protocol names, peer IDs, and file extensions untranslated.

Shipping untranslated English to all locales is acceptable only if the store
listing says so. Silently presenting English under a localized UI is not.

## 4. Accessibility plan

Before a public release:

1. Run Narrator on Windows 10 and 11 and TalkBack on Android 9 and a current
   Android release.
2. Check high-contrast and dark modes on Windows.
3. Test at 200% system text scaling and at 320 dp width.
4. Verify focus order and focus restoration for every screen and dialog.
5. Verify live-region announcements for download, publish, and error state
   changes.
6. Verify that every icon-only control has an accessible name.
7. Add an automated semantics test for each new screen.

## 5. What is not claimed

- Ember 0.1.0 is not claimed to be screen-reader complete.
- Ember 0.1.0 is not localized.
- Ember 0.1.0 is not claimed to meet platform accessibility certification.
- The demo shell's accessibility does not prove the future core-driven screens.

The baseline component work is real and tested. The missing evidence is
assistive-technology testing, localization, and platform integration, not the
absence of any accessibility work.
