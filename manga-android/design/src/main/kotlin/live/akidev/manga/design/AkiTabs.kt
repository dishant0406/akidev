package live.akidev.manga.design

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.dp

class AkiChoice<ID>(val id: ID, val title: String)

/** Pills that switch what is shown: the chosen one is yellow glass. */
@Composable
fun <ID> AkiTabs(items: List<AkiChoice<ID>>, selection: ID, onSelect: (ID) -> Unit, modifier: Modifier = Modifier) {
    val haptic = LocalHapticFeedback.current
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        for (item in items) {
            val on = item.id == selection
            Box(
                Modifier.weight(1f).height(46.dp).akiGlass(AkiShapes.pill(46.dp), tint = if (on) Aki.colors.primary else null).akiPress {
                    haptic.performHapticFeedback(HapticFeedbackType.SegmentTick)
                    onSelect(item.id)
                },
                contentAlignment = Alignment.Center,
            ) {
                AkiText(item.title, if (on) AkiTextStyle.BodyStrong else AkiTextStyle.Caption, tone = if (on) AkiTone.OnPrimary else AkiTone.Muted, lines = 1)
            }
        }
    }
}

/** A row of choices that scrolls sideways: the sources. */
@Composable
fun <ID> AkiChips(items: List<AkiChoice<ID>>, selection: ID, onSelect: (ID) -> Unit, modifier: Modifier = Modifier) {
    val haptic = LocalHapticFeedback.current
    Row(modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for (item in items) {
            val on = item.id == selection
            Box(
                Modifier.height(44.dp).akiGlass(AkiShapes.pill(44.dp), tint = if (on) Aki.colors.primary else null).akiPress {
                    haptic.performHapticFeedback(HapticFeedbackType.SegmentTick)
                    onSelect(item.id)
                }.padding(horizontal = 16.dp),
                contentAlignment = Alignment.Center,
            ) {
                AkiText(item.title, if (on) AkiTextStyle.BodyStrong else AkiTextStyle.Caption, tone = if (on) AkiTone.OnPrimary else AkiTone.Text, lines = 1)
            }
        }
    }
}
