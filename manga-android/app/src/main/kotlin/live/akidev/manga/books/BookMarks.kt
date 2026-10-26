package live.akidev.manga.books

import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.ui.graphics.toArgb
import androidx.core.graphics.createBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import live.akidev.manga.app.coverJpeg
import uniffi.akimanga.BookHighlight
import uniffi.akimanga.BookHit
import uniffi.akimanga.BookSection

/** What the reader can do with a selection or a highlight, from the menu that opens over it. */
sealed interface BookAction {
    class Highlight(val colour: Int) : BookAction
    object Note : BookAction
    object Copy : BookAction
    object Read : BookAction
    object Remove : BookAction
}

/** What is drawn over the text of the page: the highlights, the selection, the place found, the sentence read aloud. */
val BookModel.overlays
    get() = BookOverlays(
        marks.map { BookMarkSpan(it.id, BookSpan(it.chapter.toInt(), it.start.toInt(), it.end.toInt()), it.colour.toInt(), it.note != null) },
        selection, found, speaking,
    )

/** Does what the menu over a selection or a highlight asked for. */
fun BookModel.perform(action: BookAction, span: BookSpan, mark: Long?, words: String) {
    selection = null
    when (action) {
        is BookAction.Highlight -> {
            val old = marks.firstOrNull { it.id == mark }
            if (mark != null && old != null) runCatching { engine.setBookMark(mark, action.colour.toUByte(), old.note) }
            else runCatching { engine.addBookMark(info.id, span.chapter.toUInt(), span.start.toUInt(), span.end.toUInt(), action.colour.toUByte(), words) }
        }
        BookAction.Note -> {
            val id = mark ?: runCatching { engine.addBookMark(info.id, span.chapter.toUInt(), span.start.toUInt(), span.end.toUInt(), 0u, words) }.getOrNull()
            reloadMarks()
            marks.firstOrNull { it.id == id }?.let { noteDraft = NoteDraft(it.id, it.quote) }
            return
        }
        BookAction.Read -> return readFrom(span)
        BookAction.Copy -> {
            val clipboard = appContext.getSystemService(ClipboardManager::class.java)
            clipboard.setPrimaryClip(ClipData.newPlainText("Words from the book", words))
            return
        }
        BookAction.Remove -> if (mark != null) runCatching { engine.removeBookMark(mark) }
    }
    reloadMarks()
}

fun BookModel.saveNote(draft: NoteDraft, text: String) {
    marks.firstOrNull { it.id == draft.id }?.let { runCatching { engine.setBookMark(draft.id, it.colour, text) } }
    reloadMarks()
}

fun BookModel.remove(mark: BookHighlight) {
    runCatching { engine.removeBookMark(mark.id) }
    reloadMarks()
}

/** Goes to a highlight. */
fun BookModel.open(mark: BookHighlight) {
    trail = emptyList()
    go(Spot(mark.chapter.toInt(), offset = mark.start.toInt(), highlight = (mark.end - mark.start).toInt()))
}

/** Looks for words in the whole book: the finds in reading order, or if the words are nowhere, the nearest words. */
suspend fun BookModel.search(query: String): List<BookHit> = withContext(Dispatchers.Default) { runCatching { engine.searchBook(info.id, query) }.getOrDefault(emptyList()) }

/** Goes to a place a search found. */
fun BookModel.open(hit: BookHit) {
    trail = emptyList()
    go(Spot(hit.chapter.toInt(), offset = hit.offset.toInt(), highlight = hit.length.toInt()))
}

/** The pages around the book itself (copyright, dedication, notes, colophon), in reading order. */
val BookModel.aboutPages get() = sections.indices.filter { sections[it] == BookSection.FRONT || sections[it] == BookSection.BACK }

/** A chapter's text as plain paragraphs, for a page that is only to be read (a copyright page, notes). */
suspend fun BookModel.plainText(chapter: Int): String = withContext(Dispatchers.Default) {
    val content = runCatching { reader.chapter(chapter.toUInt()) }.getOrNull()
    (content?.blocks ?: emptyList()).mapNotNull { block ->
        val kind = block.kind as? uniffi.akimanga.BookKind.Text ?: return@mapNotNull null
        kind.runs.joinToString("") { it.text }.replace(' ', '\n').replace('\t', ' ').takeIf { it.isNotBlank() }
    }.joinToString("\n\n")
}

/** The title of a page: its entry in the contents, or the first line of its text. */
fun BookModel.pageTitle(chapter: Int, text: String): String =
    contents.firstOrNull { it.chapter.toInt() == chapter }?.title ?: text.lineSequence().firstOrNull()?.take(60) ?: "Page ${chapter + 1}"

/** The title of the part of the book a chapter falls in. */
fun BookModel.title(chapter: Int): String = contents.lastOrNull { it.chapter.toInt() <= chapter }?.title ?: info.title

/** The highlights and notes as Markdown, for the share sheet; null when there are none. */
fun BookModel.markdown(): String? {
    if (marks.isEmpty()) return null
    val lines = mutableListOf("# ${info.title}")
    if (info.authors.isNotEmpty()) lines += "*${info.authors.joinToString(", ")}*"
    var part: String? = null
    for (mark in marks) {
        val heading = title(mark.chapter.toInt())
        if (heading != part) {
            lines += listOf("", "## $heading")
            part = heading
        }
        lines += listOf("", "> " + mark.quote.replace("\n", "\n> "))
        mark.note?.let { lines += listOf("", it) }
    }
    return lines.joinToString("\n") + "\n"
}

// ---- the cover -------------------------------------------------------------------------------------------------------------------

/** Makes the page on screen the book's cover, in a cover's shape. Returns whether it could. */
suspend fun BookModel.useCurrentPageAsCover(): Boolean {
    val here = place ?: return false
    val layout = layouts[here.chapter] ?: return false
    val picture = layout.coverPicture?.let { path -> withContext(Dispatchers.Default) { runCatching { reader.resource(path) }.getOrNull()?.let { android.graphics.BitmapFactory.decodeByteArray(it, 0, it.size) } } }
    val image = picture ?: renderPage(layout, here.page.coerceAtMost(layout.count - 1)) ?: return false
    val jpeg = coverJpeg(image, 600, if (picture == null) 0.5f else 0f)
    return runCatching { engine.setBookCustomCover(info.id, jpeg) }.isSuccess.also { reloadInfo() }
}

/** Goes back to the cover the book came with. */
fun BookModel.useOwnCover() {
    runCatching { engine.clearBookCustomCover(info.id) }
    reloadInfo()
}

/** The page as it is drawn on screen (without the highlights and the foot line). */
private fun BookModel.renderPage(layout: ChapterLayout, page: Int): android.graphics.Bitmap? {
    if (screenWidth == 0) return null
    val bitmap = createBitmap(screenWidth, screenHeight)
    val canvas = android.graphics.Canvas(bitmap)
    canvas.drawColor(theme.paper.toArgb())
    val insets = pageInsets
    canvas.translate(insets.left, insets.top + if (style.scroll) layout.leading(page) else 0f)
    layout.draw(canvas, page, theme.ink.toArgb(), theme.quiet.toArgb(), BookOverlays()) { path, width -> picture(path, width) }
    return bitmap
}
