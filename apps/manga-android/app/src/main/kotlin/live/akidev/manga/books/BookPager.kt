package live.akidev.manga.books

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import live.akidev.manga.design.AkiSpinner

/**
 * The pages of a book, turned by swiping or by tapping the edges. The pager holds one chapter's pages and, at each end, the page of the
 * chapter beside it: settling on one of those moves the book to that chapter, and the pager is made again for it.
 */
@Composable
fun BookPager(model: BookModel) {
    val chapter = model.place?.chapter ?: return
    val layout = model.layouts[chapter] ?: return
    // A chapter set in type again (the type changed) is a new set of pages.
    key(layout) { ChapterPager(model, layout) }
}

@Composable
private fun ChapterPager(model: BookModel, layout: ChapterLayout) {
    val chapter = layout.index
    val before = if (chapter > 0) 1 else 0
    val after = if (chapter + 1 < model.chapterCount) 1 else 0
    val still = model.style.ink
    val here = model.place?.page?.takeIf { it >= 0 }?.coerceAtMost(layout.count - 1) ?: 0
    val state = rememberPagerState(here + before) { layout.count + before + after }

    // Where the person settles: a page of this chapter, or the page of its neighbour that sits at the end.
    LaunchedEffect(state) {
        snapshotFlow { state.settledPage }.collect { index ->
            val page = index - before
            when {
                page < 0 -> model.go(Spot(chapter - 1, fraction = 1.0), forward = false)
                page >= layout.count -> model.go(Spot(chapter + 1, offset = 0), forward = true)
                else -> model.turned(BookPlace(chapter, page))
            }
        }
    }
    // An order to show a place now: a tap on an edge, a jump from the contents, a search, a link.
    LaunchedEffect(model.jump?.id) {
        val jump = model.jump ?: return@LaunchedEffect
        if (jump.place.chapter != chapter) return@LaunchedEffect
        val target = jump.place.page.coerceIn(0, layout.count - 1) + before
        if (state.currentPage == target) return@LaunchedEffect
        if (jump.animated && !still) state.animateScrollToPage(target) else state.scrollToPage(target)
    }

    // In a book read from right to left the next page comes in from the left.
    CompositionLocalProvider(LocalLayoutDirection provides if (model.rightToLeft) LayoutDirection.Rtl else LayoutDirection.Ltr) {
        HorizontalPager(
            state, Modifier.fillMaxSize().then(if (still) Modifier.stillSwipes(model) else Modifier),
            // While words are selected, the page waits; a still display turns by a swipe, not by dragging.
            userScrollEnabled = !still && model.selection == null,
        ) { index ->
            val page = index - before
            when {
                page in 0 until layout.count -> BookPageView(model, chapter, page)
                page < 0 -> NeighbourPage(model, chapter - 1, last = true)
                else -> NeighbourPage(model, chapter + 1, last = false)
            }
        }
    }
}

/** A swipe turns the page at once, with no motion: forward is to the left in a book read from left to right, and to the right in the other. */
private fun Modifier.stillSwipes(model: BookModel) = pointerInput(model.selection == null) {
    if (model.selection != null) return@pointerInput
    var travelled = 0f
    detectHorizontalDragGestures(onDragStart = { travelled = 0f }, onDragEnd = {
        if (kotlin.math.abs(travelled) > 60) model.turnPage(forward = (travelled < 0) != model.rightToLeft)
    }) { _, drag -> travelled += drag }
}

/** The page of the chapter beside this one that sits at the end, while the swipe is on it: its paper and words if the chapter is set, else the paper alone. */
@Composable
private fun NeighbourPage(model: BookModel, chapter: Int, last: Boolean) {
    LaunchedEffect(chapter) { model.layout(chapter) }
    if (model.layouts[chapter] != null) BookPageView(model, chapter, if (last) -1 else 0)
    else Box(Modifier.fillMaxSize().background(model.theme.paper), contentAlignment = Alignment.Center) { AkiSpinner() }
}

/**
 * A book read by scrolling: every chapter's pages one under another, the next chapter joined on as the end nears. A chapter not yet set in
 * type is one tall placeholder; when it is set its section becomes its pages, and what is on screen stays where it is.
 */
@Composable
fun BookScroller(model: BookModel) {
    val state = rememberLazyListState()
    val density = LocalDensity.current
    val counts = (0 until model.chapterCount).map { model.layouts[it]?.count ?: 1 }

    fun indexOf(place: BookPlace): Int {
        val layout = model.layouts[place.chapter]
        val page = if (place.page < 0) (layout?.count ?: 1) - 1 else place.page
        return counts.take(place.chapter).sum() + page.coerceIn(0, counts[place.chapter] - 1)
    }

    // An order to show a place now.
    LaunchedEffect(model.jump?.id, counts) {
        val jump = model.jump ?: return@LaunchedEffect
        if (model.layouts[jump.place.chapter] == null) return@LaunchedEffect
        val target = indexOf(jump.place)
        if (state.firstVisibleItemIndex != target) state.scrollToItem(target)
    }
    // Where the person is: the page at the top. The chapters around it are set in type before they are reached.
    LaunchedEffect(state, counts) {
        snapshotFlow { state.firstVisibleItemIndex }.collect { index ->
            var rest = index
            for ((chapter, count) in counts.withIndex()) {
                if (rest < count) {
                    if (model.layouts[chapter] != null) model.turned(BookPlace(chapter, rest))
                    for (near in chapter - 1..chapter + 1) model.layout(near)
                    break
                }
                rest -= count
            }
        }
    }

    LazyColumn(Modifier.fillMaxSize().background(model.theme.paper), state) {
        for (chapter in 0 until model.chapterCount) {
            val layout = model.layouts[chapter]
            if (layout == null) {
                item(key = "$chapter-placeholder") { Box(Modifier.fillMaxWidth().height(480.dp)) }
            } else {
                items(layout.count, key = { "$chapter-$it" }) { page ->
                    val tall = with(density) { (layout.leading(page) + layout.extent(page)).toDp() }
                    Box(Modifier.fillMaxWidth().height(tall)) { BookPageView(model, chapter, page) }
                }
            }
        }
    }
}
