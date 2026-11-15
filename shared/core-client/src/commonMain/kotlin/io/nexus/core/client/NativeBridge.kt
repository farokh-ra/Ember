@file:Suppress("MagicNumber")

package io.nexus.core.client

/** ABI-level status returned by the native library. */
public enum class NativeStatus(
    public val code: Int,
) {
    OK(0),
    TIMEOUT(1),
    INVALID_HANDLE(2),
    INVALID_ARGUMENT(3),
    ABI_MISMATCH(4),
    QUEUE_CLOSED(5),
    INTERNAL(6),
    BUFFER_TOO_LARGE(7),
    UNSUPPORTED(8),
    ;

    public companion object {
        /** Maps a raw status code, treating unknown codes as [INTERNAL]. */
        public fun fromCode(code: Int): NativeStatus = entries.firstOrNull { it.code == code } ?: INTERNAL
    }
}

/** Result of a native call that returns one opaque handle or identifier. */
public data class NativeHandleResult(
    public val status: NativeStatus,
    public val value: Long = 0,
)

/** Result of a native call that returns a byte buffer. */
public data class NativeBytesResult(
    public val status: NativeStatus,
    public val bytes: ByteArray = ByteArray(0),
)

/**
 * Low-level adapter over the versioned C ABI.
 *
 * Android implements this with JNI, Windows with JNA. Implementations copy
 * native output buffers into [ByteArray] and free every native buffer exactly
 * once before returning.
 */
public interface NativeBridge {
    /** ABI version reported by the loaded library. */
    public val abiVersion: Int

    /** Semantic version reported by the loaded library. */
    public val version: String

    /** Creates a core instance. The secret is consumed as borrowed bytes. */
    public fun create(
        config: ByteArray,
        localSecret: ByteArray,
    ): NativeHandleResult

    /** Starts the core. Idempotent. */
    public fun start(handle: Long): NativeStatus

    /** Stops the core. Idempotent. */
    public fun stop(handle: Long): NativeStatus

    /** Stops if needed and removes the handle. */
    public fun destroy(handle: Long): NativeStatus

    /** Invokes one method with a request envelope. */
    public fun invoke(
        handle: Long,
        methodId: Int,
        request: ByteArray,
    ): NativeBytesResult

    /** Waits for one event envelope. */
    public fun nextEvent(
        handle: Long,
        timeoutMs: Int,
    ): NativeBytesResult

    /** Requests cooperative cancellation of an asynchronous operation. */
    public fun cancel(
        handle: Long,
        opId: Long,
    ): NativeStatus

    /** Starts a streaming import. */
    public fun importBegin(
        handle: Long,
        displayName: ByteArray,
        expectedSize: Long,
    ): NativeHandleResult

    /** Writes one bounded chunk. */
    public fun importWrite(
        handle: Long,
        importId: Long,
        data: ByteArray,
    ): NativeStatus

    /** Finalizes an import and returns its CBOR summary. */
    public fun importFinish(
        handle: Long,
        importId: Long,
    ): NativeBytesResult

    /** Aborts an import and removes its temporary file. */
    public fun importAbort(
        handle: Long,
        importId: Long,
    ): NativeStatus

    /** Opens an export cursor for a raw SHA-256 content ID. */
    public fun exportOpen(
        handle: Long,
        contentId: ByteArray,
    ): NativeHandleResult

    /** Reads at most [maxLen] bytes; an empty result means EOF. */
    public fun exportRead(
        handle: Long,
        exportId: Long,
        maxLen: Int,
    ): NativeBytesResult

    /** Closes an export cursor. */
    public fun exportClose(
        handle: Long,
        exportId: Long,
    ): NativeStatus
}
