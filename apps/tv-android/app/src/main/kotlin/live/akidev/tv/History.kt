package live.akidev.tv

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

/** One film or episode the viewer started, and where they stopped. `titleId` is the film itself or the series it belongs to. */
data class Watched(
    val playId: String,
    val titleId: String,
    val title: String,
    val poster: String?,
    val banner: String?,
    /** "Episode 3" for an episode, null for a film. */
    val label: String?,
    val position: Long,
    val length: Long,
    val at: Long,
)

/** Past 90 % it counts as seen. */
fun Watched.finished() = length > 0 && position * 10 >= length * 9

/** Where to start `playId`: where the viewer stopped, or the beginning when there is nothing to resume or it was finished. */
fun List<Watched>.resumeAt(playId: String): Long =
    firstOrNull { it.playId == playId }?.takeIf { !it.finished() }?.position ?: 0L

/** How far through `playId` the viewer is, 0 to 1, or null when never started. */
fun List<Watched>.progressOf(playId: String): Float? =
    firstOrNull { it.playId == playId }?.takeIf { it.length > 0 }?.let { (it.position.toFloat() / it.length).coerceIn(0f, 1f) }

/** One entry for each title, the one watched last, newest first, leaving out what was finished. */
fun List<Watched>.continueWatching(most: Int = 12): List<Watched> =
    sortedByDescending { it.at }.distinctBy { it.titleId }.filter { !it.finished() }.take(most)

/** The history lives in the app's private storage as one small JSON list. */
object History {
    private const val KEEP = 300

    fun load(context: Context): List<Watched> = try {
        val list = JSONArray(prefs(context).getString("history", "[]"))
        (0 until list.length()).map { read(list.getJSONObject(it)) }
    } catch (e: Exception) {
        emptyList()
    }

    /** Adds or replaces the entry for the same film or episode. */
    fun save(context: Context, entry: Watched) {
        val all = (listOf(entry) + load(context).filter { it.playId != entry.playId }).take(KEEP)
        prefs(context).edit().putString("history", JSONArray(all.map(::write)).toString()).apply()
    }

    private fun prefs(context: Context) = context.getSharedPreferences("akidev", Context.MODE_PRIVATE)

    private fun write(w: Watched) = JSONObject()
        .put("playId", w.playId).put("titleId", w.titleId).put("title", w.title)
        .put("poster", w.poster).put("banner", w.banner).put("label", w.label)
        .put("position", w.position).put("length", w.length).put("at", w.at)

    private fun read(o: JSONObject) = Watched(
        playId = o.getString("playId"), titleId = o.getString("titleId"), title = o.getString("title"),
        poster = o.optString("poster").ifEmpty { null }, banner = o.optString("banner").ifEmpty { null },
        label = o.optString("label").ifEmpty { null },
        position = o.getLong("position"), length = o.getLong("length"), at = o.getLong("at"),
    )
}
