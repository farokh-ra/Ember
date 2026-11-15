@file:Suppress("MagicNumber", "ThrowsCount", "TooManyFunctions")

package io.nexus.core.client

import io.nexus.core.api.CoreSnapshot
import io.nexus.core.api.NexusCore
import io.nexus.model.CoreState
import io.nexus.model.NexusEvent
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/** One decoded native event. */
public data class NativeEvent(
    public val sequence: Long,
    public val timestampMs: Long,
    public val type: String,
    public val opId: Long?,
    public val data: CborValue,
)

/**
 * A [NexusCore] backed by the native library.
 *
 * The event pump is the only consumer of `next_event`; it runs on the supplied
 * dispatcher, which must not be the UI dispatcher. Calls that return native
 * buffers are synchronous and copy their output before returning.
 */
public class NativeCoreClient private constructor(
    private val bridge: NativeBridge,
    private val handle: Long,
    private val scope: CoroutineScope,
    private val dispatcher: CoroutineDispatcher,
) : NexusCore,
    AutoCloseable {
    private val state = MutableStateFlow(CoreState.STOPPED)
    private val stateEvents = MutableStateFlow<NexusEvent>(NexusEvent.StateChanged(CoreState.STOPPED))
    private val decodedEvents =
        MutableSharedFlow<NativeEvent>(replay = EVENT_REPLAY, extraBufferCapacity = EVENT_BUFFER)
    private val pumpFailure = MutableStateFlow<Throwable?>(null)
    private val pumpJob = scope.launch(dispatcher) { pumpEvents() }

    private var protocolVersion: String = UNKNOWN_PROTOCOL
    private var closed: Boolean = false

    override val events: Flow<NexusEvent> = stateEvents.asStateFlow()

    /** All decoded events, including progress and terminal events. */
    public val nativeEvents: Flow<NativeEvent> = decodedEvents.asSharedFlow()

    /** The last event-pump failure, or null while the pump is healthy. */
    public val lastPumpFailure: Flow<Throwable?> = pumpFailure.asStateFlow()

    /** Native semantic version reported by the loaded library. */
    public val nativeVersion: String
        get() = bridge.version

    /** NEXUS ABI version reported by the loaded library. */
    public val nativeAbiVersion: Int
        get() = bridge.abiVersion

    init {
        refreshProtocolVersion()
    }

    override suspend fun start() {
        checkOpen()
        throwOnStatus(bridge.start(handle))
    }

    override suspend fun stop() {
        checkOpen()
        throwOnStatus(bridge.stop(handle))
    }

    override fun snapshot(): CoreSnapshot = CoreSnapshot(state = state.value, protocolVersion = protocolVersion)

    /**
     * Invokes one stable method ID with a request envelope.
     *
     * This call is synchronous and must run on an IO dispatcher. A domain
     * error is thrown as [NexusException.Domain].
     */
    public fun invoke(
        methodId: Int,
        params: CborValue = CborValue.emptyMap(),
    ): CborValue {
        checkOpen()
        val request =
            encodeCbor(
                CborValue.textMap(
                    "v" to CborValue.Unsigned(ENVELOPE_VERSION),
                    "params" to params,
                ),
                CborLimits.Default,
            )
        val result = bridge.invoke(handle, methodId, request)
        if (result.status != NativeStatus.OK) {
            throw NexusException.Status(result.status)
        }
        return decodeResponse(result.bytes)
    }

    /** Requests cooperative cancellation of an asynchronous operation. */
    public fun cancel(opId: Long) {
        checkOpen()
        throwOnStatus(bridge.cancel(handle, opId))
    }

    /** Starts a streaming import. */
    public fun importBegin(
        displayName: String,
        expectedSize: Long,
    ): Long {
        checkOpen()
        val result = bridge.importBegin(handle, displayName.encodeToByteArray(), expectedSize)
        throwOnStatus(result.status)
        return result.value
    }

    /** Writes one import chunk of at most 1 MiB. */
    public fun importWrite(
        importId: Long,
        data: ByteArray,
    ) {
        checkOpen()
        throwOnStatus(bridge.importWrite(handle, importId, data))
    }

    /** Finalizes an import and returns its CBOR summary map. */
    public fun importFinish(importId: Long): CborValue {
        checkOpen()
        val result = bridge.importFinish(handle, importId)
        throwOnStatus(result.status)
        return decodeCbor(result.bytes, CborLimits.Default)
    }

    /** Aborts an import and removes its temporary file. */
    public fun importAbort(importId: Long) {
        checkOpen()
        throwOnStatus(bridge.importAbort(handle, importId))
    }

    /** Opens an export cursor for a raw 32-byte SHA-256 content ID. */
    public fun exportOpen(contentId: ByteArray): Long {
        checkOpen()
        require(contentId.size == CONTENT_ID_BYTES) { "content ID must be 32 bytes" }
        val result = bridge.exportOpen(handle, contentId)
        throwOnStatus(result.status)
        return result.value
    }

    /** Reads the next export chunk; an empty array means end of file. */
    public fun exportRead(
        exportId: Long,
        maxLen: Int = MAX_STREAM_CHUNK,
    ): ByteArray {
        checkOpen()
        val result = bridge.exportRead(handle, exportId, maxLen)
        throwOnStatus(result.status)
        return result.bytes
    }

    /** Closes an export cursor. */
    public fun exportClose(exportId: Long) {
        checkOpen()
        throwOnStatus(bridge.exportClose(handle, exportId))
    }

    /** Stops the core if needed and releases the native handle. */
    override fun close() {
        if (closed) {
            return
        }
        closed = true
        runCatching { bridge.stop(handle) }
        pumpJob.cancel()
        runCatching { bridge.destroy(handle) }
    }

    private suspend fun pumpEvents() {
        while (currentCoroutineContext().isActive) {
            val result = bridge.nextEvent(handle, EVENT_TIMEOUT_MS)
            when (result.status) {
                NativeStatus.OK -> {
                    runCatching { handleEvent(result.bytes) }
                        .onFailure { pumpFailure.value = it }
                }

                NativeStatus.TIMEOUT -> {
                    continue
                }

                NativeStatus.QUEUE_CLOSED, NativeStatus.INVALID_HANDLE -> {
                    return
                }

                else -> {
                    pumpFailure.value = NexusException.Status(result.status)
                    return
                }
            }
        }
    }

    private fun handleEvent(bytes: ByteArray) {
        val envelope = decodeCbor(bytes, CborLimits.Default)
        val event =
            NativeEvent(
                sequence = envelope.mapGet("seq")?.asLong() ?: throw NexusException.Decode("event is missing seq"),
                timestampMs = envelope.mapGet("ts")?.asLong() ?: 0,
                type =
                    (envelope.mapGet("type") as? CborValue.Text)?.value
                        ?: throw NexusException.Decode("event is missing type"),
                opId = (envelope.mapGet("op_id") as? CborValue.Unsigned)?.value?.toLong(),
                data = envelope.mapGet("data") ?: CborValue.Null,
            )
        decodedEvents.tryEmit(event)
        if (event.type == CORE_STATE_EVENT) {
            val name = (event.data.mapGet("state") as? CborValue.Text)?.value
            val next = mapState(name)
            state.value = next
            stateEvents.value = NexusEvent.StateChanged(next)
        }
    }

    private fun refreshProtocolVersion() {
        protocolVersion =
            try {
                val info = invoke(SYSTEM_INFO_METHOD_ID)
                (info.mapGet("protocol_version") as? CborValue.Text)?.value ?: UNKNOWN_PROTOCOL
            } catch (_: NexusException) {
                UNKNOWN_PROTOCOL
            }
    }

    private fun decodeResponse(bytes: ByteArray): CborValue {
        val envelope =
            try {
                decodeCbor(bytes, CborLimits.Response)
            } catch (error: CborException) {
                throw NexusException.Decode("response envelope is invalid", error)
            }
        val ok =
            (envelope.mapGet("ok") as? CborValue.Bool)?.value
                ?: throw NexusException.Decode("response is missing ok")
        if (ok) {
            return envelope.mapGet("result") ?: throw NexusException.Decode("response is missing result")
        }
        val error =
            envelope.mapGet("error")
                ?: throw NexusException.Decode("response is missing error")
        val code =
            (error.mapGet("code") as? CborValue.Text)?.value
                ?: throw NexusException.Decode("domain error is missing code")
        val message =
            (error.mapGet("message") as? CborValue.Text)?.value
                ?: "native domain error"
        val retryable = (error.mapGet("retryable") as? CborValue.Bool)?.value ?: false
        throw NexusException.Domain(code, retryable, error.mapGet("details"), message)
    }

    private fun checkOpen() {
        if (closed) {
            throw NexusException.Status(NativeStatus.INVALID_HANDLE)
        }
    }

    private fun throwOnStatus(status: NativeStatus) {
        if (status != NativeStatus.OK) {
            throw NexusException.Status(status)
        }
    }

    private fun mapState(name: String?): CoreState =
        when (name) {
            "starting" -> CoreState.STARTING
            "running", "ready" -> CoreState.READY
            "failed", "degraded" -> CoreState.DEGRADED
            else -> CoreState.STOPPED
        }

    public companion object {
        /** Native ABI expected by this client. */
        public const val ABI_VERSION: Int = 1

        /** Version of the request, response, and event envelopes. */
        public const val ENVELOPE_VERSION: ULong = 1uL

        /** `system.info` method ID. */
        public const val SYSTEM_INFO_METHOD_ID: Int = 0x0101

        /** `system.health` method ID. */
        public const val SYSTEM_HEALTH_METHOD_ID: Int = 0x0102

        /** `system.snapshot` method ID. */
        public const val SYSTEM_SNAPSHOT_METHOD_ID: Int = 0x0103

        /** `test.echo` method ID. */
        public const val TEST_ECHO_METHOD_ID: Int = 0x0B01

        /** `test.fault` method ID. */
        public const val TEST_FAULT_METHOD_ID: Int = 0x0B02

        /** Maximum bytes per import or export call. */
        public const val MAX_STREAM_CHUNK: Int = 1_048_576

        /** Raw SHA-256 content ID length. */
        public const val CONTENT_ID_BYTES: Int = 32

        private const val EVENT_TIMEOUT_MS: Int = 50
        private const val EVENT_BUFFER: Int = 256
        private const val EVENT_REPLAY: Int = 64
        private const val UNKNOWN_PROTOCOL: String = "unknown"
        private const val CORE_STATE_EVENT: String = "core.state"

        /**
         * Checks the native ABI, creates the core, and starts the event pump.
         *
         * The caller's secret array is zeroed after the create call. A
         * mismatch is rejected before the core opens the database.
         */
        public fun create(
            bridge: NativeBridge,
            config: NativeConfig,
            localSecret: ByteArray,
            scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default),
            dispatcher: CoroutineDispatcher = Dispatchers.Default,
        ): NativeCoreClient {
            val actual = bridge.abiVersion
            if (actual != ABI_VERSION) {
                localSecret.fill(0)
                throw NexusException.AbiMismatch(expected = ABI_VERSION, actual = actual)
            }
            require(localSecret.size == 32) { "local secret must be exactly 32 bytes" }
            val result =
                try {
                    bridge.create(config.encode(), localSecret)
                } finally {
                    localSecret.fill(0)
                }
            if (result.status != NativeStatus.OK) {
                throw NexusException.Status(result.status)
            }
            return NativeCoreClient(bridge, result.value, scope, dispatcher)
        }
    }
}
