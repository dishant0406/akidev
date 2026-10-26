package live.akidev.manga.design

import android.os.Build
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.chrisbanes.haze.ExperimentalHazeApi
import dev.chrisbanes.haze.HazeInput
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.hazeSource
import dev.chrisbanes.haze.glass.GlassStyle
import dev.chrisbanes.haze.glass.hazeGlass
import dev.chrisbanes.haze.rememberHazeState

/** The shapes glass is cut in. Corners are fixed sizes (glass cannot be cut with percentages), so a capsule is told its height. */
object AkiShapes {
    /** A capsule as tall as `height`; a circle is a capsule of equal sides. */
    fun pill(height: Dp) = RoundedCornerShape(height / 2)
    val card = RoundedCornerShape(28.dp)
    fun rounded(radius: Dp) = RoundedCornerShape(radius)
}

/**
 * Glass has two looks. `Glass` draws what is behind the control, bent and blurred, like Liquid Glass on iOS. `Solid` is a plain
 * tinted fill: phones that cannot blur (Android 10), and anyone who turns glass off in Settings.
 */
enum class AkiGlassLook { Glass, Solid }

private class GlassSource(val state: HazeState, val look: AkiGlassLook)

private val LocalGlassSource = compositionLocalOf<GlassSource?> { null }

/** Blur needs Android 12 (API 31); before that every control is solid. */
fun akiGlassSupported() = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S

/** The root of every screen that has glass: whatever is marked with [akiGlassSource] inside it is what the glass shows. */
@Composable
fun AkiGlassScope(reduce: Boolean = false, content: @Composable () -> Unit) {
    val state = rememberHazeState()
    val look = if (reduce || !akiGlassSupported()) AkiGlassLook.Solid else AkiGlassLook.Glass
    val source = remember(state, look) { GlassSource(state, look) }
    CompositionLocalProvider(LocalGlassSource provides source, content = content)
}

/**
 * Marks content that glass drawn over it shows. Glass inside a source shows only sources with a lower `layer`, so the backdrop
 * (layer -1) is what a button inside a scrolling list shows, while a bar beside the list shows the list (layer 0) as well.
 */
@Composable
fun Modifier.akiGlassSource(layer: Float = 0f): Modifier {
    val source = LocalGlassSource.current ?: return this
    return if (source.look == AkiGlassLook.Glass) hazeSource(source.state, zIndex = layer) else this
}

/** Glass on a control or a bar. A tint (the yellow) marks the current choice or the main action. */
@OptIn(ExperimentalHazeApi::class)
@Composable
fun Modifier.akiGlass(shape: RoundedCornerShape, tint: Color? = null): Modifier {
    val source = LocalGlassSource.current
    if (source == null || source.look == AkiGlassLook.Solid) return akiSolid(shape, tint)
    val style = remember(shape, tint) {
        GlassStyle.regular.then {
            shape(shape)
            if (tint != null) tint(tint.copy(alpha = 0.9f))
        }
    }
    return hazeGlass(input = HazeInput.Sources(source.state), style = style)
}

/** The plain look: a tinted fill with a hairline edge. */
@Composable
fun Modifier.akiSolid(shape: RoundedCornerShape, tint: Color? = null): Modifier {
    val colors = Aki.colors
    return background(tint ?: colors.secondary, shape).border(1.dp, colors.text.copy(alpha = 0.06f), shape)
}
