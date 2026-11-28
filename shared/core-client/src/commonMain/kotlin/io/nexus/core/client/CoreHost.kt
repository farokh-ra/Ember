package io.nexus.core.client

import io.nexus.core.api.CoreAvailability
import io.nexus.core.api.CoreStatus
import io.nexus.model.NexusEvent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Owns the native core for one application process.
 *
 * Creation is explicit and fallible: a missing library, an unavailable secure
 * store, or an ABI mismatch leaves the app running with [status] reporting why
 * the core is unavailable instead of crashing the shell. The UI never sees
 * secrets or native handles.
 *
 * [bridgeFactory] is invoked once during [start] so a shell can load the
 * platform library lazily and stay usable when it is absent.
 */
public class CoreHost(
    private val bridgeFactory: () -> NativeBridge,
    private val config: NativeConfig,
    private val secretStore: LocalSecretStore,
    private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default),
) : AutoCloseable {
    private val mutableStatus = MutableStateFlow(CoreStatus.unavailable())
    private var client: NativeCoreClient? = null
    private var statusJob: Job? = null
    private var starting: Job? = null
    private var closed: Boolean = false

    /** Current lifecycle status; safe to collect from UI code. */
    public val status: StateFlow<CoreStatus> = mutableStatus.asStateFlow()

    /** True once the core is started and its handle is open. */
    public val isStarted: Boolean
        get() = client != null

    /**
     * Starts the core once. Repeated calls are no-ops after success.
     *
     * This function never throws: every failure is reported through [status]
     * with a short, non-sensitive reason.
     */
    public suspend fun start() {
        if (closed || starting != null) {
            return
        }
        val existing = client
        if (existing != null) {
            resume(existing)
            return
        }
        mutableStatus.value = CoreStatus(CoreAvailability.STARTING, protocolVersion = "unknown")
        val job = scope.launch { createAndStart() }
        starting = job
        try {
            job.join()
        } finally {
            starting = null
        }
    }

    /** Restarts an already created core, for example after [stop]. */
    private suspend fun resume(existing: NativeCoreClient) {
        try {
            existing.start()
        } catch (error: NexusException) {
            mutableStatus.value = CoreStatus.unavailable(describe(error))
            return
        }
        publish(existing)
    }

    @Suppress("TooGenericExceptionCaught")
    private suspend fun createAndStart() {
        val created =
            try {
                NativeCoreClient.create(bridgeFactory(), config, secretStore.loadOrCreate(), scope)
            } catch (error: Throwable) {
                // Loading a platform library raises LinkageError subclasses, not
                // Exception subclasses, and the shell must stay usable.
                mutableStatus.value = CoreStatus.unavailable(describe(error))
                null
            }
        if (created == null) {
            return
        }
        val started =
            try {
                created.start()
                true
            } catch (error: NexusException) {
                mutableStatus.value = CoreStatus.unavailable(describe(error))
                false
            }
        if (!started || closed) {
            created.close()
            return
        }
        client = created
        statusJob =
            scope.launch {
                created.events.collect { event ->
                    if (event is NexusEvent.StateChanged) {
                        publish(created)
                    }
                }
            }
        publish(created)
    }

    /**
     * Invokes one stable method ID with a request envelope.
     *
     * The call runs on an IO dispatcher because the native bridge is
     * synchronous. A domain error is thrown as [NexusException.Domain].
     */
    public suspend fun invoke(
        methodId: Int,
        params: CborValue = CborValue.emptyMap(),
    ): CborValue {
        val current = checkNotNull(client) { "the core is not available" }
        return withContext(Dispatchers.IO) { current.invoke(methodId, params) }
    }

    /** Stops the core while keeping the process able to start it again. */
    public suspend fun stop() {
        val current = client ?: return
        current.stop()
        publish(current)
    }

    /** Releases the native handle and stops status collection. */
    override fun close() {
        if (closed) {
            return
        }
        closed = true
        starting?.cancel()
        starting = null
        statusJob?.cancel()
        statusJob = null
        client?.close()
        client = null
        mutableStatus.value = CoreStatus.unavailable()
    }

    private fun publish(current: NativeCoreClient) {
        val snapshot = current.snapshot()
        mutableStatus.value = CoreStatus.from(snapshot.state, snapshot.protocolVersion)
    }

    private fun describe(error: Throwable): String =
        when (error) {
            is NexusException.Status -> "native status ${error.status.name.lowercase()}"
            is NexusException.AbiMismatch -> "ABI mismatch: expected ${error.expected}, found ${error.actual}"
            is NexusException.Domain -> error.code
            is NexusException.Decode -> "native response could not be decoded"
            is UnsatisfiedLinkError -> "native library is not available"
            else -> error.message?.takeIf { it.isNotBlank() } ?: "core unavailable"
        }
}
