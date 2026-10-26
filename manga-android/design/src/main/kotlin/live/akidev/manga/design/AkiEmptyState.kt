package live.akidev.manga.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

class AkiAction(val label: String, val run: () -> Unit)

/** An empty or error screen: a bold line, a quiet line, and a next step that can help. */
@Composable
fun AkiEmptyState(title: String, message: String, modifier: Modifier = Modifier, action: AkiAction? = null, secondary: AkiAction? = null) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(Modifier.widthIn(max = 420.dp).padding(28.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
            AkiText(title, AkiTextStyle.Title, align = TextAlign.Center)
            AkiText(message, AkiTextStyle.Body, tone = AkiTone.Muted, align = TextAlign.Center)
            if (action != null) AkiButton(action.label, Modifier.padding(top = 8.dp), variant = AkiButtonVariant.Primary, onClick = action.run)
            if (secondary != null) AkiButton(secondary.label, variant = AkiButtonVariant.Ghost, size = AkiButtonSize.Small, onClick = secondary.run)
        }
    }
}
