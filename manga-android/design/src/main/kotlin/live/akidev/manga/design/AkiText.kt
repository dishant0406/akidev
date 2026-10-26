package live.akidev.manga.design

import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp

enum class AkiTextStyle { Display, Title, Heading, Body, BodyStrong, Caption, CaptionStrong, Eyebrow, Numeral }

enum class AkiTone {
    Text, Muted, OnPrimary, OnMedia, Destructive;

    val color: Color
        @Composable get() = when (this) {
            Text -> Aki.colors.text
            Muted -> Aki.colors.textMuted
            OnPrimary -> Aki.colors.onPrimary
            OnMedia -> Aki.colors.onMedia
            Destructive -> Aki.colors.destructive
        }
}

/** The rounded face every screen uses: one variable font file, so each weight costs nothing extra. */
private fun rounded(weight: Int) = FontFamily(Font(R.font.nunito, FontWeight(weight), variationSettings = FontVariation.Settings(FontVariation.weight(weight))))

private class Look(val size: TextUnit, val weight: Int, val tracking: TextUnit = 0.sp, val lineHeight: TextUnit = TextUnit.Unspecified)

private fun look(style: AkiTextStyle) = when (style) {
    AkiTextStyle.Display -> Look(34.sp, 700, (-0.8).sp, 38.sp)
    AkiTextStyle.Title -> Look(22.sp, 700, (-0.4).sp)
    AkiTextStyle.Heading -> Look(17.sp, 700, (-0.15).sp)
    AkiTextStyle.Body -> Look(17.sp, 400)
    AkiTextStyle.BodyStrong -> Look(17.sp, 600)
    AkiTextStyle.Caption -> Look(13.sp, 500, 0.1.sp)
    AkiTextStyle.CaptionStrong -> Look(12.sp, 600, 0.1.sp)
    AkiTextStyle.Eyebrow -> Look(11.sp, 700, 1.4.sp)
    AkiTextStyle.Numeral -> Look(72.sp, 900, (-2).sp)
}

/** The look of a style as a Compose TextStyle, for the one place text is typed (a field) rather than shown. */
fun akiTextStyle(style: AkiTextStyle, color: Color): TextStyle {
    val look = look(style)
    return TextStyle(fontFamily = rounded(look.weight), fontSize = look.size, fontWeight = FontWeight(look.weight), letterSpacing = look.tracking, lineHeight = look.lineHeight, color = color)
}

/** All text in the app. Sizes follow the phone's font size setting; the face is rounded. */
@Composable
fun AkiText(
    text: String,
    style: AkiTextStyle = AkiTextStyle.Body,
    modifier: Modifier = Modifier,
    tone: AkiTone = AkiTone.Text,
    lines: Int? = null,
    /** Takes the room of all `lines` lines even when the text is shorter, so a row of these stays one height. */
    reserves: Boolean = false,
    align: TextAlign = TextAlign.Start,
) {
    val color = if (style == AkiTextStyle.Eyebrow) tone.color.copy(alpha = 0.7f) else tone.color
    BasicText(
        text = if (style == AkiTextStyle.Eyebrow) text.uppercase() else text,
        modifier = modifier,
        style = akiTextStyle(style, color).copy(textAlign = align),
        maxLines = lines ?: Int.MAX_VALUE,
        minLines = if (reserves && lines != null) lines else 1,
        overflow = TextOverflow.Ellipsis,
    )
}
