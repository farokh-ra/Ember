package io.nexus.ui.components

import androidx.compose.ui.graphics.vector.ImageVector
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.NexusStrings

/** Canonical label for a simulated global network state. */
public fun networkStateLabel(
    state: PreviewNetworkState,
    strings: NexusStrings,
): String =
    when (state) {
        PreviewNetworkState.OFFLINE -> strings.networkOffline
        PreviewNetworkState.CONNECTING -> strings.networkConnecting
        PreviewNetworkState.ONLINE -> strings.networkOnline
        PreviewNetworkState.DEGRADED -> strings.networkDegraded
    }

/** Icon paired with every network state so colour is never the only signal. */
public fun networkStateIcon(state: PreviewNetworkState): ImageVector =
    when (state) {
        PreviewNetworkState.OFFLINE -> NexusIcons.Error
        PreviewNetworkState.CONNECTING -> NexusIcons.Refresh
        PreviewNetworkState.ONLINE -> NexusIcons.Check
        PreviewNetworkState.DEGRADED -> NexusIcons.Warning
    }

/** Semantic tone for a network state. */
public fun networkStateTone(state: PreviewNetworkState): StatusTone =
    when (state) {
        PreviewNetworkState.OFFLINE -> StatusTone.NEGATIVE
        PreviewNetworkState.CONNECTING -> StatusTone.INFO
        PreviewNetworkState.ONLINE -> StatusTone.AVAILABLE
        PreviewNetworkState.DEGRADED -> StatusTone.CAUTION
    }

/** Label for the selected content-state preview. */
public fun previewScenarioLabel(
    scenario: PreviewScenario,
    strings: NexusStrings,
): String =
    when (scenario) {
        PreviewScenario.READY -> strings.previewReady
        PreviewScenario.LOADING -> strings.previewLoading
        PreviewScenario.EMPTY -> strings.previewEmpty
        PreviewScenario.OFFLINE -> strings.previewOffline
        PreviewScenario.ERROR -> strings.previewError
    }
