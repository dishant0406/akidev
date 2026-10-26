package live.akidev.manga.books

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import live.akidev.manga.app.CatalogueRef
import live.akidev.manga.app.CatalogueRoute
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.Problem
import live.akidev.manga.app.describe
import live.akidev.manga.design.*
import uniffi.akimanga.CatalogueInfo
import uniffi.akimanga.CatalogueItem
import uniffi.akimanga.Engine

/** What one catalogue answered to a search: still waiting, books, or why not. */
sealed interface CatalogueAnswer {
    object Loading : CatalogueAnswer
    class Done(val items: List<CatalogueItem>) : CatalogueAnswer
    class Failed(val message: String) : CatalogueAnswer
}

class CatalogueGroup(val url: String, val name: String, val search: String?, val answer: CatalogueAnswer)

/** One search sent to every catalogue that can be searched; each answers on its own and a slow one does not hold the others. */
class BookSearchAcross {
    var groups by mutableStateOf(emptyList<CatalogueGroup>())
        private set
    private var term = ""

    fun clear() {
        term = ""
        groups = emptyList()
    }

    suspend fun run(engine: Engine, catalogues: List<CatalogueInfo>, words: String) {
        val typed = words.trim()
        if (typed.isEmpty()) return clear()
        term = typed
        groups = catalogues.map { CatalogueGroup(it.url, it.name, null, CatalogueAnswer.Loading) }
        coroutineScope {
            catalogues.map { catalogue ->
                async {
                    var address: String? = null
                    val answer = try {
                        // A catalogue says where it is searched on its first page.
                        address = engine.catalogueFeed(catalogue.url).search
                        if (address == null) CatalogueAnswer.Failed("This catalogue cannot be searched.") else CatalogueAnswer.Done(engine.catalogueSearch(address, typed).items)
                    } catch (failure: Exception) {
                        CatalogueAnswer.Failed(describe(failure).message)
                    }
                    if (term == typed) groups = groups.map { if (it.url == catalogue.url) CatalogueGroup(it.url, it.name, address, answer) else it }
                }
            }.awaitAll()
        }
    }
}

/** Where books are found: one search across every catalogue the person added, and the catalogues themselves to open. */
@Composable
fun BooksBrowseScreen() {
    val app = LocalApp.current
    val router = LocalRouter.current
    val scope = rememberCoroutineScope()
    var catalogues by remember { mutableStateOf(emptyList<CatalogueInfo>()) }
    var term by remember { mutableStateOf("") }
    val across = remember { BookSearchAcross() }
    var adding by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf<Problem?>(null) }
    fun load() { catalogues = runCatching { app.engine.catalogues() }.getOrDefault(emptyList()) }
    LaunchedEffect(Unit) { load() }

    AkiPage(spacing = 20.dp) {
        item {
            AkiHeader("Browse", subtitle = if (catalogues.isEmpty()) "Find books to read" else if (catalogues.size == 1) "1 catalogue" else "${catalogues.size} catalogues") {
                AkiIconButton("add", "Add a catalogue", tint = Aki.colors.primary) { adding = true }
            }
        }
        if (catalogues.isEmpty()) {
            item {
                AkiEmptyState(
                    "No catalogue yet",
                    "A catalogue is a shop or library of books that shares an OPDS feed, such as a Calibre-Web server or Standard Ebooks. Add one by its address. Akidev does not include or endorse any.",
                    Modifier.heightIn(min = 340.dp), action = AkiAction("Add a catalogue") { adding = true },
                )
            }
        } else {
            item { AkiInput("Search every catalogue", term, { term = it }, icon = "search", onSubmit = { scope.launch { across.run(app.engine, catalogues, term) } }, onClear = { across.clear() }) }
            if (across.groups.isEmpty()) {
                item {
                    AkiSection("Catalogues") {
                        for (catalogue in catalogues) {
                            val remove = AkiMenuItem("Remove", "trash", destructive = true) { runCatching { app.engine.removeCatalogue(catalogue.url) }; load() }
                            AkiTapMenu(listOf(remove), onClick = { router.push(CatalogueRoute(CatalogueRef(catalogue.url, catalogue.name, null))) }) {
                                AkiRow(catalogue.name, subtitle = catalogue.url, icon = "book-solid", showsChevron = true)
                            }
                        }
                    }
                }
            } else results(across)
        }
    }
    if (adding) {
        AkiPrompt(
            "Add a book catalogue", "The address of an OPDS catalogue, for example a Calibre-Web or Standard Ebooks feed.", "example.org/opds", "Add", { adding = false },
            onConfirm = { link ->
                adding = false
                scope.launch {
                    try {
                        app.engine.addCatalogue(link)
                        load()
                    } catch (failure: Exception) {
                        problem = describe(failure)
                    }
                }
            },
        )
    }
    problem?.let { AkiAlert(it.title, it.message) { problem = null } }
}

/** What each catalogue found, as its answer arrives. */
private fun LazyListScope.results(across: BookSearchAcross) {
    items(across.groups.size, key = { across.groups[it].url }) { index ->
        val group = across.groups[index]
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            AkiText(group.name, AkiTextStyle.Heading)
            when (val answer = group.answer) {
                CatalogueAnswer.Loading -> Box(Modifier.fillMaxWidth().heightIn(min = 60.dp), contentAlignment = Alignment.Center) { AkiSpinner() }
                is CatalogueAnswer.Failed -> AkiText(answer.message, AkiTextStyle.Caption, tone = AkiTone.Muted)
                is CatalogueAnswer.Done -> {
                    if (answer.items.isEmpty()) AkiText("Nothing found.", AkiTextStyle.Caption, tone = AkiTone.Muted)
                    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) { for (item in answer.items.take(8)) CatalogueItemRow(item, group.search) }
                }
            }
        }
    }
}
