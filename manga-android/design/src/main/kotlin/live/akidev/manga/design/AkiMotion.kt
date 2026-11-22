package live.akidev.manga.design

import android.provider.Settings
import androidx.compose.animation.core.spring
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext

/** The two springs everything moves with: a quick one for a press, a calmer one for a choice that settles into place. */
object AkiMotion {
    fun <T> press() = spring<T>(dampingRatio = 0.6f, stiffness = 700f)
    fun <T> settle() = spring<T>(dampingRatio = 0.78f, stiffness = 380f)
}

/** True when the person has turned animations off in the phone's settings ("Remove animations"). Movement and scale then become a fade or nothing. */
@Composable
fun akiReduceMotion(): Boolean {
    val resolver = LocalContext.current.contentResolver
    return remember { Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f }
}
