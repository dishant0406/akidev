package live.akidev.manga.books

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.util.LruCache
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.Problem
import live.akidev.manga.listen.BookListener
import uniffi.akimanga.BookAlign
import uniffi.akimanga.BookBlock
import uniffi.akimanga.BookChapter
import uniffi.akimanga.BookEntry
import uniffi.akimanga.BookHighlight
import uniffi.akimanga.BookInfo
import uniffi.akimanga.BookKind
import uniffi.akimanga.BookReader
import uniffi.akimanga.BookRun
import uniffi.akimanga.BookSection
import java.nio.ByteBuffer

/** A page of a book: a chapter and a page in it. A page of -1 is the chapter's last. */
data class BookPlace(val chapter: Int, val page: Int)

/** An order to show a place now. `id` changes with every order, so the same place can be asked for twice. */
class BookJump(val id: Int, val place: BookPlace, val forward: Boolean = true, val animated: Boolean = false)

/** A place in the book that is not yet a page: the chapter, and either a position, an id or how far through it. */
class Spot(
    val chapter: Int,
    val offset: Int? = null,
    val anchor: String? = null,
    val fraction: Double? = null,
    /** How long a stretch from `offset` to mark as found. */
    val highlight: Int? = null,
    /** The voice reading aloud moved on: the page follows it. */
    val speech: Boolean = false,
)

enum class BookTapZone { Back, Forward, Middle }

/** A footnote shown in a card, with where it is in the book. */
class Footnote(val text: String, val link: String)

/** A highlight being given a note. */
class NoteDraft(val id: Long, val quote: String)

/**
 * One open book: where the person is, how its chapters are set in type, and what the reader's screen shows.
 *
 * A chapter is read and set in type when it is needed, on a background thread, and only the chapters around the current one are kept: a
 * long book costs what a few chapters cost.
 */
class BookModel private constructor(
    val app: AppModel,
    private val context: Context,
    info: BookInfo,
    val reader: BookReader,
    val contents: List<BookEntry>,
    private val sizes: List<ULong>,
    marks: List<BookHighlight>,
    /** What each chapter is (cover, front matter, the book, what follows it). */
    val sections: List<BookSection>,
    /** Where the book itself starts. */
    val start: Int,
    private val scope: CoroutineScope,
) {
    var info by mutableStateOf(info)
        private set
    val engine get() = app.engine
    val chapterCount = sizes.size

    var style by mutableStateOf(app.prefs.bookStyle)
        private set

    /** The page being read, once the book has opened. */
    var place by mutableStateOf<BookPlace?>(null)
        private set

    /** What the pager shows now, when it should rather than wait to be turned. */
    var jump by mutableStateOf<BookJump?>(null)
        private set
    var chromeVisible by mutableStateOf(false)

    /** How far through the book, 0 to 1; the slider moves it and lifting the finger goes there. */
    var progress by mutableStateOf(info.progress)
    var failure by mutableStateOf<Problem?>(null)

    /** A link to a web page, waiting for the person to say whether to leave the app. */
    var externalLink by mutableStateOf<String?>(null)

    /** Where the person was before following a link inside the book: the way back. */
    var trail by mutableStateOf(emptyList<Spot>())
    var systemDark by mutableStateOf(false)

    /** The reader's highlights, in reading order. */
    var marks by mutableStateOf(marks)
        private set

    /** The words selected now, while a long press or the handles are at work. */
    var selection by mutableStateOf<BookSpan?>(null)

    /** The words a search found, shown until the page is turned. */
    var found by mutableStateOf<BookSpan?>(null)
    private var foundPage: BookPlace? = null

    /** The sentence being read aloud. */
    var speaking by mutableStateOf<BookSpan?>(null)

    /** A highlight whose note is being written. */
    var noteDraft by mutableStateOf<NoteDraft?>(null)

    /** A footnote shown in a card. */
    var footnote by mutableStateOf<Footnote?>(null)

    /** The chapters set in type; a page whose chapter is not in it yet shows only paper. */
    val layouts = mutableStateMapOf<Int, ChapterLayout>()

    /** The screen the book is laid out for, in pixels, and the room the system's bars take. */
    var screenWidth = 0
        private set
    var screenHeight = 0
        private set
    private var density = 1f
    private var safeTop = 0
    private var safeBottom = 0
    private val building = HashSet<Int>()
    private val fontBank = BookFontBank()
    private var generation = 0
    private var jumps = 0
    private var wanted: Triple<Spot, Boolean, Boolean>? = null
    private var restyle: Job? = null

    /** Bumped when a picture has been decoded, so the pages showing it draw again. */
    var pictureTick by mutableIntStateOf(0)
        private set
    private val pictures = object : LruCache<String, Bitmap>(40 * 1024 * 1024) {
        override fun sizeOf(key: String, value: Bitmap) = value.allocationByteCount
    }
    private val loadingPictures = HashSet<String>()
    private val brokenPictures = HashSet<String>()

    val theme get() = style.theme.resolved(systemDark)
    val rightToLeft get() = info.rtl

    /** The room round the text on the screen: the side margins, and the head and foot lines. */
    val pageInsets: Insets
        get() {
            val margin = maxOf(style.margin.dp * density, (screenWidth - style.size.toFloat() * 34 * density) / 2)
            val top = maxOf(safeTop.toFloat(), 20 * density) + 12 * density
            val bottom = maxOf(safeBottom.toFloat(), 12 * density) + 30 * density
            // A scrolled book has no foot line: the page runs on into the next.
            return if (style.scroll) Insets(margin, 0f, margin, 0f) else Insets(margin, top, margin, bottom)
        }

    class Insets(val left: Float, val top: Float, val right: Float, val bottom: Float)

    // ---- the pages ask -----------------------------------------------------------------------------------------------------

    /** The chapter's type, or null while it is being set (it then shows in [layouts]). */
    fun layout(chapter: Int): ChapterLayout? {
        layouts[chapter]?.let { return it }
        // While the page being opened is still being set in type, the chapters beside it wait: they would take the processors and the
        // first page would come later. They are made as soon as it is shown, and when a page asks for one then.
        if (wanted != null && wanted!!.first.chapter != chapter) return null
        build(chapter)
        return null
    }

    fun footer(place: BookPlace, layout: ChapterLayout): Pair<String, String> {
        val entry = contents.firstOrNull { it.chapter.toInt() == place.chapter && it.anchor == null } ?: contents.lastOrNull { it.chapter.toInt() <= place.chapter }
        return (entry?.title ?: info.title) to "${place.page + 1} / ${layout.count}"
    }

    /** A picture of the book at about the size wanted, null until it is decoded (then [pictureTick] changes). */
    fun picture(path: String, width: Float): Bitmap? {
        val pixels = width.toInt().coerceAtLeast(64)
        val key = "$path|$pixels"
        pictures.get(key)?.let { return it }
        if (key in brokenPictures || !loadingPictures.add(key)) return null
        scope.launch(Dispatchers.Default) {
            val image = runCatching { decode(reader.resource(path), pixels) }.getOrNull()
            withContext(Dispatchers.Main) {
                loadingPictures.remove(key)
                if (image == null) brokenPictures.add(key) else {
                    pictures.put(key, image)
                    pictureTick += 1
                }
            }
        }
        return null
    }

    private fun decode(bytes: ByteArray, width: Int) = ImageDecoder.decodeBitmap(ImageDecoder.createSource(ByteBuffer.wrap(bytes))) { decoder, info, _ ->
        if (info.size.width > width) decoder.setTargetSize(width, info.size.height * width / info.size.width)
    }

    fun turned(to: BookPlace) {
        if (listener != null && to != place && to != speechTurn) userMoved = true
        place = to
        if (selection != null) selection = null
        if (found != null && foundPage != to) found = null
        // A page counted from the end waits for its chapter to be set in type.
        val layout = layouts[to.chapter]
        if (layout == null || to.page < 0) return
        progress = progressOf(to, layout)
        val finished = to.chapter == chapterCount - 1 && to.page >= layout.count - 1
        runCatching { engine.saveBookPosition(info.id, to.chapter.toUInt(), maxOf(layout.start(to.page), 0).toUInt(), progress, finished) }
        // The chapters around this one are made ready before they are needed; the ones far away are let go.
        if (to.page >= layout.count - 3) build(to.chapter + 1)
        if (to.page <= 2) build(to.chapter - 1)
        val far = if (style.scroll) 6 else 2
        for (chapter in layouts.keys.toList()) if (kotlin.math.abs(chapter - to.chapter) > far) layouts.remove(chapter)
    }

    /** A tap on the page: where, the link under it if any (`isNote` when that link is a raised mark, a footnote's number), and the word under it if any. */
    fun tapped(zone: BookTapZone, link: String?, isNote: Boolean, word: BookSpan?) {
        if (link != null) return follow(link, isNote)
        // While it reads aloud, a tap on a word in the middle of the page reads on from there.
        if (listener != null && zone == BookTapZone.Middle && word != null) return readFrom(word)
        val flipped = app.prefs.invertTaps != info.rtl
        when (zone) {
            BookTapZone.Middle -> chromeVisible = !chromeVisible
            BookTapZone.Back -> turn(if (flipped) 1 else -1)
            BookTapZone.Forward -> turn(if (flipped) -1 else 1)
        }
    }

    // ---- opening and moving ----------------------------------------------------------------------------------------------------

    /** Tells the book how big the screen is; sets the first page when it is known. */
    fun resize(width: Int, height: Int, density: Float, safeTop: Int, safeBottom: Int) {
        if (width <= 0 || (width == screenWidth && height == screenHeight && safeTop == this.safeTop && safeBottom == this.safeBottom)) return
        val first = screenWidth == 0
        screenWidth = width
        screenHeight = height
        this.density = density
        this.safeTop = safeTop
        this.safeBottom = safeBottom
        if (first) {
            // A book never read is opened where the book itself starts, past the cover, the copyright page and the dedication.
            val fresh = info.openedMs == null && info.chapter == 0u && info.offset == 0u
            val chapter = if (fresh) start else minOf(info.chapter.toInt(), maxOf(chapterCount - 1, 0))
            go(Spot(chapter, offset = if (fresh) 0 else info.offset.toInt()), forward = true, animated = false)
        } else {
            typeChanged()
        }
    }

    /** Goes to a place in the book: at once if its chapter is ready, when it is if not. */
    fun go(spot: Spot, forward: Boolean = true, animated: Boolean = false) {
        wanted = Triple(spot, forward, animated)
        val layout = layouts[spot.chapter]
        if (layout != null) arrive(layout) else build(spot.chapter)
    }

    private fun arrive(layout: ChapterLayout) {
        val (spot, forward, animated) = wanted ?: return
        if (spot.chapter != layout.index) return
        wanted = null
        val page = when {
            spot.anchor != null && layout.offset(spot.anchor) != null -> layout.page(layout.offset(spot.anchor)!!)
            spot.offset != null -> layout.page(spot.offset)
            spot.fraction != null -> minOf((spot.fraction * layout.count).toInt(), layout.count - 1)
            else -> 0
        }
        val target = BookPlace(layout.index, page)
        if (spot.speech) speechTurn = target
        if (spot.highlight != null && spot.offset != null) {
            found = BookSpan(layout.index, spot.offset, spot.offset + spot.highlight)
            foundPage = target
        }
        jumps += 1
        jump = BookJump(jumps, target, forward, animated)
        turned(target)
    }

    /** Turns the page by one, across chapter ends. */
    private fun turn(step: Int) {
        val here = place ?: return
        val layout = layouts[here.chapter] ?: return
        if (here.page < 0) return
        val page = here.page + step
        when {
            page in 0 until layout.count -> move(BookPlace(here.chapter, page), step > 0)
            step > 0 && here.chapter + 1 < chapterCount -> go(Spot(here.chapter + 1, offset = 0), forward = true, animated = !style.ink)
            step < 0 && here.chapter > 0 -> go(Spot(here.chapter - 1, fraction = 1.0), forward = false, animated = !style.ink)
        }
    }

    fun turnPage(forward: Boolean) = turn(if (forward) 1 else -1)

    private fun move(target: BookPlace, forward: Boolean) {
        jumps += 1
        jump = BookJump(jumps, target, forward, !style.ink)
        turned(target)
    }

    /** Goes to a place the slider or the contents named: how far through the book, or an entry. */
    fun goToProgress(fraction: Double) {
        val total = sizes.sumOf { it.toDouble() }
        if (total <= 0) return go(Spot((fraction * chapterCount).toInt().coerceIn(0, chapterCount - 1), offset = 0))
        var before = 0.0
        for ((chapter, size) in sizes.withIndex()) {
            val share = size.toDouble() / total
            if (fraction <= before + share || chapter == sizes.lastIndex) {
                val inside = if (share > 0) (fraction - before) / share else 0.0
                return go(Spot(chapter, fraction = inside.coerceIn(0.0, 1.0)), forward = chapter >= (place?.chapter ?: 0))
            }
            before += share
        }
    }

    fun open(entry: BookEntry) {
        trail = emptyList()
        go(Spot(entry.chapter.toInt(), offset = if (entry.anchor == null) 0 else null, anchor = entry.anchor), forward = entry.chapter.toInt() >= (place?.chapter ?: 0))
    }

    private fun follow(link: String, isNote: Boolean) {
        if (link.startsWith("http://") || link.startsWith("https://")) {
            externalLink = link
            return
        }
        scope.launch {
            // Finding where a link goes can mean reading a large file: not on the main thread.
            // A footnote's number shows the note in a card; any other link goes where it points.
            val text = if (isNote) withContext(Dispatchers.Default) { reader.note(link) } else null
            if (text != null) footnote = Footnote(text, link) else followInBook(link)
        }
    }

    /** Goes where a link in the book goes, with a way back. */
    suspend fun followInBook(link: String) {
        val target = withContext(Dispatchers.Default) { reader.target(link) } ?: return
        place?.let { here -> layouts[here.chapter]?.let { trail = trail + Spot(here.chapter, offset = it.start(here.page)) } }
        go(Spot(target.chapter.toInt(), offset = if (target.anchor == null) 0 else null, anchor = target.anchor), forward = true, animated = false)
    }

    /** Back to where the person was before a link inside the book took them away. */
    fun goBack() {
        val spot = trail.lastOrNull() ?: return
        trail = trail.dropLast(1)
        go(spot, forward = false, animated = false)
    }

    // ---- the look --------------------------------------------------------------------------------------------------------------

    /** Changes how the book looks; the type is set again when a choice changes where words fall, keeping the place. */
    fun changeStyle(new: BookStyle) {
        val old = style
        if (new == old) return
        style = new
        app.prefs.bookStyle = new
        // Pages and scrolling show the same pages: changing between them keeps the place, and nothing is set in type again.
        if (new.scroll != old.scroll) place?.let { here ->
            jumps += 1
            jump = BookJump(jumps, here)
        }
        if (new.affectsType != old.affectsType) typeChanged()
    }

    // ---- setting type ----------------------------------------------------------------------------------------------------------

    /** Sets the chapter in type on a background thread. The pages are told when it is done. */
    private fun build(chapter: Int) {
        if (chapter < 0 || chapter >= chapterCount || screenWidth == 0 || layouts[chapter] != null || !building.add(chapter)) return
        val insets = pageInsets
        val typesetter = BookTypesetter(
            context, style, (screenWidth - insets.left - insets.right).toInt(), (screenHeight - insets.top - insets.bottom).toInt(), density, info.language, info.rtl,
            pictureSize = { path -> runCatching { reader.resource(path) }.getOrNull()?.let { bytes -> imageSize(bytes) } },
            fonts = if (style.bookFonts) fontBank else null,
        )
        val made = generation
        scope.launch(Dispatchers.Default) {
            val content = runCatching { reader.chapter(chapter.toUInt()) }.getOrNull() ?: unreadable
            for (face in content.fonts) if (fontBank.needs(face)) fontBank.add(face, runCatching { reader.resource(face.path) }.getOrNull())
            val layout = typesetter.layout(chapter, content)
            withContext(Dispatchers.Main) { finished(layout, made) }
        }
    }

    private fun imageSize(bytes: ByteArray): Pair<Int, Int>? {
        val options = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
        android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)
        return if (options.outWidth > 0 && options.outHeight > 0) options.outWidth to options.outHeight else null
    }

    private fun finished(layout: ChapterLayout, made: Int) {
        if (made != generation) return
        building.remove(layout.index)
        layouts[layout.index] = layout
        arrive(layout)
        val here = place
        if (here != null && here.chapter == layout.index && here.page < 0) turned(BookPlace(here.chapter, layout.count - 1))
    }

    /** Takes the place of a chapter that cannot be read, so the book still opens and turns. */
    private val unreadable = BookChapter(
        blocks = listOf(
            BookBlock(0u, BookKind.Text(listOf(BookRun("This part of the book could not be read.", false, true, false, false, false, false, 0, 1f, null, null))), 0u.toUByte(), BookAlign.CENTER, 0f, 0f, 0f, false),
        ),
        anchors = emptyList(), fonts = emptyList(), vertical = false,
    )

    /** The type or the page changed: set the chapters again, a moment after the last change, and come back to the same words. */
    private fun typeChanged() {
        restyle?.cancel()
        restyle = scope.launch {
            delay(250)
            val here = place
            val offset = here?.let { layouts[it.chapter]?.start(it.page) } ?: 0
            generation += 1
            layouts.clear()
            building.clear()
            go(Spot(here?.chapter ?: 0, offset = offset))
        }
    }

    private fun progressOf(place: BookPlace, layout: ChapterLayout): Double {
        val total = sizes.sumOf { it.toDouble() }
        if (total <= 0) return (place.chapter + place.page.toDouble() / maxOf(layout.count, 1)) / maxOf(chapterCount, 1)
        val before = sizes.take(place.chapter).sumOf { it.toDouble() }
        val inside = sizes[place.chapter].toDouble() * place.page / maxOf(layout.count, 1)
        return minOf((before + inside) / total, 1.0)
    }

    // ---- reading aloud (see Listening.kt) --------------------------------------------------------------------------------------

    /** Reading aloud, while it goes on. */
    var listener by mutableStateOf<BookListener?>(null)

    /** The person turned away from the page being read aloud: the page stops following the voice until they come back to it. */
    var userMoved = false

    /** The place the page last moved to because the voice did, so that is not taken for the person turning. */
    var speechTurn: BookPlace? = null

    /** What reading aloud runs in: as long as the book is open. */
    val listenScope get() = scope

    /** A voice model is on the phone, so "Read from here" is offered. */
    val canRead get() = app.readAloud.current != null

    /** The app's context, for the system's services. */
    val appContext: Context get() = context

    /** Reloads the book's own record (a cover changed). */
    fun reloadInfo() {
        runCatching { engine.bookInfo(info.id) }.getOrNull()?.let { info = it }
    }

    fun reloadMarks() {
        marks = runCatching { engine.bookMarks(info.id) }.getOrDefault(marks)
    }

    companion object {
        /** Opens the book, off the main thread: reading its contents can take a moment for a large one. */
        suspend fun open(app: AppModel, context: Context, info: BookInfo, scope: CoroutineScope): BookModel {
            class Opened(val reader: BookReader, val contents: List<BookEntry>, val sizes: List<ULong>, val marks: List<BookHighlight>, val sections: List<BookSection>, val start: Int)
            val opened = withContext(Dispatchers.Default) {
                val reader = app.engine.openBook(info.id)
                Opened(reader, reader.contents(), reader.chapterSizes(), runCatching { app.engine.bookMarks(info.id) }.getOrDefault(emptyList()), reader.sections(), reader.startChapter().toInt())
            }
            return BookModel(app, context, info, opened.reader, opened.contents, opened.sizes, opened.marks, opened.sections, opened.start, scope)
        }
    }
}
