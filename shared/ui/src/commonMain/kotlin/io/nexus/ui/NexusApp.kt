package io.nexus.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.dp
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.presentation.demo.DemoCatalog
import io.nexus.presentation.isDark
import io.nexus.presentation.navigation.AppDestination
import io.nexus.ui.components.NexusBottomBar
import io.nexus.ui.components.NexusCompactTopBar
import io.nexus.ui.components.NexusGlobalDemoBanner
import io.nexus.ui.components.NexusSideRail
import io.nexus.ui.components.NexusSnackbarHost
import io.nexus.ui.components.navigationLabel
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.resources.NexusStrings
import io.nexus.ui.screens.DownloadsScreen
import io.nexus.ui.screens.ExploreScreen
import io.nexus.ui.screens.FileDetailsScreen
import io.nexus.ui.screens.HostedFilesScreen
import io.nexus.ui.screens.IdentitySetupScreen
import io.nexus.ui.screens.NetworkStatusScreen
import io.nexus.ui.screens.ProfileScreen
import io.nexus.ui.screens.PublishScreen
import io.nexus.ui.screens.SearchScreen
import io.nexus.ui.screens.SettingsScreen
import io.nexus.ui.screens.WelcomeScreen
import io.nexus.ui.theme.NexusBreakpoints
import io.nexus.ui.theme.NexusSpacing
import io.nexus.ui.theme.NexusTheme
import io.nexus.ui.theme.ThemeStore
import kotlinx.coroutines.launch

/**
 * Root composable shared by the Android and Windows shells.
 *
 * The shell owns [NexusAppController] so it can install platform back handling
 * and restore navigation tokens. All product screens are selected here.
 */
@Composable
public fun NexusApp(
    controller: NexusAppController,
    modifier: Modifier = Modifier,
) {
    val darkTheme = controller.state.themeMode.isDark(isSystemInDarkTheme())
    val coreStatus by controller.coreStatus.collectAsState()

    NexusTheme(darkTheme = darkTheme) {
        CompositionLocalProvider(LocalCoreStatus provides coreStatus) {
            NexusAppFrame(controller = controller, modifier = modifier)
        }
    }
}

/** Convenience entry point for previews and simple hosts. */
@Composable
public fun NexusApp(
    themeStore: ThemeStore,
    modifier: Modifier = Modifier,
) {
    val controller = rememberNexusAppController(themeStore)
    NexusApp(controller = controller, modifier = modifier)
}

@Composable
private fun NexusAppFrame(
    controller: NexusAppController,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    val state = controller.state
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val showMessage: (String) -> Unit = { message ->
        scope.launch { snackbarHostState.showSnackbar(message) }
    }
    val showChrome =
        state.destination != AppDestination.Welcome &&
            state.destination != AppDestination.IdentitySetup

    BoxWithConstraints(
        modifier =
            modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background)
                .windowInsetsPadding(WindowInsets.safeDrawing)
                .onPreviewKeyEvent { event ->
                    if (event.type == KeyEventType.KeyDown &&
                        event.key == Key.Escape &&
                        controller.state.canGoBack
                    ) {
                        controller.back()
                        true
                    } else {
                        false
                    }
                },
    ) {
        val expanded = maxWidth >= NexusBreakpoints.expanded
        Column(modifier = Modifier.fillMaxSize()) {
            NexusGlobalDemoBanner()
            if (expanded && showChrome) {
                Row(modifier = Modifier.weight(1f).fillMaxWidth()) {
                    NexusSideRail(
                        currentSection = state.activeSection,
                        network = state.previewNetwork,
                        onNavigate = controller::selectSection,
                        onProfile = { controller.navigateTo(AppDestination.Profile) },
                        onNetwork = { controller.navigateTo(AppDestination.NetworkStatus) },
                    )
                    Box(
                        modifier =
                            Modifier
                                .weight(1f)
                                .fillMaxHeight(),
                    ) {
                        NexusScreenHost(
                            controller = controller,
                            onShowMessage = showMessage,
                        )
                    }
                }
            } else {
                if (showChrome) {
                    NexusCompactTopBar(
                        network = state.previewNetwork,
                        canGoBack = state.canGoBack,
                        title = compactTitle(state.destination, strings),
                        onBack = controller::back,
                        onNetwork = { controller.navigateTo(AppDestination.NetworkStatus) },
                        onProfile = { controller.navigateTo(AppDestination.Profile) },
                    )
                }
                Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
                    NexusScreenHost(
                        controller = controller,
                        onShowMessage = showMessage,
                    )
                }
                if (showChrome) {
                    NexusBottomBar(
                        currentSection = state.activeSection,
                        onNavigate = controller::selectSection,
                    )
                }
            }
        }
        NexusSnackbarHost(
            hostState = snackbarHostState,
            modifier =
                Modifier
                    .align(Alignment.BottomCenter)
                    .padding(
                        bottom = if (!expanded && showChrome) 88.dp else NexusSpacing.lg,
                    ),
        )
    }
}

private fun compactTitle(
    destination: AppDestination,
    strings: NexusStrings,
): String =
    when (destination) {
        is AppDestination.FileDetails -> {
            DemoCatalog.publication(destination.recordId)?.title ?: strings.fileDetailsTitle
        }

        else -> {
            navigationLabel(destination, strings)
        }
    }

@Composable
private fun NexusScreenHost(
    controller: NexusAppController,
    onShowMessage: (String) -> Unit,
) {
    val state = controller.state
    val strings = LocalNexusStrings.current
    val scenario: PreviewScenario = state.previewScenario
    val network: PreviewNetworkState = state.previewNetwork
    val onScenarioChange: (PreviewScenario) -> Unit = controller::setPreviewScenario
    val onNetworkChange: (PreviewNetworkState) -> Unit = controller::setPreviewNetwork

    when (val destination = state.destination) {
        AppDestination.Welcome -> {
            WelcomeScreen(
                onOpenDemo = { controller.startDemo() },
                onSetupIdentity = { controller.navigateTo(AppDestination.IdentitySetup) },
                onBack = if (state.canGoBack) controller::back else null,
            )
        }

        AppDestination.IdentitySetup -> {
            IdentitySetupScreen(
                onContinue = { displayName -> controller.startDemo(displayName) },
                onBack = controller::back,
            )
        }

        AppDestination.Explore -> {
            ExploreScreen(
                scenario = scenario,
                network = network,
                onScenarioChange = onScenarioChange,
                onNetworkChange = onNetworkChange,
                onOpenFile = { recordId -> controller.navigateTo(AppDestination.FileDetails(recordId)) },
                onOpenPublish = { controller.navigateTo(AppDestination.Publish) },
            )
        }

        AppDestination.Search -> {
            SearchScreen(
                scenario = scenario,
                network = network,
                onScenarioChange = onScenarioChange,
                onNetworkChange = onNetworkChange,
                onOpenFile = { recordId -> controller.navigateTo(AppDestination.FileDetails(recordId)) },
            )
        }

        AppDestination.Profile -> {
            ProfileScreen(
                displayName = state.demoDisplayName,
                scenario = scenario,
                network = network,
                onScenarioChange = onScenarioChange,
                onNetworkChange = onNetworkChange,
                onOpenFile = { recordId -> controller.navigateTo(AppDestination.FileDetails(recordId)) },
                onOpenNetwork = { controller.navigateTo(AppDestination.NetworkStatus) },
                onBack = controller::back,
            )
        }

        is AppDestination.FileDetails -> {
            FileDetailsScreen(
                recordId = destination.recordId,
                scenario = scenario,
                network = network,
                onScenarioChange = onScenarioChange,
                onNetworkChange = onNetworkChange,
                onOpenProfile = { controller.navigateTo(AppDestination.Profile) },
                onShowMessage = onShowMessage,
                onBack = controller::back,
            )
        }

        AppDestination.Publish -> {
            PublishScreen(onBack = controller::back)
        }

        AppDestination.Downloads -> {
            DownloadsScreen(
                scenario = scenario,
                network = network,
                onScenarioChange = onScenarioChange,
                onNetworkChange = onNetworkChange,
                onShowMessage = onShowMessage,
            )
        }

        AppDestination.HostedFiles -> {
            HostedFilesScreen(
                scenario = scenario,
                network = network,
                onScenarioChange = onScenarioChange,
                onNetworkChange = onNetworkChange,
                onShowMessage = onShowMessage,
            )
        }

        AppDestination.Settings -> {
            SettingsScreen(
                themeMode = state.themeMode,
                onThemeChange = controller::setThemeMode,
                onOpenWelcome = { controller.navigateTo(AppDestination.Welcome) },
                onOpenIdentity = { controller.navigateTo(AppDestination.IdentitySetup) },
            )
        }

        AppDestination.NetworkStatus -> {
            NetworkStatusScreen(
                scenario = scenario,
                network = network,
                onScenarioChange = onScenarioChange,
                onNetworkChange = onNetworkChange,
                onBack = controller::back,
            )
        }
    }
}
