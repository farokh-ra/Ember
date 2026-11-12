package io.nexus.core.client

import io.nexus.core.api.FakeNexusCore
import io.nexus.model.CoreState
import kotlin.test.Test
import kotlin.test.assertEquals

class CoreClientTest {
    @Test
    fun clientPreservesCoreContract() {
        val client = CoreClient(FakeNexusCore())

        assertEquals(CoreState.STOPPED, client.snapshot().state)
        assertEquals(1, CoreClient.ABI_VERSION)
    }
}
