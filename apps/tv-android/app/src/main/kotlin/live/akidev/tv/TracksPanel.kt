package live.akidev.tv

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Text
import java.util.Locale

/** One line of the panel: what the viewer reads, whether it is the one in use, and what picking it does. */
class Choice(val label: String, val selected: Boolean, val pick: () -> Unit)

/** The panel at the right of the player: the audio tracks, then the subtitles (Off, the ones in the file, then the ones from outside), then the source (which copy of the film is playing). */
@Composable
fun TracksPanel(audio: List<Choice>, subtitles: List<Choice>, subtitlesNote: String?, sources: List<Choice>, sourcesNote: String?) {
    // The first line takes the focus once, when the panel opens. Without this, the list made that line again each time it scrolled back
    // into view, and the line took the focus again: the remote jumped up while the viewer went down the list.
    var placed by remember { mutableStateOf(false) }
    Column(
        Modifier.fillMaxHeight().width(440.dp).background(Ink.Surface.copy(alpha = 0.96f), RoundedCornerShape(topStart = 16.dp, bottomStart = 16.dp)).padding(24.dp)
            .onFocusChanged { if (it.hasFocus) placed = true },
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (audio.size > 1) {
                item { PanelHeading("volume", "AUDIO") }
                itemsIndexed(audio) { index, choice -> PanelLine(choice, autofocus = !placed && index == 0) }
            }
            item { PanelHeading("subtitles", "SUBTITLES", Modifier.padding(top = 16.dp)) }
            itemsIndexed(subtitles) { index, choice -> PanelLine(choice, autofocus = !placed && audio.size <= 1 && index == 0) }
            if (subtitlesNote != null) item { Text(subtitlesNote, style = Type.Body, modifier = Modifier.padding(start = 8.dp)) }
            // The copy being played; picking another starts it again at the same place.
            item { PanelHeading("server", "SOURCE", Modifier.padding(top = 16.dp)) }
            itemsIndexed(sources) { _, choice -> PanelLine(choice, autofocus = false) }
            if (sourcesNote != null) item { Text(sourcesNote, style = Type.Body, modifier = Modifier.padding(start = 8.dp)) }
        }
    }
}

@Composable
fun PanelHeading(icon: String, name: String, modifier: Modifier = Modifier) {
    Row(modifier.padding(start = 8.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        AkiIcon(icon, size = 20.dp, tint = Ink.TextDim)
        Text(name, style = Type.Meta.copy(color = Ink.TextDim))
    }
}

/** One choice: a quiet row. The one in use has a tick in amber (it is state) and a raised background; none is filled white. */
@Composable
fun PanelLine(choice: Choice, autofocus: Boolean) {
    val focus = remember { FocusRequester() }
    if (autofocus) LaunchedEffect(Unit) { focus.requestFocus() }
    Focusable(choice.pick, RoundedCornerShape(12.dp), if (choice.selected) Ink.Raised else Color.Transparent, Modifier.fillMaxWidth().height(56.dp).focusRequester(focus)) {
        Row(Modifier.fillMaxSize().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            // The tick has a place of its own, so the names line up whether or not one is chosen.
            Box(Modifier.size(24.dp)) { if (choice.selected) AkiIcon("check-solid", tint = Ink.Accent) }
            Text(choice.label, style = Type.Body.copy(color = Ink.Text), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** "English", or "English · Commentary", or "Track 2" when the file names nothing. */
fun trackName(language: String?, label: String?, index: Int): String {
    val name = language?.takeIf { it.isNotEmpty() && it != "und" }?.let { Locale.forLanguageTag(it).getDisplayLanguage(Locale.ENGLISH).ifEmpty { it } }
    return listOfNotNull(name, label?.takeIf { it.isNotEmpty() && it != name }).joinToString("  ·  ").ifEmpty { "Track ${index + 1}" }
}
