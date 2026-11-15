package io.nexus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.nexus.ui.components.NexusActionRow
import io.nexus.ui.components.NexusCard
import io.nexus.ui.components.NexusDemoBadge
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusKeyValue
import io.nexus.ui.components.NexusPrimaryButton
import io.nexus.ui.components.NexusScreen
import io.nexus.ui.components.NexusSectionHeader
import io.nexus.ui.components.StatusTone
import io.nexus.ui.components.demoCategoryLabel
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusBreakpoints
import io.nexus.ui.theme.NexusSpacing

/** Publish form prototype. Every control is visibly inactive. */
@Composable
public fun PublishScreen(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    NexusScreen(
        modifier = modifier,
        title = strings.publishTitle,
        subtitle = strings.publishSubtitle,
        onBack = onBack,
    ) {
        NexusInfoBanner(
            title = strings.publishUnavailableTitle,
            body = strings.publishUnavailableBody,
            tone = StatusTone.CAUTION,
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
                    PublishForm(modifier = Modifier.weight(1f))
                    PublishNotices(modifier = Modifier.widthIn(max = 380.dp))
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg)) {
                    PublishForm(modifier = Modifier.fillMaxWidth())
                    PublishNotices(modifier = Modifier.fillMaxWidth())
                }
            }
        }
    }
}

@Composable
private fun PublishForm(modifier: Modifier = Modifier) {
    val strings = LocalNexusStrings.current
    NexusCard(modifier = modifier) {
        Row(horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm)) {
            NexusDemoBadge()
            Text(
                text = strings.publishDemoForm,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        NexusSectionHeader(title = strings.publishFileLabel)
        NexusActionRow(
            title = strings.publishFileValue,
            subtitle = strings.publishFileUnavailable,
            icon = NexusIcons.Folder,
            enabled = false,
        )
        OutlinedTextField(
            modifier = Modifier.fillMaxWidth(),
            value = "",
            onValueChange = {},
            enabled = false,
            label = { Text(strings.publishNameLabel) },
            placeholder = { Text(strings.publishNamePlaceholder) },
            singleLine = true,
        )
        OutlinedTextField(
            modifier = Modifier.fillMaxWidth(),
            value = "",
            onValueChange = {},
            enabled = false,
            label = { Text(strings.publishDescriptionLabel) },
            placeholder = { Text(strings.publishDescriptionPlaceholder) },
            minLines = 3,
            maxLines = 5,
        )
        OutlinedTextField(
            modifier = Modifier.fillMaxWidth(),
            value = "",
            onValueChange = {},
            enabled = false,
            label = { Text(strings.publishTagsLabel) },
            placeholder = { Text(strings.publishTagsPlaceholder) },
            singleLine = true,
        )
        NexusKeyValue(
            label = strings.publishCategoryLabel,
            value = demoCategoryLabel(io.nexus.presentation.demo.DemoCategory.DOCUMENTS, strings),
        )
        OutlinedTextField(
            modifier = Modifier.fillMaxWidth(),
            value = "",
            onValueChange = {},
            enabled = false,
            label = { Text(strings.publishRightsLabel) },
            placeholder = { Text(strings.publishRightsPlaceholder) },
            minLines = 2,
            maxLines = 3,
        )
        NexusPrimaryButton(
            modifier = Modifier.fillMaxWidth(),
            text = strings.publishButton,
            onClick = {},
            enabled = false,
            icon = NexusIcons.Upload,
        )
        Text(
            text = strings.publishButtonNote,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun PublishNotices(modifier: Modifier = Modifier) {
    val strings = LocalNexusStrings.current
    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(NexusSpacing.md),
    ) {
        NexusInfoBanner(
            title = strings.publishPublicTitle,
            body = strings.publishPublicBody,
            tone = StatusTone.INFO,
            icon = NexusIcons.Globe,
        )
        NexusInfoBanner(
            title = strings.publishNoDeleteTitle,
            body = strings.publishNoDeleteBody,
            tone = StatusTone.CAUTION,
            icon = NexusIcons.Warning,
        )
        NexusCard(modifier = Modifier.fillMaxWidth()) {
            NexusSectionHeader(title = strings.publishPlannedTitle)
            PlannedStep(index = 1, text = strings.publishStepOne)
            PlannedStep(index = 2, text = strings.publishStepTwo)
            PlannedStep(index = 3, text = strings.publishStepThree)
            PlannedStep(index = 4, text = strings.publishStepFour)
            PlannedStep(index = 5, text = strings.publishStepFive)
            Text(
                text = strings.publishVerificationNote,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun PlannedStep(
    index: Int,
    text: String,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(NexusSpacing.md),
    ) {
        Text(
            text = index.toString(),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
        )
        Text(
            modifier = Modifier.weight(1f),
            text = text,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
