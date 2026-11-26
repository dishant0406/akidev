package live.akidev.manga.books

import uniffi.akimanga.BookInfo

enum class BookSort(val title: String) { Recent("Last read"), Added("Date added"), Title("Title"), Author("Author"), Progress("Progress") }

enum class BookFilter(val title: String) { All("All books"), Reading("Reading"), NotStarted("Not started"), Finished("Finished") }

/**
 * What is on the shelf: the books on the category chosen (-1 is every one, -2 the ones on none), those that fit the filter and the words
 * typed, in the order asked for. Ties fall back to the title so the order never reshuffles on its own.
 */
fun shelf(books: List<BookInfo>, sort: BookSort, reversed: Boolean, filter: BookFilter, category: Long, words: String): List<BookInfo> {
    val typed = words.trim()
    val kept = books.filter { book ->
        when (category) {
            -1L -> true
            -2L -> book.categories.isEmpty()
            else -> category in book.categories
        }
    }.filter { book ->
        when (filter) {
            BookFilter.All -> true
            BookFilter.Reading -> book.openedMs != null && !book.finished
            BookFilter.NotStarted -> book.openedMs == null
            BookFilter.Finished -> book.finished
        }
    }.filter { book -> typed.isEmpty() || book.title.contains(typed, ignoreCase = true) || book.authors.any { it.contains(typed, ignoreCase = true) } }
    val byTitle = compareBy<BookInfo, String>(String.CASE_INSENSITIVE_ORDER) { it.title }
    fun biggestFirst(value: (BookInfo) -> Double) = kept.sortedWith(compareByDescending(value).then(byTitle))
    val ordered = when (sort) {
        BookSort.Recent -> biggestFirst { (it.openedMs ?: it.addedMs).toDouble() }
        BookSort.Added -> biggestFirst { it.addedMs.toDouble() }
        BookSort.Title -> kept.sortedWith(byTitle)
        BookSort.Author -> kept.sortedWith(compareBy<BookInfo, String>(String.CASE_INSENSITIVE_ORDER) { it.authors.firstOrNull() ?: "￿" })
        BookSort.Progress -> biggestFirst { if (it.finished) 1.0 else it.progress }
    }
    return if (reversed) ordered.reversed() else ordered
}
