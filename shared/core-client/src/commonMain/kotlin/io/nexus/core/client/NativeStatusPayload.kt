@file:Suppress("MagicNumber")

package io.nexus.core.client

/**
 * Decodes the JNI shim's status-plus-payload encoding: a little-endian
 * `int32` status followed by the payload bytes.
 */
internal fun decodeStatusPayload(bytes: ByteArray?): NativeBytesResult {
    if (bytes == null || bytes.size < 4) {
        return NativeBytesResult(NativeStatus.INTERNAL)
    }
    val code =
        (bytes[0].toInt() and 0xff) or
            ((bytes[1].toInt() and 0xff) shl 8) or
            ((bytes[2].toInt() and 0xff) shl 16) or
            ((bytes[3].toInt() and 0xff) shl 24)
    return NativeBytesResult(NativeStatus.fromCode(code), bytes.copyOfRange(4, bytes.size))
}

/** Decodes a JNI `long[2]` status-and-value result. */
internal fun decodeStatusHandle(values: LongArray?): NativeHandleResult {
    if (values == null || values.size != 2) {
        return NativeHandleResult(NativeStatus.INTERNAL)
    }
    return NativeHandleResult(NativeStatus.fromCode(values[0].toInt()), values[1])
}
