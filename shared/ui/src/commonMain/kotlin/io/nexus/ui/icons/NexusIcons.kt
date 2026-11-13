@file:Suppress("MagicNumber")

package io.nexus.ui.icons

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

/**
 * Small, theme-tinted icon set drawn as vectors so Stage 04 adds no icon
 * dependency. Icons are 24×24 with a 1.8 dp optical stroke.
 */
public object NexusIcons {
    public val Explore: ImageVector =
        icon("Explore") {
            stroke {
                circle(12f, 12f, 9f)
                moveTo(15.8f, 8.2f)
                lineTo(13.6f, 13.6f)
                lineTo(8.2f, 15.8f)
                lineTo(10.4f, 10.4f)
                close()
            }
        }

    public val Search: ImageVector =
        icon("Search") {
            stroke {
                circle(10.5f, 10.5f, 6.2f)
                moveTo(15.2f, 15.2f)
                lineTo(20.5f, 20.5f)
            }
        }

    public val Downloads: ImageVector =
        icon("Downloads") {
            stroke {
                moveTo(12f, 3f)
                lineTo(12f, 15f)
                moveTo(7.5f, 10.5f)
                lineTo(12f, 15f)
                lineTo(16.5f, 10.5f)
                moveTo(4f, 16.5f)
                lineTo(4f, 20.5f)
                lineTo(20f, 20.5f)
                lineTo(20f, 16.5f)
            }
        }

    public val Upload: ImageVector =
        icon("Upload") {
            stroke {
                moveTo(12f, 16f)
                lineTo(12f, 4f)
                moveTo(7.5f, 8.5f)
                lineTo(12f, 4f)
                lineTo(16.5f, 8.5f)
                moveTo(4f, 16.5f)
                lineTo(4f, 20.5f)
                lineTo(20f, 20.5f)
                lineTo(20f, 16.5f)
            }
        }

    public val Hosted: ImageVector =
        icon("Hosted") {
            stroke {
                circle(12f, 5f, 2.2f)
                circle(5f, 18f, 2.2f)
                circle(19f, 18f, 2.2f)
                moveTo(10.8f, 6.9f)
                lineTo(6.1f, 16.1f)
                moveTo(13.2f, 6.9f)
                lineTo(17.9f, 16.1f)
                moveTo(7.2f, 18f)
                lineTo(16.8f, 18f)
            }
        }

    public val Settings: ImageVector =
        icon("Settings") {
            stroke {
                moveTo(3f, 6f)
                lineTo(21f, 6f)
                moveTo(3f, 12f)
                lineTo(21f, 12f)
                moveTo(3f, 18f)
                lineTo(21f, 18f)
            }
            solid {
                circle(9f, 6f, 2f)
                circle(15f, 12f, 2f)
                circle(7f, 18f, 2f)
            }
        }

    public val User: ImageVector =
        icon("User") {
            stroke {
                circle(12f, 8f, 3.7f)
                moveTo(4.5f, 20.5f)
                curveTo(5.5f, 16.5f, 8.5f, 14.5f, 12f, 14.5f)
                curveTo(15.5f, 14.5f, 18.5f, 16.5f, 19.5f, 20.5f)
            }
        }

    public val Globe: ImageVector =
        icon("Globe") {
            stroke {
                circle(12f, 12f, 9f)
                moveTo(3f, 12f)
                lineTo(21f, 12f)
                moveTo(12f, 3f)
                curveTo(8.5f, 6.5f, 8.5f, 17.5f, 12f, 21f)
                moveTo(12f, 3f)
                curveTo(15.5f, 6.5f, 15.5f, 17.5f, 12f, 21f)
            }
        }

    public val File: ImageVector =
        icon("File") {
            stroke {
                moveTo(6f, 2.5f)
                lineTo(14f, 2.5f)
                lineTo(19f, 7.5f)
                lineTo(19f, 21.5f)
                lineTo(6f, 21.5f)
                close()
                moveTo(14f, 2.5f)
                lineTo(14f, 7.5f)
                lineTo(19f, 7.5f)
                moveTo(9f, 12f)
                lineTo(16f, 12f)
                moveTo(9f, 16f)
                lineTo(16f, 16f)
            }
        }

    public val Folder: ImageVector =
        icon("Folder") {
            stroke {
                moveTo(3f, 7f)
                lineTo(10f, 7f)
                lineTo(12f, 9f)
                lineTo(21f, 9f)
                lineTo(21f, 20f)
                lineTo(3f, 20f)
                close()
            }
        }

    public val Check: ImageVector =
        icon("Check") {
            stroke {
                moveTo(4.5f, 12.5f)
                lineTo(9.5f, 17.5f)
                lineTo(19.5f, 6.5f)
            }
        }

    public val Warning: ImageVector =
        icon("Warning") {
            stroke {
                moveTo(12f, 3f)
                lineTo(22f, 20.5f)
                lineTo(2f, 20.5f)
                close()
                moveTo(12f, 9f)
                lineTo(12f, 14f)
            }
            solid {
                circle(12f, 17.4f, 0.9f)
            }
        }

    public val Error: ImageVector =
        icon("Error") {
            stroke {
                circle(12f, 12f, 9f)
                moveTo(8.5f, 8.5f)
                lineTo(15.5f, 15.5f)
                moveTo(15.5f, 8.5f)
                lineTo(8.5f, 15.5f)
            }
        }

    public val Info: ImageVector =
        icon("Info") {
            stroke {
                circle(12f, 12f, 9f)
                moveTo(12f, 11f)
                lineTo(12f, 17f)
            }
            solid {
                circle(12f, 7.4f, 0.9f)
            }
        }

    public val Close: ImageVector =
        icon("Close") {
            stroke {
                moveTo(6f, 6f)
                lineTo(18f, 18f)
                moveTo(18f, 6f)
                lineTo(6f, 18f)
            }
        }

    public val Back: ImageVector =
        icon("Back") {
            stroke {
                moveTo(20f, 12f)
                lineTo(4f, 12f)
                moveTo(10f, 6f)
                lineTo(4f, 12f)
                lineTo(10f, 18f)
            }
        }

    public val ChevronRight: ImageVector =
        icon("ChevronRight") {
            stroke {
                moveTo(9f, 5f)
                lineTo(16f, 12f)
                lineTo(9f, 19f)
            }
        }

    public val Refresh: ImageVector =
        icon("Refresh") {
            stroke {
                moveTo(20f, 9f)
                curveTo(18.7f, 5.2f, 14.7f, 2.9f, 10.5f, 3.6f)
                curveTo(5.9f, 4.4f, 2.6f, 8.5f, 3.3f, 13.1f)
                curveTo(4f, 17.7f, 8.2f, 20.9f, 12.8f, 20.2f)
                curveTo(16.4f, 19.7f, 19.3f, 17f, 20.1f, 13.4f)
                moveTo(16.5f, 8.5f)
                lineTo(20f, 9f)
                lineTo(19.6f, 5.5f)
            }
        }

    public val Sun: ImageVector =
        icon("Sun") {
            stroke {
                circle(12f, 12f, 4.2f)
                moveTo(12f, 2f)
                lineTo(12f, 4f)
                moveTo(12f, 20f)
                lineTo(12f, 22f)
                moveTo(2f, 12f)
                lineTo(4f, 12f)
                moveTo(20f, 12f)
                lineTo(22f, 12f)
                moveTo(4.9f, 4.9f)
                lineTo(6.3f, 6.3f)
                moveTo(17.7f, 17.7f)
                lineTo(19.1f, 19.1f)
                moveTo(19.1f, 4.9f)
                lineTo(17.7f, 6.3f)
                moveTo(6.3f, 17.7f)
                lineTo(4.9f, 19.1f)
            }
        }

    public val Moon: ImageVector =
        icon("Moon") {
            stroke {
                moveTo(20.2f, 14.6f)
                curveTo(18.2f, 17.8f, 14.6f, 19.5f, 11f, 18.9f)
                curveTo(6f, 18f, 2.7f, 13.3f, 3.6f, 8.3f)
                curveTo(4.3f, 4.5f, 7.3f, 1.7f, 11f, 1.2f)
                curveTo(7.8f, 4f, 6.7f, 8.7f, 8.5f, 12.7f)
                curveTo(10.3f, 16.7f, 14.8f, 18.4f, 20.2f, 14.6f)
            }
        }

    public val Monitor: ImageVector =
        icon("Monitor") {
            stroke {
                moveTo(3f, 4.5f)
                lineTo(21f, 4.5f)
                lineTo(21f, 16.5f)
                lineTo(3f, 16.5f)
                close()
                moveTo(9f, 20.5f)
                lineTo(15f, 20.5f)
                moveTo(12f, 16.5f)
                lineTo(12f, 20.5f)
            }
        }

    public val More: ImageVector =
        icon("More") {
            solid {
                circle(5f, 12f, 1.4f)
                circle(12f, 12f, 1.4f)
                circle(19f, 12f, 1.4f)
            }
        }

    public val Clock: ImageVector =
        icon("Clock") {
            stroke {
                circle(12f, 12f, 9f)
                moveTo(12f, 6.5f)
                lineTo(12f, 12f)
                lineTo(16f, 14.5f)
            }
        }

    public val Shield: ImageVector =
        icon("Shield") {
            stroke {
                moveTo(12f, 2.5f)
                lineTo(20f, 5.5f)
                lineTo(20f, 11f)
                curveTo(20f, 16f, 16.7f, 20.2f, 12f, 21.5f)
                curveTo(7.3f, 20.2f, 4f, 16f, 4f, 11f)
                lineTo(4f, 5.5f)
                close()
                moveTo(8.5f, 12f)
                lineTo(11f, 14.5f)
                lineTo(15.5f, 9.5f)
            }
        }

    public val Storage: ImageVector =
        icon("Storage") {
            stroke {
                moveTo(4f, 6f)
                curveTo(4f, 3.8f, 7.6f, 2.2f, 12f, 2.2f)
                curveTo(16.4f, 2.2f, 20f, 3.8f, 20f, 6f)
                curveTo(20f, 8.2f, 16.4f, 9.8f, 12f, 9.8f)
                curveTo(7.6f, 9.8f, 4f, 8.2f, 4f, 6f)
                close()
                moveTo(4f, 6f)
                lineTo(4f, 18f)
                curveTo(4f, 20.2f, 7.6f, 21.8f, 12f, 21.8f)
                curveTo(16.4f, 21.8f, 20f, 20.2f, 20f, 18f)
                lineTo(20f, 6f)
                moveTo(4f, 12f)
                curveTo(4f, 14.2f, 7.6f, 15.8f, 12f, 15.8f)
                curveTo(16.4f, 15.8f, 20f, 14.2f, 20f, 12f)
            }
        }

    public val Tag: ImageVector =
        icon("Tag") {
            stroke {
                moveTo(3.5f, 11.5f)
                lineTo(11.5f, 3.5f)
                lineTo(20.5f, 3.5f)
                lineTo(20.5f, 12.5f)
                lineTo(12.5f, 20.5f)
                close()
            }
            solid {
                circle(16.5f, 7.5f, 1.2f)
            }
        }

    public val Pause: ImageVector =
        icon("Pause") {
            stroke {
                moveTo(9f, 5f)
                lineTo(9f, 19f)
                moveTo(15f, 5f)
                lineTo(15f, 19f)
            }
        }

    public val Play: ImageVector =
        icon("Play") {
            solid {
                moveTo(7f, 4.5f)
                lineTo(19f, 12f)
                lineTo(7f, 19.5f)
                close()
            }
        }

    public val Stop: ImageVector =
        icon("Stop") {
            solid {
                moveTo(5f, 5f)
                lineTo(19f, 5f)
                lineTo(19f, 19f)
                lineTo(5f, 19f)
                close()
            }
        }

    public val Lock: ImageVector =
        icon("Lock") {
            stroke {
                moveTo(6f, 10.5f)
                lineTo(18f, 10.5f)
                lineTo(18f, 20.5f)
                lineTo(6f, 20.5f)
                close()
                moveTo(8.5f, 10.5f)
                lineTo(8.5f, 8f)
                curveTo(8.5f, 5.5f, 10.2f, 3.5f, 12f, 3.5f)
                curveTo(13.8f, 3.5f, 15.5f, 5.5f, 15.5f, 8f)
                lineTo(15.5f, 10.5f)
            }
        }

    public val Copy: ImageVector =
        icon("Copy") {
            stroke {
                moveTo(8f, 8f)
                lineTo(20f, 8f)
                lineTo(20f, 20f)
                lineTo(8f, 20f)
                close()
                moveTo(16f, 8f)
                lineTo(16f, 4f)
                lineTo(4f, 4f)
                lineTo(4f, 16f)
                lineTo(8f, 16f)
            }
        }

    public val Trash: ImageVector =
        icon("Trash") {
            stroke {
                moveTo(4f, 6.5f)
                lineTo(20f, 6.5f)
                moveTo(9f, 6.5f)
                lineTo(9f, 4f)
                lineTo(15f, 4f)
                lineTo(15f, 6.5f)
                moveTo(6.5f, 6.5f)
                lineTo(7.5f, 21f)
                lineTo(16.5f, 21f)
                lineTo(17.5f, 6.5f)
                moveTo(10f, 10.5f)
                lineTo(10.5f, 17.5f)
                moveTo(14f, 10.5f)
                lineTo(13.5f, 17.5f)
            }
        }

    public val ArrowsUpDown: ImageVector =
        icon("ArrowsUpDown") {
            stroke {
                moveTo(8f, 4f)
                lineTo(8f, 20f)
                moveTo(4.5f, 7.5f)
                lineTo(8f, 4f)
                lineTo(11.5f, 7.5f)
                moveTo(16f, 20f)
                lineTo(16f, 4f)
                moveTo(12.5f, 16.5f)
                lineTo(16f, 20f)
                lineTo(19.5f, 16.5f)
            }
        }
}

private fun icon(
    name: String,
    block: ImageVector.Builder.() -> Unit,
): ImageVector =
    ImageVector
        .Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply(block)
        .build()

private fun ImageVector.Builder.stroke(block: PathBuilder.() -> Unit) {
    path(
        fill = null,
        stroke = SolidColor(Color.Black),
        strokeLineWidth = 1.8f,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
        pathBuilder = block,
    )
}

private fun ImageVector.Builder.solid(block: PathBuilder.() -> Unit) {
    path(
        fill = SolidColor(Color.Black),
        pathBuilder = block,
    )
}

private fun PathBuilder.circle(
    centerX: Float,
    centerY: Float,
    radius: Float,
) {
    moveTo(centerX, centerY - radius)
    curveTo(
        centerX + radius * 0.5523f,
        centerY - radius,
        centerX + radius,
        centerY - radius * 0.5523f,
        centerX + radius,
        centerY,
    )
    curveTo(
        centerX + radius,
        centerY + radius * 0.5523f,
        centerX + radius * 0.5523f,
        centerY + radius,
        centerX,
        centerY + radius,
    )
    curveTo(
        centerX - radius * 0.5523f,
        centerY + radius,
        centerX - radius,
        centerY + radius * 0.5523f,
        centerX - radius,
        centerY,
    )
    curveTo(
        centerX - radius,
        centerY - radius * 0.5523f,
        centerX - radius * 0.5523f,
        centerY - radius,
        centerX,
        centerY - radius,
    )
    close()
}
