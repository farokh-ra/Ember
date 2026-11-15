package io.nexus.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSizes

private val ButtonPadding = PaddingValues(horizontal = 20.dp, vertical = 12.dp)

/** Primary action. Always at least 48 dp tall. */
@Composable
public fun NexusPrimaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    icon: ImageVector? = null,
) {
    Button(
        onClick = onClick,
        modifier = modifier.heightIn(min = NexusSizes.minimumTouchTarget),
        enabled = enabled,
        contentPadding = ButtonPadding,
        colors =
            ButtonDefaults.buttonColors(
                disabledContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                disabledContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
            ),
    ) {
        ButtonLabel(text = text, icon = icon)
    }
}

/** Secondary action for an alternative path. At least 48 dp tall. */
@Composable
public fun NexusSecondaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    icon: ImageVector? = null,
) {
    OutlinedButton(
        onClick = onClick,
        modifier = modifier.heightIn(min = NexusSizes.minimumTouchTarget),
        enabled = enabled,
        contentPadding = ButtonPadding,
    ) {
        ButtonLabel(text = text, icon = icon)
    }
}

/** Low-emphasis action inside banners and cards. At least 48 dp tall. */
@Composable
public fun NexusTertiaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    icon: ImageVector? = null,
) {
    TextButton(
        onClick = onClick,
        modifier = modifier.heightIn(min = NexusSizes.minimumTouchTarget),
        enabled = enabled,
        contentPadding = ButtonPadding,
    ) {
        ButtonLabel(text = text, icon = icon)
    }
}

/** Destructive confirmation action. Use only inside a confirmation dialog. */
@Composable
public fun NexusDangerButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    Button(
        onClick = onClick,
        modifier = modifier.heightIn(min = NexusSizes.minimumTouchTarget),
        enabled = enabled,
        contentPadding = ButtonPadding,
        colors =
            ButtonDefaults.buttonColors(
                containerColor = MaterialTheme.colorScheme.error,
                contentColor = MaterialTheme.colorScheme.onError,
                disabledContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                disabledContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
            ),
    ) {
        Text(text = text, style = MaterialTheme.typography.labelLarge)
    }
}

/** Standard toolbar icon button with a required accessible name. */
@Composable
public fun NexusIconButton(
    icon: ImageVector,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    IconButton(
        onClick = onClick,
        modifier = modifier.size(NexusSizes.minimumTouchTarget),
        enabled = enabled,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = contentDescription,
            modifier = Modifier.size(22.dp),
        )
    }
}

/** Toolbar back action; the icon carries the localized accessible name. */
@Composable
public fun NexusBackButton(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    NexusIconButton(
        icon = NexusIcons.Back,
        contentDescription = strings.back,
        onClick = onBack,
        modifier = modifier,
    )
}

@Composable
private fun ButtonLabel(
    text: String,
    icon: ImageVector?,
) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                modifier = Modifier.size(18.dp),
            )
        }
        Text(text = text, style = MaterialTheme.typography.labelLarge)
    }
}
