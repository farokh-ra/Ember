package io.nexus.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.demo.DownloadStatus
import io.nexus.presentation.demo.PublicationAvailability
import io.nexus.presentation.demo.ReplicaVerification
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.resources.NexusStrings
import io.nexus.ui.theme.NexusSpacing
import io.nexus.ui.theme.NexusTheme

/** Semantic tone used by status badges, banners, and network indicators. */
public enum class StatusTone {
    AVAILABLE,
    CAUTION,
    NEGATIVE,
    INFO,
    NEUTRAL,
}

/** Resolved semantic colors for a tone. */
public data class NexusToneColors(
    public val container: Color,
    public val content: Color,
    public val accent: Color,
)

/** Resolves theme-aware status colors for banners and badges. */
@Composable
public fun nexusToneColors(tone: StatusTone): NexusToneColors {
    val colors = NexusTheme.semanticColors
    return when (tone) {
        StatusTone.AVAILABLE -> {
            NexusToneColors(colors.availableContainer, colors.onAvailableContainer, colors.available)
        }

        StatusTone.CAUTION -> {
            NexusToneColors(colors.cautionContainer, colors.onCautionContainer, colors.caution)
        }

        StatusTone.NEGATIVE -> {
            NexusToneColors(colors.offlineContainer, colors.onOfflineContainer, colors.offline)
        }

        StatusTone.INFO -> {
            NexusToneColors(colors.infoContainer, colors.onInfoContainer, colors.info)
        }

        StatusTone.NEUTRAL -> {
            NexusToneColors(
                colors.neutralContainer,
                colors.onNeutralContainer,
                MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** A text-and-icon badge; colour is never the only signal. */
@Composable
public fun StatusBadge(
    text: String,
    tone: StatusTone,
    icon: ImageVector,
    modifier: Modifier = Modifier,
) {
    val colors = nexusToneColors(tone)
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(8.dp),
        color = colors.container,
        contentColor = colors.content,
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 9.dp, vertical = 5.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                modifier = Modifier.size(14.dp),
            )
            Text(
                text = text,
                style = MaterialTheme.typography.labelSmall,
            )
        }
    }
}

/** File-level availability badge using canonical product vocabulary. */
@Composable
public fun AvailabilityBadge(
    availability: PublicationAvailability,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    when (availability) {
        PublicationAvailability.AVAILABLE_NOW -> {
            StatusBadge(strings.availabilityAvailableNow, StatusTone.AVAILABLE, NexusIcons.Check, modifier)
        }

        PublicationAvailability.SEEN_RECENTLY -> {
            StatusBadge(strings.availabilitySeenRecently, StatusTone.CAUTION, NexusIcons.Clock, modifier)
        }

        PublicationAvailability.NO_KNOWN_SOURCE -> {
            StatusBadge(strings.availabilityNoKnownSource, StatusTone.NEUTRAL, NexusIcons.Search, modifier)
        }

        PublicationAvailability.RECORD_ONLY -> {
            StatusBadge(strings.availabilityRecordOnly, StatusTone.NEUTRAL, NexusIcons.File, modifier)
        }
    }
}

/** Separate lifecycle badge for an unpublish tombstone. */
@Composable
public fun UnpublishedBadge(modifier: Modifier = Modifier) {
    val strings = LocalNexusStrings.current
    StatusBadge(
        text = strings.availabilityUnpublished,
        tone = StatusTone.CAUTION,
        icon = NexusIcons.Warning,
        modifier = modifier,
    )
}

/** Whether this device currently serves verified bytes for the publication. */
@Composable
public fun ServingBadge(
    serving: Boolean,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    StatusBadge(
        text = if (serving) strings.fileServingNow else strings.fileNotServing,
        tone = if (serving) StatusTone.AVAILABLE else StatusTone.NEUTRAL,
        icon = if (serving) NexusIcons.Hosted else NexusIcons.Clock,
        modifier = modifier,
    )
}

@Composable
public fun SourceStatusBadge(
    online: Boolean,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    StatusBadge(
        text = if (online) strings.sourceOnline else strings.sourceOffline,
        tone = if (online) StatusTone.AVAILABLE else StatusTone.NEUTRAL,
        icon = if (online) NexusIcons.Check else NexusIcons.Clock,
        modifier = modifier,
    )
}

@Composable
public fun RecordVerificationBadge(
    verified: Boolean,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    StatusBadge(
        text = if (verified) strings.recordVerified else strings.fileSignatureUnverified,
        tone = if (verified) StatusTone.AVAILABLE else StatusTone.NEUTRAL,
        icon = if (verified) NexusIcons.Shield else NexusIcons.Warning,
        modifier = modifier,
    )
}

@Composable
public fun ContentVerificationBadge(
    verified: Boolean,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    StatusBadge(
        text = if (verified) strings.contentVerified else strings.contentNotVerified,
        tone = if (verified) StatusTone.AVAILABLE else StatusTone.NEUTRAL,
        icon = if (verified) NexusIcons.Check else NexusIcons.File,
        modifier = modifier,
    )
}

@Composable
public fun DownloadStatusBadge(
    status: DownloadStatus,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    when (status) {
        DownloadStatus.DOWNLOADING -> {
            StatusBadge(strings.downloadsStatusDownloading, StatusTone.INFO, NexusIcons.Downloads, modifier)
        }

        DownloadStatus.PAUSED -> {
            StatusBadge(strings.downloadsStatusPaused, StatusTone.CAUTION, NexusIcons.Pause, modifier)
        }

        DownloadStatus.VERIFYING -> {
            StatusBadge(strings.downloadsStatusVerifying, StatusTone.INFO, NexusIcons.Refresh, modifier)
        }

        DownloadStatus.COMPLETE -> {
            StatusBadge(strings.downloadsStatusComplete, StatusTone.AVAILABLE, NexusIcons.Check, modifier)
        }

        DownloadStatus.FAILED -> {
            StatusBadge(strings.downloadsStatusFailed, StatusTone.NEGATIVE, NexusIcons.Error, modifier)
        }
    }
}

@Composable
public fun HostedStatusBadge(
    verification: ReplicaVerification,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    when (verification) {
        ReplicaVerification.VERIFIED -> {
            StatusBadge(strings.hostedStatusVerified, StatusTone.AVAILABLE, NexusIcons.Shield, modifier)
        }

        ReplicaVerification.VERIFYING -> {
            StatusBadge(strings.hostedStatusVerifying, StatusTone.INFO, NexusIcons.Refresh, modifier)
        }

        ReplicaVerification.PAUSED -> {
            StatusBadge(strings.hostedStatusPaused, StatusTone.CAUTION, NexusIcons.Pause, modifier)
        }

        ReplicaVerification.FAILED -> {
            StatusBadge(strings.hostedStatusFailed, StatusTone.NEGATIVE, NexusIcons.Error, modifier)
        }
    }
}

/** Clickable global network indicator. Text and icon distinguish every state. */
@Composable
public fun NetworkStatusPill(
    state: PreviewNetworkState,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    val colors = nexusToneColors(networkStateTone(state))
    val shape = CircleShape
    Surface(
        modifier =
            modifier
                .heightIn(min = 48.dp)
                .nexusFocusRing(shape)
                .clickable(
                    role = Role.Button,
                    onClickLabel = strings.openNetworkStatus,
                    onClick = onClick,
                ),
        shape = shape,
        color = colors.container,
        contentColor = colors.content,
        border = BorderStroke(1.dp, colors.content.copy(alpha = 0.22f)),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Icon(
                imageVector = networkStateIcon(state),
                contentDescription = null,
                modifier = Modifier.size(16.dp),
            )
            Text(
                text = networkStateLabel(state, strings),
                style = MaterialTheme.typography.labelMedium,
            )
        }
    }
}
