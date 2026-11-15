package io.nexus.core.client

/** Android JNI adapter over the C ABI. */
public class JniNativeBridge : NativeBridge {
    override val abiVersion: Int
        get() = NexusNative.nativeAbiVersion()

    override val version: String
        get() = NexusNative.nativeVersion()

    override fun create(
        config: ByteArray,
        localSecret: ByteArray,
    ): NativeHandleResult = decodeStatusHandle(NexusNative.nativeCreate(config, localSecret))

    override fun start(handle: Long): NativeStatus = NativeStatus.fromCode(NexusNative.nativeStart(handle))

    override fun stop(handle: Long): NativeStatus = NativeStatus.fromCode(NexusNative.nativeStop(handle))

    override fun destroy(handle: Long): NativeStatus = NativeStatus.fromCode(NexusNative.nativeDestroy(handle))

    override fun invoke(
        handle: Long,
        methodId: Int,
        request: ByteArray,
    ): NativeBytesResult = decodeStatusPayload(NexusNative.nativeInvoke(handle, methodId, request))

    override fun nextEvent(
        handle: Long,
        timeoutMs: Int,
    ): NativeBytesResult = decodeStatusPayload(NexusNative.nativeNextEvent(handle, timeoutMs))

    override fun cancel(
        handle: Long,
        opId: Long,
    ): NativeStatus = NativeStatus.fromCode(NexusNative.nativeCancel(handle, opId))

    override fun importBegin(
        handle: Long,
        displayName: ByteArray,
        expectedSize: Long,
    ): NativeHandleResult = decodeStatusHandle(NexusNative.nativeImportBegin(handle, displayName, expectedSize))

    override fun importWrite(
        handle: Long,
        importId: Long,
        data: ByteArray,
    ): NativeStatus = NativeStatus.fromCode(NexusNative.nativeImportWrite(handle, importId, data))

    override fun importFinish(
        handle: Long,
        importId: Long,
    ): NativeBytesResult = decodeStatusPayload(NexusNative.nativeImportFinish(handle, importId))

    override fun importAbort(
        handle: Long,
        importId: Long,
    ): NativeStatus = NativeStatus.fromCode(NexusNative.nativeImportAbort(handle, importId))

    override fun exportOpen(
        handle: Long,
        contentId: ByteArray,
    ): NativeHandleResult = decodeStatusHandle(NexusNative.nativeExportOpen(handle, contentId))

    override fun exportRead(
        handle: Long,
        exportId: Long,
        maxLen: Int,
    ): NativeBytesResult = decodeStatusPayload(NexusNative.nativeExportRead(handle, exportId, maxLen))

    override fun exportClose(
        handle: Long,
        exportId: Long,
    ): NativeStatus = NativeStatus.fromCode(NexusNative.nativeExportClose(handle, exportId))
}
