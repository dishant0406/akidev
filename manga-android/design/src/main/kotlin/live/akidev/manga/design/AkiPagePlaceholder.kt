package live.akidev.manga.design

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * What stands in for a reader page until its picture arrives: a plain dark panel with a soft light sweeping across it and the page
 * number. A failed page says so and can be tapped to try again.
 */
@Composable
fun AkiPagePlaceholder(number: Int? = null, failed: Boolean = false, retry: () -> Unit = {}) {
    val colors = Aki.colors
    Box(
        Modifier.fillMaxSize().background(colors.stageAlt)
            .semantics { contentDescription = if (failed) "Page failed to load. Tap to try again." else "Loading page" }
            .clickable(remember { MutableInteractionSource() }, null, enabled = failed) { retry() },
        contentAlignment = Alignment.Center,
    ) {
        if (failed) {
            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AkiIcon("refresh", size = 22.dp, tone = AkiTone.OnMedia)
                AkiText("Tap to try again", AkiTextStyle.CaptionStrong, tone = AkiTone.OnMedia)
            }
        } else {
            Shimmer()
            if (number != null) AkiText("$number", AkiTextStyle.Heading, Modifier.alpha(0.3f), tone = AkiTone.OnMedia)
        }
    }
}

/** A band of light crossing left to right, slow and faint. */
@Composable
private fun Shimmer() {
    val phase by rememberInfiniteTransition(label = "shimmer").animateFloat(0f, 1f, infiniteRepeatable(tween(1600, easing = LinearEasing), RepeatMode.Restart), label = "phase")
    val light = Aki.colors.onMedia.copy(alpha = 0.08f)
    Canvas(Modifier.fillMaxSize()) {
        val width = size.width * 0.7f
        val x = (phase * 1.7f - 0.7f) * size.width
        drawRect(Brush.horizontalGradient(listOf(Color.Transparent, light, Color.Transparent), x, x + width), Offset(x, 0f), Size(width, size.height))
    }
}

/**
 * What the reader shows while a chapter opens and finds the place you were at: a ring and the chapter's name, on the reader's own
 * background. Nothing else moves until the page is where it should be.
 */
@Composable
fun AkiOpeningLoader(label: String? = null) {
    val turn by rememberInfiniteTransition(label = "opening").animateFloat(0f, 360f, infiniteRepeatable(tween(1000, easing = LinearEasing), RepeatMode.Restart), label = "turn")
    val color = Aki.colors.primary
    Column(Modifier.semantics { contentDescription = "Opening chapter" }, horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Canvas(Modifier.size(30.dp)) {
            val width = 3.dp.toPx()
            drawArc(color, turn, 100f, false, Offset(width / 2, width / 2), Size(size.width - width, size.height - width), style = Stroke(width, cap = StrokeCap.Round))
        }
        if (label != null) AkiText(label, AkiTextStyle.Caption, Modifier.alpha(0.6f), tone = AkiTone.OnMedia)
    }
}
