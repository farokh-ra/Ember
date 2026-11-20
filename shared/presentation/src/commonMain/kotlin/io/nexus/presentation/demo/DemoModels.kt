package io.nexus.presentation.demo

import io.nexus.presentation.PreviewNetworkState

/** Categories used by the Stage 04 demo catalogue. */
public enum class DemoCategory {
    DOCUMENTS,
    MEDIA,
    ARCHIVES,
    OTHER,
}

/**
 * File-level availability uses the canonical vocabulary from the product
 * requirements. UI code maps each value to one exact label.
 */
public enum class PublicationAvailability {
    /** At least one source recently proved it can serve this content. */
    AVAILABLE_NOW,

    /** A source was observed recently but is not responding now. */
    SEEN_RECENTLY,

    /** The record is known, but no source is known right now. */
    NO_KNOWN_SOURCE,

    /** Only the signed record is stored; content is not available locally or online. */
    RECORD_ONLY,
}

/** Download lifecycle shown by the Downloads screen. */
public enum class DownloadStatus {
    DOWNLOADING,
    PAUSED,
    VERIFYING,
    COMPLETE,
    FAILED,
}

/** Verification state for an explicit hosted replica. */
public enum class ReplicaVerification {
    VERIFIED,
    VERIFYING,
    PAUSED,
    FAILED,
}

/** Simulated bootstrap path state. */
public enum class BootstrapPathState {
    CONNECTED,
    CONNECTING,
    UNREACHABLE,
}

/** Simulated best-path classification shown by Network status. */
public enum class DemoConnectivity {
    DIRECT,
    RELAY,
    CONNECTING,
    NO_PEERS,
    DISABLED,
}

/** Simulated NAT reachability estimate. */
public enum class DemoNatStatus {
    PUBLIC,
    PRIVATE,
    UNKNOWN,
    DISABLED,
}

/** User-safe demo error categories used by Network status. */
public enum class DemoNetworkErrorCode {
    NO_USABLE_PATH,
    BOOTSTRAP_UNREACHABLE,
    PEER_TIMEOUT,
    RELAY_UNAVAILABLE,
    RELAY_RESERVATION_FAILED,
}

/** A known source for a demo publication. */
public data class DemoSource(
    public val peerLabel: String,
    public val online: Boolean,
    public val lastSeenLabel: String,
)

/** One sample publication record. */
public data class DemoPublication(
    public val recordId: String,
    public val title: String,
    public val fileName: String,
    public val description: String,
    public val category: DemoCategory,
    public val tags: List<String>,
    public val sizeLabel: String,
    public val publishedLabel: String,
    public val uploaderDisplayName: String,
    public val ownedByViewer: Boolean,
    public val availability: PublicationAvailability,
    public val sources: List<DemoSource>,
    public val recordVerified: Boolean,
    public val contentVerified: Boolean,
    public val unpublished: Boolean,
    public val recordVersion: Int,
    public val contentId: String,
    public val isDemo: Boolean = true,
)

/** Demo profile shown on the profile screen. */
public data class DemoProfile(
    public val displayName: String,
    public val usernameLabel: String,
    public val bio: String,
    public val keyFingerprint: String,
    public val publicationCount: Int,
    public val hostedReplicaCount: Int,
    public val joinedLabel: String,
    public val isDemo: Boolean = true,
)

/** One sample download in the local library. */
public data class DemoDownload(
    public val id: String,
    public val title: String,
    public val fileName: String,
    public val sizeLabel: String,
    public val transferredLabel: String,
    public val progress: Float,
    public val status: DownloadStatus,
    public val contentVerified: Boolean,
    public val sourceCount: Int,
    public val updatedLabel: String,
    public val speedLabel: String? = null,
    public val sourcePeer: String? = null,
    public val isDemo: Boolean = true,
)

/** One sample file the user explicitly chose to host. */
public data class DemoHostedFile(
    public val id: String,
    public val title: String,
    public val originalUploader: String,
    public val sizeLabel: String,
    public val verification: ReplicaVerification,
    public val progress: Float?,
    public val peersServed: Int,
    public val lastVerifiedLabel: String?,
    public val isDemo: Boolean = true,
)

/** One configured bootstrap path. */
public data class DemoBootstrapPath(
    public val label: String,
    public val endpoint: String,
    public val state: BootstrapPathState,
)

/** One configured community relay. */
public data class DemoRelayPath(
    public val label: String,
    public val endpoint: String,
    public val state: BootstrapPathState,
    public val detail: String,
)

/** A redacted demo network error. */
public data class DemoNetworkError(
    public val code: DemoNetworkErrorCode,
    public val occurredLabel: String,
)

/** Simulated global network snapshot. */
public data class DemoNetworkSnapshot(
    public val state: PreviewNetworkState,
    public val connectivity: DemoConnectivity,
    public val natStatus: DemoNatStatus,
    public val natPublicAddress: String?,
    public val relayFallbackEnabled: Boolean,
    public val peerCount: Int,
    public val directConnections: Int,
    public val relayedConnections: Int,
    public val fallbackAttempts: Int,
    public val fallbackSuccesses: Int,
    public val relayDataLimitLabel: String?,
    public val relayDurationLimitLabel: String?,
    public val relayPaths: List<DemoRelayPath>,
    public val bootstrapPaths: List<DemoBootstrapPath>,
    public val recentErrors: List<DemoNetworkError>,
    public val troubleshootingHint: String,
    public val isDemo: Boolean = true,
)
