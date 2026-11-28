package io.nexus.desktop

import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import io.nexus.core.client.CoreHost
import io.nexus.core.client.DpapiSecretStore
import io.nexus.core.client.JnaNativeBridge
import io.nexus.core.client.NativeConfig
import io.nexus.ui.NexusApp
import io.nexus.ui.NexusAppController
import io.nexus.ui.theme.PreferencesThemeStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import java.awt.Dimension
import java.nio.file.Path

private const val INITIAL_WINDOW_WIDTH_DP = 1280
private const val INITIAL_WINDOW_HEIGHT_DP = 720
private const val MINIMUM_WINDOW_WIDTH = 720
private const val MINIMUM_WINDOW_HEIGHT = 560

fun main() =
    application {
        val themeStore = remember { PreferencesThemeStore() }
        val coreScope = remember { CoroutineScope(SupervisorJob() + Dispatchers.Default) }
        val dataRoot = remember { desktopDataRoot() }
        val coreHost =
            remember(dataRoot) {
                CoreHost(
                    bridgeFactory = { JnaNativeBridge.load() },
                    config =
                        NativeConfig(
                            dataRoot = dataRoot,
                            appVersion = APP_VERSION,
                            // Networking stays off until the discovery slice
                            // replaces demo content with real peer state.
                            networkEnabled = false,
                        ),
                    secretStore = DpapiSecretStore(dataRoot),
                    scope = coreScope,
                )
            }
        LaunchedEffect(coreHost) { coreHost.start() }
        DisposableEffect(coreHost) {
            onDispose {
                coreHost.close()
                coreScope.cancel()
            }
        }
        val controller =
            remember(coreHost) {
                NexusAppController(themeStore = themeStore, coreStatus = coreHost.status)
            }

        Window(
            onCloseRequest = ::exitApplication,
            title = "Ember",
            state =
                rememberWindowState(
                    width = INITIAL_WINDOW_WIDTH_DP.dp,
                    height = INITIAL_WINDOW_HEIGHT_DP.dp,
                ),
            // Handle Escape at the window level so Back works even while no
            // Compose child owns focus after mouse-only navigation.
            onPreviewKeyEvent = { event ->
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
            LaunchedEffect(Unit) {
                window.minimumSize = Dimension(MINIMUM_WINDOW_WIDTH, MINIMUM_WINDOW_HEIGHT)
            }
            NexusApp(controller = controller)
        }
    }

/**
 * Absolute per-user data root shared by the core database, content store, and
 * sealed local secret. `ARCHITECTURE.md` reserves `%LOCALAPPDATA%\Ember\data`,
 * which MSI upgrades preserve.
 */
private fun desktopDataRoot(): String {
    val localAppData = System.getenv("LOCALAPPDATA")
    val base =
        if (!localAppData.isNullOrBlank()) {
            Path.of(localAppData)
        } else {
            Path.of(System.getProperty("user.home"), "AppData", "Local")
        }
    return base.resolve("Ember").resolve("data").toString()
}
