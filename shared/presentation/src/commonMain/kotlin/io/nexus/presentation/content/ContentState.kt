package io.nexus.presentation.content

import io.nexus.presentation.PreviewScenario

/** User-safe error categories. Raw exceptions and local paths never cross this boundary. */
public enum class UiErrorCode {
    /** A demo error used to inspect the Stage 04 error and retry presentation. */
    DEMO_LOAD_FAILED,

    /** Local metadata or the local database could not be read. */
    LOCAL_STORE_UNAVAILABLE,
}

/** The five content states required for every data-driven screen. */
public sealed interface ContentState<out T> {
    /** Data is being read; the screen must offer a way out of this state. */
    public data object Loading : ContentState<Nothing>

    /** The data source returned no records. */
    public data object Empty : ContentState<Nothing>

    /** The local node has no usable network path; local data may still be usable. */
    public data object Offline : ContentState<Nothing>

    /** Loading failed with a user-safe category. */
    public data class Error(
        public val code: UiErrorCode,
    ) : ContentState<Nothing>

    /** Content is available for rendering. */
    public data class Ready<T>(
        public val value: T,
    ) : ContentState<T>
}

/** Maps a Stage 04 preview selection to a content state. */
public fun <T> previewContentState(
    scenario: PreviewScenario,
    readyValue: () -> T,
): ContentState<T> =
    when (scenario) {
        PreviewScenario.READY -> ContentState.Ready(readyValue())
        PreviewScenario.LOADING -> ContentState.Loading
        PreviewScenario.EMPTY -> ContentState.Empty
        PreviewScenario.OFFLINE -> ContentState.Offline
        PreviewScenario.ERROR -> ContentState.Error(UiErrorCode.DEMO_LOAD_FAILED)
    }
