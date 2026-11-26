package live.akidev.manga.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/** The row of controls along the top of a reader, under the status bar. */
@Composable
fun AkiReaderTopBar(content: @Composable RowScope.() -> Unit) {
    Row(
        Modifier.fillMaxWidth().widthIn(max = 640.dp).statusBarsPadding().padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically,
        content = content,
    )
}

/**
 * The glass pill in a top bar that says what is being read (a title over a smaller line) and opens its contents when tapped. It takes the
 * room the buttons leave and is as wide as its words. Over a picture, give the `scrim` that darkens its glass.
 */
@Composable
fun RowScope.AkiBarTitle(title: String, subtitle: String?, label: String, onClick: () -> Unit, scrim: Color? = null) {
    val tone = if (scrim != null) AkiTone.OnMedia else AkiTone.Text
    val quiet = if (scrim != null) AkiTone.OnMedia else AkiTone.Muted
    Box(Modifier.weight(1f)) {
        Box(Modifier.akiGlass(AkiShapes.pill(48.dp), tint = scrim).akiPress(label = label, onClick = onClick).padding(horizontal = 16.dp, vertical = 8.dp)) {
            Column {
                AkiText(title, AkiTextStyle.CaptionStrong, tone = tone, lines = 1)
                if (subtitle != null) AkiText(subtitle, AkiTextStyle.Caption, tone = quiet, lines = 1)
            }
        }
    }
}
