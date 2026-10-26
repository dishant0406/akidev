package live.akidev.manga.design

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.cos
import kotlin.math.sin

/** Slow soft light behind the first screen: yellow glows that drift, so the glass above always has something moving to bend. */
@Composable
fun AkiAurora(modifier: Modifier = Modifier) {
    val colors = Aki.colors
    val time by produceState(0f) { while (true) androidx.compose.runtime.withFrameNanos { value = it / 1_000_000_000f } }
    Canvas(modifier.fillMaxSize().background(colors.background)) {
        fun glow(color: Color, size: Float, x: Float, y: Float) {
            val center = Offset(this.size.width / 2 + x * density, this.size.height / 2 + y * density)
            drawCircle(Brush.radialGradient(listOf(color, Color.Transparent), center, size * density), size * density, center)
        }
        glow(colors.primary.copy(alpha = 0.55f), 330f, 150 * sin(time / 7), -280 + 60 * cos(time / 5))
        glow(colors.primary.copy(alpha = 0.30f), 280f, -160 + 50 * cos(time / 6), 300 + 70 * sin(time / 8))
        glow(colors.primary.copy(alpha = 0.20f), 230f, 120 * cos(time / 9), 20 * sin(time / 4))
    }
}

enum class AkiChoiceState { Rest, Chosen, Passed }

/** One of two big choices, as a tall glass card with a drawing on top. The chosen one fills with yellow and the other steps back. */
@Composable
fun AkiChoiceCard(title: String, detail: String, state: AkiChoiceState, onClick: () -> Unit, modifier: Modifier = Modifier, art: @Composable () -> Unit) {
    val scale by animateFloatAsState(when (state) { AkiChoiceState.Chosen -> 1.03f; AkiChoiceState.Passed -> 0.95f; else -> 1f }, AkiMotion.settle(), label = "scale")
    val fade by animateFloatAsState(if (state == AkiChoiceState.Passed) 0.4f else 1f, label = "fade")
    val chosen = state == AkiChoiceState.Chosen
    Column(
        modifier.scale(scale).alpha(fade).akiGlass(AkiShapes.card, tint = if (chosen) Aki.colors.primary else null).akiPress(label = title, onClick = onClick).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Box(Modifier.fillMaxWidth().height(150.dp), contentAlignment = Alignment.Center) { art() }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            AkiText(title, AkiTextStyle.Heading, tone = if (chosen) AkiTone.OnPrimary else AkiTone.Text, lines = 2, reserves = true)
            AkiText(detail, AkiTextStyle.Caption, tone = if (chosen) AkiTone.OnPrimary else AkiTone.Muted, lines = 3, reserves = true)
        }
    }
}

/** Three comic pages fanned out, with a speech bubble on the front one. */
@Composable
fun AkiMangaArt() {
    val colors = Aki.colors
    @Composable
    fun page(fill: Color, angle: Float, x: Dp, content: @Composable () -> Unit = {}) {
        Box(
            Modifier.offset(x = x).rotate(angle).size(76.dp, 114.dp).shadow(8.dp, RoundedCornerShape(12.dp), ambientColor = colors.shadow, spotColor = colors.shadow).background(fill, RoundedCornerShape(12.dp)),
            contentAlignment = Alignment.Center,
        ) { content() }
    }
    Box(contentAlignment = Alignment.Center) {
        page(colors.secondary, -10f, (-26).dp)
        page(colors.text.copy(alpha = 0.85f), 9f, 26.dp)
        page(colors.primary, -1f, 0.dp) {
            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Box(Modifier.size(62.dp, 44.dp).background(colors.onPrimary.copy(alpha = 0.18f), RoundedCornerShape(6.dp)))
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(28.dp, 30.dp).background(colors.onPrimary.copy(alpha = 0.18f), RoundedCornerShape(6.dp)))
                    Box(Modifier.size(30.dp, 22.dp).background(colors.background, CircleShape))
                }
            }
        }
    }
}

/** Four books standing on a shelf, one of them with a ribbon. */
@Composable
fun AkiBooksArt() {
    val colors = Aki.colors
    @Composable
    fun spine(fill: Color, height: Dp, width: Dp, ribbon: Boolean = false) {
        Box(Modifier.size(width, height).shadow(6.dp, RoundedCornerShape(6.dp), ambientColor = colors.shadow, spotColor = colors.shadow).background(fill, RoundedCornerShape(6.dp))) {
            Box(Modifier.align(Alignment.TopCenter).padding(top = 14.dp).size(width - 12.dp, 3.dp).background(colors.onMedia.copy(alpha = 0.5f), CircleShape))
            if (ribbon) Box(Modifier.align(Alignment.TopCenter).offset(y = (-4).dp).size(8.dp, 34.dp).background(colors.destructive))
        }
    }
    Box(contentAlignment = Alignment.BottomCenter) {
        Row(Modifier.padding(bottom = 4.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.Bottom) {
            spine(colors.text.copy(alpha = 0.85f), 112.dp, 30.dp)
            spine(colors.primary, 132.dp, 38.dp, ribbon = true)
            spine(colors.secondary, 100.dp, 28.dp)
            spine(colors.primary.copy(alpha = 0.55f), 120.dp, 32.dp)
        }
        Box(Modifier.offset(y = 4.dp).width(150.dp).height(4.dp).background(colors.text.copy(alpha = 0.25f), CircleShape))
    }
}
