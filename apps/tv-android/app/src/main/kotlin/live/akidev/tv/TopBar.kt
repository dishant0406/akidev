package live.akidev.tv

import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex

private val Pill = RoundedCornerShape(24.dp)
private val ItemWidth = 56.dp
private val ItemStep = 60.dp

/** One tab of the top bar. `kind` is the kind of a home row it shows; null is every row. */
class BarTab(val label: String, val icon: String, val kind: String?)

/** The icon of a tab: the three kinds every build has, and a neutral one for a kind a plugin brought. */
fun tabIcon(kind: String) = when (kind) {
    "movie" -> "film"
    "series" -> "tv"
    "anime" -> "sparkles"
    else -> "list"
}

/** Home first, then a tab for each one the catalogue definitions list. */
fun barTabs(fromDefinitions: List<uniffi.akitv.TabInfo>) = listOf(BarTab("Home", "home", null)) + fromDefinitions.map { BarTab(it.label, tabIcon(it.kind), it.kind) }

/**
 * The glass pill at the top, with every place of the app as an icon of one fixed size: the tabs, then Search and Settings. The shown
 * one has the filled, bright icon, and the name of the one the remote is on floats under it (so nothing moves).
 */
@Composable
fun TopBar(items: List<BarTab>, selected: Int, onPick: (Int) -> Unit, badgeOnLast: Boolean = false) {
    val requesters = remember(items.size) { List(items.size) { FocusRequester() } }
    val scope = rememberCoroutineScope()
    Box(Modifier.fillMaxWidth().padding(top = 16.dp), contentAlignment = Alignment.Center) {
        Box(Modifier.background(Ink.Canvas.copy(alpha = 0.7f), Pill).border(1.dp, Ink.Line, Pill).padding(horizontal = 8.dp, vertical = 6.dp)) {
            // The highlight of the shown tab glides to the next one: every item is 56 dp wide with 4 dp between them.
            val glide by animateDpAsState((ItemStep * selected), Motion.go(360), label = "glide")
            Box(Modifier.offset(x = glide).size(ItemWidth, 44.dp).background(Ink.Line, Pill))
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
                items.forEachIndexed { index, item ->
                    BarItem(item.icon, item.label, index == selected, requesters[index], badge = badgeOnLast && index == items.lastIndex) {
                        onPick(index)
                        // Swapping the screen below takes the focus away from the bar; a moment later it is put back on the item just chosen.
                        scope.launch { repeat(3) { withFrameNanos { } }; requesters[index].requestFocus() }
                    }
                }
            }
        }
    }
}

@Composable
private fun BarItem(icon: String, label: String, selected: Boolean, requester: FocusRequester, badge: Boolean = false, onClick: () -> Unit) {
    var focused by remember { mutableStateOf(false) }
    // The icon grows a little when it is the one shown or the one the remote is on.
    val scale by animateFloatAsState(if (selected || focused) 1.15f else 1f, Motion.go(240), label = "icon")
    Box(Modifier.zIndex(if (focused) 1f else 0f)) {
        Focusable(onClick, Pill, Color.Transparent, Modifier.size(56.dp, 44.dp).focusRequester(requester).onFocusChanged { focused = it.isFocused }) {
            // The outline icon turns into the filled one, and its colour warms up, as the tab is shown.
            val tint by animateColorAsState(if (selected || focused) Ink.Text else Ink.TextFaint, Motion.go(240), label = "tint")
            Crossfade(selected, Modifier.align(Alignment.Center).graphicsLayer { scaleX = scale; scaleY = scale }, Motion.go(240), label = "fill") { filled ->
                AkiIcon(if (filled) "$icon-solid" else icon, tint = tint)
            }
        }
        // A small amber dot: the one quiet sign that something is waiting here (amber is for state).
        if (badge) Box(Modifier.align(Alignment.TopEnd).padding(top = 7.dp, end = 11.dp).size(9.dp).background(Ink.Accent, CircleShape))
        Tip(label, focused)
    }
}
