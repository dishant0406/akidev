package live.akidev.manga.design

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp

/** A slider: a track with the part that is done in yellow and a round thumb. `onEnd` is called when the finger lifts. */
@Composable
fun AkiSlider(value: Double, onChange: (Double) -> Unit, modifier: Modifier = Modifier, range: ClosedFloatingPointRange<Double> = 0.0..1.0, onEnd: () -> Unit = {}) {
    val colors = Aki.colors
    val thumb = 26.dp
    var widthPx by remember { mutableStateOf(1) }
    var dragging by remember { mutableStateOf(false) }
    val scale by animateFloatAsState(if (dragging) 1.12f else 1f, label = "thumb")
    val density = LocalDensity.current
    val travel = (widthPx - with(density) { thumb.toPx() }).coerceAtLeast(1f)
    val fraction = ((value - range.start) / (range.endInclusive - range.start)).toFloat().coerceIn(0f, 1f)
    fun valueAt(x: Float): Double {
        val position = ((x - with(density) { thumb.toPx() } / 2) / travel).coerceIn(0f, 1f)
        return range.start + position * (range.endInclusive - range.start)
    }
    Box(
        modifier.fillMaxWidth().height(36.dp).onSizeChanged { widthPx = it.width }
            .pointerInput(range) { detectTapGestures { onChange(valueAt(it.x)); onEnd() } }
            .pointerInput(range) {
                detectDragGestures(
                    onDragStart = { dragging = true; onChange(valueAt(it.x)) },
                    onDragEnd = { dragging = false; onEnd() },
                    onDragCancel = { dragging = false; onEnd() },
                ) { change, _ -> onChange(valueAt(change.position.x)) }
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        Box(Modifier.fillMaxWidth().height(6.dp).background(colors.secondary, CircleShape))
        Box(Modifier.width(thumb / 2 + with(density) { (travel * fraction).toDp() }).height(6.dp).background(colors.primary, CircleShape))
        Box(Modifier.offset(x = with(density) { (travel * fraction).toDp() }).size(thumb).scale(scale).shadow(4.dp, CircleShape).background(colors.onMedia, CircleShape))
    }
}
