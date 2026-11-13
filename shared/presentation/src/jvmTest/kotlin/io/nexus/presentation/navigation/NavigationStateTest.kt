package io.nexus.presentation.navigation

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class NavigationStateTest {
    @Test
    fun initialStackStartsAtWelcome() {
        val state = NavigationState.initial()

        assertEquals(AppDestination.Welcome, state.current)
        assertFalse(state.canGoBack)
        assertNull(state.activeSection)
    }

    @Test
    fun pushKeepsBackPath() {
        val state =
            NavigationState
                .initial()
                .push(AppDestination.IdentitySetup)
                .push(AppDestination.Explore)

        assertEquals(AppDestination.Explore, state.current)
        assertTrue(state.canGoBack)
        assertEquals(
            listOf(AppDestination.Welcome, AppDestination.IdentitySetup, AppDestination.Explore),
            state.backStack,
        )
    }

    @Test
    fun repeatedPushToCurrentScreenIsIgnored() {
        val state = NavigationState.of(AppDestination.Explore).push(AppDestination.Explore)

        assertEquals(1, state.backStack.size)
    }

    @Test
    fun backPopsOneScreenAndStopsAtRoot() {
        val state =
            NavigationState
                .of(AppDestination.Explore)
                .push(AppDestination.FileDetails("nx-demo"))
                .back()

        assertEquals(AppDestination.Explore, state.current)
        assertFalse(state.canGoBack)
        assertEquals(state, state.back())
    }

    @Test
    fun tabSelectionResetsTheSectionStack() {
        val state =
            NavigationState
                .of(AppDestination.Explore)
                .push(AppDestination.FileDetails("nx-demo"))
                .selectSection(AppDestination.Downloads)

        assertEquals(listOf(AppDestination.Downloads), state.backStack)
    }

    @Test
    fun detailScreensKeepTheirOriginatingSectionActive() {
        val fromExplore =
            NavigationState
                .of(AppDestination.Explore)
                .push(AppDestination.FileDetails("nx-demo"))
        val fromSearch =
            NavigationState
                .of(AppDestination.Search)
                .push(AppDestination.FileDetails("nx-demo"))
                .push(AppDestination.Profile)

        assertEquals(AppDestination.Explore, fromExplore.activeSection)
        assertEquals(AppDestination.Search, fromSearch.activeSection)
    }

    @Test
    fun nonPrimaryDestinationCannotBeSelectedAsTab() {
        assertFailsWith<IllegalArgumentException> {
            NavigationState.of(AppDestination.Explore).selectSection(AppDestination.Profile)
        }
    }

    @Test
    fun routeTokensRoundTripIncludingFileDetails() {
        val original =
            NavigationState
                .of(AppDestination.Search)
                .push(AppDestination.FileDetails("nx-demo-coastal-field-notes"))
                .push(AppDestination.Publish)

        val restored = NavigationState.fromRouteTokens(original.toRouteTokens())

        assertEquals(original, restored)
    }

    @Test
    fun unknownOrEmptyRouteTokensAreRejected() {
        assertNull(NavigationState.fromRouteTokens(emptyList()))
        assertNull(NavigationState.fromRouteTokens(listOf("explore", "unknown")))
    }

    @Test
    fun fileDetailsRejectsAmbiguousRecordIds() {
        assertFailsWith<IllegalArgumentException> { AppDestination.FileDetails("") }
        assertFailsWith<IllegalArgumentException> { AppDestination.FileDetails("a/b") }
    }

    @Test
    fun allPrimarySectionsResolveFromTheirRoutes() {
        AppDestination.primarySections.forEach { destination ->
            assertEquals(destination, AppDestination.fromRoute(destination.route))
        }
    }
}
