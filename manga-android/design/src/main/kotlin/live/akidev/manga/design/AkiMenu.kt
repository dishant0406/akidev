package live.akidev.manga.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Popup

/** One row of a menu: its icon (a tick when it is the chosen one) and its title. */
class AkiMenuItem(val title: String, val icon: String, val destructive: Boolean = false, val isSelected: Boolean = false, val onClick: () -> Unit)

/** The list of rows, drawn on its own so a menu and a long press can both show it. */
@Composable
fun AkiMenuList(items: List<AkiMenuItem>, onClose: () -> Unit) {
    val shape = RoundedCornerShape(20.dp)
    Column(
        Modifier.widthIn(min = 200.dp, max = 300.dp).shadow(18.dp, shape, ambientColor = Aki.colors.shadow, spotColor = Aki.colors.shadow).background(Aki.colors.background, shape).padding(vertical = 6.dp),
    ) {
        for (item in items) {
            val tone = if (item.destructive) AkiTone.Destructive else AkiTone.Text
            Row(
                Modifier.fillMaxWidth().akiPress(label = item.title) { onClose(); item.onClick() }.padding(horizontal = 18.dp, vertical = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                AkiIcon(if (item.isSelected) "tick" else item.icon, size = 18.dp, tone = tone)
                AkiText(item.title, AkiTextStyle.Body, tone = tone, lines = 1)
            }
        }
    }
}

/** Shows [items] under whatever is in [anchor], when [open]. */
@Composable
fun AkiMenuPopup(open: Boolean, onClose: () -> Unit, items: List<AkiMenuItem>, anchor: @Composable () -> Unit) {
    var height by remember { mutableStateOf(0) }
    Box(Modifier.onSizeChanged { height = it.height }) {
        anchor()
        if (open) Popup(alignment = Alignment.TopEnd, offset = IntOffset(0, height), onDismissRequest = onClose) { AkiMenuList(items, onClose) }
    }
}

/** A glass round button that opens a menu. */
@Composable
fun AkiMenu(symbol: String, label: String, items: List<AkiMenuItem>, modifier: Modifier = Modifier, onMedia: Boolean = false) {
    var open by remember { mutableStateOf(false) }
    AkiMenuPopup(open, { open = false }, items) {
        AkiIconButton(symbol, label, modifier, onMedia = onMedia) { open = true }
    }
}
