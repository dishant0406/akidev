package live.akidev.manga.updates

import android.content.Context
import android.net.ConnectivityManager
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.isActive
import org.json.JSONArray
import org.json.JSONObject
import uniffi.akimanga.Engine

/**
 * New chapters waiting to be kept on the device. The list is saved, so a download that Android cut short, or that waited for Wi-Fi,
 * carries on at the next chance (the app opening, or the system giving the app time in the background).
 */
class AutoDownloader(private val context: Context, private val engine: Engine, private val wifiOnly: () -> Boolean) {
    private val store = context.getSharedPreferences("aki", Context.MODE_PRIVATE)
    private var running = false

    private class Item(val source: String, val series: String, val chapter: String) {
        val key get() = "$source/$series/$chapter"
    }

    private var queue: List<Item>
        get() {
            val saved = JSONArray(store.getString(KEY, "[]"))
            return List(saved.length()) { saved.getJSONObject(it).let { item -> Item(item.getString("source"), item.getString("series"), item.getString("chapter")) } }
        }
        set(items) {
            val saved = JSONArray(items.map { JSONObject().put("source", it.source).put("series", it.series).put("chapter", it.chapter) })
            store.edit().putString(KEY, saved.toString()).apply()
        }

    val hasWork get() = queue.isNotEmpty()

    fun enqueue(found: List<NewChapters>) {
        val items = queue.toMutableList()
        for (series in found) {
            for (chapter in series.chapters.takeLast(PER_SERIES)) {
                val item = Item(series.source, series.series, chapter.id)
                if (items.none { it.key == item.key }) items.add(item)
            }
        }
        queue = items
    }

    /**
     * Downloads what is queued, one chapter at a time, until it is done or the work is cancelled. A chapter that fails stays queued for next
     * time; one already on the device is dropped from the list.
     */
    suspend fun run() {
        if (running || !hasWork) return
        if (wifiOnly() && isMetered()) return
        running = true
        try {
            val kept = engine.downloads().filter { it.state == "done" }.map { "${it.source}/${it.series}/${it.chapter}" }.toSet()
            for (item in queue) {
                if (!currentCoroutineContext().isActive) return
                if (item.key !in kept) {
                    try { engine.downloadChapter(item.source, item.series, item.chapter) } catch (failure: Exception) { continue }
                }
                queue = queue.filter { it.key != item.key }
            }
        } finally {
            running = false
        }
    }

    /** Whether the connection right now is mobile data or a hotspot. */
    private fun isMetered() = context.getSystemService(ConnectivityManager::class.java).isActiveNetworkMetered

    companion object {
        private const val KEY = "autoDownloadQueue"

        /** Only the newest few of a series are fetched in one go, so a long-unchecked series does not pull hundreds. */
        const val PER_SERIES = 5
    }
}
