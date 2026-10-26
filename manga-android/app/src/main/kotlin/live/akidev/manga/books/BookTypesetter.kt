package live.akidev.manga.books

import android.content.Context
import android.graphics.Typeface
import android.graphics.fonts.Font
import android.graphics.fonts.FontFamily
import android.os.Build
import android.text.Layout
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.StaticLayout
import android.text.TextDirectionHeuristics
import android.text.TextPaint
import android.text.style.AbsoluteSizeSpan
import android.text.style.LeadingMarginSpan
import android.text.style.MetricAffectingSpan
import android.text.style.StrikethroughSpan
import android.text.style.TypefaceSpan
import android.text.style.UnderlineSpan
import java.nio.ByteBuffer
import java.util.Locale
import uniffi.akimanga.BookAlign
import uniffi.akimanga.BookBlock
import uniffi.akimanga.BookChapter
import uniffi.akimanga.BookFace
import uniffi.akimanga.BookKind
import uniffi.akimanga.BookRun

/** The fonts a book carries, read from its files. A font the phone cannot read is left out and the reader's own font is used for that text. */
class BookFontBank {
    private class Entry(val typeface: Typeface, val bold: Boolean, val italic: Boolean)

    private val entries = HashMap<String, MutableList<Entry>>()
    private val tried = HashSet<String>()

    /** Whether this font file has been looked at already, so each is read once. */
    @Synchronized
    fun needs(face: BookFace) = face.path !in tried

    @Synchronized
    fun add(face: BookFace, data: ByteArray?) {
        tried.add(face.path)
        if (data == null) return
        val typeface = runCatching {
            val font = Font.Builder(ByteBuffer.wrap(data)).build()
            Typeface.CustomFallbackBuilder(FontFamily.Builder(font).build()).build()
        }.getOrNull() ?: return
        entries.getOrPut(face.family.lowercase()) { mutableListOf() }.add(Entry(typeface, face.bold, face.italic))
    }

    /** The book's font of that name, in the style nearest to the one asked for. */
    @Synchronized
    fun font(family: String, bold: Boolean, italic: Boolean): Typeface? =
        entries[family.lowercase()]?.minByOrNull { (if (it.bold == bold) 0 else 2) + (if (it.italic == italic) 0 else 1) }?.typeface
}

/** A raised or lowered mark (a footnote number): its own baseline, moved up or down from the line's. */
private class BaselineShift(private val shift: Int) : MetricAffectingSpan() {
    override fun updateDrawState(paint: TextPaint) { paint.baselineShift += shift }
    override fun updateMeasureState(paint: TextPaint) { paint.baselineShift += shift }
}

private class SmallCaps : MetricAffectingSpan() {
    override fun updateDrawState(paint: TextPaint) { paint.fontFeatureSettings = "smcp" }
    override fun updateMeasureState(paint: TextPaint) { paint.fontFeatureSettings = "smcp" }
}

/** Sets a chapter in type: blocks of text and pictures in, a [ChapterLayout] out. Slow for a long chapter, so call it off the main thread. */
class BookTypesetter(
    private val context: Context,
    private val style: BookStyle,
    /** The text area of a page, in pixels. */
    private val width: Int,
    private val height: Int,
    private val density: Float,
    private val language: String?,
    private val rightToLeft: Boolean,
    /** How big a picture is in pixels, by its path in the book; null when it cannot be read. */
    private val pictureSize: (String) -> Pair<Int, Int>?,
    /** The book's own fonts, when the reader wants them used. */
    private val fonts: BookFontBank?,
) {
    private val base = (style.size * density).toFloat()
    private val locale = language?.let { Locale.forLanguageTag(it) } ?: Locale.getDefault()

    fun layout(index: Int, chapter: BookChapter): ChapterLayout {
        val blocks = ArrayList<LaidBlock>()
        // A page that is one picture and nothing else (a comic's page, a cover) fills the page.
        val only = chapter.blocks.singleOrNull()?.kind as? BookKind.Image
        for (block in chapter.blocks) blocks += laidOut(block, pictureOnly = only != null)
        val anchors = chapter.anchors.groupBy { it.id }.mapValues { it.value.first().offset.toInt() }
        return ChapterLayout(index, blocks, width.toFloat(), height.toFloat(), only?.src, anchors, blocks.lastOrNull()?.let { it.start + it.length } ?: 0)
    }

    private fun laidOut(block: BookBlock, pictureOnly: Boolean): LaidBlock {
        val start = block.start.toInt()
        val space = block.spaceBefore * base
        val heading = block.heading > 0u
        return when (val kind = block.kind) {
            is BookKind.Text -> {
                val text = spanned(kind.runs)
                LaidBlock(start, text.length + 1, staticLayout(text, block), null, space, heading, block.pageBreakBefore, text, 0f)
            }
            is BookKind.Image -> {
                val (w, h) = fit(pictureSize(kind.src), pictureOnly)
                LaidBlock(start, 2, null, PictureBox(kind.src, kind.ink, w, h), space, heading, block.pageBreakBefore, "￼", h)
            }
            BookKind.Rule -> LaidBlock(start, 2, null, null, space, heading, block.pageBreakBefore, "￼", base)
        }
    }

    /** A picture's size on the page: half its pixels at most, never wider than the page or most of its height. One that cannot be read still gets a small box. */
    private fun fit(pixels: Pair<Int, Int>?, fill: Boolean): Pair<Float, Float> {
        if (pixels == null || pixels.first <= 0 || pixels.second <= 0) return width * 0.3f to width * 0.3f
        val (w, h) = pixels.first.toFloat() to pixels.second.toFloat()
        if (fill) {
            val scale = minOf(width / w, height / h)
            return kotlin.math.floor(w * scale) to kotlin.math.floor(h * scale)
        }
        val scale = minOf(width / w * 2, height * 0.85f / h * 2, 2f) / 2
        return Math.round(w * scale).toFloat() to Math.round(h * scale).toFloat()
    }

    private fun spanned(runs: List<BookRun>): SpannableStringBuilder {
        val text = SpannableStringBuilder()
        for (run in runs) {
            val from = text.length
            // A line break inside a paragraph is a new line for the layout, one character for one.
            text.append(run.text.replace(' ', '\n'))
            val to = text.length
            if (to == from) continue
            val size = base * run.size * (if (run.shift == 0.toByte()) 1f else 0.85f) * (if (run.mono) 0.9f else 1f)
            text.setSpan(AbsoluteSizeSpan(size.toInt()), from, to, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            text.setSpan(TypefaceSpan(typeface(run)), from, to, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            if (run.smallCaps) text.setSpan(SmallCaps(), from, to, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            if (run.underline) text.setSpan(UnderlineSpan(), from, to, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            if (run.strike) text.setSpan(StrikethroughSpan(), from, to, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            if (run.shift != 0.toByte()) {
                val move = -run.shift * (if (run.shift > 0) 0.38f else 0.2f) * base
                text.setSpan(BaselineShift(move.toInt()), from, to, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            }
            run.link?.let { text.setSpan(LinkSpan(it, run.shift > 0), from, to, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE) }
        }
        return text
    }

    private fun typeface(run: BookRun): Typeface =
        run.family?.takeIf { fonts != null }?.let { fonts?.font(it, run.bold, run.italic) } ?: style.font.typeface(context, run.bold, run.italic, run.mono)

    private fun staticLayout(text: SpannableStringBuilder, block: BookBlock): StaticLayout {
        val left = minOf(block.left * base, width * 0.4f).toInt()
        val first = maxOf(0f, minOf(left + block.firstLine * base, width * 0.5f)).toInt()
        // Hyphens are drawn by the layout itself: room is left for them in the column.
        text.setSpan(LeadingMarginSpan.Standard(first, left), 0, text.length, Spanned.SPAN_INCLUSIVE_INCLUSIVE)
        val multiplier = if (block.heading > 0u) minOf(style.lineSpacing, 1.25).toFloat() else style.lineSpacing.toFloat()
        val builder = StaticLayout.Builder.obtain(text, 0, text.length, TextPaint(TextPaint.ANTI_ALIAS_FLAG).apply { textSize = base }, width)
            .setLineSpacing(0f, multiplier)
            .setIncludePad(false)
            .setTextDirection(if (rightToLeft) TextDirectionHeuristics.FIRSTSTRONG_RTL else TextDirectionHeuristics.FIRSTSTRONG_LTR)
            .setAlignment(
                when (block.align) {
                    BookAlign.CENTER -> Layout.Alignment.ALIGN_CENTER
                    BookAlign.END -> Layout.Alignment.ALIGN_OPPOSITE
                    else -> Layout.Alignment.ALIGN_NORMAL
                },
            )
        if (style.hyphenate) {
            builder.setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NORMAL).setBreakStrategy(Layout.BREAK_STRATEGY_HIGH_QUALITY)
        } else {
            builder.setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NONE).setBreakStrategy(Layout.BREAK_STRATEGY_SIMPLE)
        }
        if (justifies(block, text)) builder.setJustificationMode(Layout.JUSTIFICATION_MODE_INTER_WORD)
        return builder.build()
    }

    /** Justified when the reader asked for it, except headings, tiny blocks, and columns so narrow for the type that justifying would spread single words across the line. */
    private fun justifies(block: BookBlock, text: CharSequence) =
        style.justify && block.heading == 0.toUByte() && block.align != BookAlign.CENTER && block.align != BookAlign.END && text.length >= 3 && width / base >= 14
}
