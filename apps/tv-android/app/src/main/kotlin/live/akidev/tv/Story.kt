package live.akidev.tv

import android.os.SystemClock
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import uniffi.akitv.detail

/**
 * The story of a title, for the hero. The catalogue lists carry none, so each is one request for the title's page, and AniList
 * answers only 30 requests a minute: a quick scroll through a row asked for more than that, and the answers (and with them the story)
 * were refused for some titles. So: one request at a time, at least two seconds apart, and an answer is kept.
 */
object Story {
    private const val SPACING_MS = 2_100L
    private val known = ConcurrentHashMap<String, String>()
    private val turn = Mutex()
    private var lastAsked = 0L

    suspend fun of(id: String): String? {
        known[id]?.let { return it }
        return turn.withLock {
            known[id]?.let { return it }
            val wait = lastAsked + SPACING_MS - SystemClock.elapsedRealtime()
            if (wait > 0) delay(wait)
            try {
                withContext(Dispatchers.IO) { runCatching { detail(id).title.description }.getOrNull() }?.also { known[id] = it }
            } finally {
                lastAsked = SystemClock.elapsedRealtime()
            }
        }
    }
}
