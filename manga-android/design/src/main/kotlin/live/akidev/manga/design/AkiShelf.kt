package live.akidev.manga.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** A titled row of cards that scrolls sideways: one "shelf" of the home feed. */
@Composable
fun AkiShelf(title: String, modifier: Modifier = Modifier, subtitle: String? = null, content: LazyListScope.() -> Unit) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            AkiText(title, AkiTextStyle.Heading, lines = 1)
            if (subtitle != null) AkiText(subtitle, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
        }
        LazyRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp), content = content)
    }
}

/** A cover standing up in a shelf, its title below. With a `rank` it is a "Top 10" card: a big numeral beside it. */
@Composable
fun AkiPoster(title: String, cover: String?, modifier: Modifier = Modifier, caption: String? = null, rank: Int? = null, width: Dp = 128.dp) {
    // The numeral sits on the bottom edge of the cover, whatever the title below it takes: a cover is a fixed shape, a title is not.
    Row(modifier) {
        if (rank != null) {
            Box(Modifier.width(if (rank >= 10) 70.dp else 46.dp).height(width * 1.5f).padding(end = 4.dp), contentAlignment = Alignment.BottomEnd) {
                AkiText("$rank", AkiTextStyle.Numeral, tone = AkiTone.Muted)
            }
        }
        Column(Modifier.width(width), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            AkiImage(cover, Modifier.fillMaxWidth().clip(RoundedCornerShape(Aki.thumbRadius)), placeholder = 1.5f, fill = true)
            AkiText(title, AkiTextStyle.CaptionStrong, lines = 2, reserves = true)
            if (caption != null) AkiText(caption, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
        }
    }
}

/** The one title the feed leads with: a tall picture, a glass tag, the title and what tapping does. */
@Composable
fun AkiBillboard(title: String, cover: String?, tag: String, modifier: Modifier = Modifier, action: String = "Read now") {
    Box(modifier.fillMaxWidth().height(440.dp).clip(RoundedCornerShape(Aki.cardRadius))) {
        AkiImage(cover, Modifier.fillMaxWidth().height(440.dp), placeholder = 1.3f, free = true)
        Box(Modifier.fillMaxWidth().height(440.dp).background(Brush.verticalGradient(0.5f to Color.Transparent, 1f to Aki.colors.scrimBottom)))
        Column(Modifier.align(Alignment.BottomStart).padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            AkiBadge(tag, icon = "sparkles", style = AkiBadgeStyle.Glass)
            AkiText(title, AkiTextStyle.Display, tone = AkiTone.OnMedia, lines = 2)
            AkiBadge(action, icon = "play-solid", style = AkiBadgeStyle.Primary)
        }
    }
}
