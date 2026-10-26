package live.akidev.manga.manga

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import live.akidev.manga.app.describe
import uniffi.akimanga.Card
import uniffi.akimanga.Engine
import uniffi.akimanga.SourceInfo

/** What one source answered to a search: still waiting, titles, or why not. */
sealed interface SearchState {
    object Loading : SearchState
    class Done(val cards: List<Card>) : SearchState
    class Failed(val message: String) : SearchState
}

class SearchGroup(val source: SourceInfo, val state: SearchState)

/**
 * One search sent to every installed source at once; each source's answer appears when it arrives, and one that is slow or broken
 * shows its own state without holding the others back.
 */
class GlobalSearchModel {
    var groups by mutableStateOf(emptyList<SearchGroup>())
        private set
    private var term = ""

    /** Starts a search; one still running for an older term is ignored when it ends. */
    suspend fun search(engine: Engine, sources: List<SourceInfo>, term: String, report: (String, Throwable?) -> Unit) {
        this.term = term
        groups = sources.filter { it.canSearch }.map { SearchGroup(it, SearchState.Loading) }
        coroutineScope {
            groups.map { it.source }.map { source ->
                async {
                    val state = try {
                        SearchState.Done(engine.search(source.id, term, 1u, emptyList()).items).also { if (this@GlobalSearchModel.term == term) report(source.id, null) }
                    } catch (failure: Exception) {
                        if (this@GlobalSearchModel.term == term) report(source.id, failure)
                        SearchState.Failed(describe(failure).message)
                    }
                    if (this@GlobalSearchModel.term == term) groups = groups.map { if (it.source.id == source.id) SearchGroup(source, state) else it }
                }
            }.awaitAll()
        }
    }

    fun clear() {
        term = ""
        groups = emptyList()
    }
}
