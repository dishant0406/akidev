package live.akidev.manga.app

import android.net.Uri
import uniffi.akimanga.MangaException

/** A title and a message for a failed call, in words a person can act on. */
class Problem(val title: String, val message: String)

fun describe(error: Throwable): Problem = when (error) {
    is MangaException.Challenge -> Problem(
        "This site checks visitors first",
        "${hostOf(error.url)} asks for a browser check before it answers, and Akidev has no browser to pass it in. Try another source, or read this one through a Suwayomi or Komga server you run.",
    )
    is MangaException.Offline -> Problem("No connection", "Could not reach ${hostOf(error.url)}. Check your network and try again.")
    is MangaException.Failed -> Problem("That did not work", error.reason)
    else -> Problem("That did not work", error.message ?: error.toString())
}

private fun hostOf(url: String) = Uri.parse(url).host ?: url
