package io.nexus.ui.theme

import io.nexus.presentation.ThemeMode
import java.util.prefs.Preferences

/**
 * Windows appearance persistence using the JDK preference store.
 *
 * Storage failures are swallowed; the UI keeps the in-memory choice for the
 * session and falls back to the system theme on the next launch.
 */
public class PreferencesThemeStore(
    private val preferences: Preferences = Preferences.userRoot().node("io/nexus/ui"),
) : ThemeStore {
    override fun load(): ThemeMode =
        ThemeMode.fromStorage(
            runCatching { preferences.get(KEY_THEME, null) }.getOrNull(),
        )

    override fun save(mode: ThemeMode) {
        runCatching {
            preferences.put(KEY_THEME, mode.name)
            preferences.flush()
        }
    }

    private companion object {
        const val KEY_THEME: String = "theme_mode"
    }
}
