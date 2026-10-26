package live.akidev.manga.manga

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import live.akidev.manga.app.AppModel
import kotlinx.coroutines.launch
import live.akidev.manga.app.LibrarySort
import live.akidev.manga.design.AkiMenuItem
import uniffi.akimanga.CategoryInfo
import uniffi.akimanga.HistoryEntry
import uniffi.akimanga.LibraryEntry

enum class LibraryTab { Saved, History }

/** What the Library screen is showing: the shelf chosen, the order, what is picked. It lives as long as the app, so the tab comes back as it was left. */
class LibraryModel(private val app: AppModel) {
    val scroll = LazyListState()
    var tab by mutableStateOf(LibraryTab.Saved)
    var history by mutableStateOf(emptyList<HistoryEntry>())
        private set
    var categories by mutableStateOf(emptyList<CategoryInfo>())
        private set

    /** -1 is every category, -2 the series on none. */
    var category by mutableStateOf(-1L)

    /** Changes each time Random is chosen, so choosing it again shuffles again. */
    var shuffle by mutableStateOf(kotlin.random.Random.nextLong())
    var onlyUnread by mutableStateOf(false)
    var selecting by mutableStateOf(false)
    var picked by mutableStateOf(emptySet<String>())

    private val LibraryEntry.key get() = "$source/$id"

    val isEmpty get() = app.library.isEmpty()

    /** The library as shown: on the shelf chosen, in the order chosen. */
    val shown: List<LibraryEntry>
        get() {
            val filtered = app.library.filter { onShelf(it) && (!onlyUnread || it.unread > 0u) }
            val ordered = sorted(filtered)
            return if (app.prefs.libraryReversed) ordered.reversed() else ordered
        }

    /** The series picked that are on screen: filtering after picking does not leave hidden ones to be changed unseen. */
    val pickedShown get() = shown.filter { it.key in picked }

    private fun onShelf(item: LibraryEntry) = when (category) {
        -1L -> true
        -2L -> item.categories.isEmpty()
        else -> category in item.categories
    }

    /**
     * Each order puts the biggest first (the most recent, the most unread), except the title, which is A to Z. Ties fall back to the title so
     * the order never reshuffles on its own.
     */
    private fun sorted(items: List<LibraryEntry>): List<LibraryEntry> {
        fun byTitle() = compareBy<LibraryEntry, String>(String.CASE_INSENSITIVE_ORDER) { it.title }
        fun biggestFirst(value: (LibraryEntry) -> Long) = items.sortedWith(compareByDescending(value).then(byTitle()))
        return when (app.prefs.librarySort) {
            LibrarySort.Recent -> biggestFirst { it.lastReadMs ?: 0 }
            LibrarySort.Added -> biggestFirst { it.savedMs }
            LibrarySort.Title -> items.sortedWith(byTitle())
            LibrarySort.Unread -> biggestFirst { it.unread.toLong() }
            LibrarySort.Chapters -> biggestFirst { it.total.toLong() }
            LibrarySort.Latest -> {
                val dates = app.updates.associate { "${it.source}/${it.id}" to (it.latestDateMs ?: 0) }
                biggestFirst { dates[it.key] ?: 0 }
            }
            LibrarySort.Random -> items.sortedBy { "${it.key}$shuffle".hashCode() }
        }
    }

    /** Random shuffles anew each time it is chosen. */
    fun choose(option: LibrarySort) {
        if (option == LibrarySort.Random) shuffle = kotlin.random.Random.nextLong()
        app.prefs.librarySort = option
    }

    fun toggle(entry: LibraryEntry) {
        picked = if (entry.key in picked) picked - entry.key else picked + entry.key
    }

    fun select() {
        selecting = !selecting
        picked = emptySet()
    }

    /** The menu behind the filter button: the order, the filters, selecting, and the shelves. */
    fun menu(inMangaTab: Boolean, manage: () -> Unit): List<AkiMenuItem> = buildList {
        for (option in LibrarySort.entries) add(AkiMenuItem(option.title, "order", isSelected = option == app.prefs.librarySort) { choose(option) })
        add(AkiMenuItem("Reverse order", "swap", isSelected = app.prefs.libraryReversed) { app.prefs.libraryReversed = !app.prefs.libraryReversed })
        add(AkiMenuItem("Only unread", "check", isSelected = onlyUnread) { onlyUnread = !onlyUnread })
        add(AkiMenuItem(if (selecting) "Done selecting" else "Select", "check") { select() })
        add(AkiMenuItem("Categories", "folder", onClick = manage))
        // Inside the Manga tab there is already one row of choices, so the category is chosen from the menu instead of a second row.
        if (inMangaTab && categories.isNotEmpty()) {
            val shelves = listOf(-1L to "All categories") + categories.map { it.id to it.name } + listOf(-2L to "No category")
            for ((id, name) in shelves) add(AkiMenuItem(name, "folder", isSelected = id == category) { category = id })
        }
    }

    /** What a long press on a series offers: the shelves to put it on or take it off, reading it all, removing it. */
    fun menuOf(item: LibraryEntry, scope: kotlinx.coroutines.CoroutineScope): List<AkiMenuItem> =
        categories.map { shelf -> AkiMenuItem(shelf.name, "folder", isSelected = shelf.id in item.categories) { putOnShelf(item, shelf) } } + listOf(
            AkiMenuItem("Mark all as read", "check") { markRead(item) },
            AkiMenuItem("Remove from library", "bookmark-off", destructive = true) { scope.launch { remove(item) } },
        )

    /** Reads the categories, and leaves a shelf that has gone (deleted in the manage sheet) for All. */
    fun loadCategories() {
        categories = runCatching { app.engine.categories() }.getOrDefault(emptyList())
        if ((category >= 0 && categories.none { it.id == category }) || (category == -2L && categories.isEmpty())) category = -1
    }

    fun loadHistory() {
        history = runCatching { app.engine.history(60u) }.getOrDefault(emptyList())
    }

    fun markPickedRead() {
        for (item in pickedShown) markRead(item, refresh = false)
        app.refresh()
        picked = emptySet()
        selecting = false
    }

    fun markRead(item: LibraryEntry, refresh: Boolean = true) {
        val ids = runCatching { app.engine.cachedChapters(item.source, item.id) }.getOrDefault(emptyList()).map { it.id }
        runCatching { app.engine.mark(item.source, item.id, ids, true) }
        if (refresh) app.refresh()
    }

    suspend fun removePicked() {
        for (item in pickedShown) runCatching { app.engine.setInLibrary(item.source, item.id, false) }
        app.refresh()
        loadCategories()
        picked = emptySet()
        selecting = false
    }

    suspend fun remove(item: LibraryEntry) {
        runCatching { app.engine.setInLibrary(item.source, item.id, false) }
        app.refresh()
        loadCategories()
    }

    fun putOnShelf(item: LibraryEntry, shelf: CategoryInfo) {
        runCatching { app.engine.setCategory(item.source, item.id, shelf.id, shelf.id !in item.categories) }
        app.refresh()
        loadCategories()
    }
}
