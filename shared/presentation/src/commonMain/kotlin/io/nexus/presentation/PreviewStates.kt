package io.nexus.presentation

/** Data states that every data-driven Stage 04 screen can demonstrate. */
public enum class PreviewScenario {
    READY,
    LOADING,
    EMPTY,
    OFFLINE,
    ERROR,
}

/** Simulated global network states; no real peer connection is created. */
public enum class PreviewNetworkState {
    OFFLINE,
    CONNECTING,
    ONLINE,
    DEGRADED,
}
