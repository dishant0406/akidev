package live.akidev.manga.design

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.util.LruCache
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.nio.ByteBuffer
import java.security.MessageDigest

/**
 * Loads covers and pages: a request with the headers a source needs, decoded off the main thread at the size it will be shown (a
 * 12,000 px page is never held at full size). Covers and pages keep their address for good, so what was fetched once is read from disk
 * next time (and next launch).
 */
object AkiImageLoader {
    private const val DISK_LIMIT = 400L * 1024 * 1024
    private val memory = object : LruCache<String, Bitmap>((Runtime.getRuntime().maxMemory() / 4).toInt().coerceAtMost(160 * 1024 * 1024)) {
        override fun sizeOf(key: String, value: Bitmap) = value.allocationByteCount
    }
    private val connections = Semaphore(6)
    private val work = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val inflight = HashMap<String, Deferred<Bitmap?>>()
    private lateinit var folder: File

    /** Called once, when the app starts. */
    fun start(context: Context) {
        folder = File(context.cacheDir, "images").apply { mkdirs() }
    }

    /** The picture if it is already decoded at that width, at once; nothing is fetched. A row scrolled back into view shows it in its first frame. */
    fun cached(url: String, widthPx: Int): Bitmap? = memory.get("$url@$widthPx")

    fun clear() {
        memory.evictAll()
        folder.listFiles()?.forEach { it.delete() }
    }

    /**
     * The picture, fetched and decoded. The work belongs to the loader, not to whoever asked: a row scrolled away stops waiting, but
     * the picture still arrives for the next row that wants it.
     */
    suspend fun image(url: String, headers: Map<String, String>, widthPx: Int): Bitmap? {
        val key = "$url@$widthPx"
        memory.get(key)?.let { return it }
        val job = synchronized(inflight) {
            inflight.getOrPut(key) {
                work.async {
                    val file = File(folder, hash(url))
                    val kept = file.exists()
                    val bytes = runCatching { if (kept) readKept(file) else connections.withPermit { download(url, headers) } }.getOrNull()
                    val bitmap = bytes?.let { runCatching { decode(it, widthPx) }.getOrNull() }
                    // Only a picture that opens is kept, written under another name first so a half-written file is never taken for a whole one.
                    if (bitmap == null && kept) file.delete()
                    if (bitmap != null && !kept && !url.startsWith("file:")) keep(file, bytes)
                    if (bitmap != null) memory.put(key, bitmap)
                    synchronized(inflight) { inflight.remove(key) }
                    bitmap
                }
            }
        }
        return job.await()
    }

    private fun readKept(file: File): ByteArray {
        file.setLastModified(System.currentTimeMillis())
        return file.readBytes()
    }

    private fun keep(file: File, bytes: ByteArray) {
        val partial = File(folder, file.name + ".part")
        partial.writeBytes(bytes)
        partial.renameTo(file)
        trim()
    }

    private fun download(url: String, headers: Map<String, String>): ByteArray {
        val connection = URL(url).openConnection() as java.net.URLConnection
        connection.connectTimeout = 10_000
        connection.readTimeout = 30_000
        for ((name, value) in headers) connection.setRequestProperty(name, value)
        if (connection is HttpURLConnection && connection.responseCode !in 200..299) error("HTTP ${connection.responseCode}")
        return connection.getInputStream().use { it.readBytes() }
    }

    private fun decode(bytes: ByteArray, widthPx: Int): Bitmap =
        ImageDecoder.decodeBitmap(ImageDecoder.createSource(ByteBuffer.wrap(bytes))) { decoder, info, _ ->
            val width = info.size.width
            if (width > widthPx && widthPx > 0) decoder.setTargetSize(widthPx, (info.size.height.toLong() * widthPx / width).toInt().coerceAtLeast(1))
        }

    /** When the pictures kept on disk pass the limit, the ones looked at longest ago go. */
    private fun trim() {
        val files = folder.listFiles() ?: return
        var total = files.sumOf { it.length() }
        if (total <= DISK_LIMIT) return
        for (file in files.sortedBy { it.lastModified() }) {
            total -= file.length()
            file.delete()
            if (total <= DISK_LIMIT * 3 / 4) break
        }
    }

    private fun hash(text: String) = MessageDigest.getInstance("SHA-1").digest(text.toByteArray()).joinToString("") { "%02x".format(it) }
}
