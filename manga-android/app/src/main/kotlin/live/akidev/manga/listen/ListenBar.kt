package live.akidev.manga.listen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import live.akidev.manga.design.*

private val rates = listOf(0.75, 1.0, 1.25, 1.5, 1.75, 2.0)

/** The controls for reading aloud, over the page: sentence back, play or pause, sentence forward, speed, stop. */
@Composable
fun ListenBar(listener: BookListener) {
    Row(Modifier.fillMaxWidth().widthIn(max = 640.dp).padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally), verticalAlignment = Alignment.CenterVertically) {
        AkiIconButton("previous", "Previous sentence") { listener.previous() }
        if (listener.state == BookListener.State.Loading) {
            androidx.compose.foundation.layout.Box(Modifier.size(44.dp).akiGlass(AkiShapes.pill(44.dp)), contentAlignment = Alignment.Center) { AkiSpinner() }
        } else {
            val playing = listener.state == BookListener.State.Playing
            AkiIconButton(if (playing) "pause" else "play", if (playing) "Pause" else "Play", tint = Aki.colors.primary) { listener.toggle() }
        }
        AkiIconButton("next", "Next sentence") { listener.next() }
        AkiMenuPill(listener.rate, { listener.changeRate(it) }, rates.map { AkiOption(it, "${if (it == it.toInt().toDouble()) it.toInt().toString() else it.toString()}×") })
        AkiIconButton("close", "Stop reading aloud") { listener.stop() }
    }
}
