package live.akidev.manga.design

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp

/** A quarter-circle that turns: waiting for something. */
@Composable
fun AkiSpinner(modifier: Modifier = Modifier) {
    val color = Aki.colors.textMuted
    val turn by rememberInfiniteTransition(label = "spinner").animateFloat(0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing), RepeatMode.Restart), label = "turn")
    Canvas(modifier.size(24.dp)) {
        val width = 2.5.dp.toPx()
        drawArc(color, turn, 270f, false, Offset(width / 2, width / 2), Size(size.width - width, size.height - width), style = Stroke(width, cap = StrokeCap.Round))
    }
}

/** A bar that fills: a download. */
@Composable
fun AkiProgressBar(fraction: Double, modifier: Modifier = Modifier) {
    val colors = Aki.colors
    Box(modifier.fillMaxWidth().height(6.dp).background(colors.secondary, CircleShape)) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(fraction.coerceIn(0.0, 1.0).toFloat()).background(colors.primary, CircleShape))
    }
}
