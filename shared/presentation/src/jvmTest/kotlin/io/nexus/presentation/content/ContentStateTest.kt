package io.nexus.presentation.content

import io.nexus.presentation.PreviewScenario
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ContentStateTest {
    @Test
    fun everyPreviewScenarioMapsToADistinctContentState() {
        assertEquals(ContentState.Ready("sample"), previewContentState(PreviewScenario.READY) { "sample" })
        assertEquals(ContentState.Loading, previewContentState(PreviewScenario.LOADING) { "sample" })
        assertEquals(ContentState.Empty, previewContentState(PreviewScenario.EMPTY) { "sample" })
        assertEquals(ContentState.Offline, previewContentState(PreviewScenario.OFFLINE) { "sample" })
        assertEquals(
            ContentState.Error(UiErrorCode.DEMO_LOAD_FAILED),
            previewContentState(PreviewScenario.ERROR) { "sample" },
        )
    }

    @Test
    fun readyValueIsNotEvaluatedForNonReadyStates() {
        var evaluated = false

        previewContentState(PreviewScenario.LOADING) {
            evaluated = true
            "sample"
        }

        assertFalse(evaluated)
    }

    @Test
    fun readyValueIsEvaluatedOnlyWhenVisible() {
        var evaluations = 0

        val state =
            previewContentState(PreviewScenario.READY) {
                evaluations += 1
                "sample"
            }

        assertEquals(ContentState.Ready("sample"), state)
        assertEquals(1, evaluations)
    }

    @Test
    fun errorStatesExposeOnlyUserSafeCodes() {
        val state = previewContentState<String>(PreviewScenario.ERROR) { error("not reached") }

        assertTrue(state is ContentState.Error)
        assertEquals(UiErrorCode.DEMO_LOAD_FAILED, state.code)
    }
}
