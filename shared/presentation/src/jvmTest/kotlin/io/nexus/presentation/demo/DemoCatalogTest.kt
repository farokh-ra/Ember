package io.nexus.presentation.demo

import io.nexus.presentation.PreviewNetworkState
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class DemoCatalogTest {
    @Test
    fun everyDisplayedFixtureIsMarkedAsDemoContent() {
        assertTrue(DemoCatalog.IS_DEMO)
        assertTrue(DemoCatalog.profile.isDemo)
        assertTrue(DemoCatalog.publications.all(DemoPublication::isDemo))
        assertTrue(DemoCatalog.downloads.all(DemoDownload::isDemo))
        assertTrue(DemoCatalog.hostedFiles.all(DemoHostedFile::isDemo))
    }

    @Test
    fun publicationIdentifiersAndContentIdsAreStableAndUnique() {
        assertEquals(
            DemoCatalog.publications.size,
            DemoCatalog.publications
                .map(DemoPublication::recordId)
                .toSet()
                .size,
        )
        assertEquals(
            DemoCatalog.publications.size,
            DemoCatalog.publications
                .map(DemoPublication::contentId)
                .toSet()
                .size,
        )
        DemoCatalog.publications.forEach { publication ->
            assertEquals(64, publication.contentId.length)
            assertTrue(publication.contentId.all { it in "0123456789abcdef" })
            assertNotNull(DemoCatalog.publication(publication.recordId))
        }
    }

    @Test
    fun availabilityVocabularyMatchesSourceFacts() {
        DemoCatalog.publications.forEach { publication ->
            when (publication.availability) {
                PublicationAvailability.AVAILABLE_NOW -> {
                    assertTrue(publication.sources.any(DemoSource::online), publication.recordId)
                }

                PublicationAvailability.SEEN_RECENTLY -> {
                    assertTrue(publication.sources.isNotEmpty(), publication.recordId)
                    assertTrue(publication.sources.none(DemoSource::online), publication.recordId)
                }

                PublicationAvailability.NO_KNOWN_SOURCE,
                PublicationAvailability.RECORD_ONLY,
                -> {
                    assertTrue(publication.sources.isEmpty(), publication.recordId)
                }
            }
        }
    }

    @Test
    fun recordOnlyContentIsNeverClaimedAsLocallyVerified() {
        DemoCatalog.publications
            .filter { it.availability == PublicationAvailability.RECORD_ONLY }
            .forEach { publication ->
                assertFalse(publication.contentVerified, publication.recordId)
            }
    }

    @Test
    fun contentVerificationNeverExistsWithoutRecordVerification() {
        DemoCatalog.publications.forEach { publication ->
            if (publication.contentVerified) {
                assertTrue(publication.recordVerified, publication.recordId)
            }
        }
    }

    @Test
    fun unpublishedFixtureCarriesAnUnpublishedFlag() {
        val unpublished = DemoCatalog.publications.filter(DemoPublication::unpublished)

        assertEquals(1, unpublished.size)
        assertTrue(unpublished.single().recordVerified)
    }

    @Test
    fun servingIsOnlyClaimedForOwnedActivePublications() {
        DemoCatalog.publications.forEach { publication ->
            if (publication.serving) {
                assertTrue(publication.ownedByViewer, publication.recordId)
                assertFalse(publication.unpublished, publication.recordId)
            }
        }
        assertTrue(DemoCatalog.publications.any(DemoPublication::serving))
    }

    @Test
    fun previewsNeverAutoExecuteAndMatchSupportedFormats() {
        DemoCatalog.publications.forEach { publication ->
            assertFalse(publication.preview.autoExecute, publication.recordId)
            if (publication.preview.supported) {
                assertTrue(publication.preview.kind != DemoPreviewKind.NONE, publication.recordId)
                assertTrue(
                    publication.preview.reason != DemoPreviewReason.UNSUPPORTED_FORMAT,
                    publication.recordId,
                )
            } else {
                assertEquals(DemoPreviewKind.NONE, publication.preview.kind, publication.recordId)
                assertEquals(
                    DemoPreviewReason.UNSUPPORTED_FORMAT,
                    publication.preview.reason,
                    publication.recordId,
                )
            }
        }
        assertTrue(
            DemoCatalog.publications.any {
                it.preview.supported && it.preview.kind == DemoPreviewKind.TEXT
            },
        )
    }

    @Test
    fun completedDownloadsAreFullyVerifiedAndOthersAreNotComplete() {
        DemoCatalog.downloads.forEach { download ->
            if (download.status == DownloadStatus.COMPLETE) {
                assertTrue(download.contentVerified, download.id)
                assertEquals(1f, download.progress)
            } else {
                assertFalse(download.contentVerified, download.id)
                assertTrue(download.progress < 1f, download.id)
            }
        }
    }

    @Test
    fun hostedReplicasRetainOriginalUploaderAttribution() {
        DemoCatalog.hostedFiles.forEach { hosted ->
            assertTrue(hosted.isDemo)
            assertFalse(hosted.originalUploader == DemoCatalog.profile.displayName)
            if (hosted.verification == ReplicaVerification.FAILED) {
                assertEquals(0, hosted.peersServed)
            }
            if (hosted.verification == ReplicaVerification.VERIFYING) {
                assertEquals(0, hosted.peersServed)
            }
        }
    }

    @Test
    fun searchMatchesMetadataAndFiltersCategories() {
        val coastal = DemoCatalog.search("coastal")
        val audio = DemoCatalog.search("audio")
        val documents = DemoCatalog.search("", DemoCategory.DOCUMENTS)
        val noResults = DemoCatalog.search("definitely-not-in-the-demo-catalogue")

        assertTrue(coastal.any { it.recordId == "nx-demo-coastal-field-notes" })
        assertTrue(audio.any { it.recordId == "nx-demo-dawn-chorus" })
        assertTrue(documents.all { it.category == DemoCategory.DOCUMENTS })
        assertTrue(noResults.isEmpty())
    }

    @Test
    fun everyNetworkStateIsDistinctAndOfflineHasNoPeers() {
        val snapshots = PreviewNetworkState.entries.associateWith(DemoCatalog::network)

        assertEquals(
            PreviewNetworkState.entries.size,
            snapshots.values
                .map { it.state }
                .toSet()
                .size,
        )
        snapshots.values.forEach { snapshot ->
            assertTrue(snapshot.isDemo)
            assertEquals(3, snapshot.bootstrapPaths.size)
        }
        assertEquals(0, snapshots.getValue(PreviewNetworkState.OFFLINE).peerCount)
        assertTrue(
            snapshots
                .getValue(PreviewNetworkState.OFFLINE)
                .bootstrapPaths
                .all { it.state == BootstrapPathState.UNREACHABLE },
        )
        assertEquals(0, snapshots.getValue(PreviewNetworkState.CONNECTING).peerCount)
        assertTrue(
            snapshots
                .getValue(PreviewNetworkState.CONNECTING)
                .bootstrapPaths
                .all { it.state == BootstrapPathState.CONNECTING },
        )
        assertTrue(snapshots.getValue(PreviewNetworkState.ONLINE).recentErrors.isEmpty())
    }

    @Test
    fun profilePublicationsBelongToTheViewer() {
        assertTrue(DemoCatalog.profilePublications.isNotEmpty())
        assertTrue(DemoCatalog.profilePublications.all(DemoPublication::ownedByViewer))
        assertEquals(DemoCatalog.profilePublications.size, DemoCatalog.profile.publicationCount)
    }
}
