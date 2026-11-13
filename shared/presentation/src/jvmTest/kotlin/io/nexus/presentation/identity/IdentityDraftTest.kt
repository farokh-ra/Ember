package io.nexus.presentation.identity

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class IdentityDraftTest {
    @Test
    fun blankDisplayNameIsRejected() {
        assertEquals(DisplayNameError.EMPTY, IdentityDraft(displayName = "   ").displayNameError)
    }

    @Test
    fun shortDisplayNameIsRejected() {
        assertEquals(DisplayNameError.TOO_SHORT, IdentityDraft(displayName = "A").displayNameError)
    }

    @Test
    fun boundaryLengthsAreAccepted() {
        val lowest = "A".repeat(IdentityDraft.MIN_NAME_LENGTH)
        val highest = "A".repeat(IdentityDraft.MAX_NAME_LENGTH)

        assertNull(IdentityDraft(displayName = lowest).displayNameError)
        assertNull(IdentityDraft(displayName = highest).displayNameError)
    }

    @Test
    fun tooLongDisplayNameIsRejected() {
        assertEquals(
            DisplayNameError.TOO_LONG,
            IdentityDraft(displayName = "A".repeat(IdentityDraft.MAX_NAME_LENGTH + 1)).displayNameError,
        )
    }

    @Test
    fun surroundingWhitespaceIsIgnoredForValidation() {
        val draft = IdentityDraft(displayName = "  Sam Rivera  ")

        assertTrue(draft.canContinue)
        assertEquals("Sam Rivera", draft.trimmedDisplayName)
        assertEquals("Sam Rivera", draft.sanitized().displayName)
    }

    @Test
    fun bioLengthIsBounded() {
        val valid = IdentityDraft(displayName = "Sam", bio = "B".repeat(IdentityDraft.MAX_BIO_LENGTH))
        val invalid = IdentityDraft(displayName = "Sam", bio = "B".repeat(IdentityDraft.MAX_BIO_LENGTH + 1))

        assertNull(valid.bioError)
        assertTrue(valid.canContinue)
        assertEquals(BioError.TOO_LONG, invalid.bioError)
        assertFalse(invalid.canContinue)
    }

    @Test
    fun bioIsTrimmed() {
        val draft = IdentityDraft(displayName = "Sam", bio = "  hello  ").sanitized()

        assertEquals("hello", draft.bio)
    }
}
