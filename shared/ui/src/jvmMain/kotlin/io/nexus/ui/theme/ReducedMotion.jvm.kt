package io.nexus.ui.theme

import androidx.compose.runtime.Composable

/**
 * Desktop shells avoid non-essential animations in Stage 04.
 *
 * Windows accessibility integration for reduced motion is added with the
 * platform integration layer in a later stage.
 */
@Composable
public actual fun rememberReducedMotion(): Boolean = false
