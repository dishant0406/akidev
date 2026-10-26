package live.akidev.manga

import live.akidev.manga.manga.chapterName
import live.akidev.manga.updates.NewChapters
import live.akidev.manga.updates.UpdateAlerts
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.akimanga.ChapterEntry

class NamesTest {
    @Test
    fun chapterNameUsesTheTitleThenTheNumber() {
        assertEquals("The Tower", chapterName(3.0, "The Tower"))
        assertEquals("Chapter 12", chapterName(12.0, null))
        assertEquals("Chapter 12.5", chapterName(12.5, ""))
        assertEquals("Chapter", chapterName(null, null))
    }

    private fun chapter(number: Double) = ChapterEntry(id = "c$number", number = number, title = null, dateMs = null, scanlator = null, finished = false, page = null)
    private fun found(title: String, count: Int, adult: Boolean = false) = NewChapters("s", title, title, adult, (1..count).map { chapter(it.toDouble()) })

    @Test
    fun oneSeriesWithOneChapterNamesTheChapter() = assertEquals("Solo · Chapter 1", UpdateAlerts.body(listOf(found("Solo", 1))))

    @Test
    fun oneSeriesWithManyChaptersCountsThem() = assertEquals("Solo · 3 new chapters", UpdateAlerts.body(listOf(found("Solo", 3))))

    @Test
    fun manySeriesAreNamedThenCounted() {
        assertEquals("A and B", UpdateAlerts.body(listOf(found("A", 1), found("B", 1))))
        assertEquals("A, B and 1 more", UpdateAlerts.body(listOf(found("A", 1), found("B", 1), found("C", 1))))
    }

    @Test
    fun adultTitlesStayOffTheLockScreen() = assertEquals("3 new chapters in your library.", UpdateAlerts.body(listOf(found("Secret", 2, adult = true), found("Other", 1))))
}
