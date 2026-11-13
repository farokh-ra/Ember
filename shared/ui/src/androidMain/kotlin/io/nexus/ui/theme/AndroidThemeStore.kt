package io.nexus.ui.theme

import android.content.Context
import io.nexus.presentation.ThemeMode

/**
 * Android appearance persistence.
 *
 * Storage failures are swallowed so an unreadable preference can never block
 * startup. The UI falls back to the system theme.
 */
public class AndroidThemeStore(
    context: Context,
    preferencesName: String = "nexus_ui",
) : ThemeStore {
    private val preferences =
        context.applicationContext.getSharedPreferences(preferencesName, Context.MODE_PRIVATE)

    override fun load(): ThemeMode =
        ThemeMode.fromStorage(
            runCatching { preferences.getString(KEY_THEME, null) }.getOrNull(),
        )

    override fun save(mode: ThemeMode) {
        runCatching {
            preferences.edit().putString(KEY_THEME, mode.name).apply()
        }
    }

    private companion object {
        const val KEY_THEME: String = "theme_mode"
    }
}
