package live.akidev.manga.design

import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import android.view.WindowManager
import kotlinx.coroutines.launch

private val FloatingShape = RoundedCornerShape(40.dp)

/**
 * A sheet that rises over the screen, on the page colour so the cards inside it show. It closes when the dark area above it is tapped
 * or the sheet is pulled down. [full] makes it as tall as the screen allows.
 */
@Composable
fun AkiSheet(onDismiss: () -> Unit, full: Boolean = false, content: @Composable ColumnScope.() -> Unit) {
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        NoWindowDim()
        val scope = rememberCoroutineScope()
        val rise = remember { Animatable(1f) }
        // Opens at half the screen, like the iOS medium detent; a flick up gives it the whole screen, a pull down brings it back and then closes it.
        var expanded by remember { mutableStateOf(full) }
        val screen = with(LocalDensity.current) { LocalWindowInfo.current.containerSize.height.toDp() }
        LaunchedEffect(Unit) { rise.animateTo(0f, AkiMotion.settle()) }
        // Down is the distance pulled, as a fraction of the screen; the sheet leaves the same way it was pulled.
        fun close() = scope.launch { rise.animateTo(1f, AkiMotion.settle()); onDismiss() }
        Box(
            Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.25f * (1f - rise.value))).clickable(remember { MutableInteractionSource() }, null) { close() },
            contentAlignment = Alignment.BottomCenter,
        ) {
            Column(
                Modifier
                    .widthIn(max = 640.dp)
                    .fillMaxWidth()
                    // The sheet floats a little in from the edges, as on iOS.
                    .padding(horizontal = 8.dp)
                    .navigationBarsPadding()
                    .padding(bottom = 8.dp)
                    .animateContentSize()
                    .then(if (full) Modifier.fillMaxHeight(0.94f) else Modifier.heightIn(max = screen * (if (expanded) 0.94f else 0.55f)))
                    .graphicsLayer { translationY = rise.value * (size.height + 120.dp.toPx()) }
                    .clip(FloatingShape)
                    .background(Aki.colors.background)
                    .clickable(remember { MutableInteractionSource() }, null) {}
                    .draggable(
                        rememberDraggableState { delta -> scope.launch { rise.snapTo((rise.value + delta / (screen.value * 2)).coerceIn(0f, 1f)) } },
                        Orientation.Vertical,
                        onDragStopped = { velocity ->
                            when {
                                velocity < -300f && !expanded -> { expanded = true; rise.animateTo(0f, AkiMotion.settle()) }
                                rise.value > 0.12f && expanded && !full -> { expanded = false; rise.animateTo(0f, AkiMotion.settle()) }
                                rise.value > 0.12f -> close()
                                else -> rise.animateTo(0f, AkiMotion.settle())
                            }
                        },
                    )
                    .imePadding(),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                AkiGlassWindow(Aki.colors.background) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Box(Modifier.padding(top = 8.dp, bottom = 4.dp).width(36.dp).height(5.dp).background(Aki.colors.textMuted.copy(alpha = 0.4f), CircleShape))
                        content()
                    }
                }
            }
        }
    }
}

/** A dialog's window dims what is behind it on its own; the sheet dims by itself (and by the pull), so the window's dimming is turned off. */
@Composable
fun NoWindowDim() {
    val window = (LocalView.current.parent as? DialogWindowProvider)?.window
    SideEffect { window?.clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND) }
}
