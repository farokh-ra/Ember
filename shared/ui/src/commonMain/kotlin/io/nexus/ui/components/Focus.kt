package io.nexus.ui.components

import androidx.compose.foundation.border
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp

/**
 * Adds a visible 2 dp focus ring. Apply before the focusable/clickable modifier
 * so it observes the child focus target.
 */
@Composable
public fun Modifier.nexusFocusRing(shape: Shape = MaterialTheme.shapes.medium): Modifier {
    var focused by remember { mutableStateOf(false) }
    val color: Color =
        if (focused) {
            MaterialTheme.colorScheme.primary
        } else {
            Color.Transparent
        }
    return this
        .onFocusChanged { focused = it.isFocused }
        .border(width = 2.dp, color = color, shape = shape)
}
