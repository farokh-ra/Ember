package io.nexus.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.navigation.AppDestination
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.resources.NexusStrings
import io.nexus.ui.theme.NexusSizes
import io.nexus.ui.theme.NexusSpacing

/** One primary destination in the application chrome. */
private data class NexusNavItem(
    public val destination: AppDestination,
    public val label: String,
    public val icon: ImageVector,
)

/** Primary navigation, in product order. */
@Composable
private fun primaryNavItems(): List<NexusNavItem> {
    val strings = LocalNexusStrings.current
    return listOf(
        NexusNavItem(AppDestination.Explore, strings.navExplore, NexusIcons.Explore),
        NexusNavItem(AppDestination.Search, strings.navSearch, NexusIcons.Search),
        NexusNavItem(AppDestination.Downloads, strings.navDownloads, NexusIcons.Downloads),
        NexusNavItem(AppDestination.HostedFiles, strings.navHosted, NexusIcons.Hosted),
        NexusNavItem(AppDestination.Settings, strings.navSettings, NexusIcons.Settings),
    )
}

/**
 * Permanent side rail for desktop-width layouts.
 *
 * The rail exposes the same five primary sections as the phone bar, plus the
 * profile, network status, and a persistent demo marker.
 */
@Composable
public fun NexusSideRail(
    currentSection: AppDestination?,
    network: PreviewNetworkState,
    onNavigate: (AppDestination) -> Unit,
    onProfile: () -> Unit,
    onNetwork: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    Surface(
        modifier = modifier.width(NexusSizes.sideRailWidth).fillMaxHeight(),
        color = MaterialTheme.colorScheme.surfaceContainerLow,
    ) {
        Column(
            modifier =
                Modifier
                    .fillMaxHeight()
                    .padding(horizontal = NexusSpacing.md, vertical = NexusSpacing.lg),
            verticalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            NexusWordmark(
                modifier = Modifier.padding(horizontal = NexusSpacing.sm),
                showTagline = true,
            )
            NexusDemoBadge(modifier = Modifier.padding(horizontal = NexusSpacing.sm))
            Spacer(modifier = Modifier.height(NexusSpacing.sm))
            Column(
                modifier = Modifier.selectableGroup(),
                verticalArrangement = Arrangement.spacedBy(NexusSpacing.xs),
            ) {
                primaryNavItems().forEach { item ->
                    SideRailItem(
                        item = item,
                        selected = item.destination == currentSection,
                        onClick = { onNavigate(item.destination) },
                    )
                }
            }
            Spacer(modifier = Modifier.weight(1f))
            NetworkStatusPill(
                state = network,
                onClick = onNetwork,
                modifier = Modifier.padding(horizontal = NexusSpacing.xs),
            )
            SideRailAction(
                icon = NexusIcons.User,
                label = strings.profile,
                onClick = onProfile,
            )
            NexusDivider(modifier = Modifier.padding(horizontal = NexusSpacing.sm))
            Text(
                modifier = Modifier.padding(horizontal = NexusSpacing.sm),
                text = strings.versionLine,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun SideRailItem(
    item: NexusNavItem,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val shape = MaterialTheme.shapes.medium
    Surface(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = NexusSizes.minimumTouchTarget)
                .nexusFocusRing(shape)
                .selectable(
                    selected = selected,
                    role = Role.Tab,
                    onClick = onClick,
                ),
        shape = shape,
        color =
            if (selected) {
                MaterialTheme.colorScheme.primaryContainer
            } else {
                MaterialTheme.colorScheme.surfaceContainerLow
            },
        contentColor =
            if (selected) {
                MaterialTheme.colorScheme.onPrimaryContainer
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
    ) {
        Row(
            modifier = Modifier.padding(horizontal = NexusSpacing.md, vertical = NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
        ) {
            Icon(
                imageVector = item.icon,
                contentDescription = null,
                modifier = Modifier.size(20.dp),
            )
            Text(
                text = item.label,
                style = MaterialTheme.typography.labelLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun SideRailAction(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
) {
    val shape = MaterialTheme.shapes.medium
    Surface(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = NexusSizes.minimumTouchTarget)
                .nexusFocusRing(shape)
                .clickable(
                    role = Role.Button,
                    onClick = onClick,
                ),
        shape = shape,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        contentColor = MaterialTheme.colorScheme.onSurface,
    ) {
        Row(
            modifier = Modifier.padding(horizontal = NexusSpacing.md, vertical = NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                modifier = Modifier.size(20.dp),
            )
            Text(text = label, style = MaterialTheme.typography.labelLarge)
        }
    }
}

/** Compact top bar for phone-width layouts. */
@Composable
public fun NexusCompactTopBar(
    network: PreviewNetworkState,
    canGoBack: Boolean,
    title: String?,
    onBack: () -> Unit,
    onNetwork: () -> Unit,
    onProfile: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surfaceContainerLow,
    ) {
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 64.dp)
                    .padding(horizontal = NexusSpacing.xs),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.xs),
        ) {
            if (canGoBack) {
                NexusBackButton(onBack = onBack)
                Text(
                    modifier = Modifier.weight(1f),
                    text = title.orEmpty(),
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            } else {
                NexusWordmark(modifier = Modifier.padding(start = NexusSpacing.sm), markSize = 28.dp)
                Spacer(modifier = Modifier.weight(1f))
            }
            NetworkStatusPill(state = network, onClick = onNetwork)
            NexusIconButton(
                icon = NexusIcons.User,
                contentDescription = strings.openProfile,
                onClick = onProfile,
            )
        }
    }
}

/** Bottom navigation for phone-width layouts. */
@Composable
public fun NexusBottomBar(
    currentSection: AppDestination?,
    onNavigate: (AppDestination) -> Unit,
    modifier: Modifier = Modifier,
) {
    NavigationBar(
        modifier = modifier.fillMaxWidth(),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        tonalElevation = 0.dp,
    ) {
        primaryNavItems().forEach { item ->
            NavigationBarItem(
                selected = item.destination == currentSection,
                onClick = { onNavigate(item.destination) },
                icon = {
                    Icon(
                        imageVector = item.icon,
                        contentDescription = null,
                        modifier = Modifier.size(22.dp),
                    )
                },
                label = {
                    Text(
                        text = item.label,
                        style = MaterialTheme.typography.labelSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                colors =
                    NavigationBarItemDefaults.colors(
                        selectedIconColor = MaterialTheme.colorScheme.onPrimaryContainer,
                        selectedTextColor = MaterialTheme.colorScheme.onSurface,
                        indicatorColor = MaterialTheme.colorScheme.primaryContainer,
                        unselectedIconColor = MaterialTheme.colorScheme.onSurfaceVariant,
                        unselectedTextColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    ),
            )
        }
    }
}

/** Shared label lookup for chrome tests and previews. */
public fun navigationLabel(
    destination: AppDestination,
    strings: NexusStrings,
): String =
    when (destination) {
        AppDestination.Explore -> strings.navExplore
        AppDestination.Search -> strings.navSearch
        AppDestination.Downloads -> strings.navDownloads
        AppDestination.HostedFiles -> strings.navHosted
        AppDestination.Settings -> strings.navSettings
        AppDestination.Profile -> strings.navProfile
        AppDestination.NetworkStatus -> strings.navNetwork
        AppDestination.Publish -> strings.navPublish
        AppDestination.Welcome -> strings.appName
        AppDestination.IdentitySetup -> strings.identityTitle
        is AppDestination.FileDetails -> strings.fileDetailsTitle
    }
