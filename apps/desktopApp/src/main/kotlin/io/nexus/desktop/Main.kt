package io.nexus.desktop

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import io.nexus.ui.NexusApp
import io.nexus.ui.NexusAppController
import io.nexus.ui.theme.PreferencesThemeStore
import java.awt.Dimension

private const val INITIAL_WINDOW_WIDTH_DP = 1280
private const val INITIAL_WINDOW_HEIGHT_DP = 720
private const val MINIMUM_WINDOW_WIDTH = 720
private const val MINIMUM_WINDOW_HEIGHT = 560

fun main() =
    application {
        val themeStore = remember { PreferencesThemeStore() }
        val controller = remember { NexusAppController(themeStore = themeStore) }

        Window(
            onCloseRequest = ::exitApplication,
            title = "NEXUS",
            state =
                rememberWindowState(
                    width = INITIAL_WINDOW_WIDTH_DP.dp,
                    height = INITIAL_WINDOW_HEIGHT_DP.dp,
                ),
        ) {
            LaunchedEffect(Unit) {
                window.minimumSize = Dimension(MINIMUM_WINDOW_WIDTH, MINIMUM_WINDOW_HEIGHT)
            }
            NexusApp(controller = controller)
        }
    }
