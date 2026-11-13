package io.nexus.ui.theme

import io.nexus.presentation.ThemeMode

/**
 * Narrow persistence boundary supplied by the platform shells.
 *
 * Android uses SharedPreferences and Windows uses java.util.prefs. The shared
 * UI never reads platform storage directly.
 */
public interface ThemeStore {
    /** Loads the saved preference. Implementations must never throw. */
    public fun load(): ThemeMode

    /** Saves the preference. Implementations must never throw. */
    public fun save(mode: ThemeMode)
}

/** In-memory store used by previews and tests. */
public class InMemoryThemeStore(
    initial: ThemeMode = ThemeMode.SYSTEM,
) : ThemeStore {
    private var mode: ThemeMode = initial

    override fun load(): ThemeMode = mode

    override fun save(mode: ThemeMode) {
        this.mode = mode
    }
}
