package io.nexus.core.api

import io.nexus.model.CoreState
import io.nexus.model.NexusEvent
import kotlinx.coroutines.flow.Flow

/** Immutable view of core state exposed to presentation code. */
public data class CoreSnapshot(
    public val state: CoreState,
    public val protocolVersion: String,
)

/** Platform-independent contract implemented by the native core bridge. */
public interface NexusCore {
    /** Lifecycle and operation events emitted by the core. */
    public val events: Flow<NexusEvent>

    /** Starts the core. Implementations must be idempotent. */
    public suspend fun start()

    /** Stops the core and releases transient resources. */
    public suspend fun stop()

    /** Returns the current core snapshot. */
    public fun snapshot(): CoreSnapshot
}
