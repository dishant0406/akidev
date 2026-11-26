package live.akidev.manga.listen

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.IBinder
import live.akidev.manga.MainActivity

/** What the lock screen, the notification and the headphones' buttons ask of the voice that is reading. */
interface ListenControls {
    fun play()
    fun pause()
    fun next()
    fun previous()
    fun stop()
}

/**
 * The card on the lock screen and in the notification shade, and the headphones' buttons: a media session, and a foreground service
 * that keeps the voice going when the screen is off. Only one book is read aloud at a time.
 */
object ListenSession {
    private const val CHANNEL = "reading-aloud"
    internal const val NOTIFICATION = 2
    private const val REQUEST_OPEN = 2
    internal const val ACTION_TOGGLE = "live.akidev.manga.listen.TOGGLE"
    internal const val ACTION_NEXT = "live.akidev.manga.listen.NEXT"
    internal const val ACTION_PREVIOUS = "live.akidev.manga.listen.PREVIOUS"

    private var session: MediaSession? = null
    private var controls: ListenControls? = null
    private var title = ""
    private var author: String? = null
    private var playing = false

    /** Starts the card and the service for a book being read aloud. */
    fun start(context: Context, title: String, author: String?, controls: ListenControls) {
        this.controls = controls
        this.title = title
        this.author = author
        val made = MediaSession(context, "Akidev").apply {
            setCallback(object : MediaSession.Callback() {
                override fun onPlay() = controls.play()
                override fun onPause() = controls.pause()
                override fun onSkipToNext() = controls.next()
                override fun onSkipToPrevious() = controls.previous()
                override fun onStop() = controls.stop()
            })
            setMetadata(MediaMetadata.Builder().putString(MediaMetadata.METADATA_KEY_TITLE, title).putString(MediaMetadata.METADATA_KEY_ARTIST, author).build())
            isActive = true
        }
        session = made
        context.startForegroundService(Intent(context, ListenService::class.java))
    }

    /** Tells the card whether it is playing. */
    fun update(context: Context, playing: Boolean) {
        this.playing = playing
        val actions = PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_SKIP_TO_NEXT or PlaybackState.ACTION_SKIP_TO_PREVIOUS or PlaybackState.ACTION_STOP
        session?.setPlaybackState(PlaybackState.Builder().setActions(actions).setState(if (playing) PlaybackState.STATE_PLAYING else PlaybackState.STATE_PAUSED, PlaybackState.PLAYBACK_POSITION_UNKNOWN, if (playing) 1f else 0f).build())
        context.getSystemService(NotificationManager::class.java).notify(NOTIFICATION, notification(context))
    }

    /** Whether a book is being read aloud. */
    val active get() = session != null

    fun stop(context: Context) {
        session?.release()
        session = null
        controls = null
        context.stopService(Intent(context, ListenService::class.java))
    }

    internal fun notification(context: Context): Notification {
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(CHANNEL, "Reading aloud", NotificationManager.IMPORTANCE_LOW))
        fun action(name: String, label: String, icon: Int): Notification.Action =
            Notification.Action.Builder(android.graphics.drawable.Icon.createWithResource(context, icon), label, PendingIntent.getService(context, name.hashCode(), Intent(context, ListenService::class.java).setAction(name), PendingIntent.FLAG_IMMUTABLE)).build()
        val open = PendingIntent.getActivity(context, REQUEST_OPEN, Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE)
        return Notification.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_media_play).setContentTitle(title).setContentText(author).setContentIntent(open).setOngoing(playing)
            .addAction(action(ACTION_PREVIOUS, "Previous sentence", android.R.drawable.ic_media_previous))
            .addAction(action(ACTION_TOGGLE, if (playing) "Pause" else "Play", if (playing) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play))
            .addAction(action(ACTION_NEXT, "Next sentence", android.R.drawable.ic_media_next))
            .setStyle(Notification.MediaStyle().setMediaSession(session?.sessionToken).setShowActionsInCompactView(0, 1, 2))
            .build()
    }

    internal fun handle(action: String?) {
        val controls = controls ?: return
        when (action) {
            ACTION_TOGGLE -> if (playing) controls.pause() else controls.play()
            ACTION_NEXT -> controls.next()
            ACTION_PREVIOUS -> controls.previous()
        }
    }
}

/** Keeps the app alive, with a notification, while a book is read aloud. */
class ListenService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action != null) ListenSession.handle(intent.action)
        // Android wants this call after every start, even when reading already ended: then it goes straight away.
        startForeground(ListenSession.NOTIFICATION, ListenSession.notification(this), ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        if (!ListenSession.active) stopSelf()
        return START_NOT_STICKY
    }
}
