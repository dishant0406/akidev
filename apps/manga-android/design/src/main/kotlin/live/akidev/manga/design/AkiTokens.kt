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

/**
 * The design tokens. Screens and components use these names only, never a raw colour (see design/manga.md: the Android
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
    val primary: Color = AkiTokenValues.primary,
    val onPrimary: Color = AkiTokenValues.onPrimary,
    val destructive: Color = AkiTokenValues.destructive,
    val onMedia: Color = AkiTokenValues.onMedia,
) {
    /** The soft glows behind every screen, drawn from `primary`. */
    val accent = primary.copy(alpha = AkiTokenValues.accentOpacity)
    val accentSoft = primary.copy(alpha = AkiTokenValues.accentSoftOpacity)

    val scrim = Color.Black.copy(alpha = AkiTokenValues.scrimOpacity)
    val scrimTop = Color.Black.copy(alpha = AkiTokenValues.scrimTopOpacity)
    val scrimBottom = Color.Black.copy(alpha = AkiTokenValues.scrimBottomOpacity)
    val scrimStrong = Color.Black.copy(alpha = AkiTokenValues.scrimStrongOpacity)
    val shadow = Color.Black.copy(alpha = AkiTokenValues.shadowOpacity)

    /** The reader's two backdrops. */
    val stage = AkiTokenValues.stage
    val stageAlt = AkiTokenValues.stageAlt
}

private val lightColors = AkiColors(
    dark = false, background = AkiTokenValues.lightBackground, surface = AkiTokenValues.lightSurface,
    secondary = AkiTokenValues.lightSecondary, text = AkiTokenValues.lightText, textMuted = AkiTokenValues.lightTextMuted,
)

private val darkColors = AkiColors(
    dark = true, background = AkiTokenValues.darkBackground, surface = AkiTokenValues.darkSurface,
    secondary = AkiTokenValues.darkSecondary, text = AkiTokenValues.darkText, textMuted = AkiTokenValues.darkTextMuted,
)

private val LocalAkiColors = staticCompositionLocalOf { lightColors }

object Aki {
    val colors: AkiColors
        @Composable get() = LocalAkiColors.current

    /** Inside a card with 10dp of padding the thumbnail's corner is the card's minus that padding, so the two curves are concentric. */
    val cardRadius = AkiTokenValues.cardRadius
    val thumbRadius = AkiTokenValues.thumbRadius
    val sheetRadius = AkiTokenValues.sheetRadius
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
