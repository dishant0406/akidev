package live.akidev.manga.books

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
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
import live.akidev.manga.app.BookReaderRoute
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.ReaderKind
import live.akidev.manga.app.Sharing
import live.akidev.manga.app.rememberFilesPicker
import live.akidev.manga.app.rememberPhotoPicker
import live.akidev.manga.design.*
import live.akidev.manga.manga.CategoriesSheet
import live.akidev.manga.manga.CategoryPicker
import uniffi.akimanga.BookHighlight
import uniffi.akimanga.BookInfo
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import java.text.NumberFormat

/** The books on this device: add one from Files, pick one up where you left it. */
@Composable
fun BooksScreen() {
    val app = LocalApp.current
    val router = LocalRouter.current
    val books = app.booksScreen
    val scope = rememberCoroutineScope()
    var sheet by remember { mutableStateOf<BooksSheet?>(null) }
    var coverFor by remember { mutableStateOf<BookInfo?>(null) }
    val importer = rememberFilesPicker(arrayOf("application/epub+zip", "*/*")) { books.add(it) }
    val chooseCover = rememberPhotoPicker { jpeg ->
        coverFor?.let { book -> runCatching { app.engine.setBookCustomCover(book.id, jpeg) } }
        scope.launch { books.load() }
    }
    val columns = akiColumns()
    val open = { book: BookInfo -> router.push(BookReaderRoute(book)) }
    val shareBook = { book: BookInfo -> app.sharing = Sharing(book.title) { listOf(app.engine.shareBook(book.id)) } }

    LaunchedEffect(Unit) { books.load() }
    LaunchedEffect(app.addingBook) { if (app.addingBook) { app.addingBook = false; importer() } }

    Box(Modifier.fillMaxSize()) {
        AkiPage(narrow = false, state = books.scroll) {
            item {
                AkiHeader(if (app.prefs.readerKind == ReaderKind.Books) "Library" else "Books", subtitle = subtitle(books)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        AkiMenu("filter", "Sort and filter", sortItems(app, books))
                        AkiMenu("more", "More", moreItems(app, books) { sheet = BooksSheet.Managing })
                        AkiIconButton("add", "Add books", tint = Aki.colors.primary, onClick = importer)
                    }
                }
            }
            if (books.searching) item { AkiInput("Search your books", books.words, { books.words = it }, icon = "search", onClear = { books.words = "" }) }
            if (books.categories.isNotEmpty() && books.books.isNotEmpty()) {
                item {
                    val shelves = listOf(AkiChoice(-1L, "All")) + books.categories.map { AkiChoice(it.id, "${it.name} ${it.books}") } + listOf(AkiChoice(-2L, "No category"))
                    AkiChips(shelves, books.category, { books.category = it })
                }
            }
            if (books.books.isEmpty()) {
                item { AkiEmptyState("No books yet", "Add EPUB files from your device. They stay on this phone until you delete them.", Modifier.heightIn(min = 380.dp), action = AkiAction("Add books", importer), secondary = AkiAction("Receive from a friend") { app.sharing = Sharing(null, null) }) }
            } else {
                val shown = books.shown
                if (shown.isEmpty()) item { AkiEmptyState("Nothing here", if (books.category == -2L) "Every book is in a category." else "No books in this category yet. Press and hold a book to add it to one.", Modifier.heightIn(min = 260.dp)) }
                akiGrid(shown, columns, key = { it.id }) { book ->
                    val menu = if (books.selecting) emptyList() else menuOf(app, books, book, scope, details = { sheet = BooksSheet.Details(book, runCatching { app.engine.bookMarks(book.id) }.getOrDefault(emptyList())) }, share = { shareBook(book) }, sort = { sheet = BooksSheet.Sorting(book) }, cover = { coverFor = book; chooseCover() }, remove = { sheet = BooksSheet.Removing(book) })
                    AkiTapMenu(menu, if (books.selecting) Modifier.semantics { selected = book.id in books.picked } else Modifier, onClick = { if (books.selecting) books.toggle(book) else open(book) }) {
                        Box {
                            AkiBookCard(book.title, book.authors.firstOrNull(), book.cover, progressLine(book))
                            if (books.selecting) Tick(book.id in books.picked, Modifier.align(Alignment.TopStart))
                        }
                    }
                }
            }
        }
        val chosen = books.books.filter { it.id in books.picked }
        if (books.selecting && chosen.isNotEmpty()) {
            // What can be done to the books picked: put them on categories, mark them finished or not started, remove them.
            Row(Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 112.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                AkiIconButton("folder", "Change categories") { sheet = BooksSheet.Assigning(chosen) }
                AkiIconButton("check", "Mark as finished") { books.finish(chosen); books.picked = emptySet(); books.selecting = false; scope.launch { books.load() } }
                AkiIconButton("restore", "Mark as not started") { books.reset(chosen); books.picked = emptySet(); books.selecting = false; scope.launch { books.load() } }
                AkiIconButton("trash", "Delete books", tint = Aki.colors.destructive) { sheet = BooksSheet.RemovingPicked(chosen) }
            }
        }
    }

    when (val current = sheet) {
        BooksSheet.Managing -> CategoriesSheet { sheet = null; scope.launch { books.load() } }
        is BooksSheet.Sorting -> CategoryPicker(emptyList(), books = listOf(current.book), onDismiss = { sheet = null; scope.launch { books.load() } })
        is BooksSheet.Assigning -> CategoryPicker(emptyList(), books = current.books, onDismiss = { sheet = null; books.picked = emptySet(); books.selecting = false; scope.launch { books.load() } })
        is BooksSheet.Details -> {
            val book = current.book
            BookDetailSheet(
                book, current.marks, onDismiss = { sheet = null },
                read = { sheet = null; open(book) },
                finish = { runCatching { app.engine.finishBook(book.id) }; sheet = null; scope.launch { books.load() } },
                reset = { runCatching { app.engine.resetBook(book.id) }; sheet = null; scope.launch { books.load() } },
                share = { sheet = null; shareBook(book) },
                categories = { sheet = BooksSheet.Sorting(book) },
                chooseCover = { sheet = null; coverFor = book; chooseCover() },
                ownCover = { runCatching { app.engine.clearBookCustomCover(book.id) }; sheet = null; scope.launch { books.load() } },
                remove = { sheet = BooksSheet.Removing(book) },
            )
        }
        is BooksSheet.RemovingPicked -> AkiConfirm(if (current.books.size == 1) "Delete 1 book?" else "Delete ${current.books.size} books?", "Their files and your reading progress are deleted from this phone.", if (current.books.size == 1) "Delete book" else "Delete books", { sheet = null }) {
            books.remove(current.books)
            books.picked = emptySet(); books.selecting = false
            scope.launch { books.load() }
        }
        is BooksSheet.Removing -> AkiConfirm("Delete “${current.book.title}”?", "Its file and your reading progress are deleted from this phone.", "Delete book", { sheet = null }) {
            books.remove(listOf(current.book))
            scope.launch { books.load() }
        }
        null -> {}
    }
    books.problem?.let { AkiAlert(it.first, it.second) { books.problem = null } }
}

/** The sheet or dialog over the books: only one is open at a time. */
private sealed interface BooksSheet {
    data object Managing : BooksSheet
    data class Sorting(val book: BookInfo) : BooksSheet
    data class Assigning(val books: List<BookInfo>) : BooksSheet
    data class Details(val book: BookInfo, val marks: List<BookHighlight>) : BooksSheet
    data class Removing(val book: BookInfo) : BooksSheet
    data class RemovingPicked(val books: List<BookInfo>) : BooksSheet
}

private fun subtitle(books: BooksModel): String {
    books.adding?.let { (done, total) -> return "Adding ${done + 1} of $total…" }
    return if (books.books.size == 1) "1 book" else "${books.books.size} books"
}

private fun sortItems(app: live.akidev.manga.app.AppModel, books: BooksModel): List<AkiMenuItem> = buildList {
    for (option in BookSort.entries) add(AkiMenuItem(option.title, "order", isSelected = option == app.prefs.bookSort) { app.prefs.bookSort = option })
    add(AkiMenuItem("Reverse order", "swap", isSelected = app.prefs.bookReversed) { app.prefs.bookReversed = !app.prefs.bookReversed })
    for (option in BookFilter.entries) add(AkiMenuItem(option.title, "filter", isSelected = option == books.filter) { books.filter = option })
}

private fun moreItems(app: live.akidev.manga.app.AppModel, books: BooksModel, manage: () -> Unit): List<AkiMenuItem> = buildList {
    add(AkiMenuItem(if (books.searching) "Hide search" else "Search books", "search") { books.searching = !books.searching; if (!books.searching) books.words = "" })
    add(AkiMenuItem(if (books.selecting) "Done selecting" else "Select books", "check") { books.select() })
    add(AkiMenuItem("Edit categories", "folder", onClick = manage))
    add(AkiMenuItem("Receive from a friend", "share") { app.sharing = Sharing(null, null) })
}

private fun menuOf(
    app: live.akidev.manga.app.AppModel, books: BooksModel, book: BookInfo, scope: kotlinx.coroutines.CoroutineScope,
    details: () -> Unit, share: () -> Unit, sort: () -> Unit, cover: () -> Unit, remove: () -> Unit,
): List<AkiMenuItem> = buildList {
    add(AkiMenuItem("Show details", "list", onClick = details))
    add(AkiMenuItem("Share book", "share", onClick = share))
    add(AkiMenuItem("Change categories", "folder", onClick = sort))
    add(AkiMenuItem("Choose a cover picture", "image", onClick = cover))
    if (book.customCover) add(AkiMenuItem("Use the book’s own cover", "restore") { runCatching { app.engine.clearBookCustomCover(book.id) }; scope.launch { books.load() } })
    add(AkiMenuItem("Delete book", "trash", destructive = true, onClick = remove))
}

fun progressLine(book: BookInfo): String = when {
    book.finished -> "Finished"
    book.openedMs == null -> "New"
    else -> percent(book.progress)
}

/** A share of a book as the phone's language writes it: "45%". */
fun percent(fraction: Double): String = NumberFormat.getPercentInstance().format(fraction)

@Composable
private fun Tick(on: Boolean, modifier: Modifier) {
    Box(modifier.padding(8.dp).background(if (on) Aki.colors.primary else Aki.colors.scrim, CircleShape).padding(6.dp)) {
        AkiIcon(if (on) "check-solid" else "check", size = 22.dp, tone = if (on) AkiTone.OnPrimary else AkiTone.OnMedia)
    }
}
