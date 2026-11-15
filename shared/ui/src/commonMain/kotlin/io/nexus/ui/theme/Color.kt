@file:Suppress("MagicNumber")

package io.nexus.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

/**
 * Ember light palette.
 *
 * Contrast ratios for the primary text and status pairs are checked by
 * `ThemeContrastTest` in `jvmTest`.
 */
public val NexusLightColors: ColorScheme =
    lightColorScheme(
        primary = Color(0xFF006A72),
        onPrimary = Color(0xFFFFFFFF),
        primaryContainer = Color(0xFFA7EDF2),
        onPrimaryContainer = Color(0xFF002022),
        inversePrimary = Color(0xFF52D6DF),
        secondary = Color(0xFF4E5F7D),
        onSecondary = Color(0xFFFFFFFF),
        secondaryContainer = Color(0xFFD7E3FF),
        onSecondaryContainer = Color(0xFF0A1B33),
        tertiary = Color(0xFF7A5900),
        onTertiary = Color(0xFFFFFFFF),
        tertiaryContainer = Color(0xFFFFDEA6),
        onTertiaryContainer = Color(0xFF271900),
        background = Color(0xFFF7FAFC),
        onBackground = Color(0xFF101418),
        surface = Color(0xFFFFFFFF),
        onSurface = Color(0xFF101418),
        surfaceVariant = Color(0xFFDCE4EA),
        onSurfaceVariant = Color(0xFF3E4A52),
        surfaceTint = Color(0xFF006A72),
        inverseSurface = Color(0xFF2C3135),
        inverseOnSurface = Color(0xFFEEF1F4),
        error = Color(0xFFB3261E),
        onError = Color(0xFFFFFFFF),
        errorContainer = Color(0xFFF9DEDC),
        onErrorContainer = Color(0xFF410E0B),
        outline = Color(0xFF5E6B75),
        outlineVariant = Color(0xFFB9C6D0),
        scrim = Color(0xFF000000),
        surfaceBright = Color(0xFFFFFFFF),
        surfaceDim = Color(0xFFD7DFE5),
        surfaceContainer = Color(0xFFEBF1F5),
        surfaceContainerHigh = Color(0xFFE4EBF0),
        surfaceContainerHighest = Color(0xFFDDE5EB),
        surfaceContainerLow = Color(0xFFF1F5F8),
        surfaceContainerLowest = Color(0xFFFFFFFF),
    )

/** Ember dark palette. */
public val NexusDarkColors: ColorScheme =
    darkColorScheme(
        primary = Color(0xFF52D6DF),
        onPrimary = Color(0xFF00363B),
        primaryContainer = Color(0xFF004F55),
        onPrimaryContainer = Color(0xFF9CF2F8),
        inversePrimary = Color(0xFF006A72),
        secondary = Color(0xFFB9C7E6),
        onSecondary = Color(0xFF233148),
        secondaryContainer = Color(0xFF3A4963),
        onSecondaryContainer = Color(0xFFD7E3FF),
        tertiary = Color(0xFFF1C25C),
        onTertiary = Color(0xFF402D00),
        tertiaryContainer = Color(0xFF5C4300),
        onTertiaryContainer = Color(0xFFFFDEA6),
        background = Color(0xFF0B1116),
        onBackground = Color(0xFFE3E8EC),
        surface = Color(0xFF0F161C),
        onSurface = Color(0xFFE3E8EC),
        surfaceVariant = Color(0xFF3E4A52),
        onSurfaceVariant = Color(0xFFBEC9D1),
        surfaceTint = Color(0xFF52D6DF),
        inverseSurface = Color(0xFFE3E8EC),
        inverseOnSurface = Color(0xFF2C3135),
        error = Color(0xFFFFB4AB),
        onError = Color(0xFF690005),
        errorContainer = Color(0xFF93000A),
        onErrorContainer = Color(0xFFFFDAD6),
        outline = Color(0xFF88949D),
        outlineVariant = Color(0xFF3E4A52),
        scrim = Color(0xFF000000),
        surfaceBright = Color(0xFF354048),
        surfaceDim = Color(0xFF0B1116),
        surfaceContainer = Color(0xFF151F26),
        surfaceContainerHigh = Color(0xFF1F2A32),
        surfaceContainerHighest = Color(0xFF29353D),
        surfaceContainerLow = Color(0xFF111A20),
        surfaceContainerLowest = Color(0xFF070C10),
    )

/** Semantic colors for states that Material3 does not define. */
@Immutable
public data class NexusSemanticColors(
    public val available: Color,
    public val availableContainer: Color,
    public val onAvailableContainer: Color,
    public val caution: Color,
    public val cautionContainer: Color,
    public val onCautionContainer: Color,
    public val offline: Color,
    public val offlineContainer: Color,
    public val onOfflineContainer: Color,
    public val info: Color,
    public val infoContainer: Color,
    public val onInfoContainer: Color,
    public val neutralContainer: Color,
    public val onNeutralContainer: Color,
)

public val NexusLightSemanticColors: NexusSemanticColors =
    NexusSemanticColors(
        available = Color(0xFF0E6B3C),
        availableContainer = Color(0xFFD7F5E3),
        onAvailableContainer = Color(0xFF06381E),
        caution = Color(0xFF7A4B00),
        cautionContainer = Color(0xFFFFE8C2),
        onCautionContainer = Color(0xFF3D2500),
        offline = Color(0xFF9E2A2B),
        offlineContainer = Color(0xFFFFE0DF),
        onOfflineContainer = Color(0xFF410E0B),
        info = Color(0xFF1D4E89),
        infoContainer = Color(0xFFDCE9FF),
        onInfoContainer = Color(0xFF0B2B50),
        neutralContainer = Color(0xFFE7EDF2),
        onNeutralContainer = Color(0xFF2A363E),
    )

public val NexusDarkSemanticColors: NexusSemanticColors =
    NexusSemanticColors(
        available = Color(0xFF6FDC9E),
        availableContainer = Color(0xFF0B3B22),
        onAvailableContainer = Color(0xFFB4F5CD),
        caution = Color(0xFFF5C25B),
        cautionContainer = Color(0xFF4A3300),
        onCautionContainer = Color(0xFFFFE3AC),
        offline = Color(0xFFFFB4AB),
        offlineContainer = Color(0xFF5C1414),
        onOfflineContainer = Color(0xFFFFDAD6),
        info = Color(0xFF9EC5FF),
        infoContainer = Color(0xFF123352),
        onInfoContainer = Color(0xFFD6E6FF),
        neutralContainer = Color(0xFF26323A),
        onNeutralContainer = Color(0xFFD8E2E9),
    )
