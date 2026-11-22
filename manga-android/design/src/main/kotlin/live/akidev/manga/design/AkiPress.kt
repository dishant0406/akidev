package live.akidev.manga.design

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.constrainHeight
import androidx.compose.ui.unit.constrainWidth
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

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
    /** What a screen reader says the control is: a button, a tab, a drop-down. */
    role: Role = Role.Button,
    /** For a choice among several (a tab, a pill): whether this one is chosen. */
    selected: Boolean? = null,
    onClick: () -> Unit,
): Modifier {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    // With animations turned off in the phone's settings the control does not squeeze.
    val scale by animateFloatAsState(if (pressed && !akiReduceMotion()) squeeze else 1f, AkiMotion.press(), label = "press")
    val haptic = LocalHapticFeedback.current
    return this
        .graphicsLayer { scaleX = scale; scaleY = scale }
        .semantics { if (label != null) contentDescription = label; if (selected != null) this.selected = selected }
        .combinedClickable(
            interactionSource = source, indication = null, enabled = enabled, role = role, onClick = onClick,
            onLongClick = onLongClick?.let { run -> { haptic.performHapticFeedback(HapticFeedbackType.LongPress); run() } },
        )
}

/** A tap that does something: wraps any content (a card, a cover) as a button without the default look. */
@Composable
fun AkiTap(onClick: () -> Unit, modifier: Modifier = Modifier, squeeze: Float = 0.97f, label: String? = null, content: @Composable () -> Unit) {
    Box(modifier.akiPress(label = label, squeeze = squeeze, onClick = onClick)) { content() }
}

/**
 * Gives a small control room to touch: at least `min` wide and high, with the control still centred and looking the same. Put it after
 * [akiPress], so the whole space is what a finger can press.
 */
fun Modifier.akiTouchTarget(min: Dp = 48.dp): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints)
    val width = constraints.constrainWidth(maxOf(placeable.width, min.roundToPx()))
    val height = constraints.constrainHeight(maxOf(placeable.height, min.roundToPx()))
    layout(width, height) { placeable.place((width - placeable.width) / 2, (height - placeable.height) / 2) }
}
