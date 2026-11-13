package io.nexus.presentation

/** User-selectable appearance preference. */
public enum class ThemeMode {
    /** Follow the operating-system appearance. */
    SYSTEM,

    /** Always use the light palette. */
    LIGHT,

    /** Always use the dark palette. */
    DARK,
    ;

    public companion object {
        /** Parses a persisted value, falling back safely to [SYSTEM]. */
        public fun fromStorage(value: String?): ThemeMode = entries.firstOrNull { it.name == value } ?: SYSTEM
    }
}

/** Resolves the effective palette for a system dark-mode signal. */
public fun ThemeMode.isDark(systemDark: Boolean): Boolean =
    when (this) {
        ThemeMode.SYSTEM -> systemDark
        ThemeMode.LIGHT -> false
        ThemeMode.DARK -> true
    }
