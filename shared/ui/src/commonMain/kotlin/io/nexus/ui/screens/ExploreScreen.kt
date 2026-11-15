package io.nexus.ui.screens

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.presentation.content.previewContentState
import io.nexus.presentation.demo.DemoCatalog
import io.nexus.presentation.demo.DemoCategory
import io.nexus.presentation.demo.DemoPublication
import io.nexus.presentation.demo.PublicationAvailability
import io.nexus.ui.components.AvailabilityBadge
import io.nexus.ui.components.ContentStateHost
import io.nexus.ui.components.NexusActionRow
import io.nexus.ui.components.NexusCard
import io.nexus.ui.components.NexusChoiceChip
import io.nexus.ui.components.NexusGroupLabel
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusInlineEmpty
import io.nexus.ui.components.NexusScreen
import io.nexus.ui.components.NexusSectionHeader
import io.nexus.ui.components.PreviewStateBar
import io.nexus.ui.components.PublicationCard
import io.nexus.ui.components.StatusTone
import io.nexus.ui.components.UnpublishedBadge
import io.nexus.ui.components.demoCategoryLabel
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSpacing

/** Discoverable records grouped by the canonical availability vocabulary. */
@Composable
public fun ExploreScreen(
    scenario: PreviewScenario,
    network: PreviewNetworkState,
    onScenarioChange: (PreviewScenario) -> Unit,
    onNetworkChange: (PreviewNetworkState) -> Unit,
    onOpenFile: (String) -> Unit,
    onOpenPublish: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    var category by remember { mutableStateOf<DemoCategory?>(null) }
    val publications =
        remember(category) {
            DemoCatalog.publications.filter { category == null || it.category == category }
        }
    val content = previewContentState(scenario) { publications }

    NexusScreen(
        modifier = modifier,
        title = strings.exploreTitle,
        subtitle = strings.exploreSubtitle,
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
        Text(
            text = strings.exploreCoverage,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        NexusActionRow(
            title = strings.explorePublishPreview,
            subtitle = strings.publishSubtitle,
            icon = NexusIcons.Upload,
            onClick = onOpenPublish,
        )
        NexusGroupLabel(text = strings.exploreCategories)
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .selectableGroup(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            NexusChoiceChip(
                text = strings.categoryAll,
                selected = category == null,
                onClick = { category = null },
            )
            DemoCategory.entries.forEach { option ->
                NexusChoiceChip(
                    text = demoCategoryLabel(option, strings),
                    selected = category == option,
                    onClick = { category = option },
                )
            }
        }
        ContentStateHost(
            state = content,
            onShowData = { onScenarioChange(PreviewScenario.READY) },
            emptyTitle = strings.emptyTitle,
            emptyBody = strings.emptyBody,
        ) { visible ->
            if (visible.isEmpty()) {
                NexusInlineEmpty(
                    title = strings.exploreEmptyFilterTitle,
                    body = strings.exploreEmptyFilterBody,
                    actionLabel = strings.categoryAll,
                    onAction = { category = null },
                )
            } else {
                ExploreGroups(publications = visible, onOpenFile = onOpenFile)
            }
        }
        AvailabilityGuide()
    }
}

@Composable
private fun ExploreGroups(
    publications: List<DemoPublication>,
    onOpenFile: (String) -> Unit,
) {
    val strings = LocalNexusStrings.current
    val available = publications.filter { it.availability == PublicationAvailability.AVAILABLE_NOW }
    val seen = publications.filter { it.availability == PublicationAvailability.SEEN_RECENTLY }
    val records =
        publications.filter {
            it.availability == PublicationAvailability.NO_KNOWN_SOURCE ||
                it.availability == PublicationAvailability.RECORD_ONLY
        }

    PublicationGroup(
        title = strings.exploreAvailableSection,
        publications = available,
        onOpenFile = onOpenFile,
    )
    PublicationGroup(
        title = strings.exploreSeenSection,
        publications = seen,
        onOpenFile = onOpenFile,
    )
    PublicationGroup(
        title = strings.exploreRecordSection,
        publications = records,
        onOpenFile = onOpenFile,
    )
}

@Composable
private fun PublicationGroup(
    title: String,
    publications: List<DemoPublication>,
    onOpenFile: (String) -> Unit,
) {
    val strings = LocalNexusStrings.current
    if (publications.isEmpty()) {
        return
    }
    Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.sm)) {
        NexusSectionHeader(
            title = title,
            subtitle = strings.recordsCount(publications.size),
        )
        publications.forEach { publication ->
            PublicationCard(
                publication = publication,
                onClick = { onOpenFile(publication.recordId) },
            )
        }
    }
}

@Composable
private fun AvailabilityGuide() {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = strings.exploreStatusGuideTitle,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Text(
            text = strings.fileGlobalNote,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        AvailabilityDefinition(
            availability = PublicationAvailability.AVAILABLE_NOW,
            definition = strings.availabilityAvailableNowDefinition,
        )
        AvailabilityDefinition(
            availability = PublicationAvailability.SEEN_RECENTLY,
            definition = strings.availabilitySeenRecentlyDefinition,
        )
        AvailabilityDefinition(
            availability = PublicationAvailability.NO_KNOWN_SOURCE,
            definition = strings.availabilityNoKnownSourceDefinition,
        )
        AvailabilityDefinition(
            availability = PublicationAvailability.RECORD_ONLY,
            definition = strings.availabilityRecordOnlyDefinition,
        )
        AvailabilityDefinition(
            availability = null,
            definition = strings.availabilityUnpublishedDefinition,
        )
    }
}

@Composable
private fun AvailabilityDefinition(
    availability: PublicationAvailability?,
    definition: String,
) {
    val strings = LocalNexusStrings.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
    ) {
        if (availability == null) {
            UnpublishedBadge()
        } else {
            AvailabilityBadge(availability = availability)
        }
        Text(
            modifier = Modifier.weight(1f),
            text = definition,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
