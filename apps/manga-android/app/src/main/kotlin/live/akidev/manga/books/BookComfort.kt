package live.akidev.manga.books

import android.app.Activity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalView

/**
 * Brightness and warmth, laid over the page.
 *
 * The slider has one scale. The upper part is the phone's own brightness; below it, the phone stays at its lowest and a dark layer takes
 * the page further down, because the lowest the phone goes is still too bright for reading in a dark bedroom. On an OLED screen a darker
 * picture is a dimmer screen, and a layer does not flicker the way dimming by pulses does.
 */
object BookComfort {
    /** Where the slider turns from the phone's brightness to the dark layer. */
    private const val TURN = 0.3
    private const val DEEPEST = 0.8

    /** The phone's brightness (0 to 1) and the strength of the dark layer (0 to 0.8) for a slider value. */
    fun levels(value: Double): Pair<Float, Float> {
        val v = value.coerceIn(0.0, 1.0)
        return if (v >= TURN) (((v - TURN) / (1 - TURN)).toFloat().coerceAtLeast(0.01f)) to 0f else 0.01f to ((1 - v / TURN) * DEEPEST).toFloat()
    }
}

/** Sets the screen's brightness while the book is open (only this window: Android gives it back when the window goes), and draws the warm tint and the dark layer. They do not take touches. */
@Composable
fun BookComfortOverlay(style: BookStyle) {
    val view = LocalView.current
    DisposableEffect(style.brightness) {
        val window = (view.context as Activity).window
        val attributes = window.attributes
        attributes.screenBrightness = style.brightness?.let { BookComfort.levels(it).first } ?: android.view.WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
        window.attributes = attributes
        onDispose {
            attributes.screenBrightness = android.view.WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
            window.attributes = attributes
        }
    }
    Box(Modifier.fillMaxSize()) {
        if (style.warmth.isOn()) {
            Box(Modifier.fillMaxSize().graphicsLayer { alpha = style.warmthAmount.toFloat() }.drawWithContent { drawRect(Color(1f, 0.76f, 0.48f), blendMode = BlendMode.Multiply) })
        }
        val dim = style.brightness?.let { BookComfort.levels(it).second } ?: 0f
        if (dim > 0f) Box(Modifier.fillMaxSize().alpha(dim).background(Color.Black))
    }
}
