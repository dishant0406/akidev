package live.akidev.manga.reader

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.withFrameNanos
import kotlinx.coroutines.delay
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.design.*

/** A row of the strip: a page, or the closing row after a chapter. */
private sealed interface Row {
    val id: String

    class Page(val chapter: String, val index: Int, val url: String, val headers: Map<String, String>) : Row {
        override val id = "$chapter|$index"
    }

    class End(val segment: Segment) : Row {
        override val id = "${segment.chapterId}|end"
    }
}

private fun rowsOf(segments: List<Segment>): List<Row> = segments.flatMap { segment ->
    segment.pages.images.mapIndexed { index, url -> Row.Page(segment.chapterId, index, url, segment.headers) } + Row.End(segment)
}

/**
 * Pages one under another with no gap, the next chapter joined on as the end nears. Pinch to zoom the width (it snaps back to the screen
 * width below 1×); when zoomed, drag sideways too.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun StripReader(reader: ReaderModel) {
    val app = LocalApp.current
    val rows = remember(reader.segments) { rowsOf(reader.segments) }
    val latestRows by rememberUpdatedState(rows)
    val state = rememberLazyListState()
    val dragged by state.interactionSource.collectIsDraggedAsState()
    var live by remember { mutableFloatStateOf(1f) }
    val density = LocalDensity.current

    BoxWithConstraints(Modifier.fillMaxSize()) {
        val width = maxWidth
        val widthPx = with(density) { width.toPx() }
        // The strip is laid out at once but not shown until it has been moved to where the person was: showing it first made the chapter
        // open at the top and then jump.
        val shown = if (reader.restoring) 0f else 1f
        androidx.compose.foundation.layout.Box(
            Modifier.fillMaxSize()
                .graphicsLayer { alpha = shown; scaleX = live; scaleY = live }
                .pointerInput(Unit) { detectTapGestures { reader.chrome = !reader.chrome } }
                .pointerInput(Unit) { pinchToZoom(reader) { live = it } }
                .horizontalScroll(rememberScrollState(), enabled = reader.zoom > 1f),
        ) {
            LazyColumn(Modifier.width(width * reader.zoom).fillMaxSize(), state) {
                items(rows.size, key = { rows[it].id }) { index ->
                    when (val row = rows[index]) {
                        is Row.Page -> AkiImage(
                            row.url, Modifier.fillMaxWidth(), row.headers, placeholder = reader.aspects[row.url]?.let { 1 / it } ?: reader.typicalHeightRatio,
                            onAspect = { reader.noteAspect(it, row.url) }, pageNumber = row.index,
                        )
                        is Row.End -> ChapterEnd(reader, row.segment)
                    }
                }
            }
        }

        // Where the person is: the row at the top of the strip and how far down it is.
        LaunchedEffect(state) {
            snapshotFlow { state.firstVisibleItemIndex to state.firstVisibleItemScrollOffset }.collect { (index, offset) ->
                val row = latestRows.getOrNull(index) as? Row.Page ?: return@collect
                val height = state.layoutInfo.visibleItemsInfo.firstOrNull()?.size ?: return@collect
                reader.report(row.chapter, row.index, (offset.toDouble() / maxOf(height, 1)).coerceIn(0.0, 1.0))
            }
        }
        // A chapter is finished when its closing row comes up the screen.
        LaunchedEffect(state) {
            snapshotFlow { state.layoutInfo.visibleItemsInfo.map { it.index to it.offset } }.collect { visible ->
                for ((index, offset) in visible) {
                    val row = latestRows.getOrNull(index) as? Row.End ?: continue
                    if (offset < state.layoutInfo.viewportSize.height * 0.9f) reader.finish(row.segment.chapterId)
                }
            }
        }
        // Puts the strip back where it was: the page, and how far down it, once that page's shape is known.
        LaunchedEffect(reader.restoreToken) {
            val target = reader.restore
            val segment = target?.let { t -> reader.segments.firstOrNull { it.chapterId == t.chapter } }
            if (target == null || segment == null) {
                reader.restoring = false
                return@LaunchedEffect
            }
            reader.restoring = true
            delay(120)
            val index = latestRows.indexOfFirst { it is Row.Page && it.chapter == target.chapter && it.index == target.page }.coerceAtLeast(0)
            state.scrollToItem(index)
            reader.restore = null
            if (target.offset > 0.001) {
                val url = segment.pages.images.getOrNull(target.page)
                // How far down the page is only known once its picture is: waited for, but not for ever.
                if (url != null) repeat(15) { if (reader.aspects[url] == null) delay(100) }
                delay(150)
                reader.aspects[url]?.let { aspect -> state.scrollToItem(index, (target.offset * widthPx * reader.zoom / aspect).toInt()) }
                delay(150)
            }
            // Shown now, and the page counter told where it is (nothing reports it until the person scrolls).
            reader.restoring = false
            reader.report(target.chapter, target.page, target.offset)
        }
        AutoScroll(reader, state, dragged, app.prefs.autoScrollSpeed.dpPerSecond)
    }
}

/**
 * Moves the strip on by the time since the last frame times the speed. A finger on the strip, or just off it, holds it still. At the end of
 * the last chapter it stops; at the end of a chapter whose next one has not come in a few seconds (it failed to load) it stops too, with
 * the bars back so the person sees where they are.
 */
@Composable
private fun AutoScroll(reader: ReaderModel, state: LazyListState, dragged: Boolean, dpPerSecond: Float) {
    val density = LocalDensity.current
    val holding by rememberUpdatedState(dragged)
    LaunchedEffect(reader.autoScrolling) {
        if (!reader.autoScrolling) return@LaunchedEffect
        reader.chrome = false
        var last = withFrameNanos { it }
        var resumeAt = 0L
        var waitingSince: Long? = null
        while (true) {
            val now = withFrameNanos { it }
            // A long stall (a page decoding) must not become one big jump.
            val seconds = minOf((now - last) / 1_000_000_000f, 0.1f)
            last = now
            if (holding) {
                // After a finger lifts, auto-scroll carries on from wherever the strip came to rest, a second later.
                resumeAt = now + 1_000_000_000L
                continue
            }
            if (now < resumeAt) continue
            if (!state.canScrollForward) {
                val hasNext = reader.segments.lastOrNull()?.let { reader.newer(it.chapterId) } != null
                if (!hasNext) {
                    reader.autoScrolling = false
                } else if (waitingSince == null) {
                    waitingSince = now
                } else if (now - waitingSince > 4_000_000_000L) {
                    reader.autoScrolling = false
                    reader.chrome = true
                }
                continue
            }
            waitingSince = null
            state.scrollBy(seconds * with(density) { dpPerSecond.dp.toPx() })
        }
    }
}

/** Two fingers zoom: while they are down `onLive` is told the size to draw at, and when they lift the width of the strip changes to match. */
private suspend fun androidx.compose.ui.input.pointer.PointerInputScope.pinchToZoom(reader: ReaderModel, onLive: (Float) -> Unit) {
    awaitEachGesture {
        awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
        var scale = 1f
        do {
            val event = awaitPointerEvent(PointerEventPass.Initial)
            if (event.changes.size > 1) {
                scale = (scale * event.calculateZoom()).coerceIn(0.6f, 3f / maxOf(reader.zoom, 1f))
                onLive(scale)
                event.changes.forEach { it.consume() }
            }
        } while (event.changes.any { it.pressed })
        if (scale != 1f) reader.zoom = (reader.zoom * scale).coerceIn(1f, 3f)
        onLive(1f)
    }
}

@Composable
private fun ChapterEnd(reader: ReaderModel, segment: Segment) {
    val router = LocalRouter.current
    val next = reader.newer(segment.chapterId)
    val note = reader.skippedNote(segment.chapterId)
    Column(
        Modifier.fillMaxWidth().height(if (next == null) 220.dp else if (note == null) 96.dp else 124.dp).padding(horizontal = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterVertically),
    ) {
        AkiText("End of ${reader.chapter(segment.chapterId)?.let { reader.label(it) } ?: "chapter"}", AkiTextStyle.Eyebrow, tone = AkiTone.OnMedia)
        if (next != null) {
            AkiText(if (reader.segments.any { it.chapterId == next.id }) "Next chapter below" else "Next: ${reader.label(next)}", AkiTextStyle.Caption, tone = AkiTone.Muted)
            if (note != null) AkiText(note, AkiTextStyle.Caption, tone = AkiTone.Muted)
        } else {
            AkiText("You are up to date", AkiTextStyle.Title, tone = AkiTone.OnMedia)
            AkiButton("Back to the series", Modifier.padding(top = 6.dp)) { router.back() }
        }
    }
}
