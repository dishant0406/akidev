package live.akidev.manga.design

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier

/** Something to tap that also has a menu on a long press: a series card, a row. */
@Composable
fun AkiTapMenu(items: List<AkiMenuItem>, modifier: Modifier = Modifier, squeeze: Float = 0.97f, onClick: () -> Unit, content: @Composable () -> Unit) {
    var open by remember { mutableStateOf(false) }
    AkiMenuPopup(open, { open = false }, items, AkiMenuPlace.BesideAnchor) {
        Box(modifier.akiPress(squeeze = squeeze, onLongClick = if (items.isEmpty()) null else ({ open = true }), onClick = onClick)) { content() }
    }
}
