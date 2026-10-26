package live.akidev.manga.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

enum class AkiBadgeStyle {
    /** A flat grey chip on a card. */
    Flat,

    /** Glass, for a picture. */
    Glass,

    /** The accent, for the one thing to do. */
    Primary,
}

/** A small label: a date, a scanlator, a status. */
@Composable
fun AkiBadge(text: String, modifier: Modifier = Modifier, icon: String? = null, style: AkiBadgeStyle = AkiBadgeStyle.Flat) {
    val tone = when (style) {
        AkiBadgeStyle.Glass -> AkiTone.OnMedia
        AkiBadgeStyle.Primary -> AkiTone.OnPrimary
        AkiBadgeStyle.Flat -> AkiTone.Muted
    }
    val colors = Aki.colors
    val face = when (style) {
        AkiBadgeStyle.Flat -> Modifier.background(colors.secondary, CircleShape)
        AkiBadgeStyle.Glass -> Modifier.akiGlass(AkiShapes.pill(24.dp), tint = colors.scrim)
        AkiBadgeStyle.Primary -> Modifier.background(colors.primary, CircleShape)
    }
    Row(
        modifier.then(face).padding(horizontal = 10.dp, vertical = 5.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) AkiIcon(icon, size = 10.dp, tone = tone)
        AkiText(text, AkiTextStyle.CaptionStrong, tone = tone, lines = 1)
    }
}

/** A count over a cover or a tab: unread chapters. */
@Composable
fun AkiCount(value: Int, modifier: Modifier = Modifier, tinted: Boolean = true) {
    val colors = Aki.colors
    Row(
        modifier.defaultMinSize(minWidth = 26.dp, minHeight = 26.dp).background(if (tinted) colors.primary else colors.destructive, CircleShape).padding(horizontal = 9.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        AkiText(minOf(value, 99).toString(), AkiTextStyle.CaptionStrong, tone = if (tinted) AkiTone.OnPrimary else AkiTone.OnMedia)
    }
}
