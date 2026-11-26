package live.akidev.manga.app

import android.os.Handler
import android.os.Looper
import android.view.FrameMetrics
import android.view.Window
import live.akidev.manga.design.AkiGlassBudget

/**
 * Finds out whether this phone's graphics chip can keep up with glass while a page moves. It watches the frames drawn while something
 * scrolls or slides: when half of them take longer than a frame and a half, glass is drawn the cheap way during motion from then on
 * (see [AkiGlassBudget]), and that is remembered. A phone that keeps up is never changed.
 */
class GlassGovernor(private val prefs: Preferences) {
    private var frames = 0
    private var late = 0

    init {
        AkiGlassBudget.slowGpu = prefs.slowGlass
    }

    fun watch(window: Window) {
        if (prefs.slowGlass) return
        window.addOnFrameMetricsAvailableListener({ _, metrics, _ -> judge(metrics) }, Handler(Looper.getMainLooper()))
    }

    private fun judge(metrics: FrameMetrics) {
        if (prefs.slowGlass || !AkiGlassBudget.isMoving) return
        if (metrics.getMetric(FrameMetrics.FIRST_DRAW_FRAME) == 1L) return
        frames++
        if (metrics.getMetric(FrameMetrics.TOTAL_DURATION) > LATE_NANOS) late++
        if (frames < WINDOW) return
        if (late * 2 >= frames) {
            prefs.slowGlass = true
            AkiGlassBudget.slowGpu = true
        }
        frames = 0
        late = 0
    }

    private companion object {
        /** A frame and a half at 60 frames a second. */
        const val LATE_NANOS = 25_000_000L
        const val WINDOW = 90
    }
}
