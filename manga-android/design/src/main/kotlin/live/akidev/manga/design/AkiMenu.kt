package live.akidev.manga.design

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Popup

/** One row of a menu: its icon (a tick when it is the chosen one, a blank when the icon is empty) and its title. */
class AkiMenuItem(val title: String, val icon: String, val destructive: Boolean = false, val isSelected: Boolean = false, val onClick: () -> Unit)

/** Where a menu sits: over the control that opened it (a dropdown), or beside the thing that was pressed (a long press). */
enum class AkiMenuPlace { OverAnchor, BesideAnchor }

/** A menu that is open: what it holds, what it grew from, and what closes it. */
@Stable
class AkiShownMenu(val anchor: Rect, val items: List<AkiMenuItem>, val place: AkiMenuPlace, val onClose: () -> Unit)

/** The one menu that can be open, shown by [AkiMenuLayer]. */
@Stable
class AkiMenus {
    var shown by mutableStateOf<AkiShownMenu?>(null)
}

/** Menus draw in the app's own window, so their glass can show the screen under them; a sheet's window has none and its menus use a popup. */
val LocalAkiMenus = compositionLocalOf<AkiMenus?> { null }

private val MenuShape = AkiShapes.rounded(28.dp)

/** The list of rows on a glass card, with the shadow that lifts it off the page. */
@Composable
fun AkiMenuList(items: List<AkiMenuItem>, onClose: () -> Unit, modifier: Modifier = Modifier, glass: Boolean = true) {
    val colors = Aki.colors
    Column(
        modifier.widthIn(min = 220.dp, max = 260.dp)
            .akiShadow(MenuShape, 20.dp, 8.dp)
            .let { if (glass) it.akiGlass(MenuShape, tint = colors.background.copy(alpha = if (colors.dark) 0.45f else 0.3f)) else it.akiSolid(MenuShape, colors.background) }
            .padding(vertical = 8.dp),
    ) { AkiMenuRows(items, onClose) }
}

/** The rows alone, for a card that has more than a menu on it. Choosing one runs it, then `onClose`. */
@Composable
fun AkiMenuRows(items: List<AkiMenuItem>, onClose: () -> Unit = {}) {
    // Rows line up: when any row has an icon or a tick, every row keeps room for one.
    val room = items.any { it.isSelected || it.icon.isNotEmpty() }
    Column(Modifier.verticalScroll(rememberScrollState())) {
        for (item in items) {
            val tone = if (item.destructive) AkiTone.Destructive else AkiTone.Text
            Row(
                Modifier.fillMaxWidth().akiPress(label = item.title) { onClose(); item.onClick() }.padding(horizontal = 22.dp, vertical = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(14.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (room) {
                    val symbol = if (item.isSelected) "tick" else item.icon
                    if (symbol.isEmpty()) Box(Modifier.size(18.dp)) else AkiIcon(symbol, size = 18.dp, tone = tone)
                }
                AkiText(item.title, AkiTextStyle.Body, tone = tone, lines = 1)
            }
        }
    }
}

/** Draws the open menu over the whole app. Put it last in the root of the app, inside the glass scope. */
@Composable
fun AkiMenuLayer(menus: AkiMenus) {
    val window = LocalWindowInfo.current.containerSize
    val margin = with(LocalDensity.current) { 16.dp.roundToPx() }
    // Kept after the menu is dismissed so it can fade out instead of vanishing.
    var last by remember { mutableStateOf<AkiShownMenu?>(null) }
    menus.shown?.let { last = it }
    val menu = last ?: return
    val visible = menus.shown != null
    Box(
        Modifier.fillMaxSize().then(if (visible) Modifier.pointerInput(Unit) { detectTapGestures { menus.shown?.onClose?.invoke() } } else Modifier),
    ) {
        AnimatedVisibility(
            visible,
            enter = fadeIn(tween(120)) + scaleIn(tween(180), initialScale = 0.85f, transformOrigin = TransformOrigin(0f, 0f)),
            exit = fadeOut(tween(100)) + scaleOut(tween(100), targetScale = 0.9f, transformOrigin = TransformOrigin(0f, 0f)),
        ) {
            AkiMenuList(
                menu.items, { menus.shown?.onClose?.invoke() },
                Modifier.heightIn(max = with(LocalDensity.current) { (window.height - 2 * margin).toDp() })
                    .layout { measurable, constraints ->
                        val placeable = measurable.measure(constraints)
                        val width = placeable.width
                        val anchor = menu.anchor
                        // Lined up with the control's near edge: the left edge on the left half of the screen, the right edge on the right half.
                        val left = if (anchor.center.x < window.width / 2f) anchor.left else anchor.right - width
                        val top = if (menu.place == AkiMenuPlace.OverAnchor) anchor.top else if (anchor.bottom + placeable.height + margin <= window.height) anchor.bottom + margin / 2 else anchor.top - placeable.height - margin / 2
                        val x = left.toInt().coerceIn(margin, maxOf(margin, window.width - width - margin))
                        val y = top.toInt().coerceIn(margin, maxOf(margin, window.height - placeable.height - margin))
                        layout(constraints.maxWidth, constraints.maxHeight) { placeable.place(IntOffset(x, y)) }
                    },
            )
        }
    }
}

/** Shows [items] over whatever is in [anchor], when [open]. */
@Composable
fun AkiMenuPopup(open: Boolean, onClose: () -> Unit, items: List<AkiMenuItem>, place: AkiMenuPlace = AkiMenuPlace.OverAnchor, anchor: @Composable () -> Unit) {
    val menus = LocalAkiMenus.current
    var bounds by remember { mutableStateOf(Rect.Zero) }
    Box(Modifier.onGloballyPositioned { bounds = it.boundsInRoot() }) {
        anchor()
        if (open && menus != null) {
            DisposableEffect(Unit) {
                val mine = AkiShownMenu(bounds, items, place, onClose)
                menus.shown = mine
                onDispose { if (menus.shown === mine) menus.shown = null }
            }
        } else if (open) {
            Popup(alignment = Alignment.TopStart, onDismissRequest = onClose) { AkiMenuList(items, onClose, glass = false) }
        }
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
