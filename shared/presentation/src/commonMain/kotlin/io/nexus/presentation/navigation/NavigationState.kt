package io.nexus.presentation.navigation

/**
 * Immutable back stack for the application.
 *
 * The stack is never empty. Root destinations keep back navigation disabled so
 * the platform shell can decide whether to exit the app.
 */
@ConsistentCopyVisibility
public data class NavigationState private constructor(
    public val backStack: List<AppDestination>,
) {
    init {
        require(backStack.isNotEmpty()) { "Navigation back stack must not be empty" }
    }

    /** The screen currently shown. */
    public val current: AppDestination
        get() = backStack.last()

    /** True when a back action can pop to another screen. */
    public val canGoBack: Boolean
        get() = backStack.size > 1

    /**
     * The primary section containing the current screen.
     *
     * Detail screens keep the section they were opened from highlighted in the
     * navigation rail and bottom bar.
     */
    public val activeSection: AppDestination?
        get() = backStack.lastOrNull { it in AppDestination.primarySections }

    /** Pushes a screen, ignoring a repeated navigation to the current screen. */
    public fun push(destination: AppDestination): NavigationState =
        if (destination == current) {
            this
        } else {
            NavigationState(backStack + destination)
        }

    /** Replaces the current screen without growing the stack. */
    public fun replace(destination: AppDestination): NavigationState =
        NavigationState(backStack.dropLast(1) + destination)

    /** Pops the current screen when possible; otherwise returns this state. */
    public fun back(): NavigationState =
        if (canGoBack) {
            NavigationState(backStack.dropLast(1))
        } else {
            this
        }

    /**
     * Selects a primary section.
     *
     * Tab changes reset the section stack so the user lands on the section root
     * rather than on a stale detail screen.
     */
    public fun selectSection(destination: AppDestination): NavigationState {
        require(destination in AppDestination.primarySections) {
            "${destination.route} is not a primary section"
        }
        return if (current == destination && backStack.size == 1) {
            this
        } else {
            of(destination)
        }
    }

    /** Clears history and shows exactly one destination. */
    public fun resetTo(destination: AppDestination): NavigationState = of(destination)

    /** Serializes this stack for platform state restoration. */
    public fun toRouteTokens(): List<String> = backStack.map(AppDestination::route)

    public companion object {
        /** Creates a stack with at least one destination. */
        public fun of(vararg destinations: AppDestination): NavigationState {
            require(destinations.isNotEmpty()) { "At least one destination is required" }
            return NavigationState(destinations.toList())
        }

        /** Creates the first-launch stack. */
        public fun initial(): NavigationState = of(AppDestination.Welcome)

        /**
         * Restores a stack from persisted route tokens.
         *
         * Unknown or empty token lists are rejected so callers can fall back to
         * a safe first-launch state.
         */
        @Suppress("ReturnCount") // Unknown routes exit early.
        public fun fromRouteTokens(tokens: List<String>): NavigationState? {
            if (tokens.isEmpty()) {
                return null
            }
            val destinations = tokens.map { AppDestination.fromRoute(it) ?: return null }
            return NavigationState(destinations)
        }
    }
}
