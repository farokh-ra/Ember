package io.nexus.model

/** Lifecycle states reported by the NEXUS core. */
public enum class CoreState {
    STOPPED,
    STARTING,
    READY,
    DEGRADED,
}

/** Events emitted by the core to presentation code. */
public sealed interface NexusEvent {
    /** The core lifecycle state changed. */
    public data class StateChanged(
        public val state: CoreState,
    ) : NexusEvent
}
