package io.nexus.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSpacing

/**
 * Confirmation dialog for demo actions that would normally be destructive.
 *
 * A persistent demo badge and note make clear that confirming does not change
 * local storage, remote copies, or any network record.
 */
@Composable
public fun NexusConfirmDialog(
    title: String,
    body: String,
    confirmLabel: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    danger: Boolean = true,
) {
    val strings = LocalNexusStrings.current
    AlertDialog(
        modifier = modifier,
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = title,
                style = MaterialTheme.typography.headlineSmall,
            )
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.md)) {
                Text(
                    text = body,
                    style = MaterialTheme.typography.bodyMedium,
                )
                NexusDemoBadge()
                Text(
                    text = strings.stateDemoNote,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = {
            if (danger) {
                NexusDangerButton(text = confirmLabel, onClick = onConfirm)
            } else {
                NexusPrimaryButton(text = confirmLabel, onClick = onConfirm)
            }
        },
        dismissButton = {
            NexusTertiaryButton(text = strings.cancel, onClick = onDismiss)
        },
    )
}

/** Snackbar host styled with the NEXUS surface tokens. */
@Composable
public fun NexusSnackbarHost(
    hostState: SnackbarHostState,
    modifier: Modifier = Modifier,
) {
    SnackbarHost(
        hostState = hostState,
        modifier = modifier,
    )
}
