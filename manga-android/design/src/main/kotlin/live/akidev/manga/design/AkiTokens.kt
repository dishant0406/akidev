package live.akidev.manga.design

import android.app.Activity
import android.content.res.Configuration
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/**
 * The design tokens. Screens and components use these names only, never a raw colour (see manga-app/DESIGN.md: the Android
 * app has the same tokens as the iOS app's Tokens.swift, with the same values).
 *
 * Surfaces: `background` (the screen), `surface` (cards), `secondary` (chips, fields, quiet fills).
 * Text: `text`, `textMuted`. Action: `primary` is the one accent (the chosen item, the main action) and `onPrimary` is what is
 * written on it. Over pictures: `onMedia`, with `scrim*` darkening the picture under it.
 */
class AkiColors(
    val dark: Boolean,
    val background: Color,
    val surface: Color,
    val secondary: Color,
    val text: Color,
    val textMuted: Color,
    val primary: Color = Color(0xFFFFD21F),
    val onPrimary: Color = Color(0xFF141414),
    val destructive: Color = Color(0xFFE5484D),
    val onMedia: Color = Color(0xFFFFFFFF),
) {
    /** The soft glows behind every screen, drawn from `primary`. */
    val accent = primary.copy(alpha = 0.30f)
    val accentSoft = primary.copy(alpha = 0.16f)

    val scrim = Color.Black.copy(alpha = 0.35f)
    val scrimTop = Color.Black.copy(alpha = 0.35f)
    val scrimBottom = Color.Black.copy(alpha = 0.78f)
    val scrimStrong = Color.Black.copy(alpha = 0.7f)
    val shadow = Color.Black.copy(alpha = 0.12f)

    /** The reader's two backdrops. */
    val stage = Color.Black
    val stageAlt = Color(0xFF1C1C1C)
}

private val lightColors = AkiColors(
    dark = false, background = Color(0xFFFFFFFF), surface = Color(0xFFF4F4F5), secondary = Color(0xFFE8E8EB),
    text = Color(0xFF111111), textMuted = Color(0xFF6E6E73),
)

private val darkColors = AkiColors(
    dark = true, background = Color(0xFF0D0D0E), surface = Color(0xFF1B1B1D), secondary = Color(0xFF2C2C2F),
    text = Color(0xFFF5F5F5), textMuted = Color(0xFFA1A1A8),
)

private val LocalAkiColors = staticCompositionLocalOf { lightColors }

object Aki {
    val colors: AkiColors
        @Composable get() = LocalAkiColors.current

    /** Inside a card with 10dp of padding the thumbnail's corner is the card's minus that padding, so the two curves are concentric. */
    val cardRadius = 28.dp
    val thumbRadius = 18.dp
    val sheetRadius = 32.dp
}

/**
 * Light or dark (the app decides, from the person's Appearance setting); everything under it reads the tokens through [Aki]. The
 * configuration under it says the same, so what asks the phone whether it is night (the glass does) hears the app's answer.
 */
@Composable
fun AkiTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val configuration = LocalConfiguration.current
    val matched = remember(configuration, dark) {
        Configuration(configuration).apply { uiMode = (uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or if (dark) Configuration.UI_MODE_NIGHT_YES else Configuration.UI_MODE_NIGHT_NO }
    }
    SystemBarIcons(dark)
    CompositionLocalProvider(LocalConfiguration provides matched, LocalAkiColors provides if (dark) darkColors else lightColors, content = content)
}

/** The clock and the battery in the bars are dark on a light page and light on a dark one; a theme inside another gives them back when it goes. */
@Composable
private fun SystemBarIcons(dark: Boolean) {
    val view = LocalView.current
    if (view.isInEditMode) return
    DisposableEffect(dark) {
        val controller = WindowCompat.getInsetsController((view.context as Activity).window, view)
        val before = controller.isAppearanceLightStatusBars to controller.isAppearanceLightNavigationBars
        controller.isAppearanceLightStatusBars = !dark
        controller.isAppearanceLightNavigationBars = !dark
        onDispose {
            controller.isAppearanceLightStatusBars = before.first
            controller.isAppearanceLightNavigationBars = before.second
        }
    }
}

/** While `on`, the clock and battery are white: for a page that starts with a dark picture under the status bar. */
@Composable
fun AkiLightStatusBar(on: Boolean) {
    val view = LocalView.current
    // What the bar goes back to is what the page's own look asks for, not whatever it was when this started.
    val ownLook = !Aki.colors.dark
    if (view.isInEditMode) return
    DisposableEffect(on, ownLook) {
        val controller = WindowCompat.getInsetsController((view.context as Activity).window, view)
        if (on) controller.isAppearanceLightStatusBars = false
        onDispose { if (on) controller.isAppearanceLightStatusBars = ownLook }
    }
}
