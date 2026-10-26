package live.akidev.manga.books

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.describe
import uniffi.akimanga.BookInfo
import uniffi.akimanga.CategoryInfo
import java.io.File

/** What the Books screen shows: the books, the shelf chosen, the order, what is picked. It lives as long as the app, so the tab comes back as it was left. */
class BooksModel(private val app: AppModel) {
    val scroll = LazyListState()
    var books by mutableStateOf(emptyList<BookInfo>())
        private set
    var categories by mutableStateOf(emptyList<CategoryInfo>())
        private set
    var loaded by mutableStateOf(false)
        private set

    /** -1 is every category, -2 the books on none. */
    var category by mutableStateOf(-1L)
    var filter by mutableStateOf(BookFilter.All)
    var searching by mutableStateOf(false)
    var words by mutableStateOf("")
    var selecting by mutableStateOf(false)
    var picked by mutableStateOf(emptySet<String>())

    /** What is being added now, for the line under the title: how many are done and how many there are. */
    var adding by mutableStateOf<Pair<Int, Int>?>(null)
        private set

    /** Said when a book could not be added, until it is dismissed. */
    var problem by mutableStateOf<Pair<String, String>?>(null)

    val shown get() = shelf(books, app.prefs.bookSort, app.prefs.bookReversed, filter, category, words)

    suspend fun load() {
        val found = withContext(Dispatchers.Default) { runCatching { app.engine.books() }.getOrDefault(emptyList()) }
        if (found != books) books = found
        categories = runCatching { app.engine.categories() }.getOrDefault(emptyList())
        if ((category >= 0 && categories.none { it.id == category }) || (category == -2L && categories.isEmpty())) category = -1
        loaded = true
    }

    fun toggle(book: BookInfo) {
        picked = if (book.id in picked) picked - book.id else picked + book.id
    }

    fun select() {
        selecting = !selecting
        picked = emptySet()
    }

    /** Adds the files one after another, each off the main thread; a book that cannot be read says why and the rest go on. */
    suspend fun add(files: List<File>) {
        if (files.isEmpty()) return
        val failures = mutableListOf<String>()
        for ((index, file) in files.withIndex()) {
            adding = index to files.size
            val result = withContext(Dispatchers.Default) { runCatching { app.engine.importBook(file.path) } }
            result.exceptionOrNull()?.let { failures += "${file.nameWithoutExtension}: ${describe(it).message}" }
        }
        adding = null
        load()
        if (failures.isNotEmpty()) problem = (if (failures.size == 1) "A book could not be added" else "${failures.size} books could not be added") to failures.joinToString("\n")
    }

    fun finish(books: List<BookInfo>) {
        for (book in books) runCatching { app.engine.finishBook(book.id) }
    }

    fun reset(books: List<BookInfo>) {
        for (book in books) runCatching { app.engine.resetBook(book.id) }
    }

    fun remove(books: List<BookInfo>) {
        for (book in books) runCatching { app.engine.removeBook(book.id) }
    }
}
