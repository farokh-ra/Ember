package io.nexus.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusBreakpoints
import io.nexus.ui.theme.NexusSizes
import io.nexus.ui.theme.NexusSpacing

/**
 * Standard screen layout: a fixed header, optional preview controls, and a
 * scrollable content area constrained to a readable width on desktop.
 */
@Composable
public fun NexusScreen(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    onBack: (() -> Unit)? = null,
    headerActions: (@Composable RowScope.() -> Unit)? = null,
    previewControls: (@Composable () -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val expanded = maxWidth >= NexusBreakpoints.expanded
        val gutter = if (expanded) NexusSizes.expandedGutter else NexusSizes.compactGutter
        Column(modifier = Modifier.fillMaxSize()) {
            NexusScreenHeader(
                title = title,
                subtitle = subtitle,
                onBack = onBack,
                expanded = expanded,
                modifier =
                    Modifier.padding(
                        start = gutter,
                        end = gutter,
                        top = if (expanded) NexusSpacing.xl else NexusSpacing.lg,
                        bottom = NexusSpacing.md,
                    ),
                actions = headerActions,
            )
            if (previewControls != null) {
                Box(modifier = Modifier.padding(horizontal = gutter)) {
                    previewControls()
                }
            }
            Box(
                modifier =
                    Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                contentAlignment = Alignment.TopCenter,
            ) {
                Column(
                    modifier =
                        Modifier
                            .widthIn(max = NexusSizes.maxContentWidth)
                            .fillMaxWidth()
                            .verticalScroll(rememberScrollState())
                            .padding(
                                start = gutter,
                                end = gutter,
                                top = NexusSpacing.lg,
                                bottom = NexusSpacing.xxl,
                            ),
                    verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                    content = content,
                )
            }
        }
    }
}

/** Page header with optional back action and trailing controls. */
@Composable
public fun NexusScreenHeader(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    onBack: (() -> Unit)? = null,
    expanded: Boolean = true,
    actions: (@Composable RowScope.() -> Unit)? = null,
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
    ) {
        // Compact layouts carry Back in the shell's top bar, so rendering it
        // again here would duplicate the control.
        if (onBack != null && expanded) {
            NexusBackButton(onBack = onBack)
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style =
                    if (expanded) {
                        MaterialTheme.typography.headlineLarge
                    } else {
                        MaterialTheme.typography.headlineSmall
                    },
                color = MaterialTheme.colorScheme.onSurface,
            )
            if (subtitle != null) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (actions != null) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.xs),
                content = actions,
            )
        }
    }
}

/**
 * Persistent global banner. It cannot be dismissed because the prototype must
 * never imply that network features are active.
 */
@Composable
public fun NexusGlobalDemoBanner(modifier: Modifier = Modifier) {
    val strings = LocalNexusStrings.current
    val colors = nexusToneColors(StatusTone.CAUTION)
    BoxWithConstraints(modifier = modifier.fillMaxWidth()) {
        val expanded = maxWidth >= NexusBreakpoints.expanded
        Surface(
            modifier = Modifier.fillMaxWidth(),
            color = colors.container,
            contentColor = colors.content,
        ) {
            Row(
                modifier =
                    Modifier.padding(
                        horizontal = if (expanded) NexusSpacing.xl else NexusSpacing.md,
                        vertical = NexusSpacing.sm,
                    ),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
            ) {
                Icon(
                    imageVector = NexusIcons.Info,
                    contentDescription = null,
                    modifier = Modifier.size(18.dp),
                )
                Text(
                    text = strings.demoBannerShort,
                    style = MaterialTheme.typography.labelMedium,
                )
                if (expanded) {
                    Spacer(modifier = Modifier.weight(1f))
                    Text(
                        text = strings.demoBannerBody,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }
        }
    }
}

/** Full-width action cluster that wraps into a column on narrow screens. */
@Composable
public fun NexusActionCluster(
    modifier: Modifier = Modifier,
    content: @Composable RowScope.() -> Unit,
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
        verticalAlignment = Alignment.CenterVertically,
        content = content,
    )
}
