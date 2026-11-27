package live.akidev.tv

import android.content.Context
import android.provider.Settings
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.TweenSpec
import androidx.compose.animation.core.tween

/**
 * How long the app's animations last. Compose multiplies the length of every animation by the system's "animator duration scale", and a
 * TV often ships with it at 0.5: a 260 ms fade then lasts 130 ms and reads as a cut (measured on the Lumio Vision: a tab switch moved for
 * 0.27 s at 0.5 and for 0.8 s at 1.0). So when the system speeds animations up, the app asks for proportionally more time and the result is
 * the length meant here. A setting of 0 ("remove animations") is respected: nothing moves.
 */
object Motion {
    private var factor = 1f

    /** Reads the system setting; called when the app starts and each time it comes back to the front. */
    fun refresh(context: Context) {
        factor = Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
    }

    /** `base` milliseconds as the viewer should see them. */
    fun ms(base: Int): Int = if (factor in 0.01f..0.99f) (base / factor).toInt() else base

    /** Starts fast and settles gently: the curve of everything that arrives or changes (`cubic-bezier(0.2, 0, 0, 1)`). */
    private val out = CubicBezierEasing(0.2f, 0f, 0f, 1f)

    /** A tween of `base` ms with the app's curve. Enters take longer than exits: a small thing about 150 ms, a menu 250, a panel 350 to 450. */
    fun <T> go(base: Int, delay: Int = 0): TweenSpec<T> = tween(ms(base), ms(delay), out)
}
