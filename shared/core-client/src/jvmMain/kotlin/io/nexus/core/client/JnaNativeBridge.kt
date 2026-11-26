@file:Suppress("FunctionNaming", "TooManyFunctions")

package io.nexus.core.client

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.Structure
import com.sun.jna.ptr.LongByReference

/** JNA view of the C ABI. */
@Suppress("ktlint:standard:function-naming")
internal interface NexusLibrary : Library {
    fun nexus_core_abi_version(): Int

    fun nexus_core_version(): String

    fun nexus_core_create(
        config: ByteArray,
        configLen: Long,
        localSecret: ByteArray,
        localSecretLen: Long,
        outHandle: LongByReference,
    ): Int

    fun nexus_core_start(handle: Long): Int

    fun nexus_core_stop(handle: Long): Int

    fun nexus_core_destroy(handle: Long): Int

    fun nexus_core_invoke(
        handle: Long,
        methodId: Int,
        request: ByteArray,
        requestLen: Long,
        outResponse: NexusBuffer,
    ): Int

    fun nexus_core_next_event(
        handle: Long,
        timeoutMs: Int,
        outEvent: NexusBuffer,
    ): Int

    fun nexus_core_cancel(
        handle: Long,
        opId: Long,
    ): Int

    fun nexus_core_free_buffer(buffer: NexusBuffer.ByValue)

    fun nexus_core_import_begin(
        handle: Long,
        displayName: ByteArray,
        displayNameLen: Long,
        expectedSize: Long,
        outImportId: LongByReference,
    ): Int

    fun nexus_core_import_write(
        handle: Long,
        importId: Long,
        data: ByteArray,
        len: Long,
    ): Int

    fun nexus_core_import_finish(
        handle: Long,
        importId: Long,
        outSummary: NexusBuffer,
    ): Int

    fun nexus_core_import_abort(
        handle: Long,
        importId: Long,
    ): Int

    fun nexus_core_export_open(
        handle: Long,
        contentId: ByteArray,
        contentIdLen: Long,
        outExportId: LongByReference,
    ): Int

    fun nexus_core_export_read(
        handle: Long,
        exportId: Long,
        maxLen: Int,
        outChunk: NexusBuffer,
    ): Int

    fun nexus_core_export_close(
        handle: Long,
        exportId: Long,
    ): Int
}

/** Native `NexusBuffer` layout for x64. */
@Structure.FieldOrder("ptr", "len")
public open class NexusBuffer : Structure {
    @JvmField
    public var ptr: Pointer? = null

    @JvmField
    public var len: Long = 0

    public constructor() : super()

    public constructor(pointer: Pointer?) : super(pointer)

    /** By-value variant used only for `nexus_core_free_buffer`. */
    public class ByValue :
        NexusBuffer(),
        Structure.ByValue
}

/** Windows JNA adapter over the C ABI. */
public class JnaNativeBridge private constructor(
    private val library: NexusLibrary,
) : NativeBridge {
    override val abiVersion: Int
        get() = library.nexus_core_abi_version()

    override val version: String
        get() = library.nexus_core_version()

    override fun create(
        config: ByteArray,
        localSecret: ByteArray,
    ): NativeHandleResult {
        val handle = LongByReference()
        val status =
            library.nexus_core_create(
                config,
                config.size.toLong(),
                localSecret,
                localSecret.size.toLong(),
                handle,
            )
        return NativeHandleResult(NativeStatus.fromCode(status), handle.value)
    }

    override fun start(handle: Long): NativeStatus = NativeStatus.fromCode(library.nexus_core_start(handle))

    override fun stop(handle: Long): NativeStatus = NativeStatus.fromCode(library.nexus_core_stop(handle))

    override fun destroy(handle: Long): NativeStatus = NativeStatus.fromCode(library.nexus_core_destroy(handle))

    override fun invoke(
        handle: Long,
        methodId: Int,
        request: ByteArray,
    ): NativeBytesResult {
        val output = NexusBuffer()
        return try {
            val status =
                library.nexus_core_invoke(
                    handle,
                    methodId,
                    request,
                    request.size.toLong(),
                    output,
                )
            output.read()
            NativeBytesResult(NativeStatus.fromCode(status), copyBuffer(output))
        } finally {
            free(output)
        }
    }

    override fun nextEvent(
        handle: Long,
        timeoutMs: Int,
    ): NativeBytesResult {
        val output = NexusBuffer()
        return try {
            val status = library.nexus_core_next_event(handle, timeoutMs, output)
            output.read()
            NativeBytesResult(NativeStatus.fromCode(status), copyBuffer(output))
        } finally {
            free(output)
        }
    }

    override fun cancel(
        handle: Long,
        opId: Long,
    ): NativeStatus = NativeStatus.fromCode(library.nexus_core_cancel(handle, opId))

    override fun importBegin(
        handle: Long,
        displayName: ByteArray,
        expectedSize: Long,
    ): NativeHandleResult {
        val importId = LongByReference()
        val status =
            library.nexus_core_import_begin(
                handle,
                displayName,
                displayName.size.toLong(),
                expectedSize,
                importId,
            )
        return NativeHandleResult(NativeStatus.fromCode(status), importId.value)
    }

    override fun importWrite(
        handle: Long,
        importId: Long,
        data: ByteArray,
    ): NativeStatus = NativeStatus.fromCode(library.nexus_core_import_write(handle, importId, data, data.size.toLong()))

    override fun importFinish(
        handle: Long,
        importId: Long,
    ): NativeBytesResult {
        val output = NexusBuffer()
        return try {
            val status = library.nexus_core_import_finish(handle, importId, output)
            output.read()
            NativeBytesResult(NativeStatus.fromCode(status), copyBuffer(output))
        } finally {
            free(output)
        }
    }

    override fun importAbort(
        handle: Long,
        importId: Long,
    ): NativeStatus = NativeStatus.fromCode(library.nexus_core_import_abort(handle, importId))

    override fun exportOpen(
        handle: Long,
        contentId: ByteArray,
    ): NativeHandleResult {
        val exportId = LongByReference()
        val status =
            library.nexus_core_export_open(
                handle,
                contentId,
                contentId.size.toLong(),
                exportId,
            )
        return NativeHandleResult(NativeStatus.fromCode(status), exportId.value)
    }

    override fun exportRead(
        handle: Long,
        exportId: Long,
        maxLen: Int,
    ): NativeBytesResult {
        val output = NexusBuffer()
        return try {
            val status = library.nexus_core_export_read(handle, exportId, maxLen, output)
            output.read()
            NativeBytesResult(NativeStatus.fromCode(status), copyBuffer(output))
        } finally {
            free(output)
        }
    }

    override fun exportClose(
        handle: Long,
        exportId: Long,
    ): NativeStatus = NativeStatus.fromCode(library.nexus_core_export_close(handle, exportId))

    private fun copyBuffer(buffer: NexusBuffer): ByteArray {
        val pointer = buffer.ptr
        val length = buffer.len
        if (pointer == null || length <= 0L) {
            return ByteArray(0)
        }
        require(length <= Int.MAX_VALUE) { "native buffer is too large for a Kotlin array" }
        return pointer.getByteArray(0, length.toInt())
    }

    private fun free(buffer: NexusBuffer) {
        val value = NexusBuffer.ByValue()
        value.ptr = buffer.ptr
        value.len = buffer.len
        value.write()
        library.nexus_core_free_buffer(value)
    }

    public companion object {
        /**
         * Loads the library from an absolute path, or from the packaged
         * application resources directory, or by name.
         *
         * Compose Desktop copies `appResourcesRootDir` content into
         * `app/resources` and exposes it through `compose.application.resources.dir`.
         * Checking it first keeps a packaged MSI self-contained without
         * requiring PATH changes or a working-directory assumption.
         */
        public fun load(libraryPath: String? = null): JnaNativeBridge {
            val resolved =
                libraryPath?.takeUnless { it.isBlank() }
                    ?: packagedLibraryPath()
            val library =
                if (resolved == null) {
                    Native.load("nexus_core", NexusLibrary::class.java)
                } else {
                    Native.load(resolved, NexusLibrary::class.java)
                }
            return JnaNativeBridge(library)
        }

        private fun packagedLibraryPath(): String? {
            val resourcesDir = System.getProperty("compose.application.resources.dir") ?: return null
            val candidate = java.io.File(resourcesDir, "nexus_core.dll")
            return if (candidate.isFile) candidate.absolutePath else null
        }
    }
}
