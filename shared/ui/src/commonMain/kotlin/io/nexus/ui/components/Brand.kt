@file:Suppress("MagicNumber")

package io.nexus.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSpacing
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * Decorative NEXUS node mark. The adjacent wordmark carries the accessible
 * name, so the mark itself is intentionally silent.
 */
@Composable
public fun NexusMark(
    modifier: Modifier = Modifier,
    tint: Color = MaterialTheme.colorScheme.primary,
    size: Dp = 36.dp,
) {
    Canvas(modifier = modifier.size(size)) {
        val center = Offset(this.size.width / 2f, this.size.height / 2f)
        val radius = this.size.minDimension * 0.37f
        val nodeRadius = this.size.minDimension * 0.06f
        val lineWidth = this.size.minDimension * 0.025f
        val nodes =
            List(6) { index ->
                val angle = (-PI / 2.0 + index * PI / 3.0).toFloat()
                Offset(
                    x = center.x + cos(angle) * radius,
                    y = center.y + sin(angle) * radius,
                )
            }

        drawCircle(color = tint.copy(alpha = 0.12f), radius = radius * 1.14f, center = center)
        drawCircle(color = tint.copy(alpha = 0.20f), radius = radius * 0.82f, center = center)
        nodes.forEach { node ->
            drawLine(
                color = tint.copy(alpha = 0.55f),
                start = center,
                end = node,
                strokeWidth = lineWidth,
                cap = StrokeCap.Round,
            )
        }
        nodes.forEach { node ->
            drawCircle(color = tint, radius = nodeRadius, center = node)
        }
        drawCircle(color = tint, radius = nodeRadius * 1.4f, center = center)
    }
}

/** Brand lockup used in the welcome screen and desktop navigation rail. */
@Composable
public fun NexusWordmark(
    modifier: Modifier = Modifier,
    markSize: Dp = 34.dp,
    showTagline: Boolean = false,
) {
    val strings = LocalNexusStrings.current
    Row(
        modifier = modifier,
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
    ) {
        NexusMark(size = markSize)
        Column {
            Text(
                text = strings.appName,
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            if (showTagline) {
                Spacer(modifier = Modifier.width(NexusSpacing.xxs))
                Text(
                    text = strings.appTagline,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
