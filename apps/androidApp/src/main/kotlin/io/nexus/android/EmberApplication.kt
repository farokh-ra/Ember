package io.nexus.android

import android.app.Application
import io.nexus.core.client.AndroidKeystoreSecretStore
import io.nexus.core.client.CoreHost
import io.nexus.core.client.JniNativeBridge
import io.nexus.core.client.NativeConfig
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * Process-scoped owner of the native core.
 *
 * The core survives activity recreation because the identity vault and the
 * Android Keystore key are process resources. Networking stays off until the
 * discovery slice wires real peer state into the UI.
 */
class EmberApplication : Application() {
    private val coreScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    lateinit var coreHost: CoreHost
        private set

    override fun onCreate() {
        super.onCreate()
        coreHost =
            CoreHost(
                bridgeFactory = ::JniNativeBridge,
                config =
                    NativeConfig(
                        dataRoot = filesDir.absolutePath,
                        appVersion = BuildConfig.VERSION_NAME,
                        networkEnabled = false,
                    ),
                secretStore = AndroidKeystoreSecretStore(this),
                scope = coreScope,
            )
        coreScope.launch { coreHost.start() }
    }

    override fun onTerminate() {
        coreHost.close()
        coreScope.cancel()
        super.onTerminate()
    }
}
