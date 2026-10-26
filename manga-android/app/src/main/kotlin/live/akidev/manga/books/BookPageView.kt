package live.akidev.manga.books

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Popup
import kotlin.math.roundToInt
import live.akidev.manga.design.*

/** What a menu over the page is about: a selection, or a highlight that was tapped, and where the finger was. */
private class MenuTarget(val span: BookSpan, val mark: Long?, val point: Offset)

/**
 * One page of a book: a chapter and a page in it (a page of -1 is the chapter's last). While the chapter is being set in type it shows
 * only the paper. A tap turns the page, follows a link or shows the bars; a long press selects a word, the handles move the ends, and a
 * menu offers what can be done with it. All of that is worked out in [ChapterLayout]: there is no text view.
 */
@Composable
fun BookPageView(model: BookModel, chapter: Int, page: Int, modifier: Modifier = Modifier) {
    val theme = model.theme
    val layout = model.layouts[chapter]
    var menu by remember { mutableStateOf<MenuTarget?>(null) }
    Box(modifier.fillMaxSize().background(theme.paper)) {
        if (layout == null) return@Box
        val shown = if (page < 0) layout.count - 1 else page.coerceAtMost(layout.count - 1)
        val insets = model.pageInsets
        val top = insets.top + if (model.style.scroll) layout.leading(shown) else 0f
        val overlays = model.overlays
        val footer = remember(layout, shown) { model.footer(BookPlace(chapter, shown), layout) }
        val density = LocalDensity.current
        val foot = remember(density) { Paint(Paint.ANTI_ALIAS_FLAG).apply { textSize = 12 * density.density; typeface = Typeface.create(Typeface.SANS_SERIF, 500, false) } }

        Canvas(
            Modifier.fillMaxSize()
                .pointerInput(layout, shown) {
                    detectTapGestures(
                        onTap = { tap(model, layout, shown, it, size.width, insets, top) { target -> menu = target } },
                        onLongPress = { point ->
                            val at = layout.position(point.x - insets.left, point.y - top, shown) ?: return@detectTapGestures
                            val word = layout.word(at) ?: return@detectTapGestures
                            val span = BookSpan(chapter, word.first, word.last + 1)
                            model.selection = span
                            menu = MenuTarget(span, null, point)
                        },
                    )
                },
        ) {
            // The pictures decoded since this page was drawn make it draw again.
            model.pictureTick
            drawIntoCanvas { canvas ->
                val native = canvas.nativeCanvas
                val cover = layout.coverPicture
                if (cover != null) {
                    model.picture(cover, maxOf(size.width, size.height))?.let { picture ->
                        val scale = minOf(size.width / picture.width, size.height / picture.height)
                        val w = picture.width * scale
                        val h = picture.height * scale
                        native.drawBitmap(picture, null, android.graphics.RectF((size.width - w) / 2, (size.height - h) / 2, (size.width + w) / 2, (size.height + h) / 2), null)
                    }
                    return@drawIntoCanvas
                }
                native.save()
                native.translate(insets.left, top)
                layout.draw(native, shown, theme.ink.toArgb(), theme.quiet.toArgb(), overlays) { path, width -> model.picture(path, width) }
                native.restore()
                if (!model.style.scroll) {
                    foot.color = theme.quiet.toArgb()
                    val y = size.height - insets.bottom + 6 * density.density + foot.textSize
                    val number = footer.second
                    foot.textAlign = Paint.Align.RIGHT
                    native.drawText(number, size.width - insets.right, y, foot)
                    foot.textAlign = Paint.Align.LEFT
                    val room = size.width - insets.left - insets.right - foot.measureText(number) - 16 * density.density
                    native.drawText(android.text.TextUtils.ellipsize(footer.first, android.text.TextPaint(foot), maxOf(room, 0f), android.text.TextUtils.TruncateAt.END).toString(), insets.left, y, foot)
                }
            }
        }

        // The handles at the two ends of the selection, when it is on this page.
        val selection = model.selection
        if (selection != null && selection.chapter == chapter) {
            val boxes = layout.boxes(selection.start, selection.end, shown)
            val first = boxes.firstOrNull()
            val last = boxes.lastOrNull()
            if (first != null && last != null) {
                Handle(true, Offset(insets.left + first.left, top + first.top), layout, shown, model, insets, top) { menu = MenuTarget(it, null, Offset(insets.left + first.left, top + first.top)) }
                Handle(false, Offset(insets.left + last.right, top + last.bottom), layout, shown, model, insets, top) { menu = MenuTarget(it, null, Offset(insets.left + last.right, top + last.bottom)) }
            }
        }
        menu?.let { target ->
            ActionMenu(model, layout, target) {
                menu = null
                // Closing the menu on a selection drops it, as tapping elsewhere does.
                if (model.selection != null) model.selection = null
            }
        }
    }
}

/** A tap: on a highlight it opens that highlight's menu; elsewhere it is a link, a word, or the left, middle or right of the page. */
private fun tap(model: BookModel, layout: ChapterLayout, page: Int, point: Offset, width: Int, insets: BookModel.Insets, top: Float, open: (MenuTarget) -> Unit) {
    if (model.selection != null) {
        model.selection = null
        return
    }
    val x = point.x - insets.left
    val y = point.y - top
    val mark = model.overlays.marks.lastOrNull { mark ->
        mark.span.chapter == layout.index && layout.boxes(mark.span.start, mark.span.end, page).any { it.left - 4 <= x && x <= it.right + 4 && it.top - 4 <= y && y <= it.bottom + 4 }
    }
    if (mark != null) return open(MenuTarget(mark.span, mark.id, point))
    val link = layout.link(x, y, page)
    val zone = if (point.x < width * 0.3f) BookTapZone.Back else if (point.x > width * 0.7f) BookTapZone.Forward else BookTapZone.Middle
    // The word the finger is on: the point must be on its letters, not between the lines.
    val word = layout.position(x, y, page)?.let { layout.word(it) }?.let { word ->
        val onIt = layout.boxes(word.first, word.last + 1, page).any { it.left - 4 <= x && x <= it.right + 4 && it.top - 4 <= y && y <= it.bottom + 4 }
        if (onIt) BookSpan(layout.index, word.first, word.last + 1) else null
    }
    model.tapped(zone, link?.link, link?.raised ?: false, word)
}

/**
 * A dot on a stem at an end of the selection, dragged to change where the selection stops. The one at the start hangs above the first line,
 * the one at the end below the last. `anchor` is the corner of the selection it belongs to, in the page's own coordinates.
 */
@Composable
private fun Handle(atStart: Boolean, anchor: Offset, layout: ChapterLayout, page: Int, model: BookModel, insets: BookModel.Insets, top: Float, released: (BookSpan) -> Unit) {
    val density = LocalDensity.current.density
    val origin = Offset(anchor.x - 22 * density, if (atStart) anchor.y - 28 * density else anchor.y - 12 * density)
    Box(
        Modifier.offset { IntOffset(origin.x.roundToInt(), origin.y.roundToInt()) }
            .size(44.dp)
            .pointerInput(layout, page, origin) {
                detectDragGestures(onDragEnd = { model.selection?.let(released) }) { change, _ ->
                    val selection = model.selection ?: return@detectDragGestures
                    // The dot is above or below the line it belongs to: the point read is the line's own.
                    val point = origin + change.position + Offset(0f, (if (atStart) 14 else -14) * density)
                    val position = layout.position(point.x - insets.left, point.y - top, page, end = !atStart) ?: return@detectDragGestures
                    model.selection = if (atStart) selection.copy(start = minOf(position, selection.end - 1)) else selection.copy(end = maxOf(position, selection.start + 1))
                }
            },
    ) {
        Canvas(Modifier.fillMaxSize()) {
            val colour = HighlightColour.selection
            val centreX = size.width / 2
            if (atStart) {
                drawCircle(colour, 6.dp.toPx(), Offset(centreX, 12.dp.toPx()))
                drawRect(colour, Offset(centreX - 1.dp.toPx(), 14.dp.toPx()), androidx.compose.ui.geometry.Size(2.dp.toPx(), size.height - 14.dp.toPx()))
            } else {
                drawRect(colour, Offset(centreX - 1.dp.toPx(), 0f), androidx.compose.ui.geometry.Size(2.dp.toPx(), size.height - 14.dp.toPx()))
                drawCircle(colour, 6.dp.toPx(), Offset(centreX, size.height - 12.dp.toPx()))
            }
        }
    }
}

/** What can be done with a selection or a highlight: a row of colours, and the actions. */
@Composable
private fun ActionMenu(model: BookModel, layout: ChapterLayout, target: MenuTarget, close: () -> Unit) {
    val words = remember(target) { layout.words(target.span.start, target.span.end) }
    fun act(action: BookAction) {
        close()
        model.perform(action, target.span, target.mark, words)
    }
    val shape = RoundedCornerShape(22.dp)
    Popup(offset = IntOffset(target.point.x.toInt(), target.point.y.toInt()), onDismissRequest = close) {
        Column(Modifier.widthIn(min = 220.dp).shadow(18.dp, shape).background(Aki.colors.background, shape).padding(vertical = 8.dp)) {
            Row(Modifier.padding(horizontal = 18.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                for (index in HighlightColour.names.indices) {
                    Box(Modifier.size(30.dp).background(HighlightColour.colour(index), CircleShape).akiPress(label = HighlightColour.names[index]) { act(BookAction.Highlight(index)) })
                }
            }
            val items = buildList {
                if (model.canRead) add(AkiMenuItem("Read from here", "play") { act(BookAction.Read) })
                add(AkiMenuItem("Note", "bookmark") { act(BookAction.Note) })
                add(AkiMenuItem("Copy", "share") { act(BookAction.Copy) })
                if (target.mark != null) add(AkiMenuItem("Remove highlight", "trash", destructive = true) { act(BookAction.Remove) })
            }
            AkiMenuRows(items)
        }
    }
}
