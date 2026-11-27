package live.akidev.tv

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.net.Uri
import android.os.Build
import android.os.SystemClock
import android.provider.Settings
import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.withContext
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject

/** A release of this app on GitHub. `sha256` is what GitHub says the file is (null when it says nothing). */
class AppRelease(val version: String, val apkUrl: String, val size: Long, val sha256: String?, val notes: String)

/**
 * Updates of the app, from the GitHub releases of its repository (made with `fastlane release`, see fastlane/Fastfile). The newest release
 * whose tag starts with `tv-android-v` and that holds an APK counts. It is looked for when the app starts and when it comes back to the
 * screen (not more often than every 30 minutes), the answer is kept for the next start, and Settings offers it when it is newer than this
 * app. Installing streams the APK straight into Android's installer, so no APK is kept on the TV, and Android installs it only when it is
 * signed with the same key as this app: nothing else can replace the app.
 */
object Updates {
    private const val PREFS = "updates"
    private const val CHECK_EVERY_MS = 30 * 60 * 1000L
    const val TAG_PREFIX = "tv-android-v"

    /** A reason the last install did not go through, set by the installer's answer (see `InstallStatusReceiver`). */
    val problem = MutableStateFlow<String?>(null)

    /** "1.4.2" as [1, 4, 2]; "1.0" is [1, 0, 0]; null when it is not numbers. */
    fun parts(version: String): List<Int>? {
        val numbers = version.removePrefix("v").split(".").map { it.toIntOrNull() ?: return null }
        return numbers + List((3 - numbers.size).coerceAtLeast(0)) { 0 }
    }

    fun isNewer(candidate: String, installed: String): Boolean {
        val a = parts(candidate) ?: return false
        val b = parts(installed) ?: return false
        for (i in 0 until maxOf(a.size, b.size)) {
            val (x, y) = (a.getOrElse(i) { 0 }) to (b.getOrElse(i) { 0 })
            if (x != y) return x > y
        }
        return false
    }

    /** The highest version among the releases of this app that are published (not a draft or a pre-release) and hold an APK. */
    fun pick(json: String): AppRelease? {
        val list = JSONArray(json)
        var best: AppRelease? = null
        for (i in 0 until list.length()) {
            val release = list.getJSONObject(i)
            val tag = release.optString("tag_name")
            if (!tag.startsWith(TAG_PREFIX) || release.optBoolean("draft") || release.optBoolean("prerelease")) continue
            val version = tag.removePrefix(TAG_PREFIX)
            val apk = apkOf(release.optJSONArray("assets") ?: continue) ?: continue
            if (parts(version) == null || (best != null && !isNewer(version, best.version))) continue
            best = AppRelease(version, apk.getString("browser_download_url"), apk.optLong("size", -1), apk.optString("digest").removePrefix("sha256:").ifEmpty { null }, release.optString("body"))
        }
        return best
    }

    private fun apkOf(assets: JSONArray): JSONObject? =
        (0 until assets.length()).map { assets.getJSONObject(it) }.firstOrNull { it.optString("name").endsWith(".apk") }

    /** What an earlier check found, if it is newer than this app. Read at once at the start, so the sign shows before the network answers. */
    fun cached(context: Context): AppRelease? {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val version = prefs.getString("version", null) ?: return null
        val url = prefs.getString("url", null) ?: return null
        if (!isNewer(version, BuildConfig.VERSION_NAME)) return null
        return AppRelease(version, url, prefs.getLong("size", -1), prefs.getString("sha256", null), prefs.getString("notes", "").orEmpty())
    }

    /** Looks at GitHub (at most every 30 minutes) and returns the newer release, or null. A failed look keeps what is known. */
    suspend fun check(context: Context): AppRelease? = withContext(Dispatchers.IO) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val now = System.currentTimeMillis()
        if (now - prefs.getLong("checkedAt", 0) < CHECK_EVERY_MS) return@withContext cached(context)
        try {
            val request = Request.Builder().url(BuildConfig.UPDATE_URL).header("Accept", "application/vnd.github+json").build()
            val body = HttpClient.video.newCall(request).execute().use { if (it.isSuccessful) it.body?.string() else null } ?: return@withContext cached(context)
            val found = pick(body)
            prefs.edit().putLong("checkedAt", now).apply {
                if (found == null) remove("version") else putString("version", found.version).putString("url", found.apkUrl).putLong("size", found.size)
                    .putString("sha256", found.sha256).putString("notes", found.notes)
            }.apply()
        } catch (_: Exception) {
            // Offline or GitHub is busy: the next look is at the next start.
        }
        cached(context)
    }

    /**
     * Downloads the release into Android's installer and asks it to install. Returns a sentence when it cannot start (the installer
     * itself answers later, see `InstallStatusReceiver`), null when the installer's own screen is on its way.
     */
    suspend fun install(context: Context, release: AppRelease, onProgress: (Float) -> Unit): String? = withContext(Dispatchers.IO) {
        problem.value = null
        if (!context.packageManager.canRequestPackageInstalls()) {
            // The viewer must allow this once, in Android's own screen.
            context.startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            return@withContext "Allow this app to install updates, then press the button again."
        }
        val installer = context.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
            setAppPackageName(context.packageName)
            setSize(release.size)
        }
        val id = installer.createSession(params)
        try {
            installer.openSession(id).use { session ->
                val digest = MessageDigest.getInstance("SHA-256")
                HttpClient.video.newCall(Request.Builder().url(release.apkUrl).build()).execute().use { response ->
                    if (!response.isSuccessful) throw IllegalStateException("The download failed (${response.code}).")
                    val body = response.body ?: throw IllegalStateException("The download was empty.")
                    val total = release.size.takeIf { it > 0 } ?: body.contentLength()
                    session.openWrite("update.apk", 0, total).use { out ->
                        val buffer = ByteArray(64 * 1024)
                        var done = 0L
                        var lastReport = 0L
                        body.byteStream().use { input ->
                            while (true) {
                                val n = input.read(buffer)
                                if (n < 0) break
                                out.write(buffer, 0, n)
                                digest.update(buffer, 0, n)
                                done += n
                                val now = SystemClock.elapsedRealtime()
                                if (total > 0 && now - lastReport > 250) { lastReport = now; onProgress((done.toFloat() / total).coerceIn(0f, 1f)) }
                            }
                        }
                        session.fsync(out)
                    }
                }
                val got = digest.digest().joinToString("") { "%02x".format(it) }
                if (release.sha256 != null && !got.equals(release.sha256, ignoreCase = true)) throw IllegalStateException("The download is damaged. Try again.")
                onProgress(1f)
                val answer = PendingIntent.getBroadcast(context, id, Intent(context, InstallStatusReceiver::class.java), PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
                session.commit(answer.intentSender)
            }
            null
        } catch (e: Exception) {
            runCatching { installer.abandonSession(id) }
            e.message?.takeIf { it.isNotBlank() } ?: "The update could not be downloaded."
        }
    }
}

/** The installer's answers: the screen that asks the viewer to confirm, then success or the reason it failed. */
class InstallStatusReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                @Suppress("DEPRECATION")
                val confirm = if (Build.VERSION.SDK_INT >= 33) intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java) else intent.getParcelableExtra(Intent.EXTRA_INTENT)
                confirm?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)?.let(context::startActivity)
            }
            PackageInstaller.STATUS_SUCCESS -> Unit
            // The viewer pressed Cancel: that is their choice, not an error.
            PackageInstaller.STATUS_FAILURE_ABORTED -> Unit
            // Android's own text is for developers ("INSTALL_FAILED_..."), so the screen gets a plain sentence.
            else -> Updates.problem.value = "The update was not installed. Try again."
        }
    }
}
