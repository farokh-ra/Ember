package io.nexus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import io.nexus.presentation.demo.DemoHostedFile
import io.nexus.presentation.demo.ReplicaVerification
import io.nexus.ui.components.ContentStateHost
import io.nexus.ui.components.ContentStatePanel
import io.nexus.ui.components.HostedFileCard
import io.nexus.ui.components.NexusConfirmDialog
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusScreen
import io.nexus.ui.components.NexusSectionHeader
import io.nexus.ui.components.PreviewStateBar
import io.nexus.ui.components.StatusTone
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSpacing

/** Files the user explicitly chose to replicate and serve. */
@Composable
public fun HostedFilesScreen(
    scenario: PreviewScenario,
    network: PreviewNetworkState,
    onScenarioChange: (PreviewScenario) -> Unit,
    onNetworkChange: (PreviewNetworkState) -> Unit,
    onShowMessage: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    var hidden by remember { mutableStateOf(emptySet<String>()) }
    var overrides by remember { mutableStateOf(emptyMap<String, ReplicaVerification>()) }
    var pendingRemove by remember { mutableStateOf<DemoHostedFile?>(null) }
    val visible =
        DemoCatalog.hostedFiles
            .filterNot { it.id in hidden }
            .map { hosted ->
                val verification = overrides[hosted.id] ?: hosted.verification
                hosted.copy(
                    verification = verification,
                    serving = verification == ReplicaVerification.VERIFIED,
                    progress = if (verification == ReplicaVerification.VERIFIED) 1f else hosted.progress,
                )
            }
    val content = previewContentState(scenario) { visible }

    fun restore() {
        hidden = emptySet()
        overrides = emptyMap()
        onScenarioChange(PreviewScenario.READY)
    }

    NexusScreen(
        modifier = modifier,
        title = strings.hostedTitle,
        subtitle = strings.hostedSubtitle,
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
            body = strings.hostedDemoNotice,
            tone = StatusTone.CAUTION,
            icon = NexusIcons.Info,
        )
        NexusInfoBanner(
            title = strings.hostedHowTitle,
            body = strings.hostedHowBody,
            tone = StatusTone.INFO,
            icon = NexusIcons.Hosted,
        )
        ContentStateHost(
            state = content,
            onShowData = { onScenarioChange(PreviewScenario.READY) },
            emptyTitle = strings.hostedEmptyTitle,
            emptyBody = strings.hostedEmptyBody,
            emptyActionLabel = strings.hostedRestore,
            onEmptyAction = { restore() },
        ) { hostedFiles ->
            if (hostedFiles.isEmpty()) {
                ContentStatePanel(
                    title = strings.hostedEmptyTitle,
                    body = strings.hostedEmptyBody,
                    icon = NexusIcons.Hosted,
                    tone = StatusTone.NEUTRAL,
                    actionLabel = strings.hostedRestore,
                    onAction = { restore() },
                )
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.sm)) {
                    NexusSectionHeader(
                        title = strings.profileHostedReplicas,
                        subtitle = strings.recordsCount(hostedFiles.size),
                    )
                    hostedFiles.forEach { hosted ->
                        HostedFileCard(
                            hosted = hosted,
                            onPause = {
                                overrides = overrides + (hosted.id to ReplicaVerification.PAUSED)
                                onShowMessage(strings.hostedPausedDone)
                            },
                            onResume = {
                                overrides = overrides + (hosted.id to ReplicaVerification.VERIFIED)
                                onShowMessage(strings.hostedResumedDone)
                            },
                            onRemove = { pendingRemove = hosted },
                        )
                    }
                }
            }
        }
    }

    val removing = pendingRemove
    if (removing != null) {
        NexusConfirmDialog(
            title = strings.hostedRemoveDialogTitle,
            body = strings.hostedRemoveDialogBody,
            confirmLabel = strings.hostedRemoveDialogConfirm,
            onConfirm = {
                hidden = hidden + removing.id
                overrides = overrides - removing.id
                pendingRemove = null
                onShowMessage(strings.hostedRemovedDone)
            },
            onDismiss = { pendingRemove = null },
        )
    }
}
