package live.akidev.manga.app

import androidx.core.net.toUri
import uniffi.akimanga.MangaException

/** A title and a message for a failed call, in words a person can act on. `failed` names what did not work ("Could not load this series"); the message says why. */
class Problem(val title: String, val message: String)

fun describe(error: Throwable, failed: String = "Could not load this screen"): Problem = when (error) {
    is MangaException.Challenge -> Problem(
        "This site needs a browser check",
        "${hostOf(error.url)} asks for a browser check before it answers, and Akidev cannot pass it. Try another source, or read this one through a Suwayomi or Komga server you run.",
    )
    is MangaException.Offline -> Problem("Could not reach the site", "${hostOf(error.url)} did not answer. Check your connection and try again.")
    is MangaException.Failed -> Problem(failed, error.reason)
    else -> Problem(failed, error.message ?: error.toString())
}

private fun hostOf(url: String) = url.toUri().host ?: url
