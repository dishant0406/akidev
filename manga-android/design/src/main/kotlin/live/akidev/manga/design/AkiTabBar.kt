package live.akidev.manga.design

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.dp

/** One place the bar can go: its icon (and the filled one for when it is chosen), what it is called, and an unread count. */
class AkiTabItem<ID>(val id: ID, val icon: String, val iconSolid: String, val label: String, val badge: Int = 0)

private val CellWidth = 62.dp
private val CellHeight = 58.dp
private val Gap = 6.dp
private val Padding = 6.dp

/**
 * The floating tab bar: a glass capsule, with the current tab as a yellow glass tile that slides to the next. The bar has the
 * corner radius of a card; the tile sits inside it with the padding taken off, so the curves are concentric.
 */
@Composable
fun <ID> AkiTabBar(tabs: List<AkiTabItem<ID>>, selection: ID, onSelect: (ID) -> Unit, modifier: Modifier = Modifier) {
    val haptic = LocalHapticFeedback.current
    val selected = tabs.indexOfFirst { it.id == selection }.coerceAtLeast(0)
    val slide by animateDpAsState((CellWidth + Gap) * selected, AkiMotion.settle(), label = "tab")
    val inner = AkiShapes.rounded(Aki.cardRadius - Padding)
    Box(modifier.shadow(20.dp, AkiShapes.card, ambientColor = Aki.colors.shadow, spotColor = Aki.colors.shadow).akiGlass(AkiShapes.card).padding(Padding)) {
        Box(Modifier.offset(x = slide).size(CellWidth, CellHeight).akiGlass(inner, tint = Aki.colors.primary))
        Row(horizontalArrangement = Arrangement.spacedBy(Gap)) {
            for (tab in tabs) {
                val on = tab.id == selection
                Box(
                    Modifier.size(CellWidth, CellHeight).akiPress(label = tab.label) {
                        haptic.performHapticFeedback(HapticFeedbackType.SegmentTick)
                        onSelect(tab.id)
                    },
                    contentAlignment = Alignment.Center,
                ) {
                    AkiIcon(if (on) tab.iconSolid else tab.icon, size = 24.dp, tone = if (on) AkiTone.OnPrimary else AkiTone.Muted)
                    if (tab.badge > 0) AkiCount(tab.badge, Modifier.align(Alignment.TopEnd).offset(x = (-2).dp, y = 2.dp), tinted = false)
                }
            }
        }
    }
}
