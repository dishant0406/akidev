package live.akidev.manga.app

import android.content.Context
import androidx.core.net.toUri
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
import live.akidev.manga.books.BooksModel
import live.akidev.manga.listen.ReadAloudModel
import live.akidev.manga.manga.DiscoverModel
import live.akidev.manga.manga.LibraryModel
import live.akidev.manga.manga.UpdatesModel
import live.akidev.manga.updates.AutoDownloader
import live.akidev.manga.updates.NewChapters
import live.akidev.manga.updates.UpdateAlerts
import live.akidev.manga.updates.UpdateWorker
import uniffi.akimanga.Engine
import uniffi.akimanga.LibraryEntry
import uniffi.akimanga.RepoInfo
import uniffi.akimanga.ShareCard
import uniffi.akimanga.SourceInfo
import uniffi.akimanga.UpdateEntry
import java.io.File

/** An address to add as a repository, waiting for the person to confirm (a pasted link or an opened one). */
class PendingRepo(val address: String)

/** A series from a source that another phone shared, waiting for the person to say yes to adding it (and its source, if this phone does not have it). */
class PendingCard(val path: String, val card: ShareCard)

/** The Share sheet, open: what is being sent (a line for the sheet, and the files, made when the sheet opens), or nothing, to only receive. */
class Sharing(val label: String?, val files: (() -> List<uniffi.akimanga.ShareFile>)?)

/** Series the last refresh could not check, with why. */
class UpdateFailure(val key: String, val title: String, val source: String, val message: String)

/**
 * What the whole app shares: the engine, the settings, and what is installed and saved. Screens read these fields (they update when
 * the fields change) and ask the model to change them.
 */
class AppModel(private val context: Context) {
    /** The app's context, for what a screen needs from the system (the clipboard, the cache). */
    val appContext: Context get() = context
    val engine = Engine(File(context.filesDir, "AkiManga").path)
    val prefs = Preferences(context)
    val glass = GlassGovernor(prefs)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val downloader = AutoDownloader(context, engine) { prefs.downloadOnWifiOnly }

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
    val readAloud = ReadAloudModel(engine, prefs, scope)
    val discover = DiscoverModel(this)
    val libraryScreen = LibraryModel(this)
    val updatesScreen = UpdatesModel()
    val booksScreen = BooksModel(this)
    var tab by mutableStateOf(AppTab.order(prefs.readerKind).first())
    var pendingRepo by mutableStateOf<PendingRepo?>(null)
    var pendingCard by mutableStateOf<PendingCard?>(null)
    var sharing by mutableStateOf<Sharing?>(null)

    /** Something to tell the person, over whatever screen they are on. */
    var notice by mutableStateOf<Problem?>(null)

    /** Set to open the file picker on the Books tab (from the Sources screen). */
    var addingBook by mutableStateOf(false)

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
        scheduleBackgroundWork()
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
        if (address.startsWith("akimanga:")) address = address.toUri().getQueryParameter("url") ?: address
        if (address.isNotEmpty()) pendingRepo = PendingRepo(address)
    }

    /**
     * Puts a file that came from another phone, or from another app, where it belongs, and says what happened. A series from a source is
     * not added here: it may bring a source with it, so the person is asked first (see [pendingCard]).
     */
    suspend fun takeIn(path: String, name: String): String = try {
        val lower = name.lowercase()
        when {
            lower.endsWith(".epub") -> {
                val book = io { engine.importBook(path) }
                booksScreen.load()
                "Added “${book.title}” to your books."
            }
            lower.endsWith(".cbz") -> {
                io { engine.importFile(path) }
                refresh()
                "Added “${name.removeSuffix(".cbz")}” to your library."
            }
            lower.endsWith(".akidev.json") -> {
                // The file is in a folder that is emptied soon, and the question may wait.
                val kept = File(File(context.cacheDir, "cards"), java.util.UUID.randomUUID().toString())
                io { kept.parentFile?.mkdirs(); File(path).copyTo(kept) }
                val card = io { engine.shareReadCard(kept.path) }
                if (card.installed) {
                    engine.shareAddCard(kept.path)
                    refresh()
                    "Added “${card.title}” to your library."
                } else {
                    pendingCard = PendingCard(kept.path, card)
                    ""
                }
            }
            else -> "“$name” is not a book or a comic."
        }
    } catch (failure: Exception) {
        "“$name”: ${describe(failure).message}"
    }

    /** A book, a comic or a shared series sent to the app from another one ("Open with", "Share"). */
    fun openFile(uri: android.net.Uri) {
        scope.launch {
            val file = withContext(Dispatchers.IO) { copyToCache(context, uri) }
            if (file == null) {
                notice = Problem("Could not open it", "Android would not let Akidev read that file.")
                return@launch
            }
            // The file's name may not say what it is; what the sender said it is may.
            val extension = when (context.contentResolver.getType(uri)) {
                "application/epub+zip" -> ".epub"
                "application/vnd.comicbook+zip", "application/x-cbz" -> ".cbz"
                "application/vnd.akidev.share+json" -> ".akidev.json"
                else -> ""
            }
            val name = if (file.name.lowercase().endsWith(extension)) file.name else file.name + extension
            val said = takeIn(file.path, name)
            withContext(Dispatchers.IO) { file.parentFile?.deleteRecursively() }
            if (said.isNotBlank()) notice = Problem("Opened", said)
        }
    }

    fun report(source: String, error: Throwable?) {
        sourceProblems = if (error == null) sourceProblems - source else sourceProblems + (source to describe(error).message)
    }

    /**
     * Asks the sites for the chapters of saved series that are due (not asked in the last hour, a finished series not in the last day),
     * four at a time. Skipped when done in the last half hour unless `force` is set (a pull or a tap), which asks about every series
     * not asked in the last minute.
     */
    suspend fun refreshLibrary(force: Boolean = false, background: Boolean = false) {
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
            val adult = sources.filter { it.nsfw }.map { it.id }.toSet()
            val results = withContext(Dispatchers.IO) {
                due.map { entry ->
                    async {
                        gate.withPermit {
                            try {
                                val list = engine.chapters(entry.source, entry.id)
                                val fresh = list.newIds.toSet()
                                val arrived = list.entries.filter { it.id in fresh }.reversed()
                                val news = if (arrived.isEmpty()) null else NewChapters(entry.source, entry.id, entry.title, entry.source in adult, arrived)
                                Triple(entry, null, news)
                            } catch (error: Exception) {
                                runCatching { engine.noteFailedCheck(entry.source, entry.id) }
                                Triple(entry, error, null)
                            }
                        }
                    }
                }.awaitAll()
            }
            // What failed before and was not asked this time stays on the list.
            val asked = results.map { "${it.first.source}/${it.first.id}" }.toSet()
            val stillFailing = updateFailures.filter { it.key !in asked }
            updateFailures = stillFailing + results.mapNotNull { (entry, error, _) ->
                error?.let { UpdateFailure("${entry.source}/${entry.id}", entry.title, name(entry.source), describe(it).message) }
            }
            // A source is "not working" when any of its series failed this time; one that is gone loses its mark.
            for (source in results.map { it.first.source }.toSet()) report(source, null)
            for ((entry, error, _) in results) if (error != null) report(entry.source, error)
            sourceProblems = sourceProblems.filterKeys { problem -> sources.any { it.id == problem } }
            if (results.isEmpty() || results.any { it.second == null }) lastRefreshMs = System.currentTimeMillis()
            refresh()
            val found = results.mapNotNull { it.third }
            if (found.isEmpty()) return
            // New chapters are announced only when Android woke the app: with the app open the Updates tab already shows them.
            if (background && prefs.notifyUpdates) UpdateAlerts.post(context, found)
            if (prefs.autoDownload) {
                downloader.enqueue(found)
                downloader.run()
            }
        } finally {
            refreshing = false
        }
    }

    /** Carries on with downloads that were cut short or waited for Wi-Fi. */
    suspend fun resumeDownloads() {
        if (prefs.autoDownload) downloader.run()
    }

    /** Looks for new chapters in the background only while alerts or downloads want them. */
    fun scheduleBackgroundWork() {
        UpdateWorker.schedule(context, prefs.notifyUpdates || prefs.autoDownload)
    }

    /** Updates installed sources whose repository has a newer version; quiet if there is nothing or no network. */
    suspend fun updateSources() {
        val updated = runCatching { engine.updateSources() }.getOrDefault(emptyList())
        if (updated.isNotEmpty()) refresh()
    }

    fun launch(block: suspend () -> Unit): Job = scope.launch { block() }

    /** Reads from the engine off the main thread: a list that has grown (every chapter, every download) is not made while a screen slides in. */
    suspend fun <T> io(block: () -> T): T = withContext(Dispatchers.IO) { block() }
}
