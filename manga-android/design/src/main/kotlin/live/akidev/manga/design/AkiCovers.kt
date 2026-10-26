package live.akidev.manga.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp

/** A cover with its title: the grid card on Home and in the Library. */
@Composable
fun AkiCoverCard(title: String, cover: String?, modifier: Modifier = Modifier, subtitle: String? = null, badge: Int? = null) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Box {
            AkiImage(cover, Modifier.fillMaxWidth().clip(RoundedCornerShape(Aki.cardRadius)), placeholder = 1.4f, fill = true)
            if (badge != null && badge > 0) AkiCount(badge, Modifier.align(Alignment.TopEnd).padding(8.dp))
        }
        AkiText(title, AkiTextStyle.Heading, lines = 2)
        if (subtitle != null) AkiText(subtitle, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
    }
}

/** A row in a list of cards: a thumbnail, a small line, a bold title, badges. Updates, History and chapters. */
@Composable
fun AkiListCard(
    thumbnail: String?,
    label: String,
    title: String,
    modifier: Modifier = Modifier,
    badges: List<String> = emptyList(),
    dimmed: Boolean = false,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    AkiCard(modifier, padding = 10.dp, dimmed = dimmed) {
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
            AkiImage(thumbnail, Modifier.width(66.dp).clip(RoundedCornerShape(Aki.thumbRadius)), placeholder = 1.25f, fill = true)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                AkiText(label, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
                AkiText(title, AkiTextStyle.Heading, lines = 2)
                if (badges.isNotEmpty()) Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) { for (badge in badges) AkiBadge(badge) }
            }
            trailing()
        }
    }
}

/** A book on a shelf: its cover, or the title set on a plain card when it has none, and how far through it you are. */
@Composable
fun AkiBookCard(title: String, author: String?, cover: String?, subtitle: String, modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (cover != null) {
            AkiImage(cover, Modifier.fillMaxWidth().clip(RoundedCornerShape(Aki.cardRadius)), placeholder = 1.5f, fill = true)
        } else {
            Column(Modifier.fillMaxWidth().aspectRatio(1f / 1.5f).clip(RoundedCornerShape(Aki.cardRadius)).background(Aki.colors.surface).padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                AkiText(title, AkiTextStyle.Heading, lines = 4)
                if (author != null) AkiText(author, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 2)
            }
        }
        AkiText(title, AkiTextStyle.Heading, lines = 2)
        AkiText(subtitle, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
    }
}
