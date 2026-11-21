package io.nexus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.presentation.content.previewContentState
import io.nexus.presentation.demo.DemoCatalog
import io.nexus.ui.components.ContentStateHost
import io.nexus.ui.components.NexusActionRow
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusInlineEmpty
import io.nexus.ui.components.NexusScreen
import io.nexus.ui.components.NexusSectionHeader
import io.nexus.ui.components.PreviewStateBar
import io.nexus.ui.components.ProfileSummaryCard
import io.nexus.ui.components.PublicationCard
import io.nexus.ui.components.StatusTone
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSpacing

/** The current user's demo profile and publications. */
@Composable
public fun ProfileScreen(
    displayName: String?,
    scenario: PreviewScenario,
    network: PreviewNetworkState,
    onScenarioChange: (PreviewScenario) -> Unit,
    onNetworkChange: (PreviewNetworkState) -> Unit,
    onOpenFile: (String) -> Unit,
    onOpenNetwork: () -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    val profile =
        DemoCatalog.profile.copy(
            displayName = displayName?.takeIf(String::isNotBlank) ?: DemoCatalog.profile.displayName,
        )
    val publications = DemoCatalog.profilePublications
    val content = previewContentState(scenario) { profile to publications }

    NexusScreen(
        modifier = modifier,
        title = strings.profileTitle,
        subtitle = strings.profileSubtitle,
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
            title = strings.profileDemoBadge,
            body = strings.profileSubtitle,
            tone = StatusTone.CAUTION,
            icon = NexusIcons.Info,
        )
        ContentStateHost(
            state = content,
            onShowData = { onScenarioChange(PreviewScenario.READY) },
        ) { ready ->
            val (demoProfile, ownPublications) = ready
            Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg)) {
                ProfileSummaryCard(
                    displayName = demoProfile.displayName,
                    usernameLabel = demoProfile.usernameLabel,
                    bio = demoProfile.bio,
                    keyFingerprint = demoProfile.keyFingerprint,
                    publicationCount = demoProfile.publicationCount,
                    hostedReplicaCount = demoProfile.hostedReplicaCount,
                    joinedLabel = demoProfile.joinedLabel,
                    identityId = demoProfile.identityId,
                    recordRevision = demoProfile.recordRevision,
                    recordState = demoProfile.recordState,
                    freshness = demoProfile.freshness,
                    publishedLabel = demoProfile.publishedLabel,
                    lastCheckedLabel = demoProfile.lastCheckedLabel,
                    conflictCount = demoProfile.conflictCount,
                    usernameCollisions = demoProfile.usernameCollisions,
                    onNetworkStatus = onOpenNetwork,
                )
                Text(
                    text = strings.profileUsernameNote,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                NexusInfoBanner(
                    title = strings.profileKeyLabel,
                    body = strings.profileKeyNote,
                    tone = StatusTone.INFO,
                    icon = NexusIcons.Shield,
                )
                NexusSectionHeader(
                    title = strings.profilePublications,
                    subtitle = strings.recordsCount(ownPublications.size),
                )
                if (ownPublications.isEmpty()) {
                    NexusInlineEmpty(
                        title = strings.profileNoPublications,
                        body = strings.emptyBody,
                    )
                } else {
                    ownPublications.forEach { publication ->
                        PublicationCard(
                            publication = publication,
                            onClick = { onOpenFile(publication.recordId) },
                            showUploader = false,
                        )
                    }
                }
                NexusActionRow(
                    title = strings.profileEdit,
                    subtitle = strings.profileEditUnavailable,
                    icon = NexusIcons.User,
                    enabled = false,
                )
                NexusActionRow(
                    title = strings.profileIdentityBackup,
                    subtitle = strings.profileBackupUnavailable,
                    icon = NexusIcons.Lock,
                    enabled = false,
                )
            }
        }
    }
}
