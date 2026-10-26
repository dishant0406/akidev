package live.akidev.manga.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

class AkiOption<V>(val value: V, val label: String)

/**
 * One small pill that shows the current choice and opens a menu of the rest: for lists too long to lay out as a row of pills (every
 * source, every genre). `icon` leads the label; `placeholder` is what the pill says while the first option is chosen ("Genre" rather
 * than "All").
 */
@Composable
fun <V> AkiMenuPill(selection: V, onSelect: (V) -> Unit, options: List<AkiOption<V>>, modifier: Modifier = Modifier, icon: String? = null, placeholder: String? = null) {
    var open by remember { mutableStateOf(false) }
    val current = if (placeholder != null && selection == options.firstOrNull()?.value) placeholder else options.firstOrNull { it.value == selection }?.label ?: ""
    val items = options.map { option -> AkiMenuItem(option.label, "tick", isSelected = option.value == selection) { onSelect(option.value) } }
    AkiMenuPopup(open, { open = false }, items) {
        Row(
            modifier.height(44.dp).akiGlass(AkiShapes.pill(44.dp)).akiPress(label = current) { open = true }.padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (icon != null) AkiIcon(icon, size = 14.dp, tone = AkiTone.Muted)
            AkiText(current, AkiTextStyle.BodyStrong, lines = 1)
            AkiIcon("down", size = 11.dp, tone = AkiTone.Muted)
        }
    }
}

/** A choice among a few options, shown as a menu. */
@Composable
fun <V> AkiSelect(title: String, selection: V, onSelect: (V) -> Unit, options: List<AkiOption<V>>, modifier: Modifier = Modifier) {
    var open by remember { mutableStateOf(false) }
    val items = options.map { option -> AkiMenuItem(option.label, "tick", isSelected = option.value == selection) { onSelect(option.value) } }
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        AkiText(title, AkiTextStyle.BodyStrong, Modifier.weight(1f))
        AkiMenuPopup(open, { open = false }, items) {
            // A chip, so it reads as something to tap.
            AkiText(
                options.firstOrNull { it.value == selection }?.label ?: "", AkiTextStyle.BodyStrong, lines = 1,
                modifier = Modifier.akiPress(label = title) { open = true }.background(Aki.colors.secondary, AkiShapes.pill(36.dp)).padding(horizontal = 14.dp, vertical = 8.dp),
            )
        }
    }
}

