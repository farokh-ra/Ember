package io.nexus.presentation

import io.nexus.presentation.demo.DemoCatalog
import io.nexus.presentation.navigation.AppDestination
import io.nexus.presentation.navigation.NavigationState

/**
 * Immutable application state for the Stage 04 UI prototype.
 *
 * The state is platform-free so navigation, theme, and preview transitions can
 * be covered by JVM tests.
 */
public data class NexusAppState(
    public val navigation: NavigationState,
    public val themeMode: ThemeMode,
    public val previewScenario: PreviewScenario,
    public val previewNetwork: PreviewNetworkState,
    public val demoMode: Boolean,
    public val demoDisplayName: String?,
) {
    /** The screen currently shown. */
    public val destination: AppDestination
        get() = navigation.current

    /** Primary section highlighted in navigation chrome. */
    public val activeSection: AppDestination?
        get() = navigation.activeSection

    /** True when a back action can pop to another screen. */
    public val canGoBack: Boolean
        get() = navigation.canGoBack

    /** Pushes a screen onto the current journey. */
    public fun navigateTo(destination: AppDestination): NexusAppState = copy(navigation = navigation.push(destination))

    /** Switches primary section and resets that section's stack. */
    public fun selectSection(destination: AppDestination): NexusAppState =
        copy(navigation = navigation.selectSection(destination))

    /** Pops one screen. At the root this is a no-op. */
    public fun back(): NexusAppState = copy(navigation = navigation.back())

    /** Starts the demo session and lands on Explore. */
    public fun completeDemoOnboarding(displayName: String? = null): NexusAppState =
        copy(
            navigation = NavigationState.of(AppDestination.Explore),
            demoMode = true,
            demoDisplayName =
                displayName?.trim()?.takeIf { it.isNotEmpty() }
                    ?: DemoCatalog.profile.displayName,
        )

    /** Changes the saved appearance preference. */
    public fun setThemeMode(mode: ThemeMode): NexusAppState = copy(themeMode = mode)

    /** Changes the content state shown by data-driven preview screens. */
    public fun setPreviewScenario(scenario: PreviewScenario): NexusAppState = copy(previewScenario = scenario)

    /** Changes the simulated global network state. */
    public fun setPreviewNetwork(state: PreviewNetworkState): NexusAppState = copy(previewNetwork = state)

    /** Serializes the full app state into platform-saveable strings. */
    public fun toPersistenceTokens(): List<String> =
        buildList {
            add(PERSISTENCE_VERSION)
            add(themeMode.name)
            add(previewScenario.name)
            add(previewNetwork.name)
            add(demoMode.toString())
            add(demoDisplayName.orEmpty())
            add(navigation.backStack.size.toString())
            addAll(navigation.toRouteTokens())
        }

    public companion object {
        private const val PERSISTENCE_VERSION: String = "nexus-app-state-v1"
        private const val HEADER_SIZE: Int = 7
        private const val TOKEN_THEME: Int = 1
        private const val TOKEN_SCENARIO: Int = 2
        private const val TOKEN_NETWORK: Int = 3
        private const val TOKEN_DEMO_MODE: Int = 4
        private const val TOKEN_DISPLAY_NAME: Int = 5
        private const val TOKEN_ROUTE_COUNT: Int = 6

        /** Creates the first-launch state. */
        public fun initial(
            themeMode: ThemeMode = ThemeMode.SYSTEM,
            firstLaunch: Boolean = true,
        ): NexusAppState =
            NexusAppState(
                navigation =
                    if (firstLaunch) {
                        NavigationState.initial()
                    } else {
                        NavigationState.of(AppDestination.Explore)
                    },
                themeMode = themeMode,
                previewScenario = PreviewScenario.READY,
                previewNetwork = PreviewNetworkState.ONLINE,
                demoMode = !firstLaunch,
                demoDisplayName = if (firstLaunch) null else DemoCatalog.profile.displayName,
            )

        /**
         * Restores app state from persistence tokens.
         *
         * Invalid versions, malformed fields, and unknown routes return null so
         * the shell can safely fall back to a first-launch state.
         */
        @Suppress("ReturnCount") // Each malformed field exits early.
        public fun fromPersistenceTokens(tokens: List<String>): NexusAppState? {
            if (tokens.size < HEADER_SIZE || tokens.first() != PERSISTENCE_VERSION) {
                return null
            }
            val themeMode = ThemeMode.fromStorage(tokens[TOKEN_THEME])
            val scenario =
                PreviewScenario.entries.firstOrNull { it.name == tokens[TOKEN_SCENARIO] } ?: return null
            val network =
                PreviewNetworkState.entries.firstOrNull { it.name == tokens[TOKEN_NETWORK] } ?: return null
            val demoMode = tokens[TOKEN_DEMO_MODE].toBooleanStrictOrNull() ?: return null
            val displayName = tokens[TOKEN_DISPLAY_NAME].trim().takeIf { it.isNotEmpty() }
            val routeCount = tokens[TOKEN_ROUTE_COUNT].toIntOrNull()?.takeIf { it > 0 } ?: return null
            val routes = tokens.drop(HEADER_SIZE)
            if (routes.size != routeCount) {
                return null
            }
            val navigation = NavigationState.fromRouteTokens(routes) ?: return null
            return NexusAppState(
                navigation = navigation,
                themeMode = themeMode,
                previewScenario = scenario,
                previewNetwork = network,
                demoMode = demoMode,
                demoDisplayName = displayName,
            )
        }
    }
}
