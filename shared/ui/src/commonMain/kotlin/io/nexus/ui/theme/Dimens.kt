package io.nexus.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** Company-wide spacing scale. Use these values instead of arbitrary dp literals. */
public object NexusSpacing {
    public val xxs: Dp = 2.dp
    public val xs: Dp = 4.dp
    public val sm: Dp = 8.dp
    public val md: Dp = 12.dp
    public val lg: Dp = 16.dp
    public val xl: Dp = 24.dp
    public val xxl: Dp = 32.dp
    public val xxxl: Dp = 48.dp
    public val huge: Dp = 64.dp
}

/** Layout and accessibility dimensions. */
public object NexusSizes {
    public val minimumTouchTarget: Dp = 48.dp
    public val compactGutter: Dp = 16.dp
    public val expandedGutter: Dp = 32.dp
    public val maxContentWidth: Dp = 1120.dp
    public val maxReadingWidth: Dp = 760.dp
    public val sideRailWidth: Dp = 236.dp
    public val topBarMinHeight: Dp = 72.dp
    public val demoBarMinHeight: Dp = 40.dp
    public val cardMinHeight: Dp = 72.dp
    public val statePanelMinHeight: Dp = 280.dp
}

/** Responsive breakpoints used by the shell. */
public object NexusBreakpoints {
    public val expanded: Dp = 840.dp
}

/** Ember shape scale. */
public val NexusShapes: Shapes =
    Shapes(
        extraSmall = RoundedCornerShape(4.dp),
        small = RoundedCornerShape(8.dp),
        medium = RoundedCornerShape(12.dp),
        large = RoundedCornerShape(16.dp),
        extraLarge = RoundedCornerShape(24.dp),
    )
