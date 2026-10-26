package live.akidev.manga.design

import androidx.compose.foundation.isSystemInDarkTheme
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
    background = Color(0xFFFFFFFF), surface = Color(0xFFF4F4F5), secondary = Color(0xFFE8E8EB),
    text = Color(0xFF111111), textMuted = Color(0xFF6E6E73),
)

private val darkColors = AkiColors(
    background = Color(0xFF0D0D0E), surface = Color(0xFF1B1B1D), secondary = Color(0xFF2C2C2F),
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

/** Light or dark (the app decides, from the person's Appearance setting); everything under it reads the tokens through [Aki]. */
@Composable
fun AkiTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalAkiColors provides if (dark) darkColors else lightColors, content = content)
}
