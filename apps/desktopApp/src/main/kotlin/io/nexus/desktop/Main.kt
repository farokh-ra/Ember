package io.nexus.desktop

import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import io.nexus.ui.NexusApp

fun main() =
    application {
        Window(
            onCloseRequest = ::exitApplication,
            title = "NEXUS",
            state = rememberWindowState(width = 960.dp, height = 640.dp),
        ) {
            NexusApp()
        }
    }
