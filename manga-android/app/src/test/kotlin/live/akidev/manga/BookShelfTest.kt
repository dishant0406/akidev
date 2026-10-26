package live.akidev.manga

import live.akidev.manga.books.BookFilter
import live.akidev.manga.books.BookSort
import live.akidev.manga.books.shelf
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.akimanga.BookInfo

class BookShelfTest {
    private fun book(title: String, authors: List<String> = emptyList(), added: Long = 0, opened: Long? = null, progress: Double = 0.0, finished: Boolean = false, categories: List<Long> = emptyList()) =
        BookInfo(
            id = title, title = title, authors = authors, language = null, rtl = false, bytes = 0uL, cover = null, customCover = false, categories = categories,
            addedMs = added, openedMs = opened, chapter = 0u, offset = 0u, progress = progress, finished = finished,
        )

    private val books = listOf(
        book("Dune", listOf("Herbert"), added = 1, opened = 50, progress = 0.5),
        book("alice", listOf("Carroll"), added = 3, categories = listOf(7)),
        book("Emma", listOf("Austen"), added = 2, opened = 90, progress = 1.0, finished = true),
    )

    private fun titles(sort: BookSort = BookSort.Title, reversed: Boolean = false, filter: BookFilter = BookFilter.All, category: Long = -1, words: String = "") =
        shelf(books, sort, reversed, filter, category, words).map { it.title }

    @Test
    fun sortsByTitleIgnoringCase() = assertEquals(listOf("alice", "Dune", "Emma"), titles())

    @Test
    fun recentPutsTheLastOpenedFirstAndUnopenedByDateAdded() = assertEquals(listOf("Emma", "Dune", "alice"), titles(BookSort.Recent))

    @Test
    fun reversedTurnsTheOrderRound() = assertEquals(listOf("Emma", "Dune", "alice"), titles(reversed = true))

    @Test
    fun filtersByReadingState() {
        assertEquals(listOf("Dune"), titles(filter = BookFilter.Reading))
        assertEquals(listOf("alice"), titles(filter = BookFilter.NotStarted))
        assertEquals(listOf("Emma"), titles(filter = BookFilter.Finished))
    }

    @Test
    fun filtersByCategory() {
        assertEquals(listOf("alice"), titles(category = 7))
        assertEquals(listOf("Dune", "Emma"), titles(category = -2))
    }

    @Test
    fun searchesTitleAndAuthor() {
        assertEquals(listOf("Emma"), titles(words = " aust "))
        assertEquals(listOf("Dune"), titles(words = "DUN"))
    }

    @Test
    fun authorSortPutsBooksWithoutOneLast() {
        val shelf = shelf(books + book("Anon"), BookSort.Author, false, BookFilter.All, -1, "")
        assertEquals("Anon", shelf.last().title)
        assertEquals("Emma", shelf.first().title)
    }
}
