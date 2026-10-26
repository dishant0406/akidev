package live.akidev.manga.design

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch

/** Pastes what is on the clipboard into whatever wants an address. */
@Composable
fun AkiPasteButton(modifier: Modifier = Modifier, onPaste: (String) -> Unit) {
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    AkiButton("Paste an address", modifier.fillMaxWidth(), icon = "download", fullWidth = true) {
        scope.launch {
            val text = clipboard.getClipEntry()?.clipData?.getItemAt(0)?.text?.toString()
            if (!text.isNullOrBlank()) onPaste(text)
        }
    }
}
