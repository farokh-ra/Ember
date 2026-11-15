package io.nexus.presentation.navigation

/**
 * Every destination in the Stage 04 navigation prototype.
 *
 * Routes are stable tokens used for state restoration. They contain no display
 * text, so screens can render localized labels without changing navigation
 * state.
 */
public sealed interface AppDestination {
    /** Stable token used to persist and restore this destination. */
    public val route: String

    /** First-launch welcome and honest preview introduction. */
    public data object Welcome : AppDestination {
        override val route: String = "welcome"
    }

    /** Local identity setup form; Stage 04 only creates a demo session. */
    public data object IdentitySetup : AppDestination {
        override val route: String = "identity"
    }

    /** Discoverable records grouped by honest availability states. */
    public data object Explore : AppDestination {
        override val route: String = "explore"
    }

    /** Local and discovered metadata search prototype. */
    public data object Search : AppDestination {
        override val route: String = "search"
    }

    /** The current user's demo profile and publications. */
    public data object Profile : AppDestination {
        override val route: String = "profile"
    }

    /** Details for one publication record. */
    public data class FileDetails(
        public val recordId: String,
    ) : AppDestination {
        init {
            require(recordId.isNotBlank()) { "recordId must not be blank" }
            require(!recordId.contains('/')) { "recordId must not contain '/'" }
        }

        override val route: String = "$FILE_PREFIX$recordId"

        private companion object {
            const val FILE_PREFIX: String = "file/"
        }
    }

    /** Publish form prototype; publishing stays unavailable in Stage 04. */
    public data object Publish : AppDestination {
        override val route: String = "publish"
    }

    /** Local download library and transfer states. */
    public data object Downloads : AppDestination {
        override val route: String = "downloads"
    }

    /** Files the user explicitly chose to replicate and serve. */
    public data object HostedFiles : AppDestination {
        override val route: String = "hosted"
    }

    /** Local settings and honest placeholders for later stages. */
    public data object Settings : AppDestination {
        override val route: String = "settings"
    }

    /** Global connectivity, bootstrap paths, peers, and redacted errors. */
    public data object NetworkStatus : AppDestination {
        override val route: String = "network"
    }

    public companion object {
        private const val FILE_ROUTE_PREFIX: String = "file/"

        /** Destinations shown in the primary navigation shell. */
        public val primarySections: List<AppDestination> =
            listOf(
                Explore,
                Search,
                Downloads,
                HostedFiles,
                Settings,
            )

        /** Resolves a persisted route token, or null when it is unknown. */
        public fun fromRoute(route: String): AppDestination? =
            when {
                route == Welcome.route -> {
                    Welcome
                }

                route == IdentitySetup.route -> {
                    IdentitySetup
                }

                route == Explore.route -> {
                    Explore
                }

                route == Search.route -> {
                    Search
                }

                route == Profile.route -> {
                    Profile
                }

                route == Publish.route -> {
                    Publish
                }

                route == Downloads.route -> {
                    Downloads
                }

                route == HostedFiles.route -> {
                    HostedFiles
                }

                route == Settings.route -> {
                    Settings
                }

                route == NetworkStatus.route -> {
                    NetworkStatus
                }

                route.startsWith(FILE_ROUTE_PREFIX) -> {
                    route
                        .removePrefix(FILE_ROUTE_PREFIX)
                        .takeIf { it.isNotBlank() && !it.contains('/') }
                        ?.let(::FileDetails)
                }

                else -> {
                    null
                }
            }
    }
}
