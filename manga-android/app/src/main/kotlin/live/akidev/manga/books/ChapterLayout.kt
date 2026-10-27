package live.akidev.manga.books

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.PorterDuffColorFilter
import android.graphics.RectF
import androidx.core.graphics.withTranslation
import androidx.compose.ui.graphics.toArgb
import android.text.Spanned
import android.text.StaticLayout
import android.text.style.CharacterStyle

/** A stretch of a chapter's text, in the book's own positions. */
data class BookSpan(val chapter: Int, val start: Int, val end: Int)

/** Where a run of text links to, as the book wrote it; `raised` marks a footnote's number. It draws nothing of its own. */
class LinkSpan(val link: String, val raised: Boolean) : CharacterStyle() {
    override fun updateDrawState(paint: android.text.TextPaint) = Unit
}

/** A picture in the text: `ink` pictures are dark on clear, drawn in the colour of the text so they show on a dark page too. */
class PictureBox(val path: String, val ink: Boolean, val width: Float, val height: Float)

/**
 * One block of a chapter set in type: a paragraph (its `layout`), a picture or a rule. `start` and `length` are in the book's own
 * positions; the text is laid out character for character, so a place in the layout is a place in the book.
 */
class LaidBlock(
    val start: Int,
    val length: Int,
    val layout: StaticLayout?,
    val picture: PictureBox?,
    val spaceBefore: Float,
    val heading: Boolean,
    val pageBreakBefore: Boolean,
    val text: CharSequence,
    /** The height of a picture or rule; a paragraph has its layout's. */
    val height: Float,
    /** Its lines but the last are stretched to the full width: the layout draws them so, but answers every question as if they were not. */
    val justified: Boolean = false,
)

/** What a page shows of one block: lines `firstLine` until `endLine` of its layout (a picture has one), at `top` on the page. */
class Piece(val block: Int, val firstLine: Int, val endLine: Int, val top: Float, val height: Float)

class Page(val pieces: List<Piece>, val start: Int, val extent: Float)

/** What is drawn over the text of a page besides the text: highlights, the selection and the place a search found. */
class BookMarkSpan(val id: Long, val span: BookSpan, val colour: Int)

class BookOverlays(
    val marks: List<BookMarkSpan> = emptyList(),
    val selection: BookSpan? = null,
    val found: BookSpan? = null,
    /** The sentence being read aloud. */
    val speaking: BookSpan? = null,
)

/**
 * One chapter set in type for one page size: its blocks, and where each page of it begins and ends. A page is a list of pieces of
 * blocks. Everything asked of it (the page of a place, the boxes of a stretch of words, the word under a finger) is answered in the
 * book's own positions.
 */
class ChapterLayout(
    val index: Int,
    val blocks: List<LaidBlock>,
    /** The text area of a page. */
    val width: Float,
    val height: Float,
    /** A chapter that is one picture and nothing else (a cover): its path in the book. The page shows it over the whole screen. */
    val coverPicture: String?,
    private val anchors: Map<String, Int>,
    private val totalLength: Int,
) {
    val pages: List<Page> = paginate()
    val count get() = pages.size

    // ---- positions -------------------------------------------------------------------------------------------------------------

    /** The page showing a place of the book. */
    fun page(containing: Int): Int {
        var low = 0
        var high = pages.size
        while (low < high) {
            val middle = (low + high) / 2
            if (pages[middle].start <= containing) low = middle + 1 else high = middle
        }
        return maxOf(low - 1, 0)
    }

    fun start(page: Int) = pages[page.coerceIn(0, pages.lastIndex)].start

    fun offset(anchor: String): Int? = anchors[anchor]

    /** The first place after what the page shows. */
    private fun end(page: Int) = if (page + 1 < pages.size) pages[page + 1].start else totalLength

    // ---- scrolling -------------------------------------------------------------------------------------------------------------

    /** How tall a page's text really is: pages are cut at the foot of the text area, a scrolled book stacks them, and what is empty under the last line is not kept. */
    fun extent(page: Int) = pages[page].extent

    /** The space a paragraph asks for above it, which a page leaves out at its top: put back, a scrolled book keeps it. */
    fun leading(page: Int): Float {
        val first = pages[page].pieces.firstOrNull() ?: return 0f
        return if (page > 0 && first.firstLine == 0) blocks[first.block].spaceBefore else 0f
    }

    // ---- asking ----------------------------------------------------------------------------------------------------------------

    /** The boxes a stretch of the book's text takes on a page, one for each line it touches, in the text area with y down. */
    fun boxes(from: Int, to: Int, page: Int): List<RectF> {
        val found = mutableListOf<RectF>()
        for (piece in pages[page].pieces) {
            val block = blocks[piece.block]
            val layout = block.layout ?: continue
            val top = layout.getLineTop(piece.firstLine)
            for (line in piece.firstLine until piece.endLine) {
                val low = maxOf(from, block.start + layout.getLineStart(line))
                val high = minOf(to, block.start + layout.getLineEnd(line))
                if (low >= high) continue
                val one = horizontal(block, line, low - block.start)
                // The end of a line that wraps is the start of the next one, so asking where it is would answer with the left margin: the line's own right edge is used.
                val other = if (high - block.start >= layout.getLineEnd(line) && line < layout.lineCount - 1) lineRight(block, line) else horizontal(block, line, high - block.start)
                found += RectF(minOf(one, other), piece.top + layout.getLineTop(line) - top, maxOf(one, other).coerceAtLeast(minOf(one, other) + 2f), piece.top + layout.getLineBottom(line) - top)
            }
        }
        return found
    }

    /** Whether the layout stretched this line: a justified paragraph's lines, but its last and any that ends in a line break. */
    private fun stretched(block: LaidBlock, line: Int): Boolean {
        val layout = block.layout ?: return false
        return block.justified && line < layout.lineCount - 1 && block.text[layout.getLineEnd(line) - 1] != '\n'
    }

    /** The end of a line's text without the spaces after it. */
    private fun trimmedEnd(block: LaidBlock, line: Int): Int {
        val layout = block.layout!!
        var end = layout.getLineEnd(line)
        while (end > layout.getLineStart(line) && block.text[end - 1].isWhitespace()) end -= 1
        return end
    }

    /**
     * Where a place in a block is on its line, as drawn. The layout answers with the unstretched place, so on a justified line the room
     * the line has to spare (shared equally by its spaces) is put back for each space before the place.
     */
    private fun horizontal(block: LaidBlock, line: Int, at: Int): Float {
        val layout = block.layout!!
        val x = layout.getPrimaryHorizontal(at)
        if (!stretched(block, line)) return x
        val first = layout.getLineStart(line)
        val end = trimmedEnd(block, line)
        var spaces = 0
        for (index in first until end) if (block.text[index] == ' ') spaces += 1
        if (spaces == 0) return x
        // A line cut in the middle of a word ends with a hyphen that is drawn but is not in the text.
        val cut = end < block.text.length && block.text[end - 1].isLetter() && block.text[end].isLetter()
        val natural = layout.getPrimaryHorizontal(end) + if (cut) layout.paint.measureText("-") else 0f
        val before = (first until minOf(at, end)).count { block.text[it] == ' ' }
        return x + (layout.width - natural) / spaces * before
    }

    /** The right edge of a line as drawn: the full width when it is stretched. */
    private fun lineRight(block: LaidBlock, line: Int) = if (stretched(block, line)) block.layout!!.width.toFloat() else block.layout!!.getLineRight(line)

    /** The place on a line nearest to x, as drawn. */
    private fun offsetAt(block: LaidBlock, line: Int, x: Float): Int {
        val layout = block.layout!!
        if (!stretched(block, line)) return layout.getOffsetForHorizontal(line, x)
        return (layout.getLineStart(line)..trimmedEnd(block, line)).minBy { kotlin.math.abs(horizontal(block, line, it) - x) }
    }

    /** Where in the book's text a point (in the text area, y down) is: the nearest letter, on the nearest line. */
    fun position(x: Float, y: Float, page: Int, end: Boolean = false): Int? {
        val shown = pages[page]
        val piece = shown.pieces.minByOrNull { distance(y, it.top, it.top + it.height) } ?: return null
        val block = blocks[piece.block]
        val layout = block.layout
        val at = if (layout == null) block.start else {
            val line = layout.getLineForVertical((y - piece.top + layout.getLineTop(piece.firstLine)).toInt()).coerceIn(piece.firstLine, piece.endLine - 1)
            block.start + offsetAt(block, line, x)
        }
        return at.coerceIn(shown.start, if (end) end(page) else maxOf(end(page) - 1, shown.start))
    }

    private fun distance(y: Float, top: Float, bottom: Float) = if (y < top) top - y else if (y > bottom) y - bottom else 0f

    /** The word around a position, as a stretch of the book's text. */
    fun word(around: Int): IntRange? {
        val block = blocks.lastOrNull { it.start <= around } ?: return null
        val text = block.text
        var low = around - block.start
        var high = low
        if (low >= text.length) return null
        fun inWord(at: Int) = text[at].isLetterOrDigit() || text[at] == '­' || text[at] == '’' || text[at] == '\''
        // A point between a word and the space after it belongs to the word.
        if (!inWord(low) && low > 0 && inWord(low - 1)) {
            low -= 1
            high = low
        }
        if (!inWord(low)) return null
        while (low > 0 && inWord(low - 1)) low -= 1
        while (high < text.length && inWord(high)) high += 1
        return (block.start + low) until (block.start + high)
    }

    /** The words of a stretch of the book's text, as a person would read them. */
    fun words(from: Int, to: Int): String {
        val parts = StringBuilder()
        for (block in blocks) {
            val low = maxOf(from, block.start)
            val high = minOf(to, block.start + block.text.length)
            if (low >= high) continue
            if (parts.isNotEmpty()) parts.append(' ')
            parts.append(block.text.subSequence(low - block.start, high - block.start))
        }
        return parts.toString().replace("­", "").replace(' ', ' ').replace('\n', ' ')
    }

    /** What a tap at a point (in the text area, y down) lands on: the link under it, if any. `raised` is a note's number. */
    fun link(x: Float, y: Float, page: Int): LinkSpan? {
        for (piece in pages[page].pieces) {
            val block = blocks[piece.block]
            val layout = block.layout ?: continue
            val top = layout.getLineTop(piece.firstLine)
            for (line in piece.firstLine until piece.endLine) {
                val lineTop = piece.top + layout.getLineTop(line) - top
                if (y < lineTop - 4 || y > piece.top + layout.getLineBottom(line) - top + 4) continue
                if (x < layout.getLineLeft(line) || x > lineRight(block, line)) return null
                val at = offsetAt(block, line, x)
                val spans = (block.text as? Spanned)?.getSpans(at, minOf(at + 1, block.text.length), LinkSpan::class.java)
                return spans?.firstOrNull()
            }
        }
        return null
    }

    // ---- drawing ---------------------------------------------------------------------------------------------------------------

    /** Draws a page into a canvas whose origin is the top left of the text area. Highlights go under the text. */
    fun draw(canvas: Canvas, page: Int, ink: Int, quiet: Int, overlays: BookOverlays, picture: (String, Float) -> Bitmap?) {
        val paint = Paint()
        val pageWidth = width
        fun fill(span: BookSpan, colour: Int) {
            if (span.chapter != index) return
            paint.color = colour
            for (box in boxes(span.start, span.end, page)) canvas.drawRect(box.left - 1, box.top + 1, box.right + 1, box.bottom - 1, paint)
        }
        for (mark in overlays.marks) fill(mark.span, HighlightColour.fill(mark.colour).toArgb())
        overlays.found?.let { fill(it, HighlightColour.fill(0).toArgb()) }
        overlays.speaking?.let { fill(it, HighlightColour.fill(0).toArgb()) }
        overlays.selection?.let { fill(it, HighlightColour.selection.copy(alpha = 0.28f).toArgb()) }
        for (piece in pages[page].pieces) {
            val block = blocks[piece.block]
            val layout = block.layout
            val shown = block.picture
            canvas.withTranslation(0f, piece.top) {
                when {
                    layout != null -> {
                        clipRect(0f, 0f, pageWidth, piece.height)
                        translate(0f, -layout.getLineTop(piece.firstLine).toFloat())
                        layout.paint.color = ink
                        layout.draw(this)
                    }
                    shown != null -> drawPicture(this, shown, ink, picture)
                    else -> {
                        paint.color = quiet
                        drawRect(0f, piece.height / 2, pageWidth, piece.height / 2 + 1f, paint)
                    }
                }
            }
        }
    }

    private fun drawPicture(canvas: Canvas, box: PictureBox, ink: Int, picture: (String, Float) -> Bitmap?) {
        val bitmap = picture(box.path, box.width) ?: return
        val area = RectF((width - box.width) / 2, 0f, (width + box.width) / 2, box.height)
        val paint = Paint(Paint.FILTER_BITMAP_FLAG)
        if (box.ink) paint.colorFilter = PorterDuffColorFilter(ink, PorterDuff.Mode.SRC_IN)
        canvas.drawBitmap(bitmap, null, area, paint)
    }

    // ---- pagination ------------------------------------------------------------------------------------------------------------

    private fun paginate(): List<Page> {
        val pages = mutableListOf<Page>()
        var current = mutableListOf<Piece>()
        var y = 0f

        fun startOf(piece: Piece): Int {
            val block = blocks[piece.block]
            return block.start + (block.layout?.getLineStart(piece.firstLine) ?: 0)
        }

        fun flush() {
            if (current.isEmpty()) return
            pages += Page(current.toList(), startOf(current.first()), y)
            current = mutableListOf()
            y = 0f
        }

        /** Ends the page; a heading that would be left at its foot, with its text on the next page, goes over to the next with it. */
        fun breakPage() {
            val last = current.lastOrNull()
            if (last != null && current.size > 1 && blocks[last.block].heading && last.endLine == (blocks[last.block].layout?.lineCount ?: 1)) {
                current.removeAt(current.lastIndex)
                y = current.last().let { it.top + it.height }
                flush()
                current.add(Piece(last.block, last.firstLine, last.endLine, 0f, last.height))
                y = last.height
                return
            }
            flush()
        }

        for ((index, block) in blocks.withIndex()) {
            if (block.pageBreakBefore && current.isNotEmpty()) flush()
            val layout = block.layout
            if (layout == null) {
                val gap = if (current.isEmpty()) 0f else block.spaceBefore
                if (current.isNotEmpty() && y + gap + block.height > height) breakPage()
                val top = if (current.isEmpty()) 0f else y + block.spaceBefore
                current.add(Piece(index, 0, 1, top, block.height))
                y = top + block.height
                continue
            }
            var line = 0
            while (line < layout.lineCount) {
                val gap = if (current.isEmpty() || line > 0) 0f else block.spaceBefore
                val from = layout.getLineTop(line)
                var end = line
                while (end < layout.lineCount && y + gap + (layout.getLineBottom(end) - from) <= height) end += 1
                if (end == line) {
                    // Not even one line fits what is left: the page ends, unless it is empty (a line taller than a page goes on one).
                    if (current.isNotEmpty()) {
                        breakPage()
                        continue
                    }
                    end = line + 1
                }
                val shown = (layout.getLineBottom(end - 1) - from).toFloat()
                current.add(Piece(index, line, end, y + gap, shown))
                y += gap + shown
                line = end
                if (line < layout.lineCount) breakPage()
            }
        }
        flush()
        return pages.ifEmpty { listOf(Page(emptyList(), 0, 0f)) }
    }
}
