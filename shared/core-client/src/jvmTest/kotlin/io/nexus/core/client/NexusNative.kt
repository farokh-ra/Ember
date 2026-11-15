package io.nexus.core.client

/**
 * Test-only JNI entry points.
 *
 * The class name and method signatures are identical to the Android binding so
 * the Windows JVM can exercise the same `Java_io_nexus_core_client_NexusNative_*`
 * symbols. This is a host-side proxy for the Android runtime, not evidence that
 * Android itself was executed.
 */
internal object NexusNative {
    init {
        val path =
            System.getProperty("nexus.native.path")
                ?: error("nexus.native.path must point at the native core library")
        System.load(path)
    }

    @JvmStatic
    external fun nativeAbiVersion(): Int

    @JvmStatic
    external fun nativeVersion(): String

    @JvmStatic
    external fun nativeCreate(
        config: ByteArray,
        localSecret: ByteArray,
    ): LongArray

    @JvmStatic
    external fun nativeStart(handle: Long): Int

    @JvmStatic
    external fun nativeStop(handle: Long): Int

    @JvmStatic
    external fun nativeDestroy(handle: Long): Int

    @JvmStatic
    external fun nativeInvoke(
        handle: Long,
        methodId: Int,
        request: ByteArray,
    ): ByteArray

    @JvmStatic
    external fun nativeNextEvent(
        handle: Long,
        timeoutMs: Int,
    ): ByteArray

    @JvmStatic
    external fun nativeCancel(
        handle: Long,
        opId: Long,
    ): Int

    @JvmStatic
    external fun nativeImportBegin(
        handle: Long,
        displayName: ByteArray,
        expectedSize: Long,
    ): LongArray

    @JvmStatic
    external fun nativeImportWrite(
        handle: Long,
        importId: Long,
        data: ByteArray,
    ): Int

    @JvmStatic
    external fun nativeImportFinish(
        handle: Long,
        importId: Long,
    ): ByteArray

    @JvmStatic
    external fun nativeImportAbort(
        handle: Long,
        importId: Long,
    ): Int

    @JvmStatic
    external fun nativeExportOpen(
        handle: Long,
        contentId: ByteArray,
    ): LongArray

    @JvmStatic
    external fun nativeExportRead(
        handle: Long,
        exportId: Long,
        maxLen: Int,
    ): ByteArray

    @JvmStatic
    external fun nativeExportClose(
        handle: Long,
        exportId: Long,
    ): Int
}
