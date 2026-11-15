package io.nexus.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import io.nexus.presentation.ThemeMode
import io.nexus.presentation.isDark
import kotlin.math.max
import kotlin.math.min
import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * Guards the Stage 04 accessibility contract.
 *
 * Text pairs must reach WCAG 2.2 AA 4.5:1 and non-text controls 3:1. The test
 * intentionally uses only the palette tokens, so a future palette edit that
 * lowers contrast fails before the change ships.
 */
class ThemeContrastTest {
    @Test
    fun lightThemeTextPairsMeetWcagAa() {
        assertTextPairs(NexusLightColors)
    }

    @Test
    fun darkThemeTextPairsMeetWcagAa() {
        assertTextPairs(NexusDarkColors)
    }

    @Test
    fun lightSemanticStatusPairsMeetWcagAa() {
        assertSemanticTextPairs(NexusLightSemanticColors)
    }

    @Test
    fun darkSemanticStatusPairsMeetWcagAa() {
        assertSemanticTextPairs(NexusDarkSemanticColors)
    }

    @Test
    fun statusAccentsMeetNonTextContrast() {
        assertSemanticAccentPairs(NexusLightSemanticColors)
        assertSemanticAccentPairs(NexusDarkSemanticColors)
    }

    @Test
    fun outlinesMeetNonTextContrast() {
        assertAtLeast("light outline on surface", NexusLightColors.outline, NexusLightColors.surface, 3.0)
        assertAtLeast("light outline on background", NexusLightColors.outline, NexusLightColors.background, 3.0)
        assertAtLeast(
            "light outline on low container",
            NexusLightColors.outline,
            NexusLightColors.surfaceContainerLow,
            3.0,
        )
        assertAtLeast("dark outline on surface", NexusDarkColors.outline, NexusDarkColors.surface, 3.0)
        assertAtLeast("dark outline on background", NexusDarkColors.outline, NexusDarkColors.background, 3.0)
        assertAtLeast(
            "dark outline on low container",
            NexusDarkColors.outline,
            NexusDarkColors.surfaceContainerLow,
            3.0,
        )
    }

    @Test
    fun themeModesResolvePredictably() {
        assertEquals(false, ThemeMode.LIGHT.isDark(systemDark = true))
        assertEquals(true, ThemeMode.DARK.isDark(systemDark = false))
        assertEquals(true, ThemeMode.SYSTEM.isDark(systemDark = true))
        assertEquals(false, ThemeMode.SYSTEM.isDark(systemDark = false))
    }

    private fun assertTextPairs(colors: ColorScheme) {
        assertAtLeast("onBackground/background", colors.onBackground, colors.background, 4.5)
        assertAtLeast("onSurface/surface", colors.onSurface, colors.surface, 4.5)
        assertAtLeast("onSurface/container", colors.onSurface, colors.surfaceContainer, 4.5)
        assertAtLeast("onSurface/containerHigh", colors.onSurface, colors.surfaceContainerHigh, 4.5)
        assertAtLeast("onSurfaceVariant/surface", colors.onSurfaceVariant, colors.surface, 4.5)
        assertAtLeast("onSurfaceVariant/background", colors.onSurfaceVariant, colors.background, 4.5)
        assertAtLeast("primary/surface", colors.primary, colors.surface, 4.5)
        assertAtLeast("onPrimary/primary", colors.onPrimary, colors.primary, 4.5)
        assertAtLeast(
            "onPrimaryContainer/primaryContainer",
            colors.onPrimaryContainer,
            colors.primaryContainer,
            4.5,
        )
        assertAtLeast(
            "onSecondaryContainer/secondaryContainer",
            colors.onSecondaryContainer,
            colors.secondaryContainer,
            4.5,
        )
        assertAtLeast(
            "onTertiaryContainer/tertiaryContainer",
            colors.onTertiaryContainer,
            colors.tertiaryContainer,
            4.5,
        )
        assertAtLeast("onError/error", colors.onError, colors.error, 4.5)
        assertAtLeast(
            "onErrorContainer/errorContainer",
            colors.onErrorContainer,
            colors.errorContainer,
            4.5,
        )
        assertAtLeast(
            "inverseOnSurface/inverseSurface",
            colors.inverseOnSurface,
            colors.inverseSurface,
            4.5,
        )
    }

    private fun assertSemanticTextPairs(colors: NexusSemanticColors) {
        assertAtLeast(
            "onAvailableContainer/availableContainer",
            colors.onAvailableContainer,
            colors.availableContainer,
            4.5,
        )
        assertAtLeast(
            "onCautionContainer/cautionContainer",
            colors.onCautionContainer,
            colors.cautionContainer,
            4.5,
        )
        assertAtLeast(
            "onOfflineContainer/offlineContainer",
            colors.onOfflineContainer,
            colors.offlineContainer,
            4.5,
        )
        assertAtLeast(
            "onInfoContainer/infoContainer",
            colors.onInfoContainer,
            colors.infoContainer,
            4.5,
        )
        assertAtLeast(
            "onNeutralContainer/neutralContainer",
            colors.onNeutralContainer,
            colors.neutralContainer,
            4.5,
        )
    }

    private fun assertSemanticAccentPairs(colors: NexusSemanticColors) {
        assertAtLeast("available accent", colors.available, colors.availableContainer, 3.0)
        assertAtLeast("caution accent", colors.caution, colors.cautionContainer, 3.0)
        assertAtLeast("offline accent", colors.offline, colors.offlineContainer, 3.0)
        assertAtLeast("info accent", colors.info, colors.infoContainer, 3.0)
        assertAtLeast("neutral accent", colors.onNeutralContainer, colors.neutralContainer, 3.0)
    }

    private fun assertAtLeast(
        name: String,
        foreground: Color,
        background: Color,
        minimum: Double,
    ) {
        val ratio = contrastRatio(foreground, background)
        check(ratio >= minimum) {
            "$name contrast ${"%.2f".format(ratio)}:1 is below the required $minimum:1"
        }
    }

    private fun contrastRatio(
        first: Color,
        second: Color,
    ): Double {
        val firstLuminance = first.luminance().toDouble()
        val secondLuminance = second.luminance().toDouble()
        val lighter = max(firstLuminance, secondLuminance)
        val darker = min(firstLuminance, secondLuminance)
        return (lighter + 0.05) / (darker + 0.05)
    }
}
