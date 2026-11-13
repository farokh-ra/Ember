package io.nexus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import io.nexus.ui.components.NexusBackButton
import io.nexus.ui.components.NexusDemoBadge
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusPrimaryButton
import io.nexus.ui.components.NexusSecondaryButton
import io.nexus.ui.components.NexusWordmark
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusBreakpoints
import io.nexus.ui.theme.NexusSizes
import io.nexus.ui.theme.NexusSpacing

/** First-launch welcome and honest preview introduction. */
@Composable
public fun WelcomeScreen(
    onOpenDemo: () -> Unit,
    onSetupIdentity: () -> Unit,
    modifier: Modifier = Modifier,
    onBack: (() -> Unit)? = null,
) {
    val strings = LocalNexusStrings.current
    BoxWithConstraints(
        modifier =
            modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
    ) {
        val expanded = maxWidth >= NexusBreakpoints.expanded
        val gutter = if (expanded) NexusSpacing.xxxl else NexusSpacing.lg
        Column(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .widthIn(max = NexusSizes.maxContentWidth)
                    .padding(horizontal = gutter, vertical = NexusSpacing.xl),
            verticalArrangement = Arrangement.spacedBy(NexusSpacing.xl),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                NexusWordmark(showTagline = true)
                Spacer(modifier = Modifier.weight(1f))
                if (onBack != null) {
                    NexusBackButton(onBack = onBack)
                } else {
                    Text(
                        text = strings.previewLabel,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            Spacer(modifier = Modifier.height(NexusSpacing.xl))
            Text(
                text = strings.welcomeEyebrow,
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
            )
            Text(
                text = strings.welcomeTitle,
                modifier = Modifier.widthIn(max = 760.dp),
                style =
                    if (expanded) {
                        MaterialTheme.typography.displayMedium
                    } else {
                        MaterialTheme.typography.displaySmall
                    },
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = strings.welcomeBody,
                modifier = Modifier.widthIn(max = 680.dp),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (expanded) {
                Row(horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md)) {
                    NexusPrimaryButton(
                        text = strings.welcomePrimary,
                        onClick = onOpenDemo,
                        icon = NexusIcons.Explore,
                    )
                    NexusSecondaryButton(
                        text = strings.welcomeSecondary,
                        onClick = onSetupIdentity,
                        icon = NexusIcons.User,
                    )
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.sm)) {
                    NexusPrimaryButton(
                        modifier = Modifier.fillMaxWidth(),
                        text = strings.welcomePrimary,
                        onClick = onOpenDemo,
                        icon = NexusIcons.Explore,
                    )
                    NexusSecondaryButton(
                        modifier = Modifier.fillMaxWidth(),
                        text = strings.welcomeSecondary,
                        onClick = onSetupIdentity,
                        icon = NexusIcons.User,
                    )
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                NexusDemoBadge()
                Spacer(modifier = Modifier.size(NexusSpacing.sm))
                Text(
                    text = strings.previewLabel,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            NexusInfoBanner(
                title = strings.welcomeDemoNoticeTitle,
                body = strings.welcomeDemoNoticeBody,
                tone = io.nexus.ui.components.StatusTone.CAUTION,
                icon = NexusIcons.Info,
            )
            Spacer(modifier = Modifier.height(NexusSpacing.sm))
            if (expanded) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                ) {
                    WelcomeValueCard(
                        modifier = Modifier.weight(1f),
                        icon = NexusIcons.Lock,
                        title = strings.welcomeValueLocalTitle,
                        body = strings.welcomeValueLocalBody,
                    )
                    WelcomeValueCard(
                        modifier = Modifier.weight(1f),
                        icon = NexusIcons.Shield,
                        title = strings.welcomeValueHonestTitle,
                        body = strings.welcomeValueHonestBody,
                    )
                    WelcomeValueCard(
                        modifier = Modifier.weight(1f),
                        icon = NexusIcons.Monitor,
                        title = strings.welcomeValueCrossPlatformTitle,
                        body = strings.welcomeValueCrossPlatformBody,
                    )
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.md)) {
                    WelcomeValueCard(
                        modifier = Modifier.fillMaxWidth(),
                        icon = NexusIcons.Lock,
                        title = strings.welcomeValueLocalTitle,
                        body = strings.welcomeValueLocalBody,
                    )
                    WelcomeValueCard(
                        modifier = Modifier.fillMaxWidth(),
                        icon = NexusIcons.Shield,
                        title = strings.welcomeValueHonestTitle,
                        body = strings.welcomeValueHonestBody,
                    )
                    WelcomeValueCard(
                        modifier = Modifier.fillMaxWidth(),
                        icon = NexusIcons.Monitor,
                        title = strings.welcomeValueCrossPlatformTitle,
                        body = strings.welcomeValueCrossPlatformBody,
                    )
                }
            }
            Text(
                text = strings.welcomeFooter,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun WelcomeValueCard(
    icon: ImageVector,
    title: String,
    body: String,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.heightIn(min = 140.dp),
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        border =
            androidx.compose.foundation.BorderStroke(
                1.dp,
                MaterialTheme.colorScheme.outlineVariant,
            ),
    ) {
        Column(
            modifier = Modifier.padding(NexusSpacing.lg),
            verticalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                modifier = Modifier.size(24.dp),
                tint = MaterialTheme.colorScheme.primary,
            )
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = body,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
