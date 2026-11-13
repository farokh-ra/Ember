package io.nexus.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import io.nexus.ui.resources.LocalNexusStrings

/**
 * Initial-based avatar used by the demo profile and uploader rows.
 *
 * The avatar is decorative except for its semantic label, so screen readers do
 * not read the initials twice.
 */
@Composable
public fun NexusAvatar(
    name: String,
    modifier: Modifier = Modifier,
    size: Dp = 48.dp,
) {
    val strings = LocalNexusStrings.current
    val initials = initialsOf(name)
    Surface(
        modifier =
            modifier
                .size(size)
                .clearAndSetSemantics {
                    contentDescription = strings.avatarDescription(name)
                },
        shape = CircleShape,
        color = MaterialTheme.colorScheme.primaryContainer,
        contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
    ) {
        Box(contentAlignment = Alignment.Center) {
            Text(
                text = initials,
                style =
                    if (size >= 72.dp) {
                        MaterialTheme.typography.headlineMedium
                    } else {
                        MaterialTheme.typography.titleMedium
                    },
            )
        }
    }
}

private fun initialsOf(name: String): String {
    val words = name.trim().split(Regex("\\s+")).filter(String::isNotEmpty)
    if (words.isEmpty()) {
        return "?"
    }
    val first = words.first().first().uppercaseChar()
    val last = if (words.size > 1) words.last().first().uppercaseChar() else null
    return if (last == null) "$first" else "$first$last"
}
