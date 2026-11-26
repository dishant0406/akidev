package live.akidev.manga

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.text.Spannable
import android.text.Spanned
import android.text.style.ForegroundColorSpan
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import live.akidev.manga.books.BookOverlays
import live.akidev.manga.books.BookStyle
import live.akidev.manga.books.BookTypesetter
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.akimanga.BookAlign
import uniffi.akimanga.BookBlock
import uniffi.akimanga.BookChapter
import uniffi.akimanga.BookKind
import uniffi.akimanga.BookRun

/** On a justified page the highlight box of a word, and a tap on it, must be where the word is drawn (the layout alone answers as if the lines were not stretched). */
@RunWith(AndroidJUnit4::class)
class JustifiedHighlightTest {
    private val text = "It was a bright cold day in April, and the clocks were striking thirteen. Winston Smith, his chin nuzzled into his breast in an effort to escape the vile wind, slipped quickly through the glass doors of Victory Mansions, though not quickly enough to prevent a swirl of gritty dust from entering along with him."

    @Test
    fun aWordIsHighlightedAndTappedWhereItIsDrawn() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val density = context.resources.displayMetrics.density
        val block = BookBlock(
            start = 0u, kind = BookKind.Text(listOf(BookRun(text, false, false, false, false, false, false, 0, 1f, null, null))), heading = 0u.toUByte(),
            align = BookAlign.JUSTIFY, left = 0f, firstLine = 0f, spaceBefore = 0f, pageBreakBefore = false,
        )
        val width = (360 * density).toInt()
        val height = (600 * density).toInt()
        val chapter = BookTypesetter(context, BookStyle(), width, height, density, "en", false, pictureSize = { null }, fonts = null)
            .layout(0, BookChapter(listOf(block), emptyList(), emptyList(), false))
        val laid = chapter.blocks.single()
        var measured = 0
        for (word in Regex("\\S+").findAll(text)) {
            val (from, to) = word.range.first to word.range.last + 1
            val boxes = chapter.boxes(from, to, 0)
            // A word split over two lines has two boxes: only whole words are measured.
            if (boxes.size != 1) continue
            // The word is drawn in red among black text, and where the red is on the page is where the word is.
            (laid.text as Spannable).setSpan(ForegroundColorSpan(Color.RED), from, to, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            val page = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
            val canvas = Canvas(page).apply { drawColor(Color.WHITE) }
            chapter.draw(canvas, 0, Color.BLACK, Color.GRAY, BookOverlays()) { _, _ -> null }
            (laid.text as Spannable).removeSpan((laid.text as Spanned).getSpans(from, to, ForegroundColorSpan::class.java).single())
            val box = boxes.single()
            var left = Int.MAX_VALUE
            var right = -1
            for (y in box.top.toInt() until box.bottom.toInt()) for (x in 0 until width) {
                val pixel = page.getPixel(x, y)
                if (Color.red(pixel) > 150 && Color.green(pixel) < 90 && Color.blue(pixel) < 90) { left = minOf(left, x); right = maxOf(right, x + 1) }
            }
            assertTrue("'${word.value}' is drawn", right > 0)
            val slack = 6 * density
            assertTrue("'${word.value}' starts at $left, its box at ${box.left}", kotlin.math.abs(box.left - left) <= slack)
            assertTrue("'${word.value}' ends at $right, its box at ${box.right}", kotlin.math.abs(box.right - right) <= slack)
            // A tap in the middle of the word lands on the word.
            val at = chapter.position((left + right) / 2f, (box.top + box.bottom) / 2, 0)!!
            val tapped = chapter.word(at)
            // Punctuation is not part of a word.
            assertTrue("a tap on '${word.value}' lands at $at, the word $tapped", tapped != null && tapped.first >= from && tapped.last < to)
            measured += 1
        }
        assertTrue("$measured words measured", measured > 30)
        assertEquals(1, chapter.count)
    }
}
