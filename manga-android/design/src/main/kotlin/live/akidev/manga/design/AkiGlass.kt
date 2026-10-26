package live.akidev.manga.design

import android.os.Build
import androidx.annotation.ChecksSdkIntAtLeast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.ClipOp
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.addOutline
import androidx.compose.ui.graphics.asAndroidPath
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
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

/** Inside a glass card (a dialog) controls are flat grey: glass on glass would show whatever is behind the card, scrambled. */
val LocalAkiFlat = compositionLocalOf { false }

/** Blur needs Android 12 (API 31); before that every control is solid. */
@ChecksSdkIntAtLeast(api = Build.VERSION_CODES.S)
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
 * A window of its own (a sheet, a dialog) cannot show what is on the screen behind it, so its glass shows the window's own background:
 * `content` is drawn over a source of `color`, in a glass scope of its own.
 */
@Composable
fun AkiGlassWindow(color: Color, content: @Composable () -> Unit) {
    val state = rememberHazeState()
    val look = LocalGlassSource.current?.look ?: AkiGlassLook.Solid
    val source = remember(state, look) { GlassSource(state, look) }
    // A window of its own also has no layer for menus: they open as popups in it.
    CompositionLocalProvider(LocalGlassSource provides source, LocalAkiMenus provides null, LocalAkiDialogs provides null) {
        Box {
            if (look == AkiGlassLook.Glass) Box(Modifier.matchParentSize().hazeSource(state, zIndex = -1f).background(color))
            content()
        }
    }
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

/** Glass on a control or a bar. A solid tint (the yellow) marks the current choice or the main action and shows at 90%; a see-through tint (a scrim) shows as strong as it is (half as strong on a dark page, where glass is darker already). */
@OptIn(ExperimentalHazeApi::class)
@Composable
fun Modifier.akiGlass(shape: RoundedCornerShape, tint: Color? = null): Modifier {
    val source = LocalGlassSource.current
    if (LocalAkiFlat.current) return background(tint ?: Aki.colors.secondary.copy(alpha = 0.75f), shape)
    if (source == null || source.look == AkiGlassLook.Solid) return akiSolid(shape, tint)
    val dark = Aki.colors.dark
    val base = Aki.colors.secondary
    val style = remember(shape, tint, dark) {
        GlassStyle.regular.then {
            shape(shape)
            // On the light page iOS glass has no visible edge line, only its soft shadow; the dark page keeps a little edge light.
            if (!dark) { specularIntensity(0.25f); edgeShadow(Color.Transparent) }
            // On a dark page the glass is the dark grey of a quiet fill, so what is written on it can be read.
            if (tint != null) tint(if (tint.alpha >= 1f) tint.copy(alpha = 0.9f) else if (dark) tint.copy(alpha = tint.alpha * 0.5f) else tint) else if (dark) tint(base.copy(alpha = 0.85f))
        }
    }
    return akiShadow(shape).hazeGlass(input = HazeInput.Sources(source.state), style = style)
}

/** The plain look: a tinted fill with a hairline edge. */
@Composable
fun Modifier.akiSolid(shape: RoundedCornerShape, tint: Color? = null): Modifier {
    val colors = Aki.colors
    return akiShadow(shape).background(tint ?: colors.secondary, shape).border(1.dp, colors.text.copy(alpha = 0.06f), shape)
}

/**
 * A soft shadow around a shape, drawn only outside it: glass is see-through, and a shadow that showed through would grey it.
 * The numbers are the ones iOS uses (`radius` and `dy` in dp).
 */
@Composable
fun Modifier.akiShadow(shape: RoundedCornerShape, radius: Dp = 12.dp, dy: Dp = 4.dp): Modifier {
    val color = Aki.colors.shadow.toArgb()
    return drawBehind {
        val path = Path().apply { addOutline(shape.createOutline(size, layoutDirection, this@drawBehind)) }
        clipPath(path, ClipOp.Difference) {
            drawIntoCanvas { canvas ->
                val paint = android.graphics.Paint().apply { this.color = android.graphics.Color.BLACK; setShadowLayer(radius.toPx(), 0f, dy.toPx(), color) }
                canvas.nativeCanvas.drawPath(path.asAndroidPath(), paint)
            }
        }
    }
}
