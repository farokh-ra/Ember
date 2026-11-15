package io.nexus.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf

/** Semantic status colors for the current theme. */
public val LocalNexusSemanticColors: androidx.compose.runtime.ProvidableCompositionLocal<NexusSemanticColors> =
    staticCompositionLocalOf { NexusLightSemanticColors }

/** True when the platform asks the app to reduce non-essential motion. */
public val LocalReducedMotion: androidx.compose.runtime.ProvidableCompositionLocal<Boolean> =
    staticCompositionLocalOf { false }

/** Convenience accessor for theme-aware semantic colors. */
public object NexusTheme {
    public val semanticColors: NexusSemanticColors
        @Composable get() = LocalNexusSemanticColors.current

    public val reducedMotion: Boolean
        @Composable get() = LocalReducedMotion.current
}

/**
 * Applies the Ember palette, typography, shape scale, and accessibility motion
 * preference. The chosen light/dark mode is supplied by the caller.
 */
@Composable
public fun NexusTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    reducedMotion: Boolean = rememberReducedMotion(),
    content: @Composable () -> Unit,
) {
    val colorScheme = if (darkTheme) NexusDarkColors else NexusLightColors
    val semanticColors = if (darkTheme) NexusDarkSemanticColors else NexusLightSemanticColors

    CompositionLocalProvider(
        LocalNexusSemanticColors provides semanticColors,
        LocalReducedMotion provides reducedMotion,
    ) {
        MaterialTheme(
            colorScheme = colorScheme,
            typography = NexusTypography,
            shapes = NexusShapes,
            content = content,
        )
    }
}

/** Platform hook for the system reduce-motion preference. */
@Composable
public expect fun rememberReducedMotion(): Boolean
