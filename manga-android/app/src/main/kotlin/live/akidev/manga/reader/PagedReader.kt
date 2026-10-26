package live.akidev.manga.reader

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.ReadingMode
import live.akidev.manga.design.*

/**
 * One page at a time, turned by swiping or by tapping the edges (the right edge goes on in left-to-right reading, the left edge in
 * right-to-left). Double tap or pinch to zoom.
 */
@Composable
fun PagedReader(reader: ReaderModel) {
    val app = LocalApp.current
    val scope = rememberCoroutineScope()
    val segment = reader.currentSegment ?: reader.segments[0]
    val count = segment.pages.images.size
    val next = reader.newer(segment.chapterId)
    val rtl = reader.mode == ReadingMode.RightToLeft
    var zoomed by remember { mutableStateOf(false) }
    // A new chapter is a new set of pages: the pager starts again on its own.
    key(segment.chapterId, rtl) {
        val pager = rememberPagerState(initialPage = reader.pagedIndex.coerceAtMost(count)) { count + 1 + if (next != null) 1 else 0 }
        LaunchedEffect(pager) {
            var was = pager.currentPage
            snapshotFlow { pager.settledPage }.collect { index ->
                reader.pagedIndex = index
                // Only a swipe from the end page to the one after it opens the next chapter.
                when {
                    index == count + 1 -> if (was == count && next != null) reader.open(next.id, 0, 0.0)
                    index == count -> reader.finish(segment.chapterId)
                    else -> reader.report(segment.chapterId, index, 0.0)
                }
                was = index
            }
        }
        LaunchedEffect(Unit) { reader.restoring = false }

        fun turn(x: Float) {
            // x is 0 at the left edge of the screen whatever the reading direction.
            val flipped = rtl != app.prefs.invertTaps
            val step = when {
                x > 0.7f -> if (flipped) -1 else 1
                x < 0.3f -> if (flipped) 1 else -1
                else -> 0
            }
            if (step == 0) {
                reader.chrome = !reader.chrome
                return
            }
            val target = pager.currentPage + step
            if (target in 0 until pager.pageCount) scope.launch { pager.animateScrollToPage(target) }
        }

        AkiGlassMotion(pager.isScrollInProgress)
        CompositionLocalProvider(LocalLayoutDirection provides if (rtl) LayoutDirection.Rtl else LayoutDirection.Ltr) {
            HorizontalPager(pager, Modifier.fillMaxSize(), userScrollEnabled = !zoomed, beyondViewportPageCount = 1) { index ->
                when {
                    index < count -> ZoomPage(index, segment.pages.images[index], segment.headers, ::turn) { zoomed = it }
                    index == count -> EndPage(reader, segment)
                    else -> ContinuePage(reader, next!!)
                }
            }
        }
    }
}

/** Where the swipe past the end page lands while the next chapter is fetched; if that fails it says so and can try again. */
@Composable
private fun ContinuePage(reader: ReaderModel, next: uniffi.akimanga.ChapterEntry) {
    val scope = rememberCoroutineScope()
    val error = reader.error
    CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Ltr) {
        Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterVertically)) {
            if (error != null) {
                AkiText(error.title, AkiTextStyle.Title, tone = AkiTone.OnMedia)
                AkiButton("Try again", variant = AkiButtonVariant.Primary) { scope.launch { reader.open(next.id, 0, 0.0) } }
            } else {
                AkiText(reader.label(next), AkiTextStyle.Eyebrow, tone = AkiTone.OnMedia)
                AkiSpinner()
            }
        }
    }
}

@Composable
private fun EndPage(reader: ReaderModel, segment: Segment) {
    val router = LocalRouter.current
    val scope = rememberCoroutineScope()
    val next = reader.newer(segment.chapterId)
    CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Ltr) {
        Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterVertically)) {
            AkiText("End of ${reader.chapter(segment.chapterId)?.let { reader.label(it) } ?: "chapter"}", AkiTextStyle.Eyebrow, tone = AkiTone.OnMedia)
            reader.skippedNote(segment.chapterId)?.let { AkiText(it, AkiTextStyle.Caption, tone = AkiTone.Muted) }
            if (next != null) {
                AkiButton("Next · ${reader.label(next)}", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large) { scope.launch { reader.open(next.id, 0, 0.0) } }
            } else {
                AkiText("You are up to date", AkiTextStyle.Title, tone = AkiTone.OnMedia)
            }
            AkiButton("Back to the series") { router.back() }
        }
    }
}
