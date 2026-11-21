package io.nexus.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import io.nexus.presentation.demo.DemoUsernameCollision
import io.nexus.presentation.demo.ProfileFreshness
import io.nexus.presentation.demo.ProfileRecordState
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.resources.NexusStrings
import io.nexus.ui.theme.NexusSpacing

/** Profile summary with explicit demo labelling. */
@Composable
public fun ProfileSummaryCard(
    displayName: String,
    usernameLabel: String,
    bio: String,
    keyFingerprint: String,
    publicationCount: Int,
    hostedReplicaCount: Int,
    joinedLabel: String,
    identityId: String,
    recordRevision: Int,
    recordState: ProfileRecordState,
    freshness: ProfileFreshness,
    publishedLabel: String,
    lastCheckedLabel: String,
    conflictCount: Int,
    usernameCollisions: List<DemoUsernameCollision>,
    onNetworkStatus: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = modifier.fillMaxWidth()) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
        ) {
            NexusAvatar(name = displayName, size = 64.dp)
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = displayName,
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    text = "@$usernameLabel",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    text = strings.sampleRecord,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            NexusDemoBadge()
        }
        Text(
            text = bio,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        ProfileRecordBadges(recordState = recordState, freshness = freshness)
        ProfileRecordMetadata(
            identityId = identityId,
            recordRevision = recordRevision,
            publishedLabel = publishedLabel,
            lastCheckedLabel = lastCheckedLabel,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            NexusMetric(
                modifier = Modifier.weight(1f),
                value = publicationCount.toString(),
                label = strings.profilePublications,
                icon = NexusIcons.File,
            )
            NexusMetric(
                modifier = Modifier.weight(1f),
                value = hostedReplicaCount.toString(),
                label = strings.profileHostedReplicas,
                icon = NexusIcons.Hosted,
            )
        }
        Text(
            text = "${strings.profileJoined}: $joinedLabel",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        NexusKeyValue(
            label = strings.profileKeyLabel,
            value = keyFingerprint,
            monospace = true,
        )
        ProfileTrustNotices(
            conflictCount = conflictCount,
            usernameCollisions = usernameCollisions,
        )
        Text(
            text = strings.profileSignedNote,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        NexusActionRow(
            title = strings.profileNetworkLink,
            icon = NexusIcons.Globe,
            onClick = onNetworkStatus,
        )
    }
}

@Composable
private fun ProfileRecordBadges(
    recordState: ProfileRecordState,
    freshness: ProfileFreshness,
) {
    val strings = LocalNexusStrings.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
    ) {
        StatusBadge(
            text = strings.profileSignedRecord,
            tone = StatusTone.INFO,
            icon = NexusIcons.Shield,
        )
        StatusBadge(
            text = profileStateLabel(strings, recordState),
            tone = profileStateTone(recordState),
            icon = profileStateIcon(recordState),
        )
    }
    StatusBadge(
        text = "${strings.profileFreshnessLabel}: ${profileFreshnessLabel(strings, freshness)}",
        tone = profileFreshnessTone(freshness),
        icon = profileFreshnessIcon(freshness),
    )
}

private fun profileStateLabel(
    strings: NexusStrings,
    state: ProfileRecordState,
): String =
    when (state) {
        ProfileRecordState.DRAFT -> strings.profileRecordDraft
        ProfileRecordState.PENDING -> strings.profileRecordPending
        ProfileRecordState.PUBLISHED -> strings.profileRecordPublished
    }

private fun profileStateTone(state: ProfileRecordState): StatusTone =
    when (state) {
        ProfileRecordState.DRAFT -> StatusTone.NEUTRAL
        ProfileRecordState.PENDING -> StatusTone.CAUTION
        ProfileRecordState.PUBLISHED -> StatusTone.AVAILABLE
    }

private fun profileStateIcon(state: ProfileRecordState): ImageVector =
    when (state) {
        ProfileRecordState.DRAFT -> NexusIcons.Info
        ProfileRecordState.PENDING -> NexusIcons.Clock
        ProfileRecordState.PUBLISHED -> NexusIcons.Check
    }

private fun profileFreshnessLabel(
    strings: NexusStrings,
    freshness: ProfileFreshness,
): String =
    when (freshness) {
        ProfileFreshness.CURRENT -> strings.profileFreshnessCurrent
        ProfileFreshness.STALE -> strings.profileFreshnessStale
        ProfileFreshness.UNKNOWN -> strings.profileFreshnessUnknown
    }

private fun profileFreshnessTone(freshness: ProfileFreshness): StatusTone =
    when (freshness) {
        ProfileFreshness.CURRENT -> StatusTone.AVAILABLE
        ProfileFreshness.STALE -> StatusTone.CAUTION
        ProfileFreshness.UNKNOWN -> StatusTone.NEUTRAL
    }

private fun profileFreshnessIcon(freshness: ProfileFreshness): ImageVector =
    when (freshness) {
        ProfileFreshness.CURRENT -> NexusIcons.Check
        ProfileFreshness.STALE -> NexusIcons.Clock
        ProfileFreshness.UNKNOWN -> NexusIcons.Info
    }

@Composable
private fun ProfileRecordMetadata(
    identityId: String,
    recordRevision: Int,
    publishedLabel: String,
    lastCheckedLabel: String,
) {
    val strings = LocalNexusStrings.current
    NexusKeyValue(
        label = strings.profileIdentityId,
        value = identityId,
        monospace = true,
    )
    NexusKeyValue(
        label = strings.profileRevision,
        value = recordRevision.toString(),
    )
    Text(
        text =
            "${strings.profilePublishedAt}: $publishedLabel · " +
                "${strings.profileLastChecked}: $lastCheckedLabel",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun ProfileTrustNotices(
    conflictCount: Int,
    usernameCollisions: List<DemoUsernameCollision>,
) {
    val strings = LocalNexusStrings.current
    if (conflictCount > 0) {
        NexusInfoBanner(
            title = "${strings.profileConflictsLabel}: $conflictCount",
            body = strings.profileConflictNote,
            tone = StatusTone.CAUTION,
            icon = NexusIcons.Info,
        )
    }
    if (usernameCollisions.isEmpty()) {
        Text(
            text = strings.profileNoCollisions,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    } else {
        NexusInfoBanner(
            title = strings.profileCollisionsLabel,
            body = strings.profileCollisionNote,
            tone = StatusTone.INFO,
            icon = NexusIcons.User,
        )
        usernameCollisions.forEach { collision ->
            NexusKeyValue(
                label = "${collision.displayName} (@${collision.usernameLabel})",
                value = collision.identityId,
                monospace = true,
            )
        }
    }
}
