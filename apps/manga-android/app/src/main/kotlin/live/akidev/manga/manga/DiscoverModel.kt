package live.akidev.manga.manga

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.Router
import live.akidev.manga.design.AkiMenuItem
import uniffi.akimanga.Choice
import uniffi.akimanga.FilterInfo
import uniffi.akimanga.HistoryEntry
import uniffi.akimanga.SourceInfo

/**
 * What the Discover screen is showing: the source chosen, the search typed, the lists asked for. It lives as long as the app, so
 * coming back to the tab finds the titles, the pages asked for and the scroll place as they were.
 */
class DiscoverModel(private val app: AppModel) {
    val browse = BrowseModel()
    val feed = FeedModel()
    val global = GlobalSearchModel()
    val scroll = LazyListState()

    var source by mutableStateOf(app.prefs.source)
    var term by mutableStateOf("")
    var submitted by mutableStateOf("")
    var filters by mutableStateOf(emptyList<FilterInfo>())
    var choices by mutableStateOf(emptyMap<String, String>())
    var reading by mutableStateOf(emptyList<HistoryEntry>())
        private set

    /** The sources the lists on screen were asked from, so coming back to this tab does not ask again. */
    var loadedKey = ""

    val isGlobal get() = source == EVERY_SOURCE
    val info: SourceInfo? get() = app.sources.firstOrNull { it.id == source }

    /** The first select filter a source defines (usually its genres), shown as a pill. */
    val category get() = filters.firstOrNull { it.kind == uniffi.akimanga.FilterKind.SELECT && it.options.size > 1 }

    /** Everything else a source lets a search be narrowed by, behind the one filter button; the category has its own pill. */
    val otherFilters get() = filters.filter { it.id != category?.id }

    /** One card per series: the history has a row for every chapter read, newest first, so only a series' latest row counts. */
    val continueReading: List<HistoryEntry>
        get() {
            val seen = HashSet<String>()
            return reading.filter { seen.add("${it.source}/${it.series}") }.filter { !it.finished }
        }

    fun choose(source: String) {
        this.source = source
        app.prefs.source = source
        submitted = ""
        term = ""
        choices = emptyMap()
        filters = runCatching { app.engine.sourceFilters(source) }.getOrDefault(emptyList())
    }

    /**
     * The Home icon tapped while Home is open: closes the series opened from here, clears the search and filters, and shows the
     * dashboard from the top. A Home with nothing to undo is left as it is.
     */
    fun goHome(router: Router) {
        val searched = submitted.isNotEmpty() || choices.isNotEmpty()
        if (router.top == null && !searched && term.isEmpty()) return
        router.clear()
        term = ""
        submitted = ""
        choices = emptyMap()
        app.launch {
            scroll.scrollToItem(0)
            if (searched) reload()
        }
    }

    /** What a long press on a "Continue reading" card offers: finish the series, or forget it was started. */
    fun continueMenu(entry: HistoryEntry, scope: CoroutineScope): List<AkiMenuItem> = listOf(
        AkiMenuItem("Mark all as read", "check") {
            scope.launch {
                app.io {
                    val ids = runCatching { app.engine.cachedChapters(entry.source, entry.series) }.getOrDefault(emptyList()).map { it.id }
                    runCatching { app.engine.mark(entry.source, entry.series, ids, true) }
                }
                afterHistoryChange()
            }
        },
        AkiMenuItem("Remove from history", "trash", destructive = true) {
            scope.launch {
                app.io { runCatching { app.engine.forgetHistory(entry.source, entry.series) } }
                afterHistoryChange()
            }
        },
    )

    private suspend fun afterHistoryChange() {
        app.refresh()
        reading = app.io { runCatching { app.engine.history(24u) }.getOrDefault(emptyList()) }
    }

    /** Called when what is installed changes, and when the screen opens: picks a source if the one chosen is gone, and loads once. */
    suspend fun sourcesChanged() {
        if (!isGlobal && app.visibleSources.none { it.id == source }) choose(app.visibleSources.firstOrNull()?.id ?: "")
        if (source.isEmpty()) return
        filters = app.io { runCatching { app.engine.sourceFilters(source) }.getOrDefault(emptyList()) }
        // Coming back from a series runs this again: keep the titles, the pages asked for and the scroll place.
        if (loadedKey == app.sourcesKey && (if (isGlobal) feed.shelves.isNotEmpty() else browse.items.isNotEmpty())) {
            reading = app.io { runCatching { app.engine.history(24u) }.getOrDefault(emptyList()) }
            return
        }
        loadedKey = app.sourcesKey
        reload()
    }

    /** `force` (pull to refresh, Try again) asks the sites again even for a listing kept a moment ago. */
    suspend fun reload(force: Boolean = false) {
        reading = app.io { runCatching { app.engine.history(24u) }.getOrDefault(emptyList()) }
        if (isGlobal) {
            if (submitted.isEmpty()) {
                global.clear()
                feed.loadAll(app.engine, app::report, app.visibleSources, force)
            } else {
                global.search(app.engine, app.visibleSources, submitted, app::report)
            }
        } else {
            // New releases load beside the popular list, not after it.
            val info = info
            coroutineScope {
                val latest = async { if (info != null) feed.loadLatest(app.engine, app::report, info, force) }
                load(reset = true, force = force)
                latest.await()
            }
        }
    }

    suspend fun load(reset: Boolean, force: Boolean = false) {
        val info = info ?: return
        browse.load(app.engine, app::report, info, submitted, choices.map { Choice(it.key, it.value) }.sortedBy { it.id }, reset, force)
    }

    companion object {
        const val EVERY_SOURCE = "*"
    }
}
