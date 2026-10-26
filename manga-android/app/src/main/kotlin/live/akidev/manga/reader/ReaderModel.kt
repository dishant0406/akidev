package live.akidev.manga.reader

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.Problem
import live.akidev.manga.app.ReadingMode
import live.akidev.manga.app.SeriesRef
import live.akidev.manga.app.coverJpeg
import live.akidev.manga.app.describe
import live.akidev.manga.design.AkiImageLoader
import live.akidev.manga.manga.ReaderTarget
import live.akidev.manga.manga.chapterName
import uniffi.akimanga.Bookmark
import uniffi.akimanga.ChapterEntry
import uniffi.akimanga.PageList
import uniffi.akimanga.Reading

/** One chapter's pages, laid out in the strip. */
class Segment(val chapterId: String, val pages: PageList) {
    val headers: Map<String, String> = pages.headers.associate { it.name to it.value }
}

/** Where to put the reader when its pages are on screen. `offset` is how far down that page, 0 to 1 of its height. */
class Restore(val chapter: String, val page: Int, val offset: Double)

/**
 * Everything the reader knows: the chapters laid out, which one the person is in and how far, the mode and zoom, and the work done
 * ahead (the next chapter's pages and first images).
 */
class ReaderModel(private val app: AppModel, val ref: SeriesRef, chapters: List<ChapterEntry>, start: ReaderTarget, private val scope: CoroutineScope) {
    private val engine get() = app.engine

    /** Newest first, as the engine returns them: the next chapter to read is the one before. */
    val chapters = chapters

    var segments by mutableStateOf(emptyList<Segment>())
        private set
    var mode by mutableStateOf(app.prefs.modeOf(ref.source, ref.id) ?: ReadingMode.Strip)
        private set
    var zoom by mutableStateOf(1f)
    var current by mutableStateOf(start.chapterId)
        private set
    var page by mutableStateOf(start.page)
        private set
    var pagedIndex by mutableStateOf(start.page)
    var chrome by mutableStateOf(true)

    /** The strip is scrolling by itself. */
    var autoScrolling by mutableStateOf(false)

    /** The pages marked in this series. */
    var bookmarks by mutableStateOf(emptyList<Bookmark>())
        private set
    var loading by mutableStateOf(false)
        private set
    var error by mutableStateOf<Problem?>(null)
        private set
    var restore by mutableStateOf<Restore?>(Restore(start.chapterId, start.page, start.offset))
    var restoreToken by mutableStateOf(0)
        private set

    /** Set while the strip is being put back where it was: the scrolling that does it is not saved, and the strip is not shown. */
    var restoring by mutableStateOf(false)

    /** Each page's width over height once its picture is known, by address. */
    val aspects = mutableStateMapOf<String, Float>()
    private var heightRatios = 0f

    private val cache = HashMap<String, PageList>()
    private var latest: Triple<String, Int, Double>? = null
    private val finished = HashSet<String>()
    private var saveJob: Job? = null
    private var appending = false
    private val warmed = HashSet<String>()

    /** The width of the screen in pixels, set by the screen, for the size pictures are fetched at ahead of being shown. */
    var screenWidthPx = 1080
    var screenHeightPx = 2400

    init {
        loadBookmarks()
    }

    /**
     * The height, as a multiple of the width, a page is likely to have: the average of those already seen, so a page that has not
     * arrived holds about the room it will take and the strip does not jump when it does.
     */
    val typicalHeightRatio: Float
        get() = if (aspects.isEmpty()) 1.4f else (heightRatios / aspects.size).coerceIn(0.8f, 8f)

    /** Notes a page's width-to-height ratio once its picture is known. */
    fun noteAspect(aspect: Float, url: String) {
        if (aspect <= 0f || !aspect.isFinite()) return
        aspects[url]?.let { heightRatios -= 1 / it }
        aspects[url] = aspect
        heightRatios += 1 / aspect
    }

    // ---- chapters ------------------------------------------------------------------------------------------------------------

    fun chapter(id: String) = chapters.firstOrNull { it.id == id }

    fun newer(id: String): ChapterEntry? = chapters.indexOfFirst { it.id == id }.let { if (it > 0) chapters[it - 1] else null }

    fun older(id: String): ChapterEntry? = chapters.indexOfFirst { it.id == id }.let { if (it >= 0 && it + 1 < chapters.size) chapters[it + 1] else null }

    /** "Chapter 179 is missing from this source" when the chapter after this one skips ahead, else null. */
    fun skippedNote(id: String): String? {
        val number = chapter(id)?.number ?: return null
        val next = newer(id)?.number ?: return null
        val skipped = skippedChapters(number, next) ?: return null
        return if (skipped.count() == 1) "Chapter ${skipped.first} is missing from this source" else "Chapters ${skipped.first} to ${skipped.last} are missing from this source"
    }

    /** The whole chapter numbers a source skips between a chapter and the next one it lists: 178.5 then 180 skips 179. A jump of more than ten is a different way of numbering. */
    private fun skippedChapters(number: Double, next: Double): IntRange? {
        if (!number.isFinite() || !next.isFinite() || kotlin.math.abs(number) >= 100_000 || kotlin.math.abs(next) >= 100_000) return null
        val first = kotlin.math.floor(number).toInt() + 1
        val last = kotlin.math.floor(next).toInt() - 1
        return if (first <= last && last - first < 10) first..last else null
    }

    fun label(chapter: ChapterEntry) = chapter.title ?: chapterName(chapter.number, null)

    val currentSegment get() = segments.firstOrNull { it.chapterId == current }

    private suspend fun pageList(chapterId: String): PageList =
        cache[chapterId] ?: engine.pages(ref.source, ref.id, chapterId).also { cache[chapterId] = it }

    /** Opens a chapter, at a page and how far down it, using the pages already fetched if there are. */
    suspend fun open(chapterId: String, page: Int, offset: Double) {
        autoScrolling = false
        loading = true
        error = null
        try {
            val list = pageList(chapterId)
            val at = page.coerceIn(0, maxOf(list.images.size - 1, 0))
            if (app.prefs.modeOf(ref.source, ref.id) == null) {
                mode = when (list.reading) {
                    Reading.PAGED_RTL -> ReadingMode.RightToLeft
                    Reading.PAGED -> ReadingMode.Paged
                    Reading.LONG_STRIP -> ReadingMode.Strip
                    Reading.UNKNOWN -> app.prefs.defaultMode
                }
            }
            current = chapterId
            this.page = at
            pagedIndex = at
            // The strip's first rows report themselves as "the page at the top" while it is laid out; that is not where the person is,
            // so reporting waits until the strip has been put back.
            restoring = mode == ReadingMode.Strip
            segments = listOf(Segment(chapterId, list))
            restore = Restore(chapterId, at, offset)
            restoreToken += 1
            latest = Triple(chapterId, at, offset)
            loading = false
            prefetchNewer()
        } catch (failure: Exception) {
            loading = false
            error = describe(failure)
        }
    }

    fun changeMode(new: ReadingMode) {
        val segment = currentSegment
        if (new == mode || segment == null) return
        autoScrolling = false
        app.prefs.setModeOf(ref.source, ref.id, new)
        val offset = latest?.takeIf { it.first == current }?.third ?: 0.0
        mode = new
        zoom = 1f
        restoring = new == ReadingMode.Strip
        segments = listOf(segment)
        pagedIndex = page
        restore = Restore(current, page, if (new == ReadingMode.Strip) offset else 0.0)
        restoreToken += 1
    }

    // ---- cover ---------------------------------------------------------------------------------------------------------------

    /**
     * Makes what is on screen the series' cover: the part of the page under the middle of the screen, in a cover's shape (in the strip, a
     * page can be many screens tall, so the top of it is not what the person is looking at). False when the picture could not be read.
     */
    suspend fun useCurrentPageAsCover(): Boolean {
        val segment = currentSegment ?: return false
        val url = segment.pages.images.getOrNull(page) ?: return false
        val picture = AkiImageLoader.image(url, segment.headers, 600) ?: return false
        if (picture.height == 0) return false
        // How far down the page the top of the screen is (strip only), and then half a screen further to the middle of it.
        val scrolled = if (mode == ReadingMode.Strip && latest?.first == current && latest?.second == page) latest?.third ?: 0.0 else 0.0
        val halfScreen = screenHeightPx.toFloat() / screenWidthPx * picture.width / 2
        val centre = if (mode == ReadingMode.Strip) ((scrolled * picture.height + halfScreen) / picture.height).toFloat() else 0f
        val jpeg = coverJpeg(picture, 600, centre)
        return runCatching { engine.setCustomCover(ref.source, ref.id, jpeg) }.isSuccess
    }

    // ---- bookmarks -----------------------------------------------------------------------------------------------------------

    /** Whether the page the person is on (the one at the top of the strip) is marked. */
    val isBookmarked get() = bookmarks.any { it.chapter == current && it.page.toInt() == page }

    fun loadBookmarks() {
        bookmarks = runCatching { engine.bookmarks(ref.source, ref.id) }.getOrDefault(emptyList())
    }

    /** Marks the page on screen, and how far down it in the strip; marking a marked page takes the mark off. */
    fun toggleBookmark() {
        if (isBookmarked) {
            runCatching { engine.removeBookmark(ref.source, ref.id, current, page.toUInt()) }
        } else {
            val offset = if (mode == ReadingMode.Strip && latest?.first == current && latest?.second == page) latest?.third ?: 0.0 else 0.0
            runCatching { engine.addBookmark(ref.source, ref.id, current, page.toUInt(), offset) }
        }
        loadBookmarks()
    }

    // ---- ahead ---------------------------------------------------------------------------------------------------------------

    /** The next chapter's pages and its first images, fetched while this one is being read. */
    private fun prefetchNewer() {
        val next = newer(current) ?: return
        scope.launch {
            val list = runCatching { pageList(next.id) }.getOrNull() ?: return@launch
            warm(list, 0, 3)
        }
    }

    /** Loads the pictures just after `index` so they are there when the page is turned. */
    private fun warm(list: PageList, index: Int, count: Int) {
        val headers = list.headers.associate { it.name to it.value }
        val width = screenWidthPx * (if (mode == ReadingMode.Strip) zoom else 2f)
        for (url in list.images.drop(maxOf(index, 0)).take(count)) {
            if (!warmed.add("$url@${width.toInt()}")) continue
            scope.launch(Dispatchers.Default) {
                val picture = AkiImageLoader.image(url, headers, width.toInt()) ?: return@launch
                // The shape is noted as soon as it is known, so a row not yet on screen already has the right height.
                scope.launch { noteAspect(picture.width.toFloat() / picture.height, url) }
            }
        }
    }

    /** Continues the strip with the next chapter once the reader is near the end of the last one. */
    private suspend fun appendNewerIfNeeded() {
        val last = segments.lastOrNull() ?: return
        val next = newer(last.chapterId)
        if (mode != ReadingMode.Strip || appending || next == null) return
        appending = true
        try {
            val list = runCatching { pageList(next.id) }.getOrNull()
            if (list != null && segments.lastOrNull()?.chapterId == last.chapterId) segments = segments + Segment(next.id, list)
        } finally {
            appending = false
        }
    }

    // ---- position ------------------------------------------------------------------------------------------------------------

    /** The row at the top of the strip, or the page on screen: where the person is. */
    fun report(chapter: String, page: Int, offset: Double) {
        if (restoring) return
        // Called on every frame of a scroll; the work ahead only changes when the page does.
        val moved = chapter != current || this.page != page
        if (chapter != current) {
            // Reading on into the next chapter finishes the one before.
            val at = chapters.indexOfFirst { it.id == chapter }
            val was = chapters.indexOfFirst { it.id == current }
            if (at in 0 until was) finish(current)
            current = chapter
            prefetchNewer()
        }
        if (this.page != page) this.page = page
        latest = Triple(chapter, page, offset)
        scheduleSave()
        if (moved) {
            val segment = segments.firstOrNull { it.chapterId == chapter } ?: return
            warm(segment.pages, page + 1, 3)
            // And the pages just above: scrolling back up meets them already loaded and already the right height.
            warm(segment.pages, maxOf(page - 3, 0), minOf(3, page))
            if (mode == ReadingMode.Strip && page >= segment.pages.images.size - 3) scope.launch { appendNewerIfNeeded() }
        }
    }

    /** The end of a chapter has been reached. */
    fun finish(chapterId: String) {
        // Only a chapter still laid out can have been reached: opening another chapter tears the strip down, and its rows then report
        // positions that are not the person reading to the end.
        if (restoring || segments.none { it.chapterId == chapterId } || !finished.add(chapterId)) return
        runCatching { engine.setProgress(ref.source, ref.id, chapterId, 0u, 0.0, true) }
    }

    private fun scheduleSave() {
        saveJob?.cancel()
        saveJob = scope.launch {
            delay(350)
            flush()
        }
    }

    /** Writes the position down now. */
    fun flush() {
        saveJob?.cancel()
        val (chapter, page, offset) = latest ?: return
        if (chapter in finished) return
        runCatching { engine.setProgress(ref.source, ref.id, chapter, page.toUInt(), offset, false) }
    }
}
