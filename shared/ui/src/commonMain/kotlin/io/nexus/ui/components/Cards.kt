package io.nexus.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.nexus.presentation.demo.DemoDownload
import io.nexus.presentation.demo.DemoHostedFile
import io.nexus.presentation.demo.DemoPublication
import io.nexus.presentation.demo.DemoSource
import io.nexus.presentation.demo.DownloadStatus
import io.nexus.presentation.demo.ReplicaVerification
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.resources.NexusStrings
import io.nexus.ui.theme.NexusSpacing

/** Discoverable publication card used by Explore, Search, and profile lists. */
@Composable
public fun PublicationCard(
    publication: DemoPublication,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    showUploader: Boolean = true,
) {
    val strings = LocalNexusStrings.current
    NexusCard(
        modifier = modifier.fillMaxWidth(),
        onClick = onClick,
        onClickLabel = strings.openRecord(publication.title),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
        ) {
            Surface(
                shape = MaterialTheme.shapes.medium,
                color = MaterialTheme.colorScheme.surfaceContainerHigh,
                contentColor = MaterialTheme.colorScheme.primary,
            ) {
                Row(modifier = Modifier.padding(NexusSpacing.sm)) {
                    Icon(
                        imageVector = NexusIcons.File,
                        contentDescription = null,
                        modifier = Modifier.size(22.dp),
                    )
                }
            }
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = publication.title,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text =
                        if (showUploader) {
                            "${publication.uploaderDisplayName} · ${strings.sampleRecord}"
                        } else {
                            strings.sampleRecord
                        },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Icon(
                imageVector = NexusIcons.ChevronRight,
                contentDescription = null,
                modifier = Modifier.size(20.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(
            text = publication.description,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            AvailabilityBadge(availability = publication.availability)
            if (publication.unpublished) {
                UnpublishedBadge()
            }
            if (publication.ownedByViewer) {
                NexusTag(text = strings.fileOwnedByYou)
            }
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            NexusTag(text = demoCategoryLabel(publication.category, strings))
            publication.tags.take(2).forEach { tag ->
                NexusTag(text = tag)
            }
            Spacer(modifier = Modifier.weight(1f))
            Text(
                text = publication.sizeLabel,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (publication.sources.isNotEmpty()) {
            Text(
                text = sourceSummary(publication.sources, strings),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

private fun sourceSummary(
    sources: List<DemoSource>,
    strings: NexusStrings,
): String {
    val lastSeen = sources.firstOrNull()?.lastSeenLabel
    val count = strings.sourceCount(sources.size)
    return if (lastSeen == null) {
        count
    } else {
        "$count · ${strings.lastSeen(lastSeen)}"
    }
}

/** Download row with status, progress, verification, and a demo delete action. */
@Composable
public fun DownloadCard(
    download: DemoDownload,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.Top,
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = download.title,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = download.fileName,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            DownloadStatusBadge(status = download.status)
        }
        if (download.status != DownloadStatus.COMPLETE) {
            LinearProgressIndicator(
                progress = { download.progress.coerceIn(0f, 1f) },
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .height(6.dp)
                        .clip(RoundedCornerShape(3.dp)),
                color = MaterialTheme.colorScheme.primary,
                trackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
            )
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = download.transferredLabel,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                download.speedLabel?.let { speed ->
                    Text(
                        text = strings.downloadsSpeedLabel(speed),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                download.sourcePeer?.let { peer ->
                    Text(
                        text = strings.downloadsSourcePeerLabel(peer),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Text(
                    text = strings.sourceCount(download.sourceCount),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            ContentVerificationBadge(verified = download.contentVerified)
        }
        Text(
            text = downloadNote(download.status, strings),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            Text(
                modifier = Modifier.weight(1f),
                text = download.updatedLabel,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            NexusTertiaryButton(
                text = strings.downloadsDelete,
                onClick = onDelete,
                icon = NexusIcons.Trash,
            )
        }
    }
}

private fun downloadNote(
    status: DownloadStatus,
    strings: NexusStrings,
): String =
    when (status) {
        DownloadStatus.DOWNLOADING -> strings.downloadsNotVerified
        DownloadStatus.PAUSED -> strings.downloadsPausedNote
        DownloadStatus.VERIFYING -> strings.downloadsVerifyingNote
        DownloadStatus.COMPLETE -> strings.downloadsCompleteNote
        DownloadStatus.FAILED -> strings.downloadsFailedNote
    }

/** Hosted replica card. Attribution always stays with the original uploader. */
@Composable
public fun HostedFileCard(
    hosted: DemoHostedFile,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onRemove: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.Top,
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = hosted.title,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = "${strings.hostedOriginalUploader}: ${hosted.originalUploader}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            HostedStatusBadge(verification = hosted.verification)
        }
        Text(
            text = strings.hostedPublication(hosted.publicationId),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            text = strings.hostedYouHost,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        val verificationProgress = hosted.progress
        if (verificationProgress != null && hosted.verification == ReplicaVerification.VERIFYING) {
            LinearProgressIndicator(
                progress = { verificationProgress.coerceIn(0f, 1f) },
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .height(6.dp)
                        .clip(RoundedCornerShape(3.dp)),
                color = MaterialTheme.colorScheme.primary,
                trackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
            )
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                modifier = Modifier.weight(1f),
                text = hosted.sizeLabel,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = strings.hostedStorageUse(hosted.storageLabel),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                modifier = Modifier.weight(1f),
                text = if (hosted.serving) strings.hostedServing else strings.hostedNotServing,
                style = MaterialTheme.typography.bodySmall,
                color =
                    if (hosted.serving) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
            )
            Text(
                text = strings.hostedPeers(hosted.peersServed),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                modifier = Modifier.weight(1f),
                text =
                    hosted.lastVerifiedLabel
                        ?.let(strings.hostedLastVerified)
                        ?: strings.hostedNotVerified,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            hosted.lastSeenLabel?.let { lastSeen ->
                Text(
                    text = strings.hostedLastSeen(lastSeen),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (hosted.verification == ReplicaVerification.FAILED) {
            NexusInfoBanner(
                title = strings.hostedStatusFailed,
                body = strings.hostedFailedNote,
                tone = StatusTone.NEGATIVE,
                icon = NexusIcons.Error,
            )
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.xs, Alignment.End),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            when {
                hosted.verification == ReplicaVerification.VERIFYING -> {
                    // No hosting action while the copy is still being verified.
                }

                hosted.serving -> {
                    NexusTertiaryButton(
                        text = strings.hostedPause,
                        onClick = onPause,
                        icon = NexusIcons.Pause,
                    )
                }

                else -> {
                    NexusTertiaryButton(
                        text = strings.hostedResume,
                        onClick = onResume,
                        icon = NexusIcons.Play,
                    )
                }
            }
            NexusTertiaryButton(
                text = strings.hostedRemove,
                onClick = onRemove,
                icon = NexusIcons.Stop,
            )
        }
    }
}

/** One known source with its online state and last-seen label. */
@Composable
public fun SourceRow(
    source: DemoSource,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.small,
        color = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Row(
            modifier = Modifier.padding(horizontal = NexusSpacing.md, vertical = NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            Text(
                modifier = Modifier.weight(1f),
                text = source.peerLabel,
                style = io.nexus.ui.theme.NexusCodeTextStyle,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = strings.lastSeen(source.lastSeenLabel),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
            )
            SourceStatusBadge(online = source.online)
        }
    }
}

/** Small status line for network endpoints. */
@Composable
public fun BootstrapPathRow(
    label: String,
    endpoint: String,
    stateLabel: String,
    tone: StatusTone,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.small,
        color = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(
            modifier = Modifier.padding(NexusSpacing.md),
            verticalArrangement = Arrangement.spacedBy(NexusSpacing.xs),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            ) {
                Text(
                    modifier = Modifier.weight(1f),
                    text = label,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                StatusBadge(text = stateLabel, tone = tone, icon = icon)
            }
            Text(
                modifier = Modifier.widthIn(max = 520.dp),
                text = endpoint,
                style = io.nexus.ui.theme.NexusCodeTextStyle,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
