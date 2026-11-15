package io.nexus.presentation

import io.nexus.core.api.CoreSnapshot
import io.nexus.core.api.NexusCore

/** Minimal presentation facade established in Stage 03. */
public class StartupPresenter(
    private val core: NexusCore,
) {
    /** Initial state before the first core snapshot is read. */
    public fun initialState(): StartupState = StartupState.Loading

    /** Reads the current core snapshot without starting network work. */
    public fun snapshot(): CoreSnapshot = core.snapshot()
}
