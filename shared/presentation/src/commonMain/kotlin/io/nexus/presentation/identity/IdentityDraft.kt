package io.nexus.presentation.identity

/** Input errors that the identity setup screen can display without exposing internals. */
public enum class DisplayNameError {
    EMPTY,
    TOO_SHORT,
    TOO_LONG,
}

/** Input errors for the optional profile bio. */
public enum class BioError {
    TOO_LONG,
}

/**
 * Local identity setup form state.
 *
 * Stage 04 never generates keys or stores a profile. The draft exists so
 * validation, error presentation, and navigation can be tested before the real
 * identity stage.
 */
public data class IdentityDraft(
    public val displayName: String = "",
    public val bio: String = "",
) {
    public val trimmedDisplayName: String
        get() = displayName.trim()

    public val trimmedBio: String
        get() = bio.trim()

    public val displayNameError: DisplayNameError?
        get() =
            when {
                trimmedDisplayName.isEmpty() -> DisplayNameError.EMPTY
                trimmedDisplayName.length < MIN_NAME_LENGTH -> DisplayNameError.TOO_SHORT
                trimmedDisplayName.length > MAX_NAME_LENGTH -> DisplayNameError.TOO_LONG
                else -> null
            }

    public val bioError: BioError?
        get() = if (trimmedBio.length > MAX_BIO_LENGTH) BioError.TOO_LONG else null

    /** True when the demo profile can be opened. No data is saved or published. */
    public val canContinue: Boolean
        get() = displayNameError == null && bioError == null

    /** Returns the draft with surrounding whitespace removed. */
    public fun sanitized(): IdentityDraft =
        copy(
            displayName = trimmedDisplayName,
            bio = trimmedBio,
        )

    public companion object {
        public const val MIN_NAME_LENGTH: Int = 2
        public const val MAX_NAME_LENGTH: Int = 48
        public const val MAX_BIO_LENGTH: Int = 280
    }
}
