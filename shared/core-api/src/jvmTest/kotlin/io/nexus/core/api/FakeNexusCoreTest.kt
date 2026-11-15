package io.nexus.core.api

import io.nexus.model.CoreState
import io.nexus.model.NexusEvent
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals

class FakeNexusCoreTest {
    @Test
    fun startPublishesReadyState() =
        runTest {
            val core = FakeNexusCore()

            core.start()

            assertEquals(CoreState.READY, core.snapshot().state)
            val event = core.events.first() as NexusEvent.StateChanged
            assertEquals(CoreState.READY, event.state)
        }
}
