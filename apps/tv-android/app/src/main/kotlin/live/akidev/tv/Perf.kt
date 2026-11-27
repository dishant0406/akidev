package live.akidev.tv

import android.os.Process
import android.os.SystemClock
import android.util.Log

/** Timing lines for `adb logcat -s AkiTV`: how long the engine took for a screen, and when the first things reached the screen. */
object Perf {
    private val seen = mutableSetOf<String>()

    fun log(what: String, ms: Long) { Log.i("AkiTV", "perf $what $ms ms") }

    /** The time since this app's process started, the first time `what` happens. */
    fun once(what: String) {
        if (seen.add(what)) log(what, SystemClock.elapsedRealtime() - Process.getStartElapsedRealtime())
    }
}
