package io.nexus.ui.components

import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.ui.resources.EnglishNexusStrings
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class StateLabelsTest {
    private val strings = EnglishNexusStrings

    @Test
    fun everyNetworkStateHasADistinctTextLabel() {
        val labels = PreviewNetworkState.entries.map { networkStateLabel(it, strings) }

        assertEquals(PreviewNetworkState.entries.size, labels.toSet().size)
        assertTrue(labels.all(String::isNotBlank))
    }

    @Test
    fun everyNetworkStateHasADistinctIcon() {
        val icons = PreviewNetworkState.entries.map { networkStateIcon(it).name }

        assertEquals(PreviewNetworkState.entries.size, icons.toSet().size)
    }

    @Test
    fun networkTonesMatchProductMeaning() {
        assertEquals(StatusTone.NEGATIVE, networkStateTone(PreviewNetworkState.OFFLINE))
        assertEquals(StatusTone.INFO, networkStateTone(PreviewNetworkState.CONNECTING))
        assertEquals(StatusTone.AVAILABLE, networkStateTone(PreviewNetworkState.ONLINE))
        assertEquals(StatusTone.CAUTION, networkStateTone(PreviewNetworkState.DEGRADED))
    }

    @Test
    fun everyPreviewScenarioHasADistinctTextLabel() {
        val labels = PreviewScenario.entries.map { previewScenarioLabel(it, strings) }

        assertEquals(PreviewScenario.entries.size, labels.toSet().size)
        assertTrue(labels.all(String::isNotBlank))
    }

    @Test
    fun navigationLabelsAreAvailableForEveryPrimaryDestination() {
        val destinations =
            listOf(
                io.nexus.presentation.navigation.AppDestination.Explore,
                io.nexus.presentation.navigation.AppDestination.Search,
                io.nexus.presentation.navigation.AppDestination.Downloads,
                io.nexus.presentation.navigation.AppDestination.HostedFiles,
                io.nexus.presentation.navigation.AppDestination.Settings,
            )

        val labels = destinations.map { navigationLabel(it, strings) }

        assertEquals(destinations.size, labels.toSet().size)
        assertTrue(labels.all(String::isNotBlank))
    }
}
