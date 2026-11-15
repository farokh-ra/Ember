package io.nexus.core.api

import io.nexus.model.CoreState
import io.nexus.model.NexusEvent
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow

/** Deterministic in-memory core used by tests and Stage 03 shells. */
public class FakeNexusCore : NexusCore {
    private val state = MutableStateFlow(CoreState.STOPPED)
    private val eventFlow = MutableStateFlow<NexusEvent>(NexusEvent.StateChanged(CoreState.STOPPED))

    override val events: Flow<NexusEvent> = eventFlow.asStateFlow()

    override suspend fun start() {
        state.value = CoreState.READY
        eventFlow.value = NexusEvent.StateChanged(CoreState.READY)
    }

    override suspend fun stop() {
        state.value = CoreState.STOPPED
        eventFlow.value = NexusEvent.StateChanged(CoreState.STOPPED)
    }

    override fun snapshot(): CoreSnapshot = CoreSnapshot(state = state.value, protocolVersion = "stage-03")
}
