package live.akidev.tv

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** An icon from the Solar set (`content/manga/fetch-icons.py --tv`): `name` is the outline, `name-solid` the filled one. */
@Composable
fun AkiIcon(name: String, modifier: Modifier = Modifier, size: Dp = 24.dp, tint: Color = Ink.Text) {
    val resource = akiIconSet[name] ?: error("No icon called $name")
    Image(painterResource(resource), contentDescription = null, modifier = modifier.size(size), colorFilter = ColorFilter.tint(tint))
}

/** The turning arrow that says "working". It turns in the draw step, so it does not make the screen compose again. */
@Composable
fun Spinner(modifier: Modifier = Modifier, size: Dp = 48.dp, tint: Color = Ink.TextDim) {
    val turn = rememberInfiniteTransition(label = "spinner").animateFloat(0f, 360f, infiniteRepeatable(tween(Motion.ms(1100), easing = LinearEasing), RepeatMode.Restart), label = "turn")
    AkiIcon("refresh", modifier.graphicsLayer { rotationZ = turn.value }, size, tint)
}
