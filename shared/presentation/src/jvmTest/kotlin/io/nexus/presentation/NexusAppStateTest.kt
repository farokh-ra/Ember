package io.nexus.presentation

import io.nexus.presentation.demo.DemoCatalog
import io.nexus.presentation.navigation.AppDestination
import io.nexus.presentation.navigation.NavigationState
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class NexusAppStateTest {
    @Test
    fun initialFirstLaunchStateStartsAtWelcome() {
        val state = NexusAppState.initial(themeMode = ThemeMode.DARK)

        assertEquals(AppDestination.Welcome, state.destination)
        assertEquals(ThemeMode.DARK, state.themeMode)
        assertEquals(PreviewScenario.READY, state.previewScenario)
        assertEquals(PreviewNetworkState.ONLINE, state.previewNetwork)
        assertFalse(state.demoMode)
        assertNull(state.demoDisplayName)
    }

    @Test
    fun completingDemoOnboardingResetsToExplore() {
        val state =
            NexusAppState
                .initial()
                .navigateTo(AppDestination.IdentitySetup)
                .completeDemoOnboarding("  Sam Rivera  ")

        assertEquals(AppDestination.Explore, state.destination)
        assertEquals(listOf(AppDestination.Explore), state.navigation.backStack)
        assertTrue(state.demoMode)
        assertEquals("Sam Rivera", state.demoDisplayName)
    }

    @Test
    fun defaultDemoProfileIsUsedWhenOnboardingIsSkipped() {
        val state = NexusAppState.initial().completeDemoOnboarding()

        assertEquals(DemoCatalog.profile.displayName, state.demoDisplayName)
    }

    @Test
    fun sectionSelectionResetsToTheSectionRoot() {
        val explore =
            NexusAppState
                .initial(firstLaunch = false)
                .navigateTo(AppDestination.FileDetails("nx-demo"))
        val downloads = explore.selectSection(AppDestination.Downloads)

        assertEquals(AppDestination.Downloads, downloads.destination)
        assertEquals(downloads, downloads.back())
    }

    @Test
    fun rootBackIsANoOp() {
        val state = NexusAppState.initial(firstLaunch = false)

        assertEquals(state, state.back())
    }

    @Test
    fun themeAndPreviewChangesAreRetained() {
        val state =
            NexusAppState
                .initial()
                .setThemeMode(ThemeMode.LIGHT)
                .setPreviewScenario(PreviewScenario.ERROR)
                .setPreviewNetwork(PreviewNetworkState.DEGRADED)

        assertEquals(ThemeMode.LIGHT, state.themeMode)
        assertEquals(PreviewScenario.ERROR, state.previewScenario)
        assertEquals(PreviewNetworkState.DEGRADED, state.previewNetwork)
    }

    @Test
    fun persistenceTokensRoundTripAllAppState() {
        val original =
            NexusAppState
                .initial(themeMode = ThemeMode.DARK)
                .completeDemoOnboarding("Demo Name")
                .navigateTo(AppDestination.Profile)
                .navigateTo(AppDestination.FileDetails("nx-demo"))
                .setPreviewScenario(PreviewScenario.OFFLINE)
                .setPreviewNetwork(PreviewNetworkState.DEGRADED)

        val restored = NexusAppState.fromPersistenceTokens(original.toPersistenceTokens())

        assertEquals(original, restored)
    }

    @Test
    fun malformedPersistenceTokensAreRejected() {
        val valid = NexusAppState.initial().toPersistenceTokens()

        assertNull(NexusAppState.fromPersistenceTokens(emptyList()))
        assertNull(NexusAppState.fromPersistenceTokens(listOf("wrong-version") + valid.drop(1)))
        assertNull(NexusAppState.fromPersistenceTokens(valid.dropLast(1)))
        assertNull(NexusAppState.fromPersistenceTokens(valid + "extra"))
        assertNull(
            NexusAppState.fromPersistenceTokens(
                valid.toMutableList().also { it[2] = "NOT_A_STATE" },
            ),
        )
        assertNull(
            NexusAppState.fromPersistenceTokens(
                valid.toMutableList().also { it[7] = "unknown-route" },
            ),
        )
    }

    @Test
    fun themeStorageFallsBackSafely() {
        assertEquals(ThemeMode.DARK, ThemeMode.fromStorage("DARK"))
        assertEquals(ThemeMode.LIGHT, ThemeMode.fromStorage("LIGHT"))
        assertEquals(ThemeMode.SYSTEM, ThemeMode.fromStorage("SYSTEM"))
        assertEquals(ThemeMode.SYSTEM, ThemeMode.fromStorage("not-a-mode"))
        assertEquals(ThemeMode.SYSTEM, ThemeMode.fromStorage(null))
    }

    @Test
    fun restoredNavigationKeepsDetailScreenAndBackPath() {
        val state =
            NexusAppState
                .initial(firstLaunch = false)
                .selectSection(AppDestination.Search)
                .navigateTo(AppDestination.FileDetails("nx-demo"))

        val restored =
            NexusAppState.fromPersistenceTokens(state.toPersistenceTokens())
                ?: error("state should restore")

        assertEquals(AppDestination.FileDetails("nx-demo"), restored.destination)
        assertEquals(
            NavigationState
                .of(AppDestination.Search)
                .push(AppDestination.FileDetails("nx-demo")),
            restored.navigation,
        )
    }
}
