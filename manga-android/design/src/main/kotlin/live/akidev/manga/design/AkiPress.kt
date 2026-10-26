package live.akidev.manga.design

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics

/**
 * Taps like an iOS control: no ripple, the control squeezes a little while a finger is on it (a big card squeezes less: `squeeze`).
 * A long press runs `onLongClick`, if there is one.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun Modifier.akiPress(
    label: String? = null,
    enabled: Boolean = true,
    squeeze: Float = 0.94f,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit,
): Modifier {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val scale by animateFloatAsState(if (pressed) squeeze else 1f, AkiMotion.press(), label = "press")
    val haptic = LocalHapticFeedback.current
    return this
        .graphicsLayer { scaleX = scale; scaleY = scale }
        .semantics { if (label != null) contentDescription = label }
        .combinedClickable(
            interactionSource = source, indication = null, enabled = enabled, role = Role.Button, onClick = onClick,
            onLongClick = onLongClick?.let { run -> { haptic.performHapticFeedback(HapticFeedbackType.LongPress); run() } },
        )
}
