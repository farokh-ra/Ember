package io.nexus.ui.resources

import androidx.compose.runtime.staticCompositionLocalOf

/**
 * All user-facing copy used by Stage 04.
 *
 * Copy lives here rather than in screens so a resource-backed provider can be
 * added in the localisation stage without rewriting screen composables. Data
 * such as sample filenames and peer labels lives in the presentation demo
 * catalogue and is always shown with a visible demo label.
 */
public interface NexusStrings {
    // Application and common actions.
    public val appName: String
    public val appTagline: String
    public val previewLabel: String
    public val demoBadge: String
    public val demoBannerShort: String
    public val demoBannerBody: String
    public val demoContent: String
    public val sampleRecord: String
    public val sampleDownload: String
    public val sampleReplica: String
    public val avatarDescription: (String) -> String
    public val openRecord: (String) -> String
    public val demoNoOp: String
    public val notAvailable: String
    public val notAvailableYet: String
    public val retry: String
    public val showDemoData: String
    public val back: String
    public val cancel: String
    public val close: String
    public val delete: String
    public val confirm: String
    public val open: String
    public val view: String
    public val learnMore: String
    public val selected: String
    public val status: String
    public val previewState: String
    public val previewStateHelp: String
    public val previewReady: String
    public val previewLoading: String
    public val previewEmpty: String
    public val previewOffline: String
    public val previewError: String
    public val previewNetworkLabel: String
    public val previewNetworkHelp: String
    public val globalNetwork: String
    public val demoNetwork: String
    public val profile: String
    public val settings: String
    public val networkStatus: String
    public val publish: String
    public val versionLine: String
    public val sourceCount: (Int) -> String
    public val recordsCount: (Int) -> String
    public val lastSeen: (String) -> String
    public val percent: (Int) -> String

    // Navigation.
    public val navExplore: String
    public val navSearch: String
    public val navDownloads: String
    public val navHosted: String
    public val navSettings: String
    public val navProfile: String
    public val navNetwork: String
    public val navPublish: String
    public val openProfile: String
    public val openNetworkStatus: String

    // Shared content states.
    public val loadingTitle: String
    public val loadingBody: String
    public val loadingAction: String
    public val emptyTitle: String
    public val emptyBody: String
    public val emptyAction: String
    public val offlineTitle: String
    public val offlineBody: String
    public val offlineRetry: String
    public val offlineViewCached: String
    public val errorTitle: String
    public val errorBody: String
    public val errorRetry: String
    public val errorLocalStoreTitle: String
    public val errorLocalStoreBody: String
    public val stateDemoNote: String

    // Welcome.
    public val welcomeEyebrow: String
    public val welcomeTitle: String
    public val welcomeBody: String
    public val welcomePrimary: String
    public val welcomeSecondary: String
    public val welcomeDemoNoticeTitle: String
    public val welcomeDemoNoticeBody: String
    public val welcomeValueLocalTitle: String
    public val welcomeValueLocalBody: String
    public val welcomeValueHonestTitle: String
    public val welcomeValueHonestBody: String
    public val welcomeValueCrossPlatformTitle: String
    public val welcomeValueCrossPlatformBody: String
    public val welcomeFooter: String

    // Identity setup.
    public val identityTitle: String
    public val identityBody: String
    public val identityNameLabel: String
    public val identityNamePlaceholder: String
    public val identityBioLabel: String
    public val identityBioPlaceholder: String
    public val identityPublicTitle: String
    public val identityPublicBody: String
    public val identityNoRecoveryTitle: String
    public val identityNoRecoveryBody: String
    public val identityDemoNote: String
    public val identityContinue: String
    public val identityNameErrorEmpty: String
    public val identityNameErrorTooShort: String
    public val identityNameErrorTooLong: String
    public val identityBioErrorTooLong: String
    public val identityNameCounter: (Int, Int) -> String
    public val identityBioCounter: (Int, Int) -> String

    // Explore.
    public val exploreTitle: String
    public val exploreSubtitle: String
    public val exploreCoverage: String
    public val exploreCategories: String
    public val explorePublishPreview: String
    public val exploreAvailableSection: String
    public val exploreSeenSection: String
    public val exploreRecordSection: String
    public val exploreEmptyFilterTitle: String
    public val exploreEmptyFilterBody: String
    public val exploreDemoNotice: String
    public val exploreCoverageOffline: String
    public val exploreCoveragePartial: String
    public val exploreShowing: (Int, Int) -> String
    public val exploreLoadMore: String
    public val exploreStatusGuideTitle: String
    public val categoryAll: String
    public val categoryDocuments: String
    public val categoryMedia: String
    public val categoryArchives: String
    public val categoryOther: String

    // Status vocabulary.
    public val availabilityAvailableNow: String
    public val availabilitySeenRecently: String
    public val availabilityNoKnownSource: String
    public val availabilityRecordOnly: String
    public val availabilityUnpublished: String
    public val availabilityAvailableNowDefinition: String
    public val availabilitySeenRecentlyDefinition: String
    public val availabilityNoKnownSourceDefinition: String
    public val availabilityRecordOnlyDefinition: String
    public val availabilityUnpublishedDefinition: String
    public val sourceOnline: String
    public val sourceOffline: String
    public val recordVerified: String
    public val contentVerified: String
    public val contentNotVerified: String

    // Search.
    public val searchTitle: String
    public val searchSubtitle: String
    public val searchPlaceholder: String
    public val searchAction: String
    public val searchClear: String
    public val searchCoverage: String
    public val searchResultCount: (Int) -> String
    public val searchBlankTitle: String
    public val searchBlankBody: String
    public val searchNoResultsTitle: String
    public val searchNoResultsBody: String
    public val searchClearFilters: String
    public val searchPartial: String
    public val searchCoverageOffline: String
    public val searchCoveragePartial: String
    public val searchShowing: (Int, Int) -> String
    public val searchLoadMore: String
    public val searchOpen: String

    // Profile.
    public val profileTitle: String
    public val profileSubtitle: String
    public val profileDemoBadge: String
    public val profilePublications: String
    public val profileHostedReplicas: String
    public val profileJoined: String
    public val profileKeyLabel: String
    public val profileKeyNote: String
    public val profileUsernameNote: String
    public val profileEdit: String
    public val profileEditUnavailable: String
    public val profileIdentityBackup: String
    public val profileBackupUnavailable: String
    public val profileNetworkLink: String
    public val profileNoPublications: String
    public val profileSignedRecord: String
    public val profileRecordDraft: String
    public val profileRecordPending: String
    public val profileRecordPublished: String
    public val profileRevision: String
    public val profileIdentityId: String
    public val profileFreshnessLabel: String
    public val profileFreshnessCurrent: String
    public val profileFreshnessStale: String
    public val profileFreshnessUnknown: String
    public val profilePublishedAt: String
    public val profileLastChecked: String
    public val profileConflictsLabel: String
    public val profileConflictNote: String
    public val profileCollisionsLabel: String
    public val profileCollisionNote: String
    public val profileNoCollisions: String
    public val profileSignedNote: String

    // File details.
    public val fileDetailsTitle: String
    public val fileSectionAvailability: String
    public val fileSectionSources: String
    public val fileSectionVerification: String
    public val fileSectionMetadata: String
    public val fileSectionActions: String
    public val fileContentId: String
    public val fileSize: String
    public val fileCategory: String
    public val filePublished: String
    public val fileUploader: String
    public val fileRecordVersion: String
    public val fileSignatureVerified: String
    public val fileSignatureVerifiedBody: String
    public val fileSignatureUnverified: String
    public val fileSignatureUnverifiedBody: String
    public val fileContentVerified: String
    public val fileContentVerifiedBody: String
    public val fileContentNotVerified: String
    public val fileContentNotVerifiedBody: String
    public val fileVerificationSeparate: String
    public val fileNoSources: String
    public val fileNoSourcesBody: String
    public val fileGlobalNote: String
    public val fileDownload: String
    public val fileDownloadUnavailable: String
    public val fileHost: String
    public val fileHostUnavailable: String
    public val fileHostExplain: String
    public val fileUnpublish: String
    public val fileUnpublishDemo: String
    public val fileAlreadyUnpublished: String
    public val fileUnpublishedWarning: String
    public val fileOwnedByYou: String
    public val fileOpenProfile: String
    public val fileUploaderProfileUnavailable: String
    public val fileNotFoundTitle: String
    public val fileNotFoundBody: String
    public val fileBackToExplore: String
    public val fileTags: String
    public val fileDescription: String
    public val fileServingNow: String
    public val fileNotServing: String
    public val fileServingBody: String
    public val fileNotServingBody: String
    public val fileSignatureStatusLabel: String
    public val fileServingStatusLabel: String
    public val fileAvailabilityStatusLabel: String
    public val fileSectionPreview: String
    public val filePreviewSupported: String
    public val filePreviewUnsupported: String
    public val filePreviewKindLabel: String
    public val filePreviewKindText: String
    public val filePreviewKindImage: String
    public val filePreviewKindNone: String
    public val filePreviewReasonPlainText: String
    public val filePreviewReasonRasterImage: String
    public val filePreviewReasonUnsupported: String
    public val filePreviewNoAutoExecute: String
    public val fileUnpublishLimitsTitle: String
    public val fileUnpublishLimitsBody: String
    public val fileUnpublishStopsServing: String
    public val fileUnpublishRetainsLocal: String
    public val fileUnpublishNotGlobal: String
    public val fileUnpublishDialogTitle: String
    public val fileUnpublishDialogBody: String
    public val fileUnpublishDialogConfirm: String
    public val fileUnpublishDone: String

    // Publish.
    public val publishTitle: String
    public val publishSubtitle: String
    public val publishUnavailableTitle: String
    public val publishUnavailableBody: String
    public val publishDemoForm: String
    public val publishFileLabel: String
    public val publishFileValue: String
    public val publishFileUnavailable: String
    public val publishNameLabel: String
    public val publishNamePlaceholder: String
    public val publishDescriptionLabel: String
    public val publishDescriptionPlaceholder: String
    public val publishTagsLabel: String
    public val publishTagsPlaceholder: String
    public val publishCategoryLabel: String
    public val publishRightsLabel: String
    public val publishRightsPlaceholder: String
    public val publishPublicTitle: String
    public val publishPublicBody: String
    public val publishNoDeleteTitle: String
    public val publishNoDeleteBody: String
    public val publishServingTitle: String
    public val publishServingBody: String
    public val publishSignatureNote: String
    public val publishAfterTitle: String
    public val publishAfterDemo: String
    public val publishAfterContentId: String
    public val publishAfterSignature: String
    public val publishAfterServing: String
    public val publishAfterAvailability: String
    public val publishAfterSignatureVerified: String
    public val publishAfterServingOnline: String
    public val publishAfterAvailabilityValue: String
    public val publishAfterNote: String
    public val publishPlannedTitle: String
    public val publishStepOne: String
    public val publishStepTwo: String
    public val publishStepThree: String
    public val publishStepFour: String
    public val publishStepFive: String
    public val publishVerificationNote: String
    public val publishButton: String
    public val publishButtonNote: String

    // Downloads.
    public val downloadsTitle: String
    public val downloadsSubtitle: String
    public val downloadsDemoNotice: String
    public val downloadsActiveSection: String
    public val downloadsCompletedSection: String
    public val downloadsStatusDownloading: String
    public val downloadsStatusPaused: String
    public val downloadsStatusVerifying: String
    public val downloadsStatusComplete: String
    public val downloadsStatusFailed: String
    public val downloadsProgressLabel: String
    public val downloadsVerified: String
    public val downloadsNotVerified: String
    public val downloadsSpeedLabel: (String) -> String
    public val downloadsSourcePeerLabel: (String) -> String
    public val downloadsPausedNote: String
    public val downloadsDelete: String
    public val downloadsDeleteDialogTitle: String
    public val downloadsDeleteDialogBody: String
    public val downloadsDeleteDialogConfirm: String
    public val downloadsDeletedDone: String
    public val downloadsEmptyTitle: String
    public val downloadsEmptyBody: String
    public val downloadsRestore: String
    public val downloadsCompleteNote: String
    public val downloadsVerifyingNote: String
    public val downloadsFailedNote: String

    // Hosted files.
    public val hostedTitle: String
    public val hostedSubtitle: String
    public val hostedDemoNotice: String
    public val hostedHowTitle: String
    public val hostedHowBody: String
    public val hostedStatusVerified: String
    public val hostedStatusVerifying: String
    public val hostedStatusPaused: String
    public val hostedStatusFailed: String
    public val hostedOriginalUploader: String
    public val hostedPublication: (String) -> String
    public val hostedStorageUse: (String) -> String
    public val hostedServing: String
    public val hostedNotServing: String
    public val hostedLastSeen: (String) -> String
    public val hostedYouHost: String
    public val hostedPeers: (Int) -> String
    public val hostedLastVerified: (String) -> String
    public val hostedNotVerified: String
    public val hostedFailedNote: String
    public val hostedPause: String
    public val hostedResume: String
    public val hostedPausedDone: String
    public val hostedResumedDone: String
    public val hostedRemove: String
    public val hostedRemoveDialogTitle: String
    public val hostedRemoveDialogBody: String
    public val hostedRemoveDialogConfirm: String
    public val hostedRemovedDone: String
    public val hostedEmptyTitle: String
    public val hostedEmptyBody: String
    public val hostedRestore: String

    // Settings.
    public val settingsTitle: String
    public val settingsSubtitle: String
    public val settingsAppearance: String
    public val settingsTheme: String
    public val settingsThemeSystem: String
    public val settingsThemeLight: String
    public val settingsThemeDark: String
    public val settingsThemeSaved: String
    public val settingsIdentity: String
    public val settingsBackupTitle: String
    public val settingsBackupBody: String
    public val settingsStorage: String
    public val settingsStorageTitle: String
    public val settingsStorageValue: String
    public val settingsStorageBody: String
    public val settingsBandwidth: String
    public val settingsDownloadLimit: String
    public val settingsUploadLimit: String
    public val settingsDownloadLimitValue: String
    public val settingsUploadLimitValue: String
    public val settingsBandwidthBody: String
    public val settingsNetwork: String
    public val settingsBootstrapPeers: String
    public val settingsBootstrapValue: String
    public val settingsBootstrapBody: String
    public val settingsLanguage: String
    public val settingsLanguageValue: String
    public val settingsLanguageBody: String
    public val settingsDiagnostics: String
    public val settingsDiagnosticsExport: String
    public val settingsDiagnosticsBody: String
    public val settingsPrivacyTitle: String
    public val settingsPrivacyBody: String
    public val settingsAbout: String
    public val settingsAboutVersion: String
    public val settingsAboutBuild: String
    public val settingsLicenses: String
    public val settingsLicensesBody: String
    public val settingsPreviewTools: String
    public val settingsPreviewToolsBody: String
    public val settingsPreviewWelcome: String
    public val settingsPreviewIdentity: String
    public val settingsPreviewStates: String

    // Network status.
    public val networkTitle: String
    public val networkSubtitle: String
    public val networkDemoNotice: String
    public val networkStateLabel: String
    public val networkNodeState: String
    public val networkOffline: String
    public val networkConnecting: String
    public val networkOnline: String
    public val networkDegraded: String
    public val networkOfflineBody: String
    public val networkConnectingBody: String
    public val networkOnlineBody: String
    public val networkDegradedBody: String
    public val networkPeerCount: String
    public val networkPeers: (Int) -> String
    public val networkBootstrapTitle: String
    public val networkBootstrapBody: String
    public val networkPathConnected: String
    public val networkPathConnecting: String
    public val networkPathUnreachable: String
    public val networkRecentErrors: String
    public val networkRecentErrorsBody: String
    public val networkNoErrors: String
    public val networkErrorNoUsablePath: String
    public val networkErrorBootstrapUnreachable: String
    public val networkErrorPeerTimeout: String
    public val networkDiagnosticsTitle: String
    public val networkDiagnosticsBody: String
    public val networkExport: String
    public val networkExportUnavailable: String
    public val networkSeparateTitle: String
    public val networkSeparateBody: String
    public val networkPathTitle: String
    public val networkPathBody: String
    public val networkConnectivityDirect: String
    public val networkConnectivityRelay: String
    public val networkConnectivityConnecting: String
    public val networkConnectivityNoPeers: String
    public val networkConnectivityDisabled: String
    public val networkNatLabel: String
    public val networkNatPublic: String
    public val networkNatPrivate: String
    public val networkNatUnknown: String
    public val networkNatDisabled: String
    public val networkRelayFallbackOn: String
    public val networkRelayFallbackOff: String
    public val networkRelayTitle: String
    public val networkRelayBody: String
    public val networkRelayNoRelays: String
    public val networkRelayConfigured: String
    public val networkRelayConnected: String
    public val networkRelayReservations: String
    public val networkRelayDataLimit: String
    public val networkRelayDurationLimit: String
    public val networkRelayLimitUnknown: String
    public val networkDirectConnections: String
    public val networkRelayedConnections: String
    public val networkFallbackAttempts: String
    public val networkFallbackSuccesses: String
    public val networkHolePunchFailures: String
    public val networkErrorRelayUnavailable: String
    public val networkErrorRelayReservation: String
    public val networkPreviewTitle: String
    public val networkPreviewBody: String
}

public object EnglishNexusStrings : NexusStrings {
    override val appName: String = "Ember"
    override val appTagline: String = "Local-first file sharing"
    override val previewLabel: String = "Stage 04 UI preview"
    override val demoBadge: String = "DEMO DATA"
    override val demoBannerShort: String = "Preview build — network features are inactive"
    override val demoBannerBody: String =
        "Identity, publishing, downloads, hosting, and networking are not active in Stage 04. " +
            "Everything shown is sample data."
    override val demoContent: String = "Demo content"
    override val sampleRecord: String = "Sample record"
    override val sampleDownload: String = "Sample transfer"
    override val sampleReplica: String = "Sample replica"
    override val avatarDescription: (String) -> String = { name -> "Profile for $name" }
    override val openRecord: (String) -> String = { title -> "Open $title" }
    override val demoNoOp: String = "Demo only — nothing was changed."
    override val notAvailable: String = "Not available in this preview"
    override val notAvailableYet: String = "Not available yet"
    override val retry: String = "Retry"
    override val showDemoData: String = "Show demo data"
    override val back: String = "Back"
    override val cancel: String = "Cancel"
    override val close: String = "Close"
    override val delete: String = "Delete"
    override val confirm: String = "Confirm"
    override val open: String = "Open"
    override val view: String = "View"
    override val learnMore: String = "Learn more"
    override val selected: String = "Selected"
    override val status: String = "Status"
    override val previewState: String = "Preview state"
    override val previewStateHelp: String = "Inspect this screen in each data state. No network request is made."
    override val previewReady: String = "Ready"
    override val previewLoading: String = "Loading"
    override val previewEmpty: String = "Empty"
    override val previewOffline: String = "Offline"
    override val previewError: String = "Error"
    override val previewNetworkLabel: String = "Demo network state"
    override val previewNetworkHelp: String = "Changes only the simulated status. No peer connection is created."
    override val globalNetwork: String = "Global network"
    override val demoNetwork: String = "Demo network"
    override val profile: String = "Profile"
    override val settings: String = "Settings"
    override val networkStatus: String = "Network status"
    override val publish: String = "Publish"
    override val versionLine: String = "Ember 0.1.0 · Stage 04"
    override val sourceCount: (Int) -> String = { count -> "Sources: $count" }
    override val recordsCount: (Int) -> String = { count -> "$count records" }
    override val lastSeen: (String) -> String = { value -> "Last seen: $value" }
    override val percent: (Int) -> String = { value -> "$value%" }

    override val navExplore: String = "Explore"
    override val navSearch: String = "Search"
    override val navDownloads: String = "Downloads"
    override val navHosted: String = "Hosted"
    override val navSettings: String = "Settings"
    override val navProfile: String = "Profile"
    override val navNetwork: String = "Network status"
    override val navPublish: String = "Publish"
    override val openProfile: String = "Open profile"
    override val openNetworkStatus: String = "Open network status"

    override val loadingTitle: String = "Loading demo data"
    override val loadingBody: String =
        "This is the loading state. No network request is in flight in the preview."
    override val loadingAction: String = "Show loaded demo data"
    override val emptyTitle: String = "No demo data in this view"
    override val emptyBody: String =
        "This is the empty state. Switch the preview state or restore the sample content."
    override val emptyAction: String = "Show demo data"
    override val offlineTitle: String = "Offline (demo)"
    override val offlineBody: String =
        "Local data can remain usable while the network is unavailable. This offline state is simulated."
    override val offlineRetry: String = "Retry"
    override val offlineViewCached: String = "View local demo data"
    override val errorTitle: String = "Could not load demo data"
    override val errorBody: String =
        "This is an actionable error preview. No stack trace, secret, or local path is shown."
    override val errorRetry: String = "Retry"
    override val errorLocalStoreTitle: String = "Could not read local metadata"
    override val errorLocalStoreBody: String =
        "The local data store is unavailable. Retry, or open Settings for diagnostics."
    override val stateDemoNote: String = "Demo state — no real operation is running."

    override val welcomeEyebrow: String = "STAGE 04 · UI PREVIEW"
    override val welcomeTitle: String = "Find files. Keep the network human."
    override val welcomeBody: String =
        "Ember is a local-first way to publish and discover files. Your identity lives on your device, " +
            "and no central account is required."
    override val welcomePrimary: String = "Open the demo"
    override val welcomeSecondary: String = "Set up a demo identity"
    override val welcomeDemoNoticeTitle: String = "Everything here is a preview"
    override val welcomeDemoNoticeBody: String =
        "Sample files and a simulated network. Nothing is uploaded, signed, or saved."
    override val welcomeValueLocalTitle: String = "Local-first"
    override val welcomeValueLocalBody: String =
        "Browse local records and saved downloads without waiting for a network."
    override val welcomeValueHonestTitle: String = "Honest states"
    override val welcomeValueHonestBody: String =
        "Availability, verification, and connectivity stay separate and clearly labelled."
    override val welcomeValueCrossPlatformTitle: String = "One shared UI"
    override val welcomeValueCrossPlatformBody: String =
        "The same journeys on Android and Windows, with platform-native integration."
    override val welcomeFooter: String = "No account. No central feed. No telemetry enabled by default."

    override val identityTitle: String = "Set up a demo profile"
    override val identityBody: String =
        "A real identity is a keypair stored on your device. This preview does not generate or save one."
    override val identityNameLabel: String = "Display name"
    override val identityNamePlaceholder: String = "How peers will see you"
    override val identityBioLabel: String = "Short bio (optional)"
    override val identityBioPlaceholder: String = "A sentence about you or your work"
    override val identityPublicTitle: String = "Your profile is public when published"
    override val identityPublicBody: String =
        "Display name, bio, and publications you choose to publish can be seen by peers. " +
            "Usernames are labels, not unique accounts. A signature proves key control, not real-world identity."
    override val identityNoRecoveryTitle: String = "There is no central recovery"
    override val identityNoRecoveryBody: String =
        "In a later stage, losing your key will mean losing your identity unless you backed it up. " +
            "Stage 04 does not create a key or backup."
    override val identityDemoNote: String =
        "Demo only — continuing opens a sample profile. Nothing is saved, signed, or published."
    override val identityContinue: String = "Continue with demo profile"
    override val identityNameErrorEmpty: String = "Enter a display name."
    override val identityNameErrorTooShort: String = "Use at least 2 characters."
    override val identityNameErrorTooLong: String = "Use 48 characters or fewer."
    override val identityBioErrorTooLong: String = "Use 280 characters or fewer."
    override val identityNameCounter: (Int, Int) -> String = { used, max -> "$used/$max" }
    override val identityBioCounter: (Int, Int) -> String = { used, max -> "$used/$max" }

    override val exploreTitle: String = "Explore"
    override val exploreSubtitle: String =
        "Browse discovered records by category. Explore is not a popularity feed."
    override val exploreCoverage: String = "Demo index · sample records · freshness: sample data"
    override val exploreCategories: String = "Filter by category"
    override val explorePublishPreview: String = "Preview the publish screen"
    override val exploreAvailableSection: String = "Available now"
    override val exploreSeenSection: String = "Seen recently"
    override val exploreRecordSection: String = "Record only and no known source"
    override val exploreEmptyFilterTitle: String = "No demo records in this filter"
    override val exploreEmptyFilterBody: String = "Choose another category or switch the preview state."
    override val exploreDemoNotice: String =
        "All records are sample data. Availability is simulated and is not a live observation."
    override val exploreCoverageOffline: String =
        "Offline · cached local sample data only · network records may be missing"
    override val exploreCoveragePartial: String =
        "Partial coverage · some index providers are unavailable · results may be incomplete"
    override val exploreShowing: (Int, Int) -> String = { shown, total ->
        "Showing $shown of $total sample records"
    }
    override val exploreLoadMore: String = "Load more records"
    override val exploreStatusGuideTitle: String = "Status terms"
    override val categoryAll: String = "All"
    override val categoryDocuments: String = "Documents"
    override val categoryMedia: String = "Media"
    override val categoryArchives: String = "Archives"
    override val categoryOther: String = "Other"

    override val availabilityAvailableNow: String = "Available now"
    override val availabilitySeenRecently: String = "Seen recently"
    override val availabilityNoKnownSource: String = "No known source"
    override val availabilityRecordOnly: String = "Record only"
    override val availabilityUnpublished: String = "Unpublished by uploader"
    override val availabilityAvailableNowDefinition: String =
        "At least one source recently proved it can serve this content."
    override val availabilitySeenRecentlyDefinition: String =
        "A source was observed recently but is not responding now."
    override val availabilityNoKnownSourceDefinition: String =
        "A verified record exists, but no source is known right now."
    override val availabilityRecordOnlyDefinition: String =
        "Only the signed metadata record is stored; the content is not available locally or online."
    override val availabilityUnpublishedDefinition: String =
        "The uploader signed an unpublish statement. Copies held by other peers may still exist."
    override val sourceOnline: String = "Online"
    override val sourceOffline: String = "Offline"
    override val recordVerified: String = "Record verified"
    override val contentVerified: String = "Content verified"
    override val contentNotVerified: String = "Content not verified"

    override val searchTitle: String = "Search"
    override val searchSubtitle: String =
        "Search local and discovered metadata. Results are not guaranteed complete."
    override val searchPlaceholder: String = "Search demo titles, tags, or uploader"
    override val searchAction: String = "Search"
    override val searchClear: String = "Clear search"
    override val searchCoverage: String =
        "Demo index only · results are not complete · freshness: sample data"
    override val searchResultCount: (Int) -> String = { count -> "Results: $count" }
    override val searchBlankTitle: String = "Search the demo catalogue"
    override val searchBlankBody: String =
        "Try a title, tag, or uploader. Results are sample data, not a live network search."
    override val searchNoResultsTitle: String = "No demo results"
    override val searchNoResultsBody: String = "No sample record matches this query and category."
    override val searchClearFilters: String = "Clear filters"
    override val searchPartial: String =
        "Partial results: this preview searches sample metadata only."
    override val searchCoverageOffline: String =
        "Offline · cached local sample data only · results may be incomplete"
    override val searchCoveragePartial: String =
        "Partial coverage · some index providers are unavailable · results may be incomplete"
    override val searchShowing: (Int, Int) -> String = { shown, total ->
        "Showing $shown of $total results"
    }
    override val searchLoadMore: String = "Load more results"
    override val searchOpen: String = "Open file details"

    override val profileTitle: String = "Profile"
    override val profileSubtitle: String = "A public profile is a signed record. This one is sample data."
    override val profileDemoBadge: String = "Demo profile"
    override val profilePublications: String = "Publications"
    override val profileHostedReplicas: String = "Hosted replicas"
    override val profileJoined: String = "Profile created"
    override val profileKeyLabel: String = "Public key fingerprint"
    override val profileKeyNote: String =
        "A signature proves key control, not real-world identity. This fingerprint is sample data."
    override val profileUsernameNote: String = "Usernames are labels, not unique accounts."
    override val profileEdit: String = "Edit profile"
    override val profileEditUnavailable: String = "Profile editing is not available in this preview."
    override val profileIdentityBackup: String = "Identity backup and restore"
    override val profileBackupUnavailable: String =
        "Not available in this preview. There is no central recovery."
    override val profileNetworkLink: String = "View network status"
    override val profileNoPublications: String = "No demo publications for this profile."
    override val profileSignedRecord: String = "Signed record"
    override val profileRecordDraft: String = "Draft, not announced"
    override val profileRecordPending: String = "Pending announcement"
    override val profileRecordPublished: String = "Published"
    override val profileRevision: String = "Revision"
    override val profileIdentityId: String = "Identity ID"
    override val profileFreshnessLabel: String = "Freshness"
    override val profileFreshnessCurrent: String = "Current"
    override val profileFreshnessStale: String = "Stale"
    override val profileFreshnessUnknown: String = "Unknown"
    override val profilePublishedAt: String = "Published"
    override val profileLastChecked: String = "Last checked"
    override val profileConflictsLabel: String = "Retained conflicts"
    override val profileConflictNote: String =
        "A same-revision record with different bytes was kept for review. The accepted revision was not overwritten."
    override val profileCollisionsLabel: String = "Username collisions"
    override val profileCollisionNote: String =
        "Usernames are labels, not unique accounts. These distinct identities also use this label."
    override val profileNoCollisions: String = "No other known identity uses this label."
    override val profileSignedNote: String =
        "The Rust core signs and publishes real profile records. This card is sample data."

    override val fileDetailsTitle: String = "File details"
    override val fileSectionAvailability: String = "Availability"
    override val fileSectionSources: String = "Known sources"
    override val fileSectionVerification: String = "Verification"
    override val fileSectionMetadata: String = "Metadata"
    override val fileSectionActions: String = "Actions"
    override val fileContentId: String = "Content ID"
    override val fileSize: String = "Size"
    override val fileCategory: String = "Category"
    override val filePublished: String = "Published"
    override val fileUploader: String = "Published by"
    override val fileRecordVersion: String = "Record version"
    override val fileSignatureVerified: String = "Signature verified"
    override val fileSignatureVerifiedBody: String =
        "The signed record is intact. This proves key control, not real-world identity."
    override val fileSignatureUnverified: String = "Signature not verified"
    override val fileSignatureUnverifiedBody: String =
        "The record signature could not be verified. Do not treat this metadata as authentic."
    override val fileContentVerified: String = "Content verified"
    override val fileContentVerifiedBody: String =
        "The local bytes match the content ID and every chunk proof."
    override val fileContentNotVerified: String = "Content not verified"
    override val fileContentNotVerifiedBody: String = "No complete verified local copy exists."
    override val fileVerificationSeparate: String =
        "Signature, content, serving, and availability are separate states."
    override val fileNoSources: String = "No known source"
    override val fileNoSourcesBody: String =
        "The record exists, but the content is not available now. This does not mean the file was deleted."
    override val fileGlobalNote: String =
        "Global network status is separate from this file's availability."
    override val fileDownload: String = "Download"
    override val fileDownloadUnavailable: String = "Downloads are not active in this preview."
    override val fileHost: String = "Host a verified replica"
    override val fileHostUnavailable: String = "Hosting is not active in this preview."
    override val fileHostExplain: String =
        "Downloading does not make you a host. Hosting requires an explicit action."
    override val fileUnpublish: String = "Unpublish"
    override val fileUnpublishDemo: String =
        "Stops serving and records an unpublish statement (demo)."
    override val fileAlreadyUnpublished: String = "Already unpublished by the uploader."
    override val fileUnpublishedWarning: String =
        "Unpublished by the uploader. Copies held by other peers may still exist."
    override val fileOwnedByYou: String = "Published by you"
    override val fileOpenProfile: String = "View uploader profile"
    override val fileUploaderProfileUnavailable: String =
        "Only your own demo profile is available in this preview."
    override val fileNotFoundTitle: String = "Demo record not found"
    override val fileNotFoundBody: String = "This record is not part of the sample catalogue."
    override val fileBackToExplore: String = "Back to Explore"
    override val fileTags: String = "Tags"
    override val fileDescription: String = "Description"
    override val fileServingNow: String = "Serving from this device"
    override val fileNotServing: String = "Not serving"
    override val fileServingBody: String =
        "This device is sharing verified content while the app is online. " +
            "Closing the app or unpublishing stops serving."
    override val fileNotServingBody: String =
        "This device is not sharing the content now. The signed record can still be discovered, " +
            "but availability depends on another source."
    override val fileSignatureStatusLabel: String = "Signature"
    override val fileServingStatusLabel: String = "Serving"
    override val fileAvailabilityStatusLabel: String = "Availability"
    override val fileSectionPreview: String = "Safe preview"
    override val filePreviewSupported: String = "Preview available"
    override val filePreviewUnsupported: String = "No preview"
    override val filePreviewKindLabel: String = "Preview type"
    override val filePreviewKindText: String = "Plain text"
    override val filePreviewKindImage: String = "Raster image"
    override val filePreviewKindNone: String = "Not previewable"
    override val filePreviewReasonPlainText: String =
        "Plain text is shown as inert text. Links and scripts are not run."
    override val filePreviewReasonRasterImage: String =
        "Raster images are decoded for display only. SVG and HTML are never previewed."
    override val filePreviewReasonUnsupported: String =
        "This format is not previewed. Open it only after downloading and verifying it."
    override val filePreviewNoAutoExecute: String =
        "Previews never run or open a file automatically."
    override val fileUnpublishLimitsTitle: String = "Unpublish limits"
    override val fileUnpublishLimitsBody: String =
        "Unpublishing stops this device from serving the file and publishes a signed unpublish statement. " +
            "The local copy stays in your library."
    override val fileUnpublishStopsServing: String = "Serving stops immediately."
    override val fileUnpublishRetainsLocal: String = "Local bytes are retained."
    override val fileUnpublishNotGlobal: String = "Copies held by other peers may remain."
    override val fileUnpublishDialogTitle: String = "Unpublish this demo publication?"
    override val fileUnpublishDialogBody: String =
        "Unpublishing stops this device from serving the publication and records a signed unpublish " +
            "statement. The local copy stays, and copies other peers already hold may remain. " +
            "In this preview, no network action is performed."
    override val fileUnpublishDialogConfirm: String = "Unpublish (demo)"
    override val fileUnpublishDone: String =
        "Demo only — the publication was not changed."

    override val publishTitle: String = "Publish a file"
    override val publishSubtitle: String =
        "Publishing creates a signed record and makes the file discoverable. It is an explicit action."
    override val publishUnavailableTitle: String = "Publishing is a demo in this preview"
    override val publishUnavailableBody: String =
        "The Rust core supports signed publications, but this form is sample data. " +
            "It does not import, sign, or announce a file."
    override val publishDemoForm: String = "Demo form — nothing is uploaded."
    override val publishFileLabel: String = "File"
    override val publishFileValue: String = "Choose a file"
    override val publishFileUnavailable: String =
        "File selection is demo-only here. Real publishing imports the file into the managed store."
    override val publishNameLabel: String = "Title"
    override val publishNamePlaceholder: String = "A clear, descriptive title"
    override val publishDescriptionLabel: String = "Description"
    override val publishDescriptionPlaceholder: String = "What is this file, and why is it useful?"
    override val publishTagsLabel: String = "Tags"
    override val publishTagsPlaceholder: String = "comma-separated, up to 10"
    override val publishCategoryLabel: String = "Category"
    override val publishRightsLabel: String = "Rights and license"
    override val publishRightsPlaceholder: String = "Confirm that you have the right to share this file."
    override val publishPublicTitle: String = "Publishing is explicit and public"
    override val publishPublicBody: String =
        "Peers can discover the signed record and the metadata you choose to share. " +
            "The file itself is served only while this device is online and serving it. " +
            "Peers may see network addresses; Ember does not guarantee anonymity."
    override val publishNoDeleteTitle: String = "Unpublishing is not a global delete"
    override val publishNoDeleteBody: String =
        "Unpublishing stops your device from serving and records a signed statement. " +
            "It cannot delete copies other peers already hold."
    override val publishServingTitle: String = "Serving while online"
    override val publishServingBody: String =
        "After publishing, this device serves the verified file only while the app is running " +
            "and the publication is serving. Availability is not permanent."
    override val publishSignatureNote: String =
        "The Rust core signs the publication with your local identity. " +
            "The private key never enters the record or the network."
    override val publishAfterTitle: String = "Publication states"
    override val publishAfterDemo: String = "Sample states — no record exists."
    override val publishAfterContentId: String = "Content ID"
    override val publishAfterSignature: String = "Signature"
    override val publishAfterServing: String = "Serving"
    override val publishAfterAvailability: String = "Availability"
    override val publishAfterSignatureVerified: String = "Verified (sample)"
    override val publishAfterServingOnline: String = "While this device is online (sample)"
    override val publishAfterAvailabilityValue: String = "Available now (sample)"
    override val publishAfterNote: String =
        "These states are independent. A verified signature does not promise that a source is online."
    override val publishPlannedTitle: String = "What publishing does"
    override val publishStepOne: String = "Choose a file and confirm its title, category, tags, and description."
    override val publishStepTwo: String = "Import and hash the bytes locally to create a stable content ID."
    override val publishStepThree: String = "Sign the publication record with your local identity."
    override val publishStepFour: String = "Announce the signed record and serve the file while this device is online."
    override val publishStepFive: String =
        "Unpublish at any time to stop serving and publish a signed unpublish statement."
    override val publishVerificationNote: String =
        "The signed record can be discovered before anyone downloads it. Serving depends on this device staying online."
    override val publishButton: String = "Publish (demo only)"
    override val publishButtonNote: String =
        "The Rust core handles real imports, signing, and serving. This preview does not publish anything."

    override val downloadsTitle: String = "Downloads"
    override val downloadsSubtitle: String =
        "Downloads are stored locally. A completed download is shown only after full verification."
    override val downloadsDemoNotice: String =
        "Demo transfers. No bytes are moving and no files are stored."
    override val downloadsActiveSection: String = "Active transfers"
    override val downloadsCompletedSection: String = "Completed downloads"
    override val downloadsStatusDownloading: String = "Downloading"
    override val downloadsStatusPaused: String = "Paused"
    override val downloadsStatusVerifying: String = "Verifying"
    override val downloadsStatusComplete: String = "Complete — verified"
    override val downloadsStatusFailed: String = "Failed"
    override val downloadsProgressLabel: String = "Progress"
    override val downloadsVerified: String = "Content verified"
    override val downloadsNotVerified: String = "Not verified yet"
    override val downloadsSpeedLabel: (String) -> String = { speed -> "Speed: $speed" }
    override val downloadsSourcePeerLabel: (String) -> String = { peer -> "Serving peer: $peer" }
    override val downloadsPausedNote: String = "Transfer controls are not active in this preview."
    override val downloadsDelete: String = "Delete download"
    override val downloadsDeleteDialogTitle: String = "Delete this demo download?"
    override val downloadsDeleteDialogBody: String =
        "Deleting a download removes the local copy only. It does not affect other peers' copies or the " +
            "publication record. In this preview, no file is deleted."
    override val downloadsDeleteDialogConfirm: String = "Delete (demo)"
    override val downloadsDeletedDone: String = "Demo only — no file was deleted."
    override val downloadsEmptyTitle: String = "No demo downloads"
    override val downloadsEmptyBody: String =
        "Delete actions are simulated. Restore the sample list to keep exploring."
    override val downloadsRestore: String = "Restore demo downloads"
    override val downloadsCompleteNote: String =
        "Completed downloads remain in the library until you delete or export them."
    override val downloadsVerifyingNote: String =
        "Verifying chunks — completion is not claimed until every chunk proof passes."
    override val downloadsFailedNote: String =
        "The source disappeared. Retrying a transfer is not active in this preview."

    override val hostedTitle: String = "Hosted files"
    override val hostedSubtitle: String =
        "Hosting is an explicit choice. A replica serves the original signed record and never becomes the uploader."
    override val hostedDemoNotice: String = "Demo replicas. Nothing is being served to peers."
    override val hostedHowTitle: String = "Hosting is an explicit choice"
    override val hostedHowBody: String =
        "Ember never silently seeds content. Downloading a file does not create a hosting obligation. " +
            "A replica keeps the original uploader's attribution."
    override val hostedStatusVerified: String = "Verified replica"
    override val hostedStatusVerifying: String = "Verifying copy"
    override val hostedStatusPaused: String = "Hosting paused"
    override val hostedStatusFailed: String = "Verification failed"
    override val hostedOriginalUploader: String = "Original uploader"
    override val hostedPublication: (String) -> String = { value -> "Publication: $value" }
    override val hostedStorageUse: (String) -> String = { value -> "Storage use: $value" }
    override val hostedServing: String = "Serving to peers"
    override val hostedNotServing: String = "Not serving"
    override val hostedLastSeen: (String) -> String = { value -> "Last served: $value" }
    override val hostedYouHost: String = "You host this replica"
    override val hostedPeers: (Int) -> String = { count -> "Peers served (demo): $count" }
    override val hostedLastVerified: (String) -> String = { value -> "Last verified: $value" }
    override val hostedNotVerified: String = "Not verified yet"
    override val hostedFailedNote: String =
        "Verification failed. This replica is not advertised or served."
    override val hostedPause: String = "Pause hosting (demo)"
    override val hostedResume: String = "Resume hosting (demo)"
    override val hostedPausedDone: String = "Demo only — hosting was paused in this preview."
    override val hostedResumedDone: String = "Demo only — hosting was resumed in this preview."
    override val hostedRemove: String = "Stop hosting"
    override val hostedRemoveDialogTitle: String = "Stop hosting this demo replica?"
    override val hostedRemoveDialogBody: String =
        "This removes your replica and stops serving it. It does not change the original uploader's record " +
            "or copies held by other peers. In this preview, nothing is changed."
    override val hostedRemoveDialogConfirm: String = "Stop hosting (demo)"
    override val hostedRemovedDone: String = "Demo only — hosting was not changed."
    override val hostedEmptyTitle: String = "No demo hosted files"
    override val hostedEmptyBody: String =
        "Hosting actions are simulated. Restore the sample list to keep exploring."
    override val hostedRestore: String = "Restore demo hosted files"

    override val settingsTitle: String = "Settings"
    override val settingsSubtitle: String =
        "Local settings work in this preview. Network and identity controls are placeholders."
    override val settingsAppearance: String = "Appearance"
    override val settingsTheme: String = "Theme"
    override val settingsThemeSystem: String = "System"
    override val settingsThemeLight: String = "Light"
    override val settingsThemeDark: String = "Dark"
    override val settingsThemeSaved: String = "Your theme choice is saved on this device."
    override val settingsIdentity: String = "Identity"
    override val settingsBackupTitle: String = "Identity backup and restore"
    override val settingsBackupBody: String =
        "Not available in Stage 04. Losing your key will mean losing your identity; there is no central recovery."
    override val settingsStorage: String = "Storage"
    override val settingsStorageTitle: String = "Demo library usage"
    override val settingsStorageValue: String = "1.8 GB of 10 GB (sample)"
    override val settingsStorageBody: String =
        "Quota enforcement and cleanup arrive with the local store."
    override val settingsBandwidth: String = "Bandwidth"
    override val settingsDownloadLimit: String = "Download limit"
    override val settingsUploadLimit: String = "Hosting limit"
    override val settingsDownloadLimitValue: String = "8 Mbps (demo)"
    override val settingsUploadLimitValue: String = "2 Mbps (demo)"
    override val settingsBandwidthBody: String = "Limits are placeholders in this preview."
    override val settingsNetwork: String = "Network"
    override val settingsBootstrapPeers: String = "Bootstrap peers"
    override val settingsBootstrapValue: String = "3 demo entry points configured"
    override val settingsBootstrapBody: String =
        "Bootstrap peers are entry points, not identity or content authorities."
    override val settingsLanguage: String = "Language"
    override val settingsLanguageValue: String = "English (United States)"
    override val settingsLanguageBody: String = "Language selection is not active in this preview."
    override val settingsDiagnostics: String = "Diagnostics and privacy"
    override val settingsDiagnosticsExport: String = "Export redacted diagnostics"
    override val settingsDiagnosticsBody: String =
        "Diagnostics export arrives in a later stage. A redacted bundle will never include keys, " +
            "passphrases, or local paths."
    override val settingsPrivacyTitle: String = "Privacy"
    override val settingsPrivacyBody: String =
        "No telemetry is enabled by default. Demo settings and sample data stay on this device."
    override val settingsAbout: String = "About"
    override val settingsAboutVersion: String = "Ember 0.1.0"
    override val settingsAboutBuild: String = "Stage 04 UI preview · no core connection"
    override val settingsLicenses: String = "Open-source licenses"
    override val settingsLicensesBody: String =
        "License inventory is documented in the repository."
    override val settingsPreviewTools: String = "Preview tools"
    override val settingsPreviewToolsBody: String =
        "Open the onboarding screens or inspect data states. No real identity or network action is performed."
    override val settingsPreviewWelcome: String = "View welcome screen"
    override val settingsPreviewIdentity: String = "View identity setup"
    override val settingsPreviewStates: String =
        "The Preview state bar on data-driven screens switches between ready, loading, empty, offline, and error."

    override val networkTitle: String = "Network status"
    override val networkSubtitle: String =
        "Global connectivity is separate from any file's availability."
    override val networkDemoNotice: String =
        "Simulated network status. No peer connection exists."
    override val networkStateLabel: String = "Global state"
    override val networkNodeState: String = "Node state"
    override val networkOffline: String = "Offline"
    override val networkConnecting: String = "Connecting"
    override val networkOnline: String = "Online"
    override val networkDegraded: String = "Degraded"
    override val networkOfflineBody: String =
        "No usable network path exists. Local data is still usable."
    override val networkConnectingBody: String =
        "Attempting to reach bootstrap peers. No content is available yet."
    override val networkOnlineBody: String =
        "At least one usable path exists. This does not mean any specific file is available."
    override val networkDegradedBody: String =
        "Some paths are failing. Remaining peers may still work, and results may be incomplete."
    override val networkPeerCount: String = "Connected peers"
    override val networkPeers: (Int) -> String = { count -> "$count peers (demo)" }
    override val networkBootstrapTitle: String = "Bootstrap paths"
    override val networkBootstrapBody: String =
        "Bootstrap peers are entry points, not identity or content authorities."
    override val networkPathConnected: String = "Connected"
    override val networkPathConnecting: String = "Connecting"
    override val networkPathUnreachable: String = "Unreachable"
    override val networkRecentErrors: String = "Recent errors"
    override val networkRecentErrorsBody: String =
        "Errors are redacted. No raw stack traces, secrets, or local paths are shown."
    override val networkNoErrors: String = "No recent demo errors."
    override val networkErrorNoUsablePath: String = "No usable network path was found."
    override val networkErrorBootstrapUnreachable: String = "A bootstrap path was unreachable."
    override val networkErrorPeerTimeout: String = "A peer connection timed out."
    override val networkDiagnosticsTitle: String = "Diagnostics"
    override val networkDiagnosticsBody: String =
        "A redacted diagnostics export will be available in a later stage."
    override val networkExport: String = "Export redacted diagnostics"
    override val networkExportUnavailable: String = "Not available in this preview"
    override val networkSeparateTitle: String = "Global state vs file availability"
    override val networkSeparateBody: String =
        "A file can be record-only while the node is online, or seen recently while the node is degraded. " +
            "Check both before expecting a download to work."
    override val networkPathTitle: String = "Path and NAT"
    override val networkPathBody: String =
        "The best available route to another peer. Relay fallback is tried only after a direct path fails."
    override val networkConnectivityDirect: String = "Direct"
    override val networkConnectivityRelay: String = "Relayed"
    override val networkConnectivityConnecting: String = "Connecting"
    override val networkConnectivityNoPeers: String = "No peers"
    override val networkConnectivityDisabled: String = "Disabled"
    override val networkNatLabel: String = "NAT reachability"
    override val networkNatPublic: String = "Public"
    override val networkNatPrivate: String = "Behind NAT"
    override val networkNatUnknown: String = "Unknown"
    override val networkNatDisabled: String = "Disabled"
    override val networkRelayFallbackOn: String = "Relay fallback on"
    override val networkRelayFallbackOff: String = "Relay fallback off"
    override val networkRelayTitle: String = "Community relays"
    override val networkRelayBody: String =
        "Optional relay nodes forward traffic when direct connections fail. Each circuit is bounded."
    override val networkRelayNoRelays: String = "No community relay is configured."
    override val networkRelayConfigured: String = "Configured"
    override val networkRelayConnected: String = "Connected"
    override val networkRelayReservations: String = "Reservations"
    override val networkRelayDataLimit: String = "Circuit data limit"
    override val networkRelayDurationLimit: String = "Circuit time limit"
    override val networkRelayLimitUnknown: String = "Not reported"
    override val networkDirectConnections: String = "Direct connections"
    override val networkRelayedConnections: String = "Relayed connections"
    override val networkFallbackAttempts: String = "Fallback attempts"
    override val networkFallbackSuccesses: String = "Fallback successes"
    override val networkHolePunchFailures: String = "Hole punch failures"
    override val networkErrorRelayUnavailable: String = "No community relay was reachable."
    override val networkErrorRelayReservation: String =
        "A relay reservation was refused or timed out."
    override val networkPreviewTitle: String = "Demo network state"
    override val networkPreviewBody: String =
        "Switch the simulated global state. This does not change a real network or file availability."
}

/** Composition-local string provider for future locale overrides. */
public val LocalNexusStrings: androidx.compose.runtime.ProvidableCompositionLocal<NexusStrings> =
    staticCompositionLocalOf { EnglishNexusStrings }
