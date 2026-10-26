package live.akidev.manga.manga

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
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

    suspend fun loadLatest(engine: Engine, report: (String, Throwable?) -> Unit, source: SourceInfo) {
        key = "latest|${source.id}"
        if (!source.canLatest) {
            latest = emptyList()
            return
        }
        engine.rememberedListing(source.id, true)?.let { latest = it.items.take(14) }
        val mine = key
        try {
            val fresh = engine.latest(source.id, 1u)
            if (key == mine) latest = fresh.items.take(14)
        } catch (failure: Exception) {
            if (key == mine) report(source.id, failure)
        }
    }

    /** Asks every source at once, showing what was remembered first and each answer as it arrives. */
    suspend fun loadAll(engine: Engine, report: (String, Throwable?) -> Unit, sources: List<SourceInfo>) {
        key = "all|" + sources.joinToString(",") { it.id }
        val mine = key
        loading = true
        try {
            shelves = sources.flatMap { source ->
                listOfNotNull(
                    engine.rememberedListing(source.id, false)?.let { Shelf("${source.id}|popular", source.name, "Popular", source.id, it.items.take(14)) },
                    if (source.canLatest) engine.rememberedListing(source.id, true)?.let { Shelf("${source.id}|latest", source.name, "New releases", source.id, it.items.take(14)) } else null,
                )
            }
            coroutineScope {
                // A source without a popular or latest list is not asked for it: asking would only surface the engine's "has no popular
                // list" as a failure the person cannot fix.
                sources.flatMap { source ->
                    listOfNotNull(
                        if (source.canPopular) async { arrived(source, false, runCatching { engine.popular(source.id, 1u).items }, mine, report) } else null,
                        if (source.canLatest) async { arrived(source, true, runCatching { engine.latest(source.id, 1u).items }, mine, report) } else null,
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
