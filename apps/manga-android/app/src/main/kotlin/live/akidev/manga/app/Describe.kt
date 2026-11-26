package live.akidev.manga.app

import androidx.core.net.toUri
import live.akidev.manga.design.Copy
import uniffi.akimanga.MangaException

/** A title and a message for a failed call, in words a person can act on. `failed` names what did not work ("Could not load this series"); the message says why. */
class Problem(val title: String, val message: String)

fun describe(error: Throwable, failed: String = Copy.Settings.couldNotLoadScreen): Problem = when (error) {
    is MangaException.Challenge -> Problem(
        Copy.Settings.browserCheckTitle,
        Copy.Settings.browserCheckMessage(hostOf(error.url)),
    )
    is MangaException.Offline -> Problem(Copy.Settings.couldNotReachTitle, Copy.Settings.couldNotReachMessage(hostOf(error.url)))
    is MangaException.Failed -> Problem(failed, error.reason)
    else -> Problem(failed, error.message ?: error.toString())
}

private fun hostOf(url: String) = url.toUri().host ?: url
