package io.nexus.presentation

/** Startup states rendered by the application shell. */
public sealed interface StartupState {
    /** The shell is waiting for the core handshake. */
    public data object Loading : StartupState

    /** The core is ready for local operations. */
    public data class Ready(
        public val protocolVersion: String,
    ) : StartupState

    /** The core could not start; the reason is user-safe and already redacted. */
    public data class Failed(
        public val reason: String,
    ) : StartupState
}
