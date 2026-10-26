package live.akidev.manga.app

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import live.akidev.manga.manga.DiscoverModel
import uniffi.akimanga.Engine
import uniffi.akimanga.LibraryEntry
import uniffi.akimanga.RepoInfo
import uniffi.akimanga.SourceInfo
import uniffi.akimanga.UpdateEntry
import java.io.File

/** An address to add as a repository, waiting for the person to confirm (a pasted link or an opened one). */
class PendingRepo(val address: String)

/** Series the last refresh could not check, with why. */
class UpdateFailure(val key: String, val title: String, val source: String, val message: String)

/**
 * What the whole app shares: the engine, the settings, and what is installed and saved. Screens read these fields (they update when
 * the fields change) and ask the model to change them.
 */
class AppModel(context: Context) {
    val engine = Engine(File(context.filesDir, "AkiManga").path)
    val prefs = Preferences(context)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    /** Everything installed; [visibleSources] is what the person's content settings let through. */
    var sources by mutableStateOf(emptyList<SourceInfo>())
        private set
    var repos by mutableStateOf(emptyList<RepoInfo>())
        private set
    var library by mutableStateOf(emptyList<LibraryEntry>())
        private set

    /** The Updates screen's rows, read in one go with the library. */
    var updates by mutableStateOf(emptyList<UpdateEntry>())
        private set
    val discover = DiscoverModel(this)
    var tab by mutableStateOf(AppTab.order(prefs.readerKind).first())
    var pendingRepo by mutableStateOf<PendingRepo?>(null)

    /** Bumped when a source's own settings change, so Discover asks it again. */
    var settingsRevision by mutableStateOf(0)
        private set

    /** Why a source is not working right now, by source id; cleared when it answers again. */
    var sourceProblems by mutableStateOf(emptyMap<String, String>())
        private set
    var updateFailures by mutableStateOf(emptyList<UpdateFailure>())
        private set
    var refreshing by mutableStateOf(false)
        private set
    private var lastRefreshMs = 0L
    private var loadingLibrary = false
    private var libraryChanged = false

    /** The tabs, with what the person mostly reads first. */
    val tabs get() = AppTab.order(prefs.readerKind)

    init {
        // Someone who already has sources or books is not asked what they read: they keep the app as it was.
        if (prefs.readerKind == null && (engine.sources().isNotEmpty() || engine.repos().isNotEmpty() || engine.books().isNotEmpty())) {
            prefs.readerKind = ReaderKind.Manga
        }
        tab = tabs.first()
        refresh()
        scope.launch {
            updateSources()
            refreshLibrary()
        }
    }

    val visibleSources
        get() = sources.filter { source ->
            (prefs.showAdult || !source.nsfw) &&
                (prefs.languages.isEmpty() || source.languages.isEmpty() || source.languages.any { it in prefs.languages })
        }

    /** Chapters not read, across the library: the Updates badge. */
    val unreadTotal get() = library.sumOf { it.unread.toInt() }

    /** Changes when a source is installed, removed or updated (not when the library changes). */
    val sourcesKey get() = sources.joinToString(",") { "${it.id}@${it.version}" } + "#$settingsRevision"

    fun name(source: String) = sources.firstOrNull { it.id == source }?.name ?: source

    fun sourceSettingsChanged() {
        settingsRevision += 1
    }

    /** Re-reads what is installed (a few rows) at once, and the library off the main thread: it grows with the collection. */
    fun refresh() {
        val installed = engine.sources().sortedBy { it.name.lowercase() }
        if (installed != sources) sources = installed
        val added = engine.repos()
        if (added != repos) repos = added
        reloadLibrary()
    }

    private fun reloadLibrary() {
        // One read at a time; a change that arrives during one asks for another when it ends.
        if (loadingLibrary) {
            libraryChanged = true
            return
        }
        loadingLibrary = true
        scope.launch {
            readLibrary()
            loadingLibrary = false
            if (libraryChanged) {
                libraryChanged = false
                reloadLibrary()
            }
        }
    }

    suspend fun readLibrary() {
        val (entries, rows) = withContext(Dispatchers.IO) { runCatching { engine.library() }.getOrDefault(emptyList()) to runCatching { engine.updates() }.getOrDefault(emptyList()) }
        if (entries != library) library = entries
        if (rows != updates) updates = rows
    }

    /** Opens the add-repository confirmation for an address; a bare address or an `akimanga://add-repo?url=` link. */
    fun propose(repository: String) {
        var address = repository.trim()
        if (address.startsWith("akimanga:")) address = android.net.Uri.parse(address).getQueryParameter("url") ?: address
        if (address.isNotEmpty()) pendingRepo = PendingRepo(address)
    }

    fun report(source: String, error: Throwable?) {
        sourceProblems = if (error == null) sourceProblems - source else sourceProblems + (source to describe(error).message)
    }

    /**
     * Asks the sites for the chapters of saved series that are due (not asked in the last hour, a finished series not in the last day),
     * four at a time. Skipped when done in the last half hour unless `force` is set (a pull or a tap), which asks about every series
     * not asked in the last minute.
     */
    suspend fun refreshLibrary(force: Boolean = false) {
        val now = System.currentTimeMillis()
        if (refreshing || (!force && now - lastRefreshMs < 30 * 60_000)) return
        refreshing = true
        try {
            readLibrary()
            val due = library.filter { entry ->
                val checked = entry.checkedMs ?: return@filter true
                val age = now - checked
                if (force) age > 60_000 else age > (if (entry.completed) 86_400_000 else 3_600_000)
            }
            val gate = Semaphore(4)
            val results = withContext(Dispatchers.IO) {
                due.map { entry ->
                    async {
                        gate.withPermit {
                            try {
                                engine.chapters(entry.source, entry.id)
                                entry to null
                            } catch (error: Exception) {
                                runCatching { engine.noteFailedCheck(entry.source, entry.id) }
                                entry to error
                            }
                        }
                    }
                }.awaitAll()
            }
            // What failed before and was not asked this time stays on the list.
            val asked = results.map { "${it.first.source}/${it.first.id}" }.toSet()
            val stillFailing = updateFailures.filter { it.key !in asked }
            updateFailures = stillFailing + results.mapNotNull { (entry, error) ->
                error?.let { UpdateFailure("${entry.source}/${entry.id}", entry.title, name(entry.source), describe(it).message) }
            }
            // A source is "not working" when any of its series failed this time; one that is gone loses its mark.
            for (source in results.map { it.first.source }.toSet()) report(source, null)
            for ((entry, error) in results) if (error != null) report(entry.source, error)
            sourceProblems = sourceProblems.filterKeys { problem -> sources.any { it.id == problem } }
            if (results.isEmpty() || results.any { it.second == null }) lastRefreshMs = System.currentTimeMillis()
            refresh()
        } finally {
            refreshing = false
        }
    }

    /** Updates installed sources whose repository has a newer version; quiet if there is nothing or no network. */
    suspend fun updateSources() {
        val updated = runCatching { engine.updateSources() }.getOrDefault(emptyList())
        if (updated.isNotEmpty()) refresh()
    }

    fun launch(block: suspend () -> Unit): Job = scope.launch { block() }
}
