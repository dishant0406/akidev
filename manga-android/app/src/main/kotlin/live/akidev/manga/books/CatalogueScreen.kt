package live.akidev.manga.books

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
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
import kotlinx.coroutines.launch
import live.akidev.manga.app.CatalogueRef
import live.akidev.manga.app.CatalogueRoute
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.Problem
import live.akidev.manga.app.describe
import live.akidev.manga.design.*
import uniffi.akimanga.CatalogueFeed
import uniffi.akimanga.CatalogueItem

/** A catalogue of books: shelves to open and books to get. Getting one downloads it onto the shelf in the Books tab. */
@Composable
fun CatalogueScreen(ref: CatalogueRef) {
    val app = LocalApp.current
    val router = LocalRouter.current
    val scope = rememberCoroutineScope()
    var feed by remember { mutableStateOf<CatalogueFeed?>(null) }
    var items by remember { mutableStateOf(emptyList<CatalogueItem>()) }
    var next by remember { mutableStateOf<String?>(null) }
    var term by remember { mutableStateOf("") }
    var searched by remember { mutableStateOf<String?>(null) }
    var failure by remember { mutableStateOf<Problem?>(null) }
    var loadingMore by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf<Problem?>(null) }
    val search = feed?.search ?: ref.search

    suspend fun run() {
        val words = term.trim()
        if (words.isEmpty() || search == null) return
        failure = null
        feed = null
        searched = words
        try {
            val found = app.engine.catalogueSearch(search, words)
            feed = found
            items = found.items
            next = found.next
        } catch (error: Exception) {
            feed = CatalogueFeed("", emptyList(), null, search)
            items = emptyList()
            failure = describe(error)
        }
    }

    suspend fun load(url: String?) {
        failure = null
        if (url == null) return run()
        try {
            val found = app.engine.catalogueFeed(url)
            feed = found
            items = found.items
            next = found.next
        } catch (error: Exception) {
            failure = describe(error)
        }
    }

    LaunchedEffect(Unit) { if (feed == null) load(ref.url) }

    AkiPage(underTabBar = false, spacing = 16.dp) {
        item { AkiBackHeader(searched?.let { "“$it”" } ?: ref.name, back = router::back, subtitle = if (feed == null) null else "${items.size} shown") }
        if (search != null) item { AkiInput("Search this catalogue", term, { term = it }, icon = "search", onSubmit = { scope.launch { run() } }, onClear = { searched = null; scope.launch { load(ref.url) } }) }
        val error = failure
        if (error != null) item { AkiEmptyState(error.title, error.message, Modifier.heightIn(min = 260.dp), action = AkiAction("Try again") { scope.launch { load(if (searched == null) ref.url else null) } }) }
        else if (feed == null) item { Box(Modifier.fillMaxWidth().heightIn(min = 200.dp), contentAlignment = Alignment.Center) { AkiSpinner() } }
        else if (items.isEmpty()) item { AkiEmptyState("Nothing here", "This page of the catalogue has no books.", Modifier.heightIn(min = 240.dp)) }
        items(items.size) { index -> CatalogueItemRow(items[index], search) }
        next?.let { address ->
            item {
                if (loadingMore) Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { AkiSpinner() }
                else AkiButton("More", fullWidth = true) {
                    scope.launch {
                        loadingMore = true
                        try {
                            val found = app.engine.catalogueFeed(address)
                            items = items + found.items
                            next = found.next
                        } catch (failure: Exception) {
                            problem = describe(failure)
                        }
                        loadingMore = false
                    }
                }
            }
        }
    }
    problem?.let { AkiAlert(it.title, it.message) { problem = null } }
}

/** One line of a catalogue: a book with a Get button, or a shelf that opens. Getting a book puts it on the shelf in the Books tab. */
@Composable
fun CatalogueItemRow(item: CatalogueItem, search: String?) {
    val app = LocalApp.current
    val router = LocalRouter.current
    val scope = rememberCoroutineScope()
    var getting by remember { mutableStateOf(false) }
    var got by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf<Problem?>(null) }
    val get = item.get
    val open = item.open
    if (get != null) {
        AkiListCard(item.cover, item.author ?: "", item.title) {
            when {
                got -> AkiBadge("On your shelf", icon = "tick")
                getting -> AkiSpinner()
                else -> AkiButton("Get", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Small) {
                    scope.launch {
                        getting = true
                        try {
                            app.engine.getBook(get)
                            got = true
                        } catch (failure: Exception) {
                            problem = describe(failure)
                        }
                        getting = false
                    }
                }
            }
        }
    } else if (open != null) {
        AkiTap({ router.push(CatalogueRoute(CatalogueRef(open, item.title, search))) }) {
            AkiListCard(item.cover, item.author ?: "Open", item.title) { AkiIcon("forward", size = 13.dp, tone = AkiTone.Muted) }
        }
    }
    problem?.let { AkiAlert(it.title, it.message) { problem = null } }
}
