package io.nexus.ui

import io.nexus.presentation.NexusAppState
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.presentation.ThemeMode
import io.nexus.presentation.navigation.AppDestination
import io.nexus.ui.theme.InMemoryThemeStore
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class NexusAppControllerTest {
    @Test
    fun initialThemeComesFromTheStore() {
        val controller = NexusAppController(InMemoryThemeStore(ThemeMode.DARK))

        assertEquals(ThemeMode.DARK, controller.state.themeMode)
    }

    @Test
    fun themeSelectionIsPersisted() {
        val store = InMemoryThemeStore(ThemeMode.SYSTEM)
        val controller = NexusAppController(store)

        controller.setThemeMode(ThemeMode.LIGHT)

        assertEquals(ThemeMode.LIGHT, controller.state.themeMode)
        assertEquals(ThemeMode.LIGHT, store.load())
    }

    @Test
    fun onboardingStartsTheDemoOnExplore() {
        val controller = NexusAppController(InMemoryThemeStore())

        controller.startDemo("  Robin Chen  ")

        assertEquals(AppDestination.Explore, controller.state.destination)
        assertEquals("Robin Chen", controller.state.demoDisplayName)
        assertTrue(controller.state.demoMode)
        assertFalse(controller.state.canGoBack)
    }

    @Test
    fun navigationPushAndBackArePlatformFree() {
        val controller = NexusAppController(InMemoryThemeStore())
        controller.startDemo()

        controller.navigateTo(AppDestination.FileDetails("nx-demo"))
        assertTrue(controller.state.canGoBack)
        assertEquals(
            AppDestination.FileDetails("nx-demo"),
            controller.state.destination,
        )

        controller.back()
        assertEquals(AppDestination.Explore, controller.state.destination)
        assertFalse(controller.state.canGoBack)
    }

    @Test
    fun selectingASectionResetsItsBackStack() {
        val controller = NexusAppController(InMemoryThemeStore())
        controller.startDemo()
        controller.navigateTo(AppDestination.FileDetails("nx-demo"))

        controller.selectSection(AppDestination.Downloads)

        assertEquals(AppDestination.Downloads, controller.state.destination)
        assertEquals(AppDestination.Downloads, controller.state.activeSection)
        assertFalse(controller.state.canGoBack)
    }

    @Test
    fun previewControlsArePrototypeOnlyState() {
        val controller = NexusAppController(InMemoryThemeStore())

        controller.setPreviewScenario(PreviewScenario.ERROR)
        controller.setPreviewNetwork(PreviewNetworkState.OFFLINE)

        assertEquals(PreviewScenario.ERROR, controller.state.previewScenario)
        assertEquals(PreviewNetworkState.OFFLINE, controller.state.previewNetwork)
    }

    @Test
    fun snapshotTokensRestoreTheWholeControllerState() {
        val controller = NexusAppController(InMemoryThemeStore(ThemeMode.DARK))
        controller.startDemo("Robin Chen")
        controller.navigateTo(AppDestination.FileDetails("nx-demo"))
        controller.setPreviewScenario(PreviewScenario.OFFLINE)
        controller.setPreviewNetwork(PreviewNetworkState.DEGRADED)
        controller.setThemeMode(ThemeMode.LIGHT)

        val restoredState = NexusAppState.fromPersistenceTokens(controller.snapshotTokens())

        assertEquals(controller.state, restoredState)
    }

    @Test
    fun invalidSnapshotTokensFallBackSafely() {
        assertNull(NexusAppState.fromPersistenceTokens(listOf("broken")))
    }

    @Test
    fun inMemoryStoreStartsAtTheRequestedModeAndKeepsTheLastValue() {
        val store = InMemoryThemeStore(ThemeMode.DARK)

        assertEquals(ThemeMode.DARK, store.load())

        store.save(ThemeMode.SYSTEM)
        assertEquals(ThemeMode.SYSTEM, store.load())
    }
}
