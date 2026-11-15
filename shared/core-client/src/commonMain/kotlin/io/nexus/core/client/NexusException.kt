package io.nexus.core.client

/** Stable Ember exception hierarchy shared by JNI and JNA adapters. */
public sealed class NexusException(
    message: String,
    cause: Throwable? = null,
) : Exception(message, cause) {
    /** The loaded library and this client expect different ABI versions. */
    public class AbiMismatch(
        public val expected: Int,
        public val actual: Int,
    ) : NexusException("native ABI $actual does not match expected ABI $expected")

    /** A native call returned a non-success ABI status. */
    public class Status(
        public val status: NativeStatus,
    ) : NexusException("native call failed with status ${status.name}")

    /** The core returned a domain error envelope. */
    public class Domain(
        public val code: String,
        public val retryable: Boolean,
        public val details: CborValue?,
        message: String,
    ) : NexusException("$code: $message")

    /** A response or event envelope could not be decoded. */
    public class Decode(
        message: String,
        cause: Throwable? = null,
    ) : NexusException(message, cause)
}
