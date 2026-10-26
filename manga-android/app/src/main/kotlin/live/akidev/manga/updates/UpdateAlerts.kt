package live.akidev.manga.updates

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import live.akidev.manga.MainActivity
import live.akidev.manga.manga.chapterName
import uniffi.akimanga.ChapterEntry

/** One series that got chapters since it was last asked, and the newest of them. */
class NewChapters(val source: String, val series: String, val title: String, val adult: Boolean, /** Oldest first. */ val chapters: List<ChapterEntry>)

/**
 * Local notifications for new chapters. They are posted by the app itself when Android lets it look for updates, so nothing leaves the
 * device and no server is involved; the price is that Android, not Akidev, decides when that happens.
 */
object UpdateAlerts {
    private const val CHANNEL = "new-chapters"

    /** Whether the app may post notifications now (Android 13 asks the person; before it they are always allowed unless turned off). */
    fun allowed(context: Context): Boolean =
        (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU || ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) &&
            NotificationManagerCompat.from(context).areNotificationsEnabled()

    /**
     * One notification for a whole refresh, however many series it found chapters for. Titles of 18+ sources stay off the lock screen.
     * Tapping it opens the app on the Updates tab.
     */
    fun post(context: Context, found: List<NewChapters>) {
        if (found.isEmpty() || !allowed(context)) return
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(CHANNEL, "New chapters", NotificationManager.IMPORTANCE_DEFAULT))
        val open = PendingIntent.getActivity(
            context, 0, Intent(context, MainActivity::class.java).putExtra(OPEN_UPDATES, true).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_more).setContentTitle("New chapters").setContentText(body(found))
            .setContentIntent(open).setAutoCancel(true).build()
        manager.notify(1, notification)
    }

    const val OPEN_UPDATES = "openUpdates"

    /** "Solo Leveling · Chapter 179", "Solo Leveling · 3 new chapters", "Solo Leveling, Tower of God and 2 more". */
    fun body(found: List<NewChapters>): String {
        val count = found.sumOf { it.chapters.size }
        if (found.any { it.adult }) return if (count == 1) "1 new chapter in your library." else "$count new chapters in your library."
        if (found.size == 1) {
            val only = found[0]
            val what = if (only.chapters.size == 1) chapterName(only.chapters[0].number, only.chapters[0].title) else "${only.chapters.size} new chapters"
            return "${only.title} · $what"
        }
        val names = found.take(2).map { it.title }
        val rest = found.size - names.size
        return if (rest > 0) names.joinToString(", ") + " and $rest more" else names.joinToString(" and ")
    }
}
