package io.nexus.presentation.demo

import io.nexus.presentation.PreviewNetworkState

/**
 * Deterministic sample content for the Stage 04 screen prototypes.
 *
 * Nothing here is fetched, signed, stored, or announced. UI screens must show a
 * visible demo label whenever this catalogue is rendered.
 */
public object DemoCatalog {
    public const val IS_DEMO: Boolean = true

    public val profile: DemoProfile =
        DemoProfile(
            displayName = "Avery Chen",
            usernameLabel = "avery",
            bio = "Demo profile for exploring the Ember interface. No real identity or key exists.",
            keyFingerprint = "nx1:9F3A 71C2 8B40 D6E5 47A1 2C88",
            publicationCount = 2,
            hostedReplicaCount = 4,
            joinedLabel = "12 Oct 2026",
            identityId = "nx1demo5d2c91a46f803b7e1a550c429d187e36",
            recordRevision = 3,
            recordState = ProfileRecordState.PUBLISHED,
            freshness = ProfileFreshness.CURRENT,
            publishedLabel = "20 Nov 2026",
            lastCheckedLabel = "just now",
            conflictCount = 1,
            usernameCollisions =
                listOf(
                    DemoUsernameCollision(
                        identityId = "nx1demo0c4f7b2a9e6d8135a0b7c4e2d9f68135",
                        displayName = "Avery Chen Archive",
                        usernameLabel = "Avery",
                    ),
                ),
        )

    public val publications: List<DemoPublication> =
        listOf(
            DemoPublication(
                recordId = "nx-demo-coastal-field-notes",
                title = "Coastal Field Notes",
                fileName = "Coastal Field Notes.txt",
                description = "Sample research notes from a coastal survey. Demo content only.",
                category = DemoCategory.DOCUMENTS,
                tags = listOf("field notes", "coast", "research"),
                sizeLabel = "2.4 MB",
                publishedLabel = "8 Nov 2026",
                uploaderDisplayName = "Avery Chen",
                ownedByViewer = true,
                availability = PublicationAvailability.AVAILABLE_NOW,
                sources =
                    listOf(
                        DemoSource(
                            peerLabel = "Peer 7F3A…9C21",
                            online = true,
                            lastSeenLabel = "just now",
                        ),
                        DemoSource(
                            peerLabel = "Peer B44D…10E8",
                            online = true,
                            lastSeenLabel = "1 min ago",
                        ),
                    ),
                recordVerified = true,
                contentVerified = true,
                unpublished = false,
                recordVersion = 1,
                contentId = "21eb3e82caf8d600b77ac313831e1b9e0d3b52b1cbb4c3c9f8e630d2a7388ac7",
                serving = true,
                preview =
                    DemoPreview(
                        kind = DemoPreviewKind.TEXT,
                        supported = true,
                        reason = DemoPreviewReason.PLAIN_TEXT,
                    ),
            ),
            DemoPublication(
                recordId = "nx-demo-community-garden-plan",
                title = "Community Garden Plan",
                fileName = "Community Garden Plan.zip",
                description = "Sample planning archive for a neighbourhood garden. Demo content only.",
                category = DemoCategory.ARCHIVES,
                tags = listOf("garden", "community", "planning"),
                sizeLabel = "18.7 MB",
                publishedLabel = "3 Nov 2026",
                uploaderDisplayName = "Mira Sol",
                ownedByViewer = false,
                availability = PublicationAvailability.SEEN_RECENTLY,
                sources =
                    listOf(
                        DemoSource(
                            peerLabel = "Peer D912…44AB",
                            online = false,
                            lastSeenLabel = "4 h ago",
                        ),
                        DemoSource(
                            peerLabel = "Peer 0C77…E3F1",
                            online = false,
                            lastSeenLabel = "1 day ago",
                        ),
                    ),
                recordVerified = true,
                contentVerified = false,
                unpublished = false,
                recordVersion = 1,
                contentId = "5c949e7d6401d358ad83b86724b9e7e2eb2535e0565fa0b23e74ddfcf96694ff",
                serving = false,
                preview =
                    DemoPreview(
                        kind = DemoPreviewKind.NONE,
                        supported = false,
                        reason = DemoPreviewReason.UNSUPPORTED_FORMAT,
                    ),
            ),
            DemoPublication(
                recordId = "nx-demo-protocol-overview",
                title = "Ember Protocol Overview",
                fileName = "Ember Protocol Overview.md",
                description = "Sample protocol notes. The record is known, but no content source is known.",
                category = DemoCategory.DOCUMENTS,
                tags = listOf("protocol", "overview", "documentation"),
                sizeLabel = "42 KB",
                publishedLabel = "1 Nov 2026",
                uploaderDisplayName = "Rowan Vale",
                ownedByViewer = false,
                availability = PublicationAvailability.RECORD_ONLY,
                sources = emptyList(),
                recordVerified = true,
                contentVerified = false,
                unpublished = false,
                recordVersion = 2,
                contentId = "876f9dbbabbaf39949bb6d471fa793d098cd72d46028cd29c8569bdf7a455430",
                serving = false,
                preview =
                    DemoPreview(
                        kind = DemoPreviewKind.TEXT,
                        supported = true,
                        reason = DemoPreviewReason.PLAIN_TEXT,
                    ),
            ),
            DemoPublication(
                recordId = "nx-demo-dawn-chorus",
                title = "Dawn Chorus — Field Recording",
                fileName = "Dawn Chorus - Field Recording.ogg",
                description = "Sample audio field recording. Demo content only.",
                category = DemoCategory.MEDIA,
                tags = listOf("audio", "nature", "field recording"),
                sizeLabel = "34.1 MB",
                publishedLabel = "2 Nov 2026",
                uploaderDisplayName = "Rowan Vale",
                ownedByViewer = false,
                availability = PublicationAvailability.AVAILABLE_NOW,
                sources =
                    listOf(
                        DemoSource(
                            peerLabel = "Peer 2A91…C0D4",
                            online = true,
                            lastSeenLabel = "just now",
                        ),
                        DemoSource(
                            peerLabel = "Peer 6E10…8B77",
                            online = true,
                            lastSeenLabel = "2 min ago",
                        ),
                        DemoSource(
                            peerLabel = "Peer 93C2…51AF",
                            online = true,
                            lastSeenLabel = "3 min ago",
                        ),
                    ),
                recordVerified = true,
                contentVerified = false,
                unpublished = false,
                recordVersion = 1,
                contentId = "9903861d4e6aeaa3beb51061adfc683297c041ac90047f0ff5ae5f236211bf27",
                serving = false,
                preview =
                    DemoPreview(
                        kind = DemoPreviewKind.NONE,
                        supported = false,
                        reason = DemoPreviewReason.UNSUPPORTED_FORMAT,
                    ),
            ),
            DemoPublication(
                recordId = "nx-demo-open-type-specimen",
                title = "Open Type Specimen",
                fileName = "Open Type Specimen.pdf",
                description = "Sample typography specimen. No source is known for this record.",
                category = DemoCategory.DOCUMENTS,
                tags = listOf("typography", "design", "specimen"),
                sizeLabel = "6.8 MB",
                publishedLabel = "28 Oct 2026",
                uploaderDisplayName = "Imani Okafor",
                ownedByViewer = false,
                availability = PublicationAvailability.NO_KNOWN_SOURCE,
                sources = emptyList(),
                recordVerified = true,
                contentVerified = false,
                unpublished = false,
                recordVersion = 1,
                contentId = "7a1e3d80de38223183f5b447435f0710159035a197573e97dae1dfc14d90cdfc",
                serving = false,
                preview =
                    DemoPreview(
                        kind = DemoPreviewKind.NONE,
                        supported = false,
                        reason = DemoPreviewReason.UNSUPPORTED_FORMAT,
                    ),
            ),
            DemoPublication(
                recordId = "nx-demo-mutual-aid-map",
                title = "Neighbourhood Mutual Aid Map",
                fileName = "Neighbourhood Mutual Aid Map.pdf",
                description = "Sample community map that the uploader unpublished. Copies may still exist.",
                category = DemoCategory.DOCUMENTS,
                tags = listOf("community", "map", "mutual aid"),
                sizeLabel = "3.1 MB",
                publishedLabel = "20 Oct 2026",
                uploaderDisplayName = "Mira Sol",
                ownedByViewer = false,
                availability = PublicationAvailability.SEEN_RECENTLY,
                sources =
                    listOf(
                        DemoSource(
                            peerLabel = "Peer D912…44AB",
                            online = false,
                            lastSeenLabel = "2 days ago",
                        ),
                    ),
                recordVerified = true,
                contentVerified = false,
                unpublished = true,
                recordVersion = 2,
                contentId = "ddb50b23b2e56e538885a53a9b7097506fd589ab10c6ae954fd5204a6243393f",
                serving = false,
                preview =
                    DemoPreview(
                        kind = DemoPreviewKind.NONE,
                        supported = false,
                        reason = DemoPreviewReason.UNSUPPORTED_FORMAT,
                    ),
            ),
            DemoPublication(
                recordId = "nx-demo-repair-cafe",
                title = "Repair Café Schedule",
                fileName = "Repair Cafe Schedule.pdf",
                description = "Sample schedule stored as a record only. Demo content only.",
                category = DemoCategory.DOCUMENTS,
                tags = listOf("repair", "schedule", "community"),
                sizeLabel = "212 KB",
                publishedLabel = "9 Nov 2026",
                uploaderDisplayName = "Avery Chen",
                ownedByViewer = true,
                availability = PublicationAvailability.RECORD_ONLY,
                sources = emptyList(),
                recordVerified = true,
                contentVerified = false,
                unpublished = false,
                recordVersion = 1,
                contentId = "7f2569de739dcf179ae45d2e7647db974f5916429f79aa740f35f121f0b9d4fa",
                serving = false,
                preview =
                    DemoPreview(
                        kind = DemoPreviewKind.NONE,
                        supported = false,
                        reason = DemoPreviewReason.UNSUPPORTED_FORMAT,
                    ),
            ),
        )

    public val downloads: List<DemoDownload> =
        listOf(
            DemoDownload(
                id = "download-garden-plan",
                title = "Community Garden Plan",
                fileName = "Community Garden Plan.zip",
                sizeLabel = "18.7 MB",
                transferredLabel = "7.9 MB of 18.7 MB",
                progress = 0.42f,
                status = DownloadStatus.DOWNLOADING,
                contentVerified = false,
                sourceCount = 2,
                updatedLabel = "2 min ago",
                speedLabel = "1.8 MB/s",
                sourcePeer = "12D3KooW...4f8a",
            ),
            DemoDownload(
                id = "download-type-specimen",
                title = "Open Type Specimen",
                fileName = "Open Type Specimen.pdf",
                sizeLabel = "6.8 MB",
                transferredLabel = "4.1 MB of 6.8 MB",
                progress = 0.61f,
                status = DownloadStatus.PAUSED,
                contentVerified = false,
                sourceCount = 1,
                updatedLabel = "18 min ago",
                sourcePeer = "12D3KooW...7c1d",
            ),
            DemoDownload(
                id = "download-dawn-chorus",
                title = "Dawn Chorus — Field Recording",
                fileName = "Dawn Chorus - Field Recording.ogg",
                sizeLabel = "34.1 MB",
                transferredLabel = "31.4 MB of 34.1 MB",
                progress = 0.92f,
                status = DownloadStatus.VERIFYING,
                contentVerified = false,
                sourceCount = 3,
                updatedLabel = "Verifying chunks",
                sourcePeer = "12D3KooW...2b9e",
            ),
            DemoDownload(
                id = "download-coastal-notes",
                title = "Coastal Field Notes",
                fileName = "Coastal Field Notes.txt",
                sizeLabel = "2.4 MB",
                transferredLabel = "2.4 MB of 2.4 MB",
                progress = 1f,
                status = DownloadStatus.COMPLETE,
                contentVerified = true,
                sourceCount = 2,
                updatedLabel = "Complete after verification",
                sourcePeer = "12D3KooW...5a3f",
            ),
            DemoDownload(
                id = "download-mutual-aid-map",
                title = "Neighbourhood Mutual Aid Map",
                fileName = "Neighbourhood Mutual Aid Map.pdf",
                sizeLabel = "3.1 MB",
                transferredLabel = "0.4 MB of 3.1 MB",
                progress = 0.12f,
                status = DownloadStatus.FAILED,
                contentVerified = false,
                sourceCount = 0,
                updatedLabel = "Source unavailable",
            ),
        )

    public val hostedFiles: List<DemoHostedFile> =
        listOf(
            DemoHostedFile(
                id = "hosted-garden-plan",
                title = "Community Garden Plan",
                originalUploader = "Mira Sol",
                publicationId = "3f9a2c7d41b8e6059d4a1c2e8f7b3a6c5d0e9f1a2b3c4d5e6f708192a3b4c5d0",
                sizeLabel = "18.7 MB",
                storageLabel = "18.7 MB",
                verification = ReplicaVerification.VERIFIED,
                serving = true,
                progress = 1f,
                peersServed = 2,
                lastVerifiedLabel = "3 h ago",
                lastSeenLabel = "12 min ago",
            ),
            DemoHostedFile(
                id = "hosted-dawn-chorus",
                title = "Dawn Chorus — Field Recording",
                originalUploader = "Rowan Vale",
                publicationId = "8b2e4d6f0a1c3e5d7f9b1d3a5c7e9f0b2d4a6c8e0f1b3d5a7c9e1f2a4b6c8d0e",
                sizeLabel = "34.1 MB",
                storageLabel = "21.8 MB",
                verification = ReplicaVerification.VERIFYING,
                serving = false,
                progress = 0.64f,
                peersServed = 0,
                lastVerifiedLabel = null,
                lastSeenLabel = null,
            ),
            DemoHostedFile(
                id = "hosted-protocol-overview",
                title = "Ember Protocol Overview",
                originalUploader = "Rowan Vale",
                publicationId = "c1d3e5f7a9b0c2e4d6f8a1b3c5e7d9f0a2b4c6e8f1a3b5d7c9e0f2a4b6d8c1e3",
                sizeLabel = "42 KB",
                storageLabel = "42 KB",
                verification = ReplicaVerification.PAUSED,
                serving = false,
                progress = 1f,
                peersServed = 0,
                lastVerifiedLabel = "6 days ago",
                lastSeenLabel = "2 days ago",
            ),
            DemoHostedFile(
                id = "hosted-type-specimen",
                title = "Open Type Specimen",
                originalUploader = "Imani Okafor",
                publicationId = "5a7c9e1f3b5d7f9a1c3e5f7b9d1a3c5e7f9b1d3a5c7e9f1b3d5a7c9e1f3b5d7f",
                sizeLabel = "6.8 MB",
                storageLabel = "6.8 MB",
                verification = ReplicaVerification.FAILED,
                serving = false,
                progress = 0f,
                peersServed = 0,
                lastVerifiedLabel = null,
                lastSeenLabel = null,
            ),
        )

    /** Publications attributed to the current demo profile. */
    public val profilePublications: List<DemoPublication> =
        publications.filter(DemoPublication::ownedByViewer)

    /** Finds one sample publication by its stable demo record ID. */
    public fun publication(recordId: String): DemoPublication? = publications.firstOrNull { it.recordId == recordId }

    /**
     * Filters sample metadata by query and category.
     *
     * This is a local sample search, not a distributed search. Callers must
     * show coverage and freshness labels.
     */
    public fun search(
        query: String,
        category: DemoCategory? = null,
    ): List<DemoPublication> {
        val needle = query.trim()
        return publications
            .asSequence()
            .filter { category == null || it.category == category }
            .filter { publication -> needle.isEmpty() || publication.matches(needle) }
            .sortedWith(
                compareBy<DemoPublication> { it.availability.ordinal }
                    .thenBy(DemoPublication::title),
            ).toList()
    }

    /** Builds a simulated network snapshot for a selected preview state. */
    public fun network(state: PreviewNetworkState): DemoNetworkSnapshot =
        when (state) {
            PreviewNetworkState.ONLINE -> onlineSnapshot()
            PreviewNetworkState.CONNECTING -> connectingSnapshot()
            PreviewNetworkState.DEGRADED -> degradedSnapshot()
            PreviewNetworkState.OFFLINE -> offlineSnapshot()
        }

    private fun onlineSnapshot(): DemoNetworkSnapshot =
        DemoNetworkSnapshot(
            state = PreviewNetworkState.ONLINE,
            connectivity = DemoConnectivity.RELAY,
            natStatus = DemoNatStatus.PRIVATE,
            natPublicAddress = null,
            relayFallbackEnabled = true,
            peerCount = 4,
            directConnections = 2,
            relayedConnections = 2,
            fallbackAttempts = 3,
            fallbackSuccesses = 2,
            relayDataLimitLabel = "128 KiB per circuit",
            relayDurationLimitLabel = "2 min per circuit",
            relayPaths = demoRelayPaths(BootstrapPathState.CONNECTED, BootstrapPathState.UNREACHABLE),
            bootstrapPaths =
                bootstrapPaths(
                    BootstrapPathState.CONNECTED,
                    BootstrapPathState.CONNECTED,
                    BootstrapPathState.CONNECTED,
                ),
            recentErrors = emptyList(),
            troubleshootingHint =
                "Two peers are reachable through a community relay. Relay limits apply to each circuit.",
        )

    private fun connectingSnapshot(): DemoNetworkSnapshot =
        DemoNetworkSnapshot(
            state = PreviewNetworkState.CONNECTING,
            connectivity = DemoConnectivity.CONNECTING,
            natStatus = DemoNatStatus.UNKNOWN,
            natPublicAddress = null,
            relayFallbackEnabled = true,
            peerCount = 0,
            directConnections = 0,
            relayedConnections = 0,
            fallbackAttempts = 0,
            fallbackSuccesses = 0,
            relayDataLimitLabel = null,
            relayDurationLimitLabel = null,
            relayPaths = emptyList(),
            bootstrapPaths =
                bootstrapPaths(
                    BootstrapPathState.CONNECTING,
                    BootstrapPathState.CONNECTING,
                    BootstrapPathState.CONNECTING,
                ),
            recentErrors = emptyList(),
            troubleshootingHint =
                "The node is still looking for a direct or relayed path. No content is available yet.",
        )

    private fun degradedSnapshot(): DemoNetworkSnapshot =
        DemoNetworkSnapshot(
            state = PreviewNetworkState.DEGRADED,
            connectivity = DemoConnectivity.RELAY,
            natStatus = DemoNatStatus.PRIVATE,
            natPublicAddress = null,
            relayFallbackEnabled = true,
            peerCount = 2,
            directConnections = 1,
            relayedConnections = 1,
            fallbackAttempts = 4,
            fallbackSuccesses = 1,
            relayDataLimitLabel = "128 KiB per circuit",
            relayDurationLimitLabel = "2 min per circuit",
            relayPaths = demoRelayPaths(BootstrapPathState.CONNECTED, BootstrapPathState.UNREACHABLE),
            bootstrapPaths =
                bootstrapPaths(
                    BootstrapPathState.CONNECTED,
                    BootstrapPathState.UNREACHABLE,
                    BootstrapPathState.CONNECTED,
                ),
            recentErrors =
                listOf(
                    DemoNetworkError(
                        code = DemoNetworkErrorCode.RELAY_RESERVATION_FAILED,
                        occurredLabel = "1 min ago",
                    ),
                    DemoNetworkError(
                        code = DemoNetworkErrorCode.BOOTSTRAP_UNREACHABLE,
                        occurredLabel = "2 min ago",
                    ),
                    DemoNetworkError(
                        code = DemoNetworkErrorCode.PEER_TIMEOUT,
                        occurredLabel = "6 min ago",
                    ),
                ),
            troubleshootingHint =
                "One relay refused a reservation. Remaining direct and relayed paths may still work.",
        )

    private fun offlineSnapshot(): DemoNetworkSnapshot =
        DemoNetworkSnapshot(
            state = PreviewNetworkState.OFFLINE,
            connectivity = DemoConnectivity.NO_PEERS,
            natStatus = DemoNatStatus.UNKNOWN,
            natPublicAddress = null,
            relayFallbackEnabled = true,
            peerCount = 0,
            directConnections = 0,
            relayedConnections = 0,
            fallbackAttempts = 2,
            fallbackSuccesses = 0,
            relayDataLimitLabel = null,
            relayDurationLimitLabel = null,
            relayPaths = demoRelayPaths(BootstrapPathState.UNREACHABLE, BootstrapPathState.UNREACHABLE),
            bootstrapPaths =
                bootstrapPaths(
                    BootstrapPathState.UNREACHABLE,
                    BootstrapPathState.UNREACHABLE,
                    BootstrapPathState.UNREACHABLE,
                ),
            recentErrors =
                listOf(
                    DemoNetworkError(
                        code = DemoNetworkErrorCode.NO_USABLE_PATH,
                        occurredLabel = "1 min ago",
                    ),
                    DemoNetworkError(
                        code = DemoNetworkErrorCode.RELAY_UNAVAILABLE,
                        occurredLabel = "5 min ago",
                    ),
                ),
            troubleshootingHint =
                "No direct or relayed path is available. Add a reachable community relay or check firewall rules.",
        )

    private fun demoRelayPaths(
        first: BootstrapPathState,
        second: BootstrapPathState,
    ): List<DemoRelayPath> =
        listOf(
            DemoRelayPath(
                label = "Community Relay A",
                endpoint = "/ip4/203.0.113.10/tcp/4001",
                state = first,
                detail = "Reservation active; 128 KiB and 2 min per circuit",
            ),
            DemoRelayPath(
                label = "Community Relay B",
                endpoint = "/dns4/relay-b.example/tcp/4001",
                state = second,
                detail = "Reservation not granted yet",
            ),
        )

    private fun bootstrapPaths(
        first: BootstrapPathState,
        second: BootstrapPathState,
        third: BootstrapPathState,
    ): List<DemoBootstrapPath> =
        listOf(
            DemoBootstrapPath("Bootstrap A", "/dns4/bootstrap-a.example/udp/443/quic-v1", first),
            DemoBootstrapPath("Bootstrap B", "/dns4/bootstrap-b.example/udp/443/quic-v1", second),
            DemoBootstrapPath("Bootstrap C", "/dns4/bootstrap-c.example/tcp/443", third),
        )

    private fun DemoPublication.matches(query: String): Boolean {
        val needle = query.lowercase()
        return sequenceOf(
            title,
            fileName,
            description,
            uploaderDisplayName,
            category.name,
        ).plus(tags.asSequence())
            .any { it.lowercase().contains(needle) }
    }
}
