package live.akidev.manga.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/** A heading and what belongs under it. */
@Composable
fun AkiSection(title: String, modifier: Modifier = Modifier, footer: String? = null, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        AkiText(title, AkiTextStyle.Heading)
        content()
        if (footer != null) AkiText(footer, AkiTextStyle.Caption, tone = AkiTone.Muted)
    }
}
