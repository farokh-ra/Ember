package io.nexus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.presentation.content.previewContentState
import io.nexus.presentation.demo.DemoCatalog
import io.nexus.presentation.demo.DemoConnectivity
import io.nexus.presentation.demo.DemoNatStatus
import io.nexus.presentation.demo.DemoNetworkErrorCode
import io.nexus.presentation.demo.DemoNetworkSnapshot
import io.nexus.presentation.demo.DemoRelayPath
import io.nexus.ui.components.BootstrapPathRow
import io.nexus.ui.components.ContentStateHost
import io.nexus.ui.components.NexusActionRow
import io.nexus.ui.components.NexusCard
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusMetric
import io.nexus.ui.components.NexusScreen
import io.nexus.ui.components.NexusSectionHeader
import io.nexus.ui.components.PreviewStateBar
import io.nexus.ui.components.StatusBadge
import io.nexus.ui.components.StatusTone
import io.nexus.ui.components.bootstrapStateIcon
import io.nexus.ui.components.bootstrapStateLabel
import io.nexus.ui.components.bootstrapStateTone
import io.nexus.ui.components.networkStateIcon
import io.nexus.ui.components.networkStateLabel
import io.nexus.ui.components.networkStateTone
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.resources.NexusStrings
import io.nexus.ui.theme.NexusBreakpoints
import io.nexus.ui.theme.NexusSpacing

/** Global connectivity, bootstrap paths, peers, and redacted errors. */
@Composable
public fun NetworkStatusScreen(
    scenario: PreviewScenario,
    network: PreviewNetworkState,
    onScenarioChange: (PreviewScenario) -> Unit,
    onNetworkChange: (PreviewNetworkState) -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    val snapshot = DemoCatalog.network(network)
    val content = previewContentState(scenario) { snapshot }

    NexusScreen(
        modifier = modifier,
        title = strings.networkTitle,
        subtitle = strings.networkSubtitle,
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
            body = strings.networkDemoNotice,
            tone = StatusTone.CAUTION,
            icon = NexusIcons.Info,
        )
        ContentStateHost(
            state = content,
            onShowData = { onScenarioChange(PreviewScenario.READY) },
        ) { visible ->
            NetworkBody(snapshot = visible)
        }
    }
}

@Composable
private fun NetworkBody(snapshot: DemoNetworkSnapshot) {
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
                    NetworkStateSection(snapshot)
                    PathAndRelaySection(snapshot)
                    BootstrapSection(snapshot)
                    ErrorsSection(snapshot)
                }
                Column(
                    modifier = Modifier.widthIn(max = 380.dp),
                    verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                ) {
                    DiagnosticsSection()
                    NexusInfoBanner(
                        title = strings.networkSeparateTitle,
                        body = strings.networkSeparateBody,
                        tone = StatusTone.INFO,
                        icon = NexusIcons.Info,
                    )
                }
            }
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg)) {
                NetworkStateSection(snapshot)
                PathAndRelaySection(snapshot)
                BootstrapSection(snapshot)
                ErrorsSection(snapshot)
                DiagnosticsSection()
                NexusInfoBanner(
                    title = strings.networkSeparateTitle,
                    body = strings.networkSeparateBody,
                    tone = StatusTone.INFO,
                    icon = NexusIcons.Info,
                )
            }
        }
    }
}

@Composable
private fun NetworkStateSection(snapshot: DemoNetworkSnapshot) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(title = strings.networkNodeState)
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
            verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
        ) {
            StatusBadge(
                text = networkStateLabel(snapshot.state, strings),
                tone = networkStateTone(snapshot.state),
                icon = networkStateIcon(snapshot.state),
            )
            Text(
                modifier = Modifier.weight(1f),
                text = networkStateBody(snapshot.state, strings),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            NexusMetric(
                modifier = Modifier.weight(1f),
                value = snapshot.peerCount.toString(),
                label = strings.networkPeerCount,
                icon = NexusIcons.Globe,
            )
            NexusMetric(
                modifier = Modifier.weight(1f),
                value = strings.sampleRecord,
                label = strings.networkStateLabel,
                icon = NexusIcons.Info,
            )
        }
    }
}

@Composable
private fun PathAndRelaySection(snapshot: DemoNetworkSnapshot) {
    val strings = LocalNexusStrings.current
    Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg)) {
        NexusCard(modifier = Modifier.fillMaxWidth()) {
            NexusSectionHeader(
                title = strings.networkPathTitle,
                subtitle = strings.networkPathBody,
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
                verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
            ) {
                StatusBadge(
                    text = connectivityLabel(snapshot.connectivity, strings),
                    tone = connectivityTone(snapshot.connectivity),
                    icon = connectivityIcon(snapshot.connectivity),
                )
                Text(
                    modifier = Modifier.weight(1f),
                    text = snapshot.troubleshootingHint,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            ) {
                NexusMetric(
                    modifier = Modifier.weight(1f),
                    value = natLabel(snapshot.natStatus, strings),
                    label = strings.networkNatLabel,
                    icon = NexusIcons.Globe,
                )
                NexusMetric(
                    modifier = Modifier.weight(1f),
                    value =
                        if (snapshot.relayFallbackEnabled) {
                            strings.networkRelayFallbackOn
                        } else {
                            strings.networkRelayFallbackOff
                        },
                    label = strings.networkRelayTitle,
                    icon = NexusIcons.Refresh,
                )
            }
            snapshot.natPublicAddress?.let { address ->
                Text(
                    text = address,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        NexusCard(modifier = Modifier.fillMaxWidth()) {
            NexusSectionHeader(
                title = strings.networkRelayTitle,
                subtitle = strings.networkRelayBody,
            )
            if (snapshot.relayPaths.isEmpty()) {
                Text(
                    text = strings.networkRelayNoRelays,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                snapshot.relayPaths.forEach { path -> RelayPathRow(path) }
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            ) {
                NexusMetric(
                    modifier = Modifier.weight(1f),
                    value = snapshot.directConnections.toString(),
                    label = strings.networkDirectConnections,
                    icon = NexusIcons.Globe,
                )
                NexusMetric(
                    modifier = Modifier.weight(1f),
                    value = snapshot.relayedConnections.toString(),
                    label = strings.networkRelayedConnections,
                    icon = NexusIcons.Refresh,
                )
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            ) {
                NexusMetric(
                    modifier = Modifier.weight(1f),
                    value = snapshot.fallbackAttempts.toString(),
                    label = strings.networkFallbackAttempts,
                    icon = NexusIcons.Clock,
                )
                NexusMetric(
                    modifier = Modifier.weight(1f),
                    value = snapshot.fallbackSuccesses.toString(),
                    label = strings.networkFallbackSuccesses,
                    icon = NexusIcons.Check,
                )
            }
            Text(
                text =
                    "${strings.networkRelayDataLimit}: " +
                        (snapshot.relayDataLimitLabel ?: strings.networkRelayLimitUnknown),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text =
                    "${strings.networkRelayDurationLimit}: " +
                        (snapshot.relayDurationLimitLabel ?: strings.networkRelayLimitUnknown),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun RelayPathRow(path: DemoRelayPath) {
    val strings = LocalNexusStrings.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = path.label,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = path.endpoint,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = path.detail,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        StatusBadge(
            text = bootstrapStateLabel(path.state, strings),
            tone = bootstrapStateTone(path.state),
            icon = bootstrapStateIcon(path.state),
        )
    }
}

@Composable
private fun BootstrapSection(snapshot: DemoNetworkSnapshot) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(
            title = strings.networkBootstrapTitle,
            subtitle = strings.networkBootstrapBody,
        )
        snapshot.bootstrapPaths.forEach { path ->
            BootstrapPathRow(
                label = path.label,
                endpoint = path.endpoint,
                stateLabel = bootstrapStateLabel(path.state, strings),
                tone = bootstrapStateTone(path.state),
                icon = bootstrapStateIcon(path.state),
            )
        }
    }
}

@Composable
private fun ErrorsSection(snapshot: DemoNetworkSnapshot) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(
            title = strings.networkRecentErrors,
            subtitle = strings.networkRecentErrorsBody,
        )
        if (snapshot.recentErrors.isEmpty()) {
            Text(
                text = strings.networkNoErrors,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            snapshot.recentErrors.forEach { error ->
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
                ) {
                    Text(
                        modifier = Modifier.weight(1f),
                        text = networkErrorText(error.code, strings),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Text(
                        text = error.occurredLabel,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun DiagnosticsSection() {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(title = strings.networkDiagnosticsTitle)
        Text(
            text = strings.networkDiagnosticsBody,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        NexusActionRow(
            title = strings.networkExport,
            subtitle = strings.networkExportUnavailable,
            icon = NexusIcons.File,
            enabled = false,
        )
    }
}

private fun connectivityLabel(
    connectivity: DemoConnectivity,
    strings: NexusStrings,
): String =
    when (connectivity) {
        DemoConnectivity.DIRECT -> strings.networkConnectivityDirect
        DemoConnectivity.RELAY -> strings.networkConnectivityRelay
        DemoConnectivity.CONNECTING -> strings.networkConnectivityConnecting
        DemoConnectivity.NO_PEERS -> strings.networkConnectivityNoPeers
        DemoConnectivity.DISABLED -> strings.networkConnectivityDisabled
    }

private fun connectivityTone(connectivity: DemoConnectivity): StatusTone =
    when (connectivity) {
        DemoConnectivity.DIRECT -> StatusTone.AVAILABLE

        DemoConnectivity.RELAY -> StatusTone.INFO

        DemoConnectivity.CONNECTING -> StatusTone.CAUTION

        DemoConnectivity.NO_PEERS,
        DemoConnectivity.DISABLED,
        -> StatusTone.NEUTRAL
    }

private fun connectivityIcon(connectivity: DemoConnectivity): ImageVector =
    when (connectivity) {
        DemoConnectivity.DIRECT -> NexusIcons.Check
        DemoConnectivity.RELAY -> NexusIcons.Refresh
        DemoConnectivity.CONNECTING -> NexusIcons.Clock
        DemoConnectivity.NO_PEERS -> NexusIcons.Search
        DemoConnectivity.DISABLED -> NexusIcons.Info
    }

private fun natLabel(
    status: DemoNatStatus,
    strings: NexusStrings,
): String =
    when (status) {
        DemoNatStatus.PUBLIC -> strings.networkNatPublic
        DemoNatStatus.PRIVATE -> strings.networkNatPrivate
        DemoNatStatus.UNKNOWN -> strings.networkNatUnknown
        DemoNatStatus.DISABLED -> strings.networkNatDisabled
    }

private fun networkStateBody(
    state: PreviewNetworkState,
    strings: NexusStrings,
): String =
    when (state) {
        PreviewNetworkState.OFFLINE -> strings.networkOfflineBody
        PreviewNetworkState.CONNECTING -> strings.networkConnectingBody
        PreviewNetworkState.ONLINE -> strings.networkOnlineBody
        PreviewNetworkState.DEGRADED -> strings.networkDegradedBody
    }

private fun networkErrorText(
    code: DemoNetworkErrorCode,
    strings: NexusStrings,
): String =
    when (code) {
        DemoNetworkErrorCode.NO_USABLE_PATH -> strings.networkErrorNoUsablePath
        DemoNetworkErrorCode.BOOTSTRAP_UNREACHABLE -> strings.networkErrorBootstrapUnreachable
        DemoNetworkErrorCode.PEER_TIMEOUT -> strings.networkErrorPeerTimeout
        DemoNetworkErrorCode.RELAY_UNAVAILABLE -> strings.networkErrorRelayUnavailable
        DemoNetworkErrorCode.RELAY_RESERVATION_FAILED -> strings.networkErrorRelayReservation
    }
