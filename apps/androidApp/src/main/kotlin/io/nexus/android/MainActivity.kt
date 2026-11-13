package io.nexus.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.core.view.WindowCompat
import io.nexus.presentation.NexusAppState
import io.nexus.presentation.isDark
import io.nexus.ui.NexusApp
import io.nexus.ui.NexusAppController
import io.nexus.ui.theme.AndroidThemeStore

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val themeStore = AndroidThemeStore(applicationContext)

        setContent {
            val controller =
                rememberSaveable(
                    saver =
                        listSaver(
                            save = { it.snapshotTokens() },
                            restore = { tokens ->
                                NexusAppController(
                                    themeStore = themeStore,
                                    initialState = NexusAppState.fromPersistenceTokens(tokens),
                                )
                            },
                        ),
                ) {
                    NexusAppController(themeStore = themeStore)
                }

            BackHandler(enabled = controller.state.canGoBack) {
                controller.back()
            }

            val darkTheme = controller.state.themeMode.isDark(isSystemInDarkTheme())
            SideEffect {
                WindowCompat.getInsetsController(window, window.decorView).apply {
                    isAppearanceLightStatusBars = !darkTheme
                    isAppearanceLightNavigationBars = !darkTheme
                }
            }

            NexusApp(controller = controller)
        }
    }
}
