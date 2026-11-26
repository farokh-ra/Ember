package io.nexus.core.client

import org.junit.jupiter.api.condition.EnabledOnOs
import org.junit.jupiter.api.condition.OS
import kotlin.test.Test

/**
 * Runs one identical contract across both Windows JVM adapters.
 *
 * The JNI case is a host-side proxy: it verifies the shared JNI shim, symbol
 * names, and result encodings on the Windows JVM. It is not Android runtime
 * evidence. Android execution remains deferred until an emulator or device is
 * available.
 *
 * Both adapters load `nexus_core.dll`, so the suite is skipped on non-Windows
 * hosts. The shared JVM tests run on Linux in CI.
 */
@EnabledOnOs(OS.WINDOWS)
class NativeContractTest {
    @Test
    fun jnaAdapterSatisfiesTheContract() {
        val path =
            requireNotNull(System.getProperty("nexus.native.path")) {
                "nexus.native.path must point at nexus_core.dll"
            }
        runNativeContract("jna", JnaNativeBridge.load(path))
    }

    @Test
    fun windowsJniProxySatisfiesTheContract() {
        runNativeContract("jni", JvmJniNativeBridge())
    }

    @Test
    fun jnaAdapterLoadsFromPackagedResourcesDirectory() {
        val dll =
            requireNotNull(System.getProperty("nexus.native.path")) {
                "nexus.native.path must point at nexus_core.dll"
            }
        val resourcesDir =
            java.io
                .File(dll)
                .parentFile.absolutePath
        val previous = System.getProperty("compose.application.resources.dir")
        System.setProperty("compose.application.resources.dir", resourcesDir)
        try {
            runNativeContract("jna-packaged", JnaNativeBridge.load())
        } finally {
            if (previous == null) {
                System.clearProperty("compose.application.resources.dir")
            } else {
                System.setProperty("compose.application.resources.dir", previous)
            }
        }
    }
}
