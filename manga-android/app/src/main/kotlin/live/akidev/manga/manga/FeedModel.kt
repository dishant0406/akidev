package live.akidev.manga.manga

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import uniffi.akimanga.Card
import uniffi.akimanga.Engine
import uniffi.akimanga.SourceInfo

/** One row of the home feed: a source's popular titles or its new releases. */
class Shelf(val id: String, val title: String, val subtitle: String?, val source: String, val items: List<Card>)

/**
 * What the home feed shows besides the popular list: each source's newest titles, and with every source chosen, a shelf of popular
 * and a shelf of new for each.
 */
class FeedModel {
    /** One source's new releases (the popular list comes from the browse model, which pages it). */
    var latest by mutableStateOf(emptyList<Card>())
        private set

    /** Every source's shelves, for the "All" view. */
    var shelves by mutableStateOf(emptyList<Shelf>())
        private set
    var loading by mutableStateOf(false)
        private set
    private var key = ""

    suspend fun loadLatest(engine: Engine, report: (String, Throwable?) -> Unit, source: SourceInfo, force: Boolean = false) {
        key = "latest|${source.id}"
        if (!source.canLatest) {
            latest = emptyList()
            return
        }
        // A listing kept in the last ten minutes is shown and the site is not asked again, unless the person pulled to refresh.
        val fresh = if (force) null else withContext(Dispatchers.IO) { engine.freshListing(source.id, true) }
        if (fresh != null) {
            latest = fresh.items.take(14)
            return
        }
        withContext(Dispatchers.IO) { engine.rememberedListing(source.id, true) }?.let { latest = it.items.take(14) }
        val mine = key
        try {
            val fresh = engine.latest(source.id, 1u)
            if (key == mine) latest = fresh.items.take(14)
        } catch (failure: Exception) {
            if (key == mine) report(source.id, failure)
        }
    }

    /** Asks every source at once, showing what was remembered first and each answer as it arrives. */
    suspend fun loadAll(engine: Engine, report: (String, Throwable?) -> Unit, sources: List<SourceInfo>, force: Boolean = false) {
        key = "all|" + sources.joinToString(",") { it.id }
        val mine = key
        loading = true
        try {
            // What was kept is shown at once. A listing kept in the last ten minutes is also not asked for again (unless pulled to refresh).
            val stale = HashSet<String>()
            shelves = withContext(Dispatchers.IO) {
                sources.flatMap { source ->
                    listOfNotNull(
                        kept(engine, source, false, force, stale),
                        if (source.canLatest) kept(engine, source, true, force, stale) else null,
                    )
                }
            }
            // At most four sites are asked at a time, so the first sources answer first and covers can load beside them.
            val limit = Semaphore(4)
            coroutineScope {
                // A source without a popular or latest list is not asked for it: asking would only surface the engine's "has no popular
                // list" as a failure the person cannot fix.
                sources.flatMap { source ->
                    listOfNotNull(
                        if (source.canPopular && "${source.id}|popular" in stale) async { limit.withPermit { arrived(source, false, runCatching { engine.popular(source.id, 1u).items }, mine, report) } } else null,
                        if (source.canLatest && "${source.id}|latest" in stale) async { limit.withPermit { arrived(source, true, runCatching { engine.latest(source.id, 1u).items }, mine, report) } } else null,
                    )
                }.awaitAll()
            }
            // Sources in the order of the chips; each one's popular shelf before its new releases.
            val order = sources.map { it.id }
            if (key == mine) shelves = shelves.sortedBy { (order.indexOf(it.source).coerceAtLeast(0)) * 2 + if (it.id.endsWith("latest")) 1 else 0 }
        } finally {
            if (key == mine) loading = false
        }
    }

    /** A source's kept shelf: the fresh listing if there is one, else the old one, which is then listed in `stale` to be asked for again. */
    private fun kept(engine: Engine, source: SourceInfo, isLatest: Boolean, force: Boolean, stale: MutableSet<String>): Shelf? {
        val id = "${source.id}|${if (isLatest) "latest" else "popular"}"
        val fresh = if (force) null else engine.freshListing(source.id, isLatest)
        if (fresh == null) stale.add(id)
        val page = fresh ?: engine.rememberedListing(source.id, isLatest) ?: return null
        return Shelf(id, source.name, if (isLatest) "New releases" else "Popular", source.id, page.items.take(14))
    }

    private fun arrived(source: SourceInfo, isLatest: Boolean, result: Result<List<Card>>, mine: String, report: (String, Throwable?) -> Unit) {
        if (key != mine) return
        val id = "${source.id}|${if (isLatest) "latest" else "popular"}"
        result.onSuccess { cards ->
            report(source.id, null)
            val shelf = Shelf(id, source.name, if (isLatest) "New releases" else "Popular", source.id, cards.take(14))
            shelves = if (shelves.any { it.id == id }) shelves.map { if (it.id == id) shelf else it } else shelves + shelf
        }
        result.onFailure { report(source.id, it) }
    }
}
