package live.akidev.manga.design

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color

/** The page behind every screen: white with a soft yellow glow, so the glass above has something to bend. */
@Composable
fun AkiBackdrop(modifier: Modifier = Modifier) {
    val colors = Aki.colors
    Box(modifier.fillMaxSize().akiGlassSource(layer = -1f).background(colors.background)) {
        // Two radial gradients rather than two blurred circles: the same soft glow, drawn once and never re-blurred.
        Canvas(Modifier.fillMaxSize()) {
            val big = size.width * 0.62f
            val small = size.width * 0.52f
            drawCircle(Brush.radialGradient(listOf(colors.accent, Color.Transparent), Offset(size.width * 0.95f, size.height * 0.12f), big), big, Offset(size.width * 0.95f, size.height * 0.12f))
            drawCircle(Brush.radialGradient(listOf(colors.accentSoft, Color.Transparent), Offset(size.width * 0.05f, size.height * 0.85f), small), small, Offset(size.width * 0.05f, size.height * 0.85f))
        }
    }
}
