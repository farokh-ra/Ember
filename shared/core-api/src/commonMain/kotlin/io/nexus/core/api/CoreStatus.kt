package io.nexus.core.api

import io.nexus.model.CoreState

/** Availability of the native core as seen by the application shell. */
public enum class CoreAvailability {
    UNAVAILABLE,
    STARTING,
    READY,
    DEGRADED,
    STOPPED,
}

/**
 * User-facing lifecycle status of the native core.
 *
 * [detail] carries a short, non-sensitive reason when the core cannot start.
 * It must never contain file paths, keys, or other private data.
 */
public data class CoreStatus(
    public val availability: CoreAvailability,
    public val protocolVersion: String = "unknown",
    public val detail: String? = null,
) {
    public val isReady: Boolean
        get() = availability == CoreAvailability.READY

    public companion object {
        /** Status used before any start attempt and when the library is absent. */
        public fun unavailable(detail: String? = null): CoreStatus =
            CoreStatus(CoreAvailability.UNAVAILABLE, protocolVersion = "unknown", detail = detail)

        /** Maps a core lifecycle state onto the shell-facing status. */
        public fun from(
            state: CoreState,
            protocolVersion: String,
            detail: String? = null,
        ): CoreStatus =
            CoreStatus(
                availability =
                    when (state) {
                        CoreState.STOPPED -> CoreAvailability.STOPPED
                        CoreState.STARTING -> CoreAvailability.STARTING
                        CoreState.READY -> CoreAvailability.READY
                        CoreState.DEGRADED -> CoreAvailability.DEGRADED
                    },
                protocolVersion = protocolVersion,
                detail = detail,
            )
    }
}
