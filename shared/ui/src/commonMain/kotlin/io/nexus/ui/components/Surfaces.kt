package io.nexus.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusCodeTextStyle
import io.nexus.ui.theme.NexusSizes
import io.nexus.ui.theme.NexusSpacing

/**
 * Standard Ember content card.
 *
 * When clickable, the whole card is a single button target with a visible focus
 * ring. Inner actions should instead use their own 48 dp controls.
 */
@Composable
public fun NexusCard(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    onClickLabel: String? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val shape = MaterialTheme.shapes.large
    val border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
    val clickableModifier =
        if (onClick == null) {
            modifier
        } else {
            modifier
                .nexusFocusRing(shape)
                .clickable(
                    role = Role.Button,
                    onClickLabel = onClickLabel,
                    onClick = onClick,
                )
        }

    Surface(
        modifier = clickableModifier,
        shape = shape,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        contentColor = MaterialTheme.colorScheme.onSurface,
        border = border,
    ) {
        Column(
            modifier = Modifier.padding(NexusSpacing.lg),
            verticalArrangement = Arrangement.spacedBy(NexusSpacing.md),
            content = content,
        )
    }
}

/** Section heading with optional trailing actions. */
@Composable
public fun NexusSectionHeader(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            if (subtitle != null) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (trailing != null) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
                content = trailing,
            )
        }
    }
}

/** Visible demo-data marker. Colour is paired with text and an icon. */
@Composable
public fun NexusDemoBadge(modifier: Modifier = Modifier) {
    val strings = LocalNexusStrings.current
    StatusBadge(
        text = strings.demoBadge,
        tone = StatusTone.CAUTION,
        icon = NexusIcons.Info,
        modifier = modifier,
    )
}

/** Inline notice used for demo disclaimers and honest-state explanations. */
@Composable
public fun NexusInfoBanner(
    title: String,
    body: String,
    modifier: Modifier = Modifier,
    tone: StatusTone = StatusTone.INFO,
    icon: ImageVector = NexusIcons.Info,
    actionLabel: String? = null,
    onAction: (() -> Unit)? = null,
) {
    val colors = nexusToneColors(tone)
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.medium,
        color = colors.container,
        contentColor = colors.content,
    ) {
        Row(
            modifier = Modifier.padding(NexusSpacing.lg),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
            verticalAlignment = Alignment.Top,
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                modifier = Modifier.size(20.dp),
            )
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(NexusSpacing.xs),
            ) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = body,
                    style = MaterialTheme.typography.bodySmall,
                )
                if (actionLabel != null && onAction != null) {
                    NexusTertiaryButton(
                        text = actionLabel,
                        onClick = onAction,
                        modifier = Modifier.padding(top = NexusSpacing.xs),
                    )
                }
            }
        }
    }
}

/** Two-line label/value pair used in metadata sections. */
@Composable
public fun NexusKeyValue(
    label: String,
    value: String,
    modifier: Modifier = Modifier,
    monospace: Boolean = false,
) {
    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(NexusSpacing.xxs),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (monospace) {
            SelectionContainer {
                Text(
                    text = value,
                    style = NexusCodeTextStyle,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        } else {
            Text(
                text = value,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

/** Compact metric used on profile and network summaries. */
@Composable
public fun NexusMetric(
    value: String,
    label: String,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
) {
    Surface(
        modifier = modifier.heightIn(min = NexusSizes.cardMinHeight),
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(
            modifier = Modifier.padding(NexusSpacing.md),
            verticalArrangement = Arrangement.spacedBy(NexusSpacing.xxs),
        ) {
            if (icon != null) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    modifier = Modifier.size(18.dp),
                    tint = MaterialTheme.colorScheme.primary,
                )
            }
            Text(
                text = value,
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/**
 * List-style action row, used for settings and profile actions.
 *
 * A disabled row keeps its explanation visible so the action never silently
 * fails. The row is at least 56 dp tall.
 */
@Composable
public fun NexusActionRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    icon: ImageVector? = null,
    onClick: (() -> Unit)? = null,
    enabled: Boolean = true,
) {
    val rowShape = MaterialTheme.shapes.medium
    val interactiveModifier =
        if (onClick != null && enabled) {
            modifier
                .nexusFocusRing(rowShape)
                .clickable(role = Role.Button, onClick = onClick)
        } else {
            modifier
        }
    Row(
        modifier =
            interactiveModifier
                .fillMaxWidth()
                .heightIn(min = 56.dp)
                .padding(horizontal = NexusSpacing.sm, vertical = NexusSpacing.md),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
    ) {
        if (icon != null) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                modifier = Modifier.size(22.dp),
                tint =
                    if (enabled) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
            )
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall,
                color =
                    if (enabled) {
                        MaterialTheme.colorScheme.onSurface
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
            )
            if (subtitle != null) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (onClick != null && enabled) {
            Icon(
                imageVector = NexusIcons.ChevronRight,
                contentDescription = null,
                modifier = Modifier.size(20.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** Horizontal divider with a token colour. */
@Composable
public fun NexusDivider(modifier: Modifier = Modifier) {
    HorizontalDivider(
        modifier = modifier,
        thickness = 1.dp,
        color = MaterialTheme.colorScheme.outlineVariant,
    )
}

/** Title used above a group of related controls. */
@Composable
public fun NexusGroupLabel(
    text: String,
    modifier: Modifier = Modifier,
) {
    Text(
        modifier = modifier.fillMaxWidth(),
        text = text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/** Compact, selectable metadata tag. */
@Composable
public fun NexusTag(
    text: String,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(6.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        contentColor = MaterialTheme.colorScheme.onSurfaceVariant,
    ) {
        Text(
            modifier = Modifier.padding(horizontal = NexusSpacing.sm, vertical = 3.dp),
            text = text,
            style = MaterialTheme.typography.labelSmall,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** Constrains reading-heavy content while keeping it left aligned on wide screens. */
@Composable
public fun NexusReadingColumn(
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier = modifier.widthIn(max = NexusSizes.maxReadingWidth),
        verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
        content = content,
    )
}
