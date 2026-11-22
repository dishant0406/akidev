package live.akidev.manga.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.semantics.semantics

/** A line in a card: a yellow icon tile, a title, a line under it, something on the right. */
@Composable
fun AkiRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    icon: String? = null,
    showsChevron: Boolean = false,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    // One thing to a screen reader: "Title, line under it". A button in `trailing` stays a button of its own.
    AkiCard(modifier.semantics(mergeDescendants = true) {}) {
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
            if (icon != null) {
                Box(Modifier.size(46.dp).background(Aki.colors.primary, RoundedCornerShape(Aki.thumbRadius)), contentAlignment = Alignment.Center) {
                    AkiIcon(icon, size = 18.dp, tone = AkiTone.OnPrimary)
                }
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                AkiText(title, AkiTextStyle.Heading, lines = 2)
                if (subtitle != null) AkiText(subtitle, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 2)
            }
            Spacer(Modifier.width(8.dp))
            trailing()
            if (showsChevron) AkiIcon("forward", size = 13.dp, tone = AkiTone.Muted)
        }
    }
}
