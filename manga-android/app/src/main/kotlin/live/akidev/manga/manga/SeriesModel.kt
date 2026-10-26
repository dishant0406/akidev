package live.akidev.manga.manga

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.Problem
import live.akidev.manga.app.SeriesRef
import live.akidev.manga.app.describe
import uniffi.akimanga.Bookmark
import uniffi.akimanga.ChapterEntry
import uniffi.akimanga.Detail

/** Where the reader opens: a chapter, a page in it, and how far down that page (0 to 1). */
class ReaderTarget(val chapterId: String, val page: Int, val offset: Double = 0.0)

/** What the Continue button opens, and what it says. */
class Resumable(val target: ReaderTarget, val label: String)

/** One series page's state: what the site said, what is saved, what is on the phone, and the things a person can do to it. */
class SeriesModel(private val app: AppModel, val ref: SeriesRef) {
    var detail by mutableStateOf<Detail?>(null)
        private set
    var chapters by mutableStateOf(emptyList<ChapterEntry>())
        private set

    /** Where Continue goes, worked out when the chapters or progress change, not on every redraw. */
    var resumable by mutableStateOf<Resumable?>(null)
        private set
    var saved by mutableStateOf(false)
        private set
    var loading by mutableStateOf(true)
        private set
    var error by mutableStateOf<Problem?>(null)
        private set

    /** Chapters kept on the phone. */
    var kept by mutableStateOf(emptySet<String>())
        private set
    var bookmarks by mutableStateOf(emptyList<Bookmark>())
        private set

    /** The cover the person chose for this series, as an address, or null. */
    var chosenCover by mutableStateOf<String?>(null)
        private set

    /** Said when something the person asked for failed (a download), until they dismiss it. */
    var problem by mutableStateOf<String?>(null)

    /** The chapter whose pages were fetched ahead, so it is done once. */
    private var fetchedAhead: String? = null
    private val engine get() = app.engine

    /** The cover the person chose, else the source's. */
    val cover get() = chosenCover ?: detail?.cover ?: ref.cover

    val statusText
        get() = when (detail?.status) {
            uniffi.akimanga.Status.ONGOING -> "Ongoing"
            uniffi.akimanga.Status.COMPLETED -> "Completed"
            uniffi.akimanga.Status.HIATUS -> "Hiatus"
            uniffi.akimanga.Status.CANCELLED -> "Cancelled"
            else -> null
        }

    private fun showChapters(list: List<ChapterEntry>) {
        chapters = list
        val resume = if (list.isEmpty()) null else runCatching { engine.resume(ref.source, ref.id) }.getOrNull()
        val chapter = resume?.let { r -> list.firstOrNull { it.id == r.chapter } }
        if (resume == null || chapter == null) {
            resumable = null
            return
        }
        val started = list.any { it.finished || it.page != null }
        val name = chapterName(chapter.number, chapter.title)
        resumable = Resumable(ReaderTarget(resume.chapter, resume.page.toInt(), resume.offset), if (started) "Continue · $name" else "Start · $name")
        // Only for a series the person keeps or has begun: browsing past one should not cost the site a request.
        if (started || saved) fetchPagesAhead(resume.chapter)
    }

    /**
     * Shows what is stored at once, then asks the site. The chapter list is not asked again within ten minutes of the last time (going
     * back and forth between a series and the reader, say) unless `force` is set.
     */
    suspend fun load(force: Boolean = false) {
        error = null
        saved = runCatching { engine.inLibrary(ref.source, ref.id) }.getOrDefault(false)
        showChapters(runCatching { engine.cachedChapters(ref.source, ref.id) }.getOrDefault(emptyList()))
        if (detail == null) detail = runCatching { engine.cachedDetails(ref.source, ref.id) }.getOrNull()
        val checked = runCatching { engine.chaptersCheckedMs(ref.source, ref.id) }.getOrNull()
        val fresh = !force && chapters.isNotEmpty() && checked != null && System.currentTimeMillis() - checked < 600_000
        loading = !fresh
        loadKept()
        loadBookmarks()
        chosenCover = runCatching { engine.customCover(ref.source, ref.id) }.getOrNull()
        coroutineScope {
            val fetchedDetail = async { runCatching { engine.details(ref.source, ref.id) } }
            val fetchedChapters = if (fresh) null else async { runCatching { engine.chapters(ref.source, ref.id) } }
            fetchedDetail.await().fold({ detail = it }, { if (detail == null) error = describe(it) })
            fetchedChapters?.await()?.fold({ showChapters(it.entries) }, { if (chapters.isEmpty()) error = describe(it) })
        }
        loading = false
        loadBookmarks()
    }

    /** Asks for the pages of the chapter Continue opens while the series page is read, so the reader finds them ready. */
    private fun fetchPagesAhead(chapter: String) {
        if (fetchedAhead == chapter) return
        fetchedAhead = chapter
        app.launch { runCatching { engine.pages(ref.source, ref.id, chapter) } }
    }

    private fun loadKept() {
        kept = runCatching { engine.downloads() }.getOrDefault(emptyList()).filter { it.source == ref.source && it.series == ref.id && it.state == "done" }.map { it.chapter }.toSet()
    }

    fun loadBookmarks() {
        bookmarks = runCatching { engine.bookmarks(ref.source, ref.id) }.getOrDefault(emptyList())
    }

    fun removeBookmark(mark: Bookmark) {
        runCatching { engine.removeBookmark(ref.source, ref.id, mark.chapter, mark.page) }
        loadBookmarks()
    }

    /** Downloads each chapter in turn, oldest first. Returns whether every one arrived. */
    suspend fun download(list: List<ChapterEntry>): Boolean {
        var failed = 0
        for (chapter in list.reversed()) {
            try { engine.downloadChapter(ref.source, ref.id, chapter.id) } catch (failure: Exception) { failed += 1 }
            loadKept()
        }
        if (failed > 0) problem = if (failed == 1) "One chapter could not be downloaded." else "$failed chapters could not be downloaded."
        return failed == 0
    }

    fun removeDownload(chapter: ChapterEntry) {
        runCatching { engine.deleteDownload(ref.source, ref.id, chapter.id) }
        loadKept()
    }

    /** Back from the reader: its progress is in the engine, not here. */
    fun refreshProgress() {
        loadBookmarks()
        showChapters(runCatching { engine.cachedChapters(ref.source, ref.id) }.getOrDefault(chapters))
        app.refresh()
    }

    /** Saves or removes the series. Returns whether it is now saved. */
    suspend fun toggleSaved(): Boolean {
        try {
            engine.setInLibrary(ref.source, ref.id, !saved)
            saved = !saved
            app.refresh()
        } catch (failure: Exception) {
            error = describe(failure)
        }
        return saved
    }

    /** What Settings says saving does about categories: nothing (0), ask which (-1), or put the series on the chosen one. Returns true to ask. */
    fun putOnSaveCategory(): Boolean {
        val id = app.prefs.saveCategory
        when {
            id == 0L -> Unit
            id == -1L -> return true
            // A category that has since been deleted (or came back from a backup under another number) is not one to put it on.
            runCatching { engine.categories() }.getOrDefault(emptyList()).any { it.id == id } -> runCatching { engine.setCategory(ref.source, ref.id, id, true) }
        }
        return false
    }

    fun setCover(jpeg: ByteArray) {
        runCatching { engine.setCustomCover(ref.source, ref.id, jpeg) }
        chosenCover = runCatching { engine.customCover(ref.source, ref.id) }.getOrNull()
        app.refresh()
    }

    fun clearCover() {
        runCatching { engine.clearCustomCover(ref.source, ref.id) }
        chosenCover = null
        app.refresh()
    }

    fun toggleRead(chapter: ChapterEntry) = mark(listOf(chapter.id), !chapter.finished)

    fun markAll(read: Boolean) = mark(chapters.map { it.id }, read)

    private fun mark(ids: List<String>, read: Boolean) {
        runCatching { engine.mark(ref.source, ref.id, ids, read) }
        showChapters(runCatching { engine.cachedChapters(ref.source, ref.id) }.getOrDefault(chapters))
        app.refresh()
    }

    fun forget() {
        runCatching { engine.forgetSeries(ref.source, ref.id) }
        app.refresh()
    }

    /** The small line over a chapter's title: where you are in it, or what it is. */
    fun label(chapter: ChapterEntry): String {
        val downloaded = chapter.id in kept
        if (chapter.finished) return if (downloaded) "Read · Downloaded" else "Read"
        if (downloaded) return "Downloaded"
        chapter.page?.let { return "Page ${it + 1u}" }
        val number = chapter.number?.let { "Chapter " + chapterName(it, null).removePrefix("Chapter ") }
        if (number != null && chapter.title?.contains(number, ignoreCase = true) == false) return number
        return "Unread"
    }
}
