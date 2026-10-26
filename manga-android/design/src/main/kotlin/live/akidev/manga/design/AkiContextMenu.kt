package live.akidev.manga.design

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.window.Popup

/** Something to tap that also has a menu on a long press: a series card, a row. */
@Composable
fun AkiTapMenu(items: List<AkiMenuItem>, modifier: Modifier = Modifier, squeeze: Float = 0.97f, onClick: () -> Unit, content: @Composable () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(modifier.akiPress(squeeze = squeeze, onLongClick = if (items.isEmpty()) null else ({ open = true }), onClick = onClick)) {
        content()
        if (open) Popup(alignment = Alignment.Center, offset = IntOffset(0, 0), onDismissRequest = { open = false }) { AkiMenuList(items) { open = false } }
    }
}
