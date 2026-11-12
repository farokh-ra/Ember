package io.nexus.presentation

import io.nexus.core.api.FakeNexusCore
import io.nexus.model.CoreState
import kotlin.test.Test
import kotlin.test.assertEquals

class StartupPresenterTest {
    @Test
    fun initialStateIsLoading() {
        val presenter = StartupPresenter(FakeNexusCore())

        assertEquals(StartupState.Loading, presenter.initialState())
        assertEquals(CoreState.STOPPED, presenter.snapshot().state)
    }
}
