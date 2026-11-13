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
import io.nexus.presentation.demo.DemoDownload
import io.nexus.presentation.demo.DownloadStatus
import io.nexus.ui.components.ContentStateHost
import io.nexus.ui.components.ContentStatePanel
import io.nexus.ui.components.DownloadCard
import io.nexus.ui.components.NexusConfirmDialog
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusScreen
import io.nexus.ui.components.NexusSectionHeader
import io.nexus.ui.components.PreviewStateBar
import io.nexus.ui.components.StatusTone
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSpacing

/** Local download library and transfer states. */
@Composable
public fun DownloadsScreen(
    scenario: PreviewScenario,
    network: PreviewNetworkState,
    onScenarioChange: (PreviewScenario) -> Unit,
    onNetworkChange: (PreviewNetworkState) -> Unit,
    onShowMessage: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    var hidden by remember { mutableStateOf(emptySet<String>()) }
    var pendingDelete by remember { mutableStateOf<DemoDownload?>(null) }
    val visible = DemoCatalog.downloads.filterNot { it.id in hidden }
    val content = previewContentState(scenario) { visible }

    fun restore() {
        hidden = emptySet()
        onScenarioChange(PreviewScenario.READY)
    }

    NexusScreen(
        modifier = modifier,
        title = strings.downloadsTitle,
        subtitle = strings.downloadsSubtitle,
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
            body = strings.downloadsDemoNotice,
            tone = StatusTone.CAUTION,
            icon = NexusIcons.Info,
        )
        ContentStateHost(
            state = content,
            onShowData = { onScenarioChange(PreviewScenario.READY) },
            emptyTitle = strings.downloadsEmptyTitle,
            emptyBody = strings.downloadsEmptyBody,
            emptyActionLabel = strings.downloadsRestore,
            onEmptyAction = { restore() },
        ) { downloads ->
            if (downloads.isEmpty()) {
                ContentStatePanel(
                    title = strings.downloadsEmptyTitle,
                    body = strings.downloadsEmptyBody,
                    icon = NexusIcons.Downloads,
                    tone = StatusTone.NEUTRAL,
                    actionLabel = strings.downloadsRestore,
                    onAction = { restore() },
                )
            } else {
                DownloadsList(
                    downloads = downloads,
                    onDelete = { pendingDelete = it },
                )
            }
        }
    }

    val deleting = pendingDelete
    if (deleting != null) {
        NexusConfirmDialog(
            title = strings.downloadsDeleteDialogTitle,
            body = strings.downloadsDeleteDialogBody,
            confirmLabel = strings.downloadsDeleteDialogConfirm,
            onConfirm = {
                hidden = hidden + deleting.id
                pendingDelete = null
                onShowMessage(strings.downloadsDeletedDone)
            },
            onDismiss = { pendingDelete = null },
        )
    }
}

@Composable
private fun DownloadsList(
    downloads: List<DemoDownload>,
    onDelete: (DemoDownload) -> Unit,
) {
    val strings = LocalNexusStrings.current
    val active = downloads.filter { it.status != DownloadStatus.COMPLETE }
    val completed = downloads.filter { it.status == DownloadStatus.COMPLETE }

    Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg)) {
        if (active.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.sm)) {
                NexusSectionHeader(
                    title = strings.downloadsActiveSection,
                    subtitle = strings.recordsCount(active.size),
                )
                active.forEach { download ->
                    DownloadCard(
                        download = download,
                        onDelete = { onDelete(download) },
                    )
                }
            }
        }
        if (completed.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.sm)) {
                NexusSectionHeader(
                    title = strings.downloadsCompletedSection,
                    subtitle = strings.recordsCount(completed.size),
                )
                completed.forEach { download ->
                    DownloadCard(
                        download = download,
                        onDelete = { onDelete(download) },
                    )
                }
            }
        }
    }
}
