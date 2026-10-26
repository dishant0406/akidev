package live.akidev.manga.manga

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import live.akidev.manga.app.Problem
import live.akidev.manga.app.describe
import uniffi.akimanga.Card
import uniffi.akimanga.CardPage
import uniffi.akimanga.Choice
import uniffi.akimanga.Engine
import uniffi.akimanga.SourceInfo

/** The titles of one source, paged: its popular list, or the answers to a search. */
class BrowseModel {
    var items by mutableStateOf(emptyList<Card>())
        private set
    var hasNext by mutableStateOf(false)
        private set
    var loading by mutableStateOf(false)
        private set
    var error by mutableStateOf<Problem?>(null)
        private set
    private var page = 0
    private var key = ""

    suspend fun load(engine: Engine, report: (String, Throwable?) -> Unit, source: SourceInfo, term: String, choices: List<Choice> = emptyList(), reset: Boolean) {
        val newKey = "${source.id}|$term|${choices.joinToString { "${it.id}=${it.value}" }}"
        if (reset || newKey != key) {
            items = emptyList()
            page = 0
            hasNext = false
            key = newKey
            error = null
        }
        if (loading) return
        loading = true
        try {
            // The first page of Popular and Latest shows at once from the last time, while the fresh one is fetched.
            if (page == 0 && items.isEmpty() && term.isEmpty() && choices.isEmpty() && source.canPopular) {
                engine.rememberedListing(source.id, false)?.let { kept ->
                    items = kept.items
                    hasNext = kept.hasNext
                }
            }
            val next = page + 1
            val result: CardPage = when {
                term.isNotEmpty() || choices.isNotEmpty() -> engine.search(source.id, term, next.toUInt(), choices)
                source.canPopular -> engine.popular(source.id, next.toUInt())
                // No popular list: fall back to the newest titles so the feed is not empty.
                source.canLatest -> engine.latest(source.id, next.toUInt())
                else -> {
                    items = emptyList()
                    hasNext = false
                    return
                }
            }
            if (key != newKey) return
            report(source.id, null)
            page = next
            items = if (next == 1) result.items else items + result.items.filter { card -> items.none { it.id == card.id } }
            hasNext = result.hasNext
        } catch (failure: Exception) {
            if (key != newKey) return
            report(source.id, failure)
            error = describe(failure)
        } finally {
            loading = false
        }
    }
}
