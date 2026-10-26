package live.akidev.manga.books

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import live.akidev.manga.app.AppTab
import live.akidev.manga.app.BookReaderRoute
import live.akidev.manga.app.CatalogueRef
import live.akidev.manga.app.CatalogueRoute
import live.akidev.manga.app.HighlightsRoute
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.design.*
import uniffi.akimanga.BookHighlightIn
import uniffi.akimanga.BookInfo
import uniffi.akimanga.CatalogueInfo
import uniffi.akimanga.CategoryInfo

/** The first tab for someone who mostly reads books: where they left off, what they have not started, their shelves and the catalogues they can get more from. */
@Composable
fun BooksHomeScreen() {
    val app = LocalApp.current
    val router = LocalRouter.current
    var books by remember { mutableStateOf(emptyList<BookInfo>()) }
    var categories by remember { mutableStateOf(emptyList<CategoryInfo>()) }
    var catalogues by remember { mutableStateOf(emptyList<CatalogueInfo>()) }
    var highlights by remember { mutableStateOf(emptyList<BookHighlightIn>()) }
    var loaded by remember { mutableStateOf(false) }
    val reading = { book: BookInfo -> router.push(BookReaderRoute(book)) }

    LaunchedEffect(router.top) {
        if (router.top != null) return@LaunchedEffect
        withContext(Dispatchers.Default) {
            books = runCatching { app.engine.books() }.getOrDefault(emptyList())
            categories = runCatching { app.engine.categories() }.getOrDefault(emptyList())
            catalogues = runCatching { app.engine.catalogues() }.getOrDefault(emptyList())
            highlights = runCatching { app.engine.recentHighlights(3u) }.getOrDefault(emptyList())
        }
        loaded = true
    }

    /** Opens the book where the words are: its place is moved there first, then it is read as usual. */
    fun open(found: BookHighlightIn) {
        val book = books.firstOrNull { it.id == found.book } ?: return
        runCatching { app.engine.saveBookPosition(book.id, found.highlight.chapter, found.highlight.start, book.progress, book.finished) }
        reading(runCatching { app.engine.bookInfo(book.id) }.getOrDefault(book))
    }

    // The book opened last that is not finished.
    val current = books.filter { !it.finished && it.openedMs != null }.maxByOrNull { it.openedMs ?: 0L }
    val unread = books.filter { it.openedMs == null }.sortedByDescending { it.addedMs }

    AkiPage(narrow = false, spacing = 26.dp) {
        item {
            AkiHeader("Home", subtitle = if (books.isEmpty()) "Your reading" else if (books.size == 1) "1 book" else "${books.size} books") {
                AkiIconButton("add", "Add books", tint = Aki.colors.primary) { app.tab = AppTab.Books; app.addingBook = true }
            }
        }
        if (loaded && books.isEmpty()) {
            item {
                AkiEmptyState(
                    "Your shelf is empty", "Add EPUB files from your device, or get books from a catalogue.", Modifier.heightIn(min = 340.dp),
                    action = AkiAction("Add books") { app.tab = AppTab.Books; app.addingBook = true }, secondary = AkiAction("Find a catalogue") { app.tab = AppTab.Browse },
                )
            }
        }
        current?.let { book -> item { ContinueCard(book) { reading(book) } } }
        if (unread.isNotEmpty()) {
            item {
                AkiShelf("Not started", subtitle = "Added, never opened") {
                    items(unread.take(12).size, key = { unread[it].id }) { index ->
                        val book = unread[index]
                        AkiTap({ reading(book) }) { AkiPoster(book.title, book.cover, caption = book.authors.firstOrNull()) }
                    }
                }
            }
        }
        for (shelf in categories.filter { shelf -> books.any { shelf.id in it.categories } }) {
            item(key = "shelf-${shelf.id}") {
                val onShelf = books.filter { shelf.id in it.categories }
                AkiShelf(shelf.name) {
                    items(onShelf.size, key = { onShelf[it].id }) { index ->
                        val book = onShelf[index]
                        AkiTap({ reading(book) }) { AkiPoster(book.title, book.cover, caption = progressLine(book)) }
                    }
                }
            }
        }
        if (highlights.isNotEmpty()) {
            item {
                AkiSection("Recent highlights") {
                    for (found in highlights.take(3)) AkiTap({ open(found) }) { HighlightCard(found) }
                    AkiButton("All highlights", icon = "forward", fullWidth = true) { router.push(HighlightsRoute { open(it) }) }
                }
            }
        }
        item {
            AkiSection("Catalogues", footer = "Get books from a catalogue you add (an OPDS feed). Akidev does not include or endorse any.") {
                for (catalogue in catalogues) {
                    AkiTap({ router.push(CatalogueRoute(CatalogueRef(catalogue.url, catalogue.name, null))) }) { AkiRow(catalogue.name, subtitle = catalogue.url, icon = "book-solid", showsChevron = true) }
                }
                AkiButton(if (catalogues.isEmpty()) "Add a book catalogue" else "Manage catalogues", icon = "add", fullWidth = true) { app.tab = AppTab.Browse }
            }
        }
    }
}

@Composable
private fun ContinueCard(book: BookInfo, onClick: () -> Unit) {
    AkiTap(onClick) {
        AkiCard(padding = 14.dp) {
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                AkiImage(book.cover, Modifier.width(84.dp).clip(RoundedCornerShape(Aki.thumbRadius)), placeholder = 1.5f, fill = true)
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    AkiText("Continue reading", AkiTextStyle.Eyebrow, tone = AkiTone.Muted)
                    AkiText(book.title, AkiTextStyle.Heading, lines = 2)
                    book.authors.firstOrNull()?.let { AkiText(it, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1) }
                    AkiProgressBar(book.progress)
                    AkiText("${Math.round(book.progress * 100)}%", AkiTextStyle.Caption, tone = AkiTone.Muted)
                }
            }
        }
    }
}

@Composable
private fun HighlightCard(found: BookHighlightIn) {
    AkiCard(padding = 14.dp) {
        Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            AkiText("“${found.highlight.quote}”", lines = 3)
            found.highlight.note?.takeIf { it.isNotEmpty() }?.let { AkiText(it, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 2) }
            AkiText(found.title, AkiTextStyle.CaptionStrong, tone = AkiTone.Muted, lines = 1)
        }
    }
}
