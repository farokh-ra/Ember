package io.nexus.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSizes
import io.nexus.ui.theme.NexusSpacing

/**
 * Global preview controls for data-driven screens.
 *
 * Both controls are visibly labelled as simulated. Changing either control only
 * mutates local prototype state; no service is called.
 */
@Composable
public fun PreviewStateBar(
    scenario: PreviewScenario,
    onScenarioChange: (PreviewScenario) -> Unit,
    network: PreviewNetworkState,
    onNetworkChange: (PreviewNetworkState) -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surfaceContainer,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        shape = MaterialTheme.shapes.medium,
    ) {
        Column(
            modifier = Modifier.padding(horizontal = NexusSpacing.md, vertical = NexusSpacing.sm),
            verticalArrangement = Arrangement.spacedBy(NexusSpacing.xs),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            ) {
                Icon(
                    imageVector = NexusIcons.Info,
                    contentDescription = null,
                    modifier = Modifier.size(16.dp),
                    tint = MaterialTheme.colorScheme.primary,
                )
                Text(
                    text = strings.previewState,
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Spacer(modifier = Modifier.weight(1f))
                NetworkPreviewMenu(
                    network = network,
                    onNetworkChange = onNetworkChange,
                )
            }
            Text(
                text = strings.previewStateHelp,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .selectableGroup(),
                horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            ) {
                PreviewScenario.entries.forEach { option ->
                    NexusChoiceChip(
                        text = previewScenarioLabel(option, strings),
                        selected = option == scenario,
                        onClick = { onScenarioChange(option) },
                    )
                }
            }
        }
    }
}

/** Compact selector for the simulated global network state. */
@Composable
public fun NetworkPreviewMenu(
    network: PreviewNetworkState,
    onNetworkChange: (PreviewNetworkState) -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    var expanded by remember { mutableStateOf(false) }

    Box(modifier = modifier) {
        OutlinedButton(
            onClick = { expanded = true },
            modifier = Modifier.heightIn(min = NexusSizes.minimumTouchTarget),
        ) {
            Icon(
                imageVector = NexusIcons.Globe,
                contentDescription = null,
                modifier = Modifier.size(16.dp),
            )
            Spacer(modifier = Modifier.width(6.dp))
            Text(
                text = networkStateLabel(network, strings),
                style = MaterialTheme.typography.labelMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        DropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false },
        ) {
            Text(
                modifier = Modifier.padding(horizontal = NexusSpacing.lg, vertical = NexusSpacing.sm),
                text = strings.previewNetworkHelp,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            PreviewNetworkState.entries.forEach { option ->
                DropdownMenuItem(
                    modifier = Modifier.heightIn(min = NexusSizes.minimumTouchTarget),
                    text = {
                        Text(
                            text = networkStateLabel(option, strings),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    },
                    leadingIcon = {
                        Icon(
                            imageVector = networkStateIcon(option),
                            contentDescription = null,
                            modifier = Modifier.size(18.dp),
                        )
                    },
                    trailingIcon = {
                        if (option == network) {
                            Icon(
                                imageVector = NexusIcons.Check,
                                contentDescription = null,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    },
                    onClick = {
                        expanded = false
                        onNetworkChange(option)
                    },
                )
            }
        }
    }
}
