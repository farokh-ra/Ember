package io.nexus.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import io.nexus.core.api.CoreStatus
import io.nexus.presentation.NexusAppState
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.presentation.ThemeMode
import io.nexus.presentation.navigation.AppDestination
import io.nexus.ui.theme.ThemeStore
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * Owns mutable UI state for the app shell.
 *
 * The controller is platform-free and can be held by either shell, which lets
 * Android install a back dispatcher and save/restore navigation tokens. All
 * transitions delegate to the tested immutable [NexusAppState].
 *
 * [coreStatus] is supplied by the platform shell, which owns the native core
 * lifecycle. Screens that still render demo content read it only to report the
 * real core state; they never infer availability from demo data.
 */
public class NexusAppController(
    private val themeStore: ThemeStore,
    initialState: NexusAppState? = null,
    public val coreStatus: StateFlow<CoreStatus> = MutableStateFlow(CoreStatus.unavailable()),
) {
    public var state: NexusAppState by mutableStateOf(
        initialState ?: NexusAppState.initial(themeMode = themeStore.load()),
    )
        private set

    /** Pushes a screen onto the current journey. */
    public fun navigateTo(destination: AppDestination) {
        state = state.navigateTo(destination)
    }

    /** Switches a primary section and resets that section's stack. */
    public fun selectSection(destination: AppDestination) {
        state = state.selectSection(destination)
    }

    /** Pops one screen when possible. */
    public fun back() {
        state = state.back()
    }

    /** Completes demo onboarding and opens Explore. */
    public fun startDemo(displayName: String? = null) {
        state = state.completeDemoOnboarding(displayName)
    }

    /** Changes and persists the appearance preference. */
    public fun setThemeMode(mode: ThemeMode) {
        state = state.setThemeMode(mode)
        themeStore.save(mode)
    }

    /** Changes the simulated content state. */
    public fun setPreviewScenario(scenario: PreviewScenario) {
        state = state.setPreviewScenario(scenario)
    }

    /** Changes the simulated global network state. */
    public fun setPreviewNetwork(network: PreviewNetworkState) {
        state = state.setPreviewNetwork(network)
    }

    /** Platform-saveable navigation and preference tokens. */
    public fun snapshotTokens(): List<String> = state.toPersistenceTokens()
}

/** Remembers a controller for shells that do not need platform restoration. */
@Composable
public fun rememberNexusAppController(themeStore: ThemeStore): NexusAppController =
    remember(themeStore) { NexusAppController(themeStore) }
