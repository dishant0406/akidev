package live.akidev.manga.books

import kotlinx.coroutines.launch
import live.akidev.manga.listen.BookListener

/** Starts reading aloud, in the voice of the model in use: from the word given, or the top of the page being read. */
fun BookModel.listen(scope: kotlinx.coroutines.CoroutineScope, from: BookSpan? = null) {
    val readAloud = app.readAloud
    val model = readAloud.current ?: return
    val here = place ?: return
    val layout = layouts[here.chapter] ?: return
    if (listener != null || here.page < 0) return
    val made = BookListener(
        appContext, scope, reader, engine, info.title, info.authors.firstOrNull(), model.id, readAloud.chosenVoice?.id, sections, app.prefs.speechRate, { app.prefs.speechRate = it },
        follow = { heard(it) }, ended = { problem -> listenerEnded(problem) },
    )
    listener = made
    userMoved = false
    val (chapter, offset) = from?.let { it.chapter to it.start } ?: (here.chapter to layout.start(here.page))
    scope.launch { made.start(chapter, offset) }
}

fun BookModel.stopListening() {
    listener?.stop()
}

/** Reads on from a word the person pointed at: from there at once if it is already reading, otherwise it starts there. */
fun BookModel.readFrom(word: BookSpan) {
    userMoved = false
    val current = listener
    if (current != null) current.jump(word.chapter, word.start) else listen(listenScope, word)
}

/**
 * The sentence is marked, and the page goes where the voice is, unless the person has turned away from it: then the page stays where they
 * put it, and follows again once they are back on the page being read.
 */
private fun BookModel.heard(span: BookSpan) {
    speaking = span
    val spoken = layouts[span.chapter]?.let { BookPlace(span.chapter, it.page(span.start)) }
    if (userMoved) {
        if (spoken != null && spoken == place) userMoved = false
        return
    }
    if (spoken == place) return
    go(Spot(span.chapter, offset = span.start, speech = true), forward = true, animated = !style.ink)
}

private fun BookModel.listenerEnded(problem: live.akidev.manga.app.Problem?) {
    listener = null
    speaking = null
    if (problem != null) failure = problem
}
