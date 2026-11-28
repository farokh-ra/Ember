package io.nexus.ui

import androidx.compose.runtime.staticCompositionLocalOf
import io.nexus.core.api.CoreStatus

/**
 * Core status provided by the platform shell.
 *
 * Screens read this only to report the real core state. Demo content never
 * implies that the core, a network peer, or a transfer is available.
 */
public val LocalCoreStatus =
    staticCompositionLocalOf {
        CoreStatus.unavailable()
    }
