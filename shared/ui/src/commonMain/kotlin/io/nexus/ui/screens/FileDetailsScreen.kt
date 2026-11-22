package io.nexus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.presentation.content.previewContentState
import io.nexus.presentation.demo.DemoCatalog
import io.nexus.presentation.demo.DemoPreviewKind
import io.nexus.presentation.demo.DemoPreviewReason
import io.nexus.presentation.demo.DemoPublication
import io.nexus.presentation.demo.PublicationAvailability
import io.nexus.ui.components.AvailabilityBadge
import io.nexus.ui.components.ContentStateHost
import io.nexus.ui.components.ContentStatePanel
import io.nexus.ui.components.ContentVerificationBadge
import io.nexus.ui.components.NexusActionRow
import io.nexus.ui.components.NexusCard
import io.nexus.ui.components.NexusConfirmDialog
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusKeyValue
import io.nexus.ui.components.NexusScreen
import io.nexus.ui.components.NexusSectionHeader
import io.nexus.ui.components.NexusTag
import io.nexus.ui.components.PreviewStateBar
import io.nexus.ui.components.RecordVerificationBadge
import io.nexus.ui.components.ServingBadge
import io.nexus.ui.components.SourceRow
import io.nexus.ui.components.StatusBadge
import io.nexus.ui.components.StatusTone
import io.nexus.ui.components.UnpublishedBadge
import io.nexus.ui.components.demoCategoryLabel
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.resources.NexusStrings
import io.nexus.ui.theme.NexusBreakpoints
import io.nexus.ui.theme.NexusSpacing

/** Details for one publication record with all availability and verification states. */
@Composable
public fun FileDetailsScreen(
    recordId: String,
    scenario: PreviewScenario,
    network: PreviewNetworkState,
    onScenarioChange: (PreviewScenario) -> Unit,
    onNetworkChange: (PreviewNetworkState) -> Unit,
    onOpenProfile: () -> Unit,
    onShowMessage: (String) -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    val publication = remember(recordId) { DemoCatalog.publication(recordId) }
    var showUnpublishDialog by remember { mutableStateOf(false) }

    if (publication == null) {
        NexusScreen(
            modifier = modifier,
            title = strings.fileDetailsTitle,
            onBack = onBack,
        ) {
            ContentStatePanel(
                title = strings.fileNotFoundTitle,
                body = strings.fileNotFoundBody,
                icon = NexusIcons.File,
                tone = StatusTone.NEUTRAL,
                actionLabel = strings.fileBackToExplore,
                onAction = onBack,
            )
        }
        return
    }

    val content = previewContentState(scenario) { publication }

    NexusScreen(
        modifier = modifier,
        title = publication.title,
        subtitle = "${publication.uploaderDisplayName} · ${strings.sampleRecord}",
        onBack = onBack,
        previewControls = {
            PreviewStateBar(
                scenario = scenario,
                onScenarioChange = onScenarioChange,
                network = network,
                onNetworkChange = onNetworkChange,
            )
        },
    ) {
        NexusInfoBanner(
            title = strings.demoContent,
            body = strings.exploreDemoNotice,
            tone = StatusTone.CAUTION,
            icon = NexusIcons.Info,
        )
        if (publication.unpublished) {
            NexusInfoBanner(
                title = strings.fileUnpublishedWarning,
                body = strings.availabilityUnpublishedDefinition,
                tone = StatusTone.CAUTION,
                icon = NexusIcons.Warning,
            )
        }
        ContentStateHost(
            state = content,
            onShowData = { onScenarioChange(PreviewScenario.READY) },
        ) { visible ->
            FileDetailsBody(
                publication = visible,
                onOpenProfile = onOpenProfile,
                onUnpublish = { showUnpublishDialog = true },
            )
        }
    }

    if (showUnpublishDialog) {
        NexusConfirmDialog(
            title = strings.fileUnpublishDialogTitle,
            body = strings.fileUnpublishDialogBody,
            confirmLabel = strings.fileUnpublishDialogConfirm,
            onConfirm = {
                showUnpublishDialog = false
                onShowMessage(strings.fileUnpublishDone)
            },
            onDismiss = { showUnpublishDialog = false },
        )
    }
}

@Composable
private fun FileDetailsBody(
    publication: DemoPublication,
    onOpenProfile: () -> Unit,
    onUnpublish: () -> Unit,
) {
    val strings = LocalNexusStrings.current
    BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
        val expanded = maxWidth >= NexusBreakpoints.expanded
        if (expanded) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                verticalAlignment = androidx.compose.ui.Alignment.Top,
            ) {
                Column(
                    modifier = Modifier.weight(1f),
                    verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                ) {
                    AvailabilitySection(publication)
                    SourcesSection(publication)
                    VerificationSection(publication)
                    PreviewSection(publication)
                    MetadataSection(publication)
                    DescriptionSection(publication)
                }
                Column(
                    modifier = Modifier.widthIn(max = 340.dp),
                    verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                ) {
                    ActionsSection(
                        publication = publication,
                        onOpenProfile = onOpenProfile,
                        onUnpublish = onUnpublish,
                    )
                }
            }
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg)) {
                AvailabilitySection(publication)
                ActionsSection(
                    publication = publication,
                    onOpenProfile = onOpenProfile,
                    onUnpublish = onUnpublish,
                )
                SourcesSection(publication)
                VerificationSection(publication)
                PreviewSection(publication)
                MetadataSection(publication)
                DescriptionSection(publication)
            }
        }
    }
}

@Composable
private fun AvailabilitySection(publication: DemoPublication) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(title = strings.fileSectionAvailability)
        Row(horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm)) {
            AvailabilityBadge(availability = publication.availability)
            ServingBadge(serving = publication.serving)
            if (publication.unpublished) {
                UnpublishedBadge()
            }
            if (publication.ownedByViewer) {
                NexusTag(text = strings.fileOwnedByYou)
            }
        }
        Text(
            text = availabilityDefinition(publication.availability, strings),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text =
                if (publication.serving) {
                    strings.fileServingBody
                } else {
                    strings.fileNotServingBody
                },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (publication.unpublished) {
            NexusInfoBanner(
                title = strings.fileUnpublish,
                body = strings.fileUnpublishedWarning,
                tone = StatusTone.CAUTION,
                icon = NexusIcons.Warning,
            )
        }
        Text(
            text = strings.fileGlobalNote,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun SourcesSection(publication: DemoPublication) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(
            title = strings.fileSectionSources,
            subtitle = strings.sourceCount(publication.sources.size),
        )
        if (publication.sources.isEmpty()) {
            Text(
                text = strings.fileNoSources,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = strings.fileNoSourcesBody,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            publication.sources.forEach { source ->
                SourceRow(source = source)
            }
        }
    }
}

@Composable
private fun VerificationSection(publication: DemoPublication) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(title = strings.fileSectionVerification)
        RecordVerificationBadge(verified = publication.recordVerified)
        Text(
            text =
                if (publication.recordVerified) {
                    strings.fileSignatureVerifiedBody
                } else {
                    strings.fileSignatureUnverifiedBody
                },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        ContentVerificationBadge(verified = publication.contentVerified)
        Text(
            text =
                if (publication.contentVerified) {
                    strings.fileContentVerifiedBody
                } else {
                    strings.fileContentNotVerifiedBody
                },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        NexusKeyValue(
            label = strings.fileSignatureStatusLabel,
            value =
                if (publication.recordVerified) {
                    strings.fileSignatureVerified
                } else {
                    strings.fileSignatureUnverified
                },
        )
        NexusKeyValue(
            label = strings.fileServingStatusLabel,
            value =
                if (publication.serving) {
                    strings.fileServingNow
                } else {
                    strings.fileNotServing
                },
        )
        NexusKeyValue(
            label = strings.fileAvailabilityStatusLabel,
            value = availabilityLabel(publication.availability, strings),
        )
        Text(
            text = strings.fileVerificationSeparate,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun PreviewSection(publication: DemoPublication) {
    val strings = LocalNexusStrings.current
    val preview = publication.preview
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(title = strings.fileSectionPreview)
        StatusBadge(
            text =
                if (preview.supported) {
                    strings.filePreviewSupported
                } else {
                    strings.filePreviewUnsupported
                },
            tone = if (preview.supported) StatusTone.INFO else StatusTone.NEUTRAL,
            icon = NexusIcons.File,
        )
        NexusKeyValue(
            label = strings.filePreviewKindLabel,
            value = previewKindLabel(preview.kind, strings),
        )
        Text(
            text = previewReason(preview.reason, strings),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = strings.filePreviewNoAutoExecute,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun MetadataSection(publication: DemoPublication) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(title = strings.fileSectionMetadata)
        BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
            val columns = if (maxWidth >= 560.dp) 2 else 1
            val entries =
                listOf(
                    strings.fileSize to publication.sizeLabel,
                    strings.fileCategory to demoCategoryLabel(publication.category, strings),
                    strings.filePublished to publication.publishedLabel,
                    strings.fileUploader to publication.uploaderDisplayName,
                    strings.fileRecordVersion to publication.recordVersion.toString(),
                )
            entries.chunked(columns).forEach { rowEntries ->
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                ) {
                    rowEntries.forEach { (label, value) ->
                        NexusKeyValue(
                            modifier = Modifier.weight(1f),
                            label = label,
                            value = value,
                        )
                    }
                    if (rowEntries.size < columns) {
                        Spacer(modifier = Modifier.weight(1f))
                    }
                }
            }
        }
        NexusKeyValue(
            label = strings.fileContentId,
            value = publication.contentId,
            monospace = true,
        )
        if (publication.tags.isNotEmpty()) {
            Text(
                text = strings.fileTags,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            ) {
                publication.tags.forEach { tag ->
                    NexusTag(text = tag)
                }
            }
        }
    }
}

@Composable
private fun DescriptionSection(publication: DemoPublication) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(title = strings.fileDescription)
        Text(
            text = publication.description,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun ActionsSection(
    publication: DemoPublication,
    onOpenProfile: () -> Unit,
    onUnpublish: () -> Unit,
) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(title = strings.fileSectionActions)
        NexusActionRow(
            title = strings.fileDownload,
            subtitle = strings.fileDownloadUnavailable,
            icon = NexusIcons.Downloads,
            enabled = false,
        )
        NexusActionRow(
            title = strings.fileHost,
            subtitle = strings.fileHostUnavailable,
            icon = NexusIcons.Hosted,
            enabled = false,
        )
        Text(
            text = strings.fileHostExplain,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (publication.ownedByViewer && !publication.unpublished) {
            NexusActionRow(
                title = strings.fileUnpublish,
                subtitle = strings.fileUnpublishDemo,
                icon = NexusIcons.Trash,
                onClick = onUnpublish,
            )
        } else if (publication.ownedByViewer) {
            NexusActionRow(
                title = strings.fileUnpublish,
                subtitle = strings.fileAlreadyUnpublished,
                icon = NexusIcons.Trash,
                enabled = false,
            )
        }
        if (publication.ownedByViewer) {
            NexusSectionHeader(title = strings.fileUnpublishLimitsTitle)
            Text(
                text = strings.fileUnpublishLimitsBody,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = "• ${strings.fileUnpublishStopsServing}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = "• ${strings.fileUnpublishRetainsLocal}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = "• ${strings.fileUnpublishNotGlobal}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (publication.ownedByViewer) {
            NexusActionRow(
                title = strings.fileOpenProfile,
                icon = NexusIcons.User,
                onClick = onOpenProfile,
            )
        } else {
            NexusActionRow(
                title = strings.fileOpenProfile,
                subtitle = strings.fileUploaderProfileUnavailable,
                icon = NexusIcons.User,
                enabled = false,
            )
        }
    }
}

private fun availabilityDefinition(
    availability: PublicationAvailability,
    strings: NexusStrings,
): String =
    when (availability) {
        PublicationAvailability.AVAILABLE_NOW -> strings.availabilityAvailableNowDefinition
        PublicationAvailability.SEEN_RECENTLY -> strings.availabilitySeenRecentlyDefinition
        PublicationAvailability.NO_KNOWN_SOURCE -> strings.availabilityNoKnownSourceDefinition
        PublicationAvailability.RECORD_ONLY -> strings.availabilityRecordOnlyDefinition
    }

private fun availabilityLabel(
    availability: PublicationAvailability,
    strings: NexusStrings,
): String =
    when (availability) {
        PublicationAvailability.AVAILABLE_NOW -> strings.availabilityAvailableNow
        PublicationAvailability.SEEN_RECENTLY -> strings.availabilitySeenRecently
        PublicationAvailability.NO_KNOWN_SOURCE -> strings.availabilityNoKnownSource
        PublicationAvailability.RECORD_ONLY -> strings.availabilityRecordOnly
    }

private fun previewKindLabel(
    kind: DemoPreviewKind,
    strings: NexusStrings,
): String =
    when (kind) {
        DemoPreviewKind.TEXT -> strings.filePreviewKindText
        DemoPreviewKind.IMAGE -> strings.filePreviewKindImage
        DemoPreviewKind.NONE -> strings.filePreviewKindNone
    }

private fun previewReason(
    reason: DemoPreviewReason,
    strings: NexusStrings,
): String =
    when (reason) {
        DemoPreviewReason.PLAIN_TEXT -> strings.filePreviewReasonPlainText
        DemoPreviewReason.RASTER_IMAGE -> strings.filePreviewReasonRasterImage
        DemoPreviewReason.UNSUPPORTED_FORMAT -> strings.filePreviewReasonUnsupported
    }
