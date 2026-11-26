package live.akidev.manga

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import live.akidev.manga.books.BookStyle
import live.akidev.manga.books.BookTypesetter
import live.akidev.manga.books.ChapterLayout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.akimanga.BookAlign
import uniffi.akimanga.BookAnchor
import uniffi.akimanga.BookBlock
import uniffi.akimanga.BookChapter
import uniffi.akimanga.BookKind
import uniffi.akimanga.BookRun

/** Sets a made-up chapter in type on a real phone (the text engine only exists there) and checks the pages add up. */
@RunWith(AndroidJUnit4::class)
class BookTypesetterTest {
    private val sentence = "It was a bright cold day in April, and the clocks were striking thirteen. "

    private fun paragraph(start: Int, text: String) = BookBlock(
        start = start.toUInt(), kind = BookKind.Text(listOf(BookRun(text, false, false, false, false, false, false, 0, 1f, null, null))), heading = 0u.toUByte(),
        align = BookAlign.JUSTIFY, left = 0f, firstLine = 0f, spaceBefore = 0f, pageBreakBefore = false,
    )

    private fun chapterOf(paragraphs: Int, sentences: Int = 4): Pair<BookChapter, Int> {
        val blocks = ArrayList<BookBlock>()
        var at = 0
        repeat(paragraphs) {
            val text = sentence.repeat(sentences).trim()
            blocks += paragraph(at, text)
            at += text.length + 1
        }
        return BookChapter(blocks, listOf(BookAnchor("middle", blocks[paragraphs / 2].start)), emptyList(), false) to at
    }

    private fun layout(paragraphs: Int, sentences: Int = 4): ChapterLayout {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val density = context.resources.displayMetrics.density
        val typesetter = BookTypesetter(context, BookStyle(), 360 * density.toInt(), 600 * density.toInt(), density, "en", false, pictureSize = { null }, fonts = null)
        return typesetter.layout(0, chapterOf(paragraphs, sentences).first)
    }

    @Test
    fun aLongChapterBecomesSeveralPagesInOrder() {
        val chapter = layout(60)
        assertTrue(chapter.count > 3)
        assertEquals(0, chapter.start(0))
        for (page in 1 until chapter.count) assertTrue("page $page starts after the one before", chapter.start(page) > chapter.start(page - 1))
    }

    @Test
    fun everyPlaceIsOnThePageThatStartsBeforeIt() {
        val chapter = layout(60)
        for (page in 0 until chapter.count) assertEquals(page, chapter.page(containing = chapter.start(page)))
        assertEquals(chapter.count - 1, chapter.page(containing = chapterOf(60).second - 1))
    }

    @Test
    fun noPieceRunsPastTheFootOfThePage() {
        val chapter = layout(60)
        for (page in chapter.pages) for (piece in page.pieces) assertTrue(piece.top + piece.height <= chapter.height + 1f)
    }

    @Test
    fun anAnchorPointsIntoThePageThatShowsIt() {
        val chapter = layout(60)
        val place = chapter.offset("middle")
        assertNotNull(place)
        val page = chapter.page(containing = place!!)
        assertTrue(page > 0 && page < chapter.count - 1)
    }

    @Test
    fun aWordCanBeFoundAroundAPlace() {
        val chapter = layout(5)
        val word = chapter.word(around = 1)
        assertNotNull(word)
        assertEquals("It", chapter.words(word!!.first, word.last + 1).trim())
    }

    @Test
    fun aStretchOfSeveralLinesIsMarkedAcrossEachLine() {
        val chapter = layout(5)
        // The first paragraph is four sentences long: more than three lines at this width.
        val boxes = chapter.boxes(0, 200, 0)
        assertTrue(boxes.size >= 3)
        // Every line but the last runs to the right edge of the text; none is a sliver.
        for (box in boxes.dropLast(1)) assertTrue("a line's box was only ${box.width()} wide", box.width() > chapter.width * 0.5f)
    }

    @Test
    fun aShortChapterIsOnePage() = assertEquals(1, layout(1, sentences = 1).count)
}
