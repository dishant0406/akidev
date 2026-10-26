package live.akidev.manga.design

import androidx.compose.animation.core.spring

/** The two springs everything moves with: a quick one for a press, a calmer one for a choice that settles into place. */
object AkiMotion {
    fun <T> press() = spring<T>(dampingRatio = 0.6f, stiffness = 700f)
    fun <T> settle() = spring<T>(dampingRatio = 0.78f, stiffness = 380f)
}
