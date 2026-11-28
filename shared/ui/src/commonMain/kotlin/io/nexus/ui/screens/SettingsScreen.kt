package io.nexus.ui.screens

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.nexus.core.api.CoreAvailability
import io.nexus.core.api.CoreStatus
import io.nexus.presentation.ThemeMode
import io.nexus.ui.LocalCoreStatus
import io.nexus.ui.components.NexusActionRow
import io.nexus.ui.components.NexusCard
import io.nexus.ui.components.NexusChoiceChip
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusKeyValue
import io.nexus.ui.components.NexusScreen
import io.nexus.ui.components.NexusSectionHeader
import io.nexus.ui.components.StatusTone
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.resources.NexusStrings
import io.nexus.ui.theme.NexusBreakpoints
import io.nexus.ui.theme.NexusSpacing

/** Local settings plus clearly labelled placeholders for later stages. */
@Composable
public fun SettingsScreen(
    themeMode: ThemeMode,
    onThemeChange: (ThemeMode) -> Unit,
    onOpenWelcome: () -> Unit,
    onOpenIdentity: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    NexusScreen(
        modifier = modifier,
        title = strings.settingsTitle,
        subtitle = strings.settingsSubtitle,
    ) {
        NexusInfoBanner(
            title = strings.settingsTheme,
            body = strings.settingsSubtitle,
            tone = StatusTone.INFO,
            icon = NexusIcons.Info,
        )
        BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
            val expanded = maxWidth >= NexusBreakpoints.expanded
            if (expanded) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                    verticalAlignment = androidx.compose.ui.Alignment.Top,
                ) {
                    Column(
                        modifier = Modifier.weight(1f),
                        verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                    ) {
                        AppearanceSection(themeMode = themeMode, onThemeChange = onThemeChange)
                        IdentitySettingsSection()
                        StorageSection()
                        BandwidthSection()
                    }
                    Column(
                        modifier = Modifier.widthIn(max = 440.dp),
                        verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
                    ) {
                        NetworkSettingsSection()
                        LanguageSection()
                        DiagnosticsSection()
                        AboutSection()
                        PreviewToolsSection(
                            onOpenWelcome = onOpenWelcome,
                            onOpenIdentity = onOpenIdentity,
                        )
                    }
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg)) {
                    AppearanceSection(themeMode = themeMode, onThemeChange = onThemeChange)
                    IdentitySettingsSection()
                    StorageSection()
                    BandwidthSection()
                    NetworkSettingsSection()
                    LanguageSection()
                    DiagnosticsSection()
                    AboutSection()
                    PreviewToolsSection(
                        onOpenWelcome = onOpenWelcome,
                        onOpenIdentity = onOpenIdentity,
                    )
                }
            }
        }
    }
}

@Composable
private fun AppearanceSection(
    themeMode: ThemeMode,
    onThemeChange: (ThemeMode) -> Unit,
) {
    val strings = LocalNexusStrings.current
    SettingsCard(title = strings.settingsAppearance) {
        Text(
            text = strings.settingsTheme,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .selectableGroup(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            ThemeMode.entries.forEach { mode ->
                val (label, icon) =
                    when (mode) {
                        ThemeMode.SYSTEM -> strings.settingsThemeSystem to NexusIcons.Monitor
                        ThemeMode.LIGHT -> strings.settingsThemeLight to NexusIcons.Sun
                        ThemeMode.DARK -> strings.settingsThemeDark to NexusIcons.Moon
                    }
                NexusChoiceChip(
                    text = label,
                    selected = themeMode == mode,
                    onClick = { onThemeChange(mode) },
                    icon = icon,
                )
            }
        }
        Text(
            text = strings.settingsThemeSaved,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun IdentitySettingsSection() {
    val strings = LocalNexusStrings.current
    SettingsCard(title = strings.settingsIdentity) {
        NexusActionRow(
            title = strings.settingsBackupTitle,
            subtitle = strings.settingsBackupBody,
            icon = NexusIcons.Lock,
            enabled = false,
        )
    }
}

@Composable
private fun StorageSection() {
    val strings = LocalNexusStrings.current
    SettingsCard(title = strings.settingsStorage) {
        NexusKeyValue(
            label = strings.settingsStorageTitle,
            value = strings.settingsStorageValue,
        )
        Text(
            text = strings.settingsStorageBody,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun BandwidthSection() {
    val strings = LocalNexusStrings.current
    SettingsCard(title = strings.settingsBandwidth) {
        NexusKeyValue(
            label = strings.settingsDownloadLimit,
            value = strings.settingsDownloadLimitValue,
        )
        NexusKeyValue(
            label = strings.settingsUploadLimit,
            value = strings.settingsUploadLimitValue,
        )
        Text(
            text = strings.settingsBandwidthBody,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun NetworkSettingsSection() {
    val strings = LocalNexusStrings.current
    SettingsCard(title = strings.settingsNetwork) {
        NexusKeyValue(
            label = strings.settingsBootstrapPeers,
            value = strings.settingsBootstrapValue,
        )
        Text(
            text = strings.settingsBootstrapBody,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun LanguageSection() {
    val strings = LocalNexusStrings.current
    SettingsCard(title = strings.settingsLanguage) {
        NexusKeyValue(
            label = strings.settingsLanguage,
            value = strings.settingsLanguageValue,
        )
        Text(
            text = strings.settingsLanguageBody,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun DiagnosticsSection() {
    val strings = LocalNexusStrings.current
    val coreStatus = LocalCoreStatus.current
    SettingsCard(title = strings.settingsDiagnostics) {
        NexusKeyValue(
            label = strings.settingsCoreStatus,
            value = coreStatusLabel(coreStatus, strings),
        )
        NexusKeyValue(
            label = strings.settingsCoreProtocol,
            value = coreStatus.protocolVersion,
            monospace = true,
        )
        coreStatus.detail?.let { detail ->
            NexusKeyValue(
                label = strings.settingsCoreDetail,
                value = detail,
            )
        }
        NexusActionRow(
            title = strings.settingsDiagnosticsExport,
            subtitle = strings.settingsDiagnosticsBody,
            icon = NexusIcons.File,
            enabled = false,
        )
        NexusInfoBanner(
            title = strings.settingsPrivacyTitle,
            body = strings.settingsPrivacyBody,
            tone = StatusTone.INFO,
            icon = NexusIcons.Shield,
        )
    }
}

/** User-facing core availability label. Never claims more than the shell reports. */
private fun coreStatusLabel(
    status: CoreStatus,
    strings: NexusStrings,
): String =
    when (status.availability) {
        CoreAvailability.READY -> strings.coreStatusReady
        CoreAvailability.STARTING -> strings.coreStatusStarting
        CoreAvailability.DEGRADED -> strings.coreStatusDegraded
        CoreAvailability.STOPPED -> strings.coreStatusStopped
        CoreAvailability.UNAVAILABLE -> strings.coreStatusUnavailable
    }

@Composable
private fun AboutSection() {
    val strings = LocalNexusStrings.current
    SettingsCard(title = strings.settingsAbout) {
        NexusKeyValue(
            label = strings.settingsAboutVersion,
            value = strings.settingsAboutBuild,
        )
        Text(
            text = strings.settingsLicensesBody,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun PreviewToolsSection(
    onOpenWelcome: () -> Unit,
    onOpenIdentity: () -> Unit,
) {
    val strings = LocalNexusStrings.current
    SettingsCard(title = strings.settingsPreviewTools) {
        Text(
            text = strings.settingsPreviewToolsBody,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        NexusActionRow(
            title = strings.settingsPreviewWelcome,
            icon = NexusIcons.Explore,
            onClick = onOpenWelcome,
        )
        NexusActionRow(
            title = strings.settingsPreviewIdentity,
            icon = NexusIcons.User,
            onClick = onOpenIdentity,
        )
        Text(
            text = strings.settingsPreviewStates,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun SettingsCard(
    title: String,
    content: @Composable ColumnScope.() -> Unit,
) {
    NexusCard(modifier = Modifier.fillMaxWidth()) {
        NexusSectionHeader(title = title)
        content()
    }
}
