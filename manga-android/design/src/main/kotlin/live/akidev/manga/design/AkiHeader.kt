package live.akidev.manga.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/** The top of a screen: a big title, a line under it, and glass buttons on the right. */
@Composable
fun AkiHeader(title: String, modifier: Modifier = Modifier, subtitle: String? = null, trailing: @Composable RowScope.() -> Unit = {}) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            AkiText(title, AkiTextStyle.Display)
            if (subtitle != null) AkiText(subtitle, AkiTextStyle.Caption, tone = AkiTone.Muted)
        }
        trailing()
    }
}

/** The top of a pushed screen: a back button, then the header. */
@Composable
fun AkiBackHeader(title: String, back: () -> Unit, modifier: Modifier = Modifier, subtitle: String? = null, trailing: @Composable RowScope.() -> Unit = {}) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(16.dp)) {
        AkiIconButton("back", "Back", onClick = back)
        AkiHeader(title, subtitle = subtitle, trailing = trailing)
    }
}
