package io.nexus.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import io.nexus.presentation.content.ContentState
import io.nexus.presentation.content.UiErrorCode
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSizes
import io.nexus.ui.theme.NexusSpacing
import io.nexus.ui.theme.NexusTheme

/**
 * Renders one of the five required content states for a data-driven screen.
 *
 * Every non-ready panel includes an explicit way out of the state and a visible
 * note that the operation is simulated. Error copy never exposes internals.
 */
@Composable
public fun <T> ContentStateHost(
    state: ContentState<T>,
    onShowData: () -> Unit,
    modifier: Modifier = Modifier,
    emptyTitle: String? = null,
    emptyBody: String? = null,
    emptyActionLabel: String? = null,
    onEmptyAction: (() -> Unit)? = null,
    content: @Composable (T) -> Unit,
) {
    val strings = LocalNexusStrings.current
    when (state) {
        ContentState.Loading -> {
            ContentStatePanel(
                title = strings.loadingTitle,
                body = strings.loadingBody,
                icon = NexusIcons.Refresh,
                tone = StatusTone.INFO,
                actionLabel = strings.loadingAction,
                onAction = onShowData,
                showProgress = true,
                modifier = modifier,
            )
        }

        ContentState.Empty -> {
            ContentStatePanel(
                title = emptyTitle ?: strings.emptyTitle,
                body = emptyBody ?: strings.emptyBody,
                icon = NexusIcons.Folder,
                tone = StatusTone.NEUTRAL,
                actionLabel = emptyActionLabel ?: strings.emptyAction,
                onAction = onEmptyAction ?: onShowData,
                modifier = modifier,
            )
        }

        ContentState.Offline -> {
            ContentStatePanel(
                title = strings.offlineTitle,
                body = strings.offlineBody,
                icon = NexusIcons.Globe,
                tone = StatusTone.NEGATIVE,
                actionLabel = strings.offlineViewCached,
                onAction = onShowData,
                modifier = modifier,
            )
        }

        is ContentState.Error -> {
            val localStore = state.code == UiErrorCode.LOCAL_STORE_UNAVAILABLE
            ContentStatePanel(
                title = if (localStore) strings.errorLocalStoreTitle else strings.errorTitle,
                body = if (localStore) strings.errorLocalStoreBody else strings.errorBody,
                icon = NexusIcons.Error,
                tone = StatusTone.NEGATIVE,
                actionLabel = strings.errorRetry,
                onAction = onShowData,
                modifier = modifier,
            )
        }

        is ContentState.Ready -> {
            content(state.value)
        }
    }
}

/** Centered state panel with an icon, explanation, action, and demo note. */
@Composable
public fun ContentStatePanel(
    title: String,
    body: String,
    icon: ImageVector,
    tone: StatusTone,
    modifier: Modifier = Modifier,
    actionLabel: String? = null,
    onAction: (() -> Unit)? = null,
    secondaryActionLabel: String? = null,
    onSecondaryAction: (() -> Unit)? = null,
    showProgress: Boolean = false,
) {
    val strings = LocalNexusStrings.current
    val colors = nexusToneColors(tone)
    Surface(
        modifier =
            modifier
                .fillMaxWidth()
                .heightIn(min = NexusSizes.statePanelMinHeight)
                .semantics { liveRegion = LiveRegionMode.Polite },
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
    ) {
        Box(
            modifier = Modifier.padding(NexusSpacing.xl),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                modifier = Modifier.widthIn(max = 480.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(NexusSpacing.md),
            ) {
                Surface(
                    shape = MaterialTheme.shapes.extraLarge,
                    color = colors.container,
                    contentColor = colors.content,
                ) {
                    Box(
                        modifier = Modifier.size(56.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        if (showProgress && !NexusTheme.reducedMotion) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(26.dp),
                                strokeWidth = 2.dp,
                                color = colors.content,
                            )
                        } else {
                            Icon(
                                imageVector = icon,
                                contentDescription = null,
                                modifier = Modifier.size(26.dp),
                            )
                        }
                    }
                }
                Text(
                    text = title,
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    textAlign = TextAlign.Center,
                )
                Text(
                    text = body,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
                if (actionLabel != null && onAction != null) {
                    NexusPrimaryButton(text = actionLabel, onClick = onAction)
                }
                if (secondaryActionLabel != null && onSecondaryAction != null) {
                    NexusTertiaryButton(text = secondaryActionLabel, onClick = onSecondaryAction)
                }
                Row(
                    horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        imageVector = NexusIcons.Info,
                        contentDescription = null,
                        modifier = Modifier.size(14.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        text = strings.stateDemoNote,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
    }
}

/** Compact inline empty state for filtered views that are not screen-level states. */
@Composable
public fun NexusInlineEmpty(
    title: String,
    body: String,
    modifier: Modifier = Modifier,
    actionLabel: String? = null,
    onAction: (() -> Unit)? = null,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
    ) {
        Column(
            modifier = Modifier.padding(NexusSpacing.xl),
            verticalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = body,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
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
