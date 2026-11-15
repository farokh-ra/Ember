package io.nexus.core.client

import kotlin.test.Test

/**
 * Runs one identical contract across both Windows JVM adapters.
 *
 * The JNI case is a host-side proxy: it verifies the shared JNI shim, symbol
 * names, and result encodings on the Windows JVM. It is not Android runtime
 * evidence. Android execution remains deferred until an emulator or device is
 * available.
 */
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
}
