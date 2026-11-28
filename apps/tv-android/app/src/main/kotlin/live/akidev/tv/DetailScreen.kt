@file:OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)

package live.akidev.tv

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import uniffi.akitv.Detail
import uniffi.akitv.Episode
import uniffi.akitv.Title
import uniffi.akitv.detail
import uniffi.akitv.prepareStream

/** What a title page keeps while the player is on top of it: the episodes' scroll, the season and the episode the remote was on. */
class DetailState {
    val episodes = LazyListState()
    var season: Int? = null
    var focused: String? = null
}

/**
 * The title pages of the viewer's way back (kept so Back from the player lands on the same episode, in the same scroll): one `DetailState`
 * for each title, and the last answer of the engine for it, shown at once while the page asks again.
 */
class DetailMemory {
    private val states = mutableMapOf<String, DetailState>()
    val details = mutableMapOf<String, Detail>()
    fun of(titleId: String) = states.getOrPut(titleId) { DetailState() }
    /** The viewer left the title page: the next time starts at the top. */
    fun forget(titleId: String) { states.remove(titleId); details.remove(titleId) }
}

/** "Episode 3", or "S2 E3" from the second season on (and "S0 E1" for a special): what the card and the player call an episode. */
fun Episode.label(): String = if (season != null && season != 1u) "S$season E$number" else "Episode $number"

/**
 * The title page. The name, the facts and the picture show at once from the card the viewer opened; the story, the main button and
 * the episodes come when the title's page has loaded. Each has its own slot of fixed size, with a quiet shape in it until then,
 * so nothing moves when they arrive.
 */
@Composable
fun DetailScreen(title: Title, watched: List<Watched>, memory: DetailMemory, onPlay: (Title, playId: String, label: String?) -> Unit) {
    var tries by remember { mutableIntStateOf(0) }
    val kept = remember { memory.of(title.id) }
    val loaded by rememberLoad("detail", title.id, tries) { detail(title.id).also { memory.details[title.id] = it } }
    // Coming back from the player shows the last answer at once (the page still asks again), so the episodes are there to keep their scroll.
    val detail = (loaded as? Load.Done)?.value ?: memory.details[title.id]
    val loading = detail == null && loaded is Load.Loading
    val shown = detail?.title ?: title
    val isFilm = title.id.startsWith("movie-")
    val episodes = detail?.episodes.orEmpty().sortedWith(compareBy({ it.season ?: 1u }, { it.number }))
    val seasons = episodes.map { it.season ?: 1u }.distinct()
    val target = detail?.let { whatToPlay(it, episodes, watched) }
    // The season the viewer chose stays; the first time it is the one the main button plays.
    var season by remember { mutableIntStateOf(kept.season ?: (seasons.firstOrNull { it == (target?.season ?: 1u) } ?: seasons.firstOrNull() ?: 1u).toInt()) }
    LaunchedEffect(detail == null) { if (kept.season == null && detail != null) season = (seasons.firstOrNull { it == (target?.season ?: 1u) } ?: seasons.firstOrNull() ?: 1u).toInt() }
    val sideways = rememberLeastToShow()
    // While the viewer reads the page, the engine already looks for the copy the main button will play.
    LaunchedEffect(target?.id) { target?.let { prepareStream(it.id) } }

    Box(Modifier.fillMaxSize().background(Ink.Canvas)) {
        Backdrop(shown.banner ?: shown.poster, fallback = shown.poster)
        Column(Modifier.fillMaxSize().padding(horizontal = 48.dp, vertical = 32.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.height(50.dp)) { Text(shown.name, style = Type.Billboard, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                Box(Modifier.height(22.dp)) { Text(detailFacts(shown, detail?.runtimeMinutes), style = Type.Meta, maxLines = 1) }
                // Each slot fades from its shape to its content when the page has loaded.
                Box(Modifier.height(104.dp).width(640.dp)) {
                    Crossfade(if (shown.description != null) 1 else if (loading) 0 else 2, animationSpec = Motion.go(300), label = "story") { state ->
                        when (state) {
                            1 -> Text(shown.description.orEmpty(), style = Type.Body, maxLines = 4, overflow = TextOverflow.Ellipsis)
                            0 -> Box(Modifier.fillMaxWidth().height(96.dp).padding(vertical = 8.dp).placeholder(RoundedCornerShape(8.dp)))
                        }
                    }
                }
            }
            // The main button's slot: a shape while loading, the button when ready, or why it could not be loaded.
            Box(Modifier.height(48.dp)) {
              Crossfade(if (shown.comingSoon != null) 5 else if (target != null) 1 else if (detail == null && loaded is Load.Failed) 2 else if (detail != null && !isFilm) (if (episodes.isEmpty()) 3 else 4) else 0, animationSpec = Motion.go(300), label = "main") { state ->
                when {
                    state == 1 && target != null -> {
                        // The text says what the button does, and when it can: which episode, and where it resumes.
                        val resumeAt = watched.resumeAt(target.id)
                        val note = listOfNotNull(target.label, if (resumeAt > 0) clock(resumeAt) else null).joinToString("  ·  ")
                        // The remote starts here, unless the viewer comes back from an episode: then it is on that episode.
                        AkiButton(note.ifEmpty { "Play" }, Look.Primary, icon = "play-solid", labelAlways = true, autofocus = kept.focused == null) { kept.focused = null; onPlay(shown, target.id, target.label) }
                    }
                    state == 2 -> Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
                        AkiButton("Try again", Look.Primary, icon = "refresh", autofocus = true) { tries++ }
                        AkiIcon("warning", tint = Ink.Danger)
                        Text((loaded as Load.Failed).reason, style = Type.Body.copy(color = Ink.Danger), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    state == 3 -> Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) { AkiIcon("list", tint = Ink.TextDim); Text("No episodes were found for this title yet.", style = Type.Body) }
                    // A title that is not out yet: when it comes, where Play would be.
                    state == 5 -> Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) { AkiIcon("clock", tint = Ink.TextDim); Text(shown.comingSoon.orEmpty(), style = Type.Body) }
                    state == 4 -> Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) { AkiIcon("clock", tint = Ink.TextDim); Text("No episode has aired yet.", style = Type.Body) }
                    else -> Box(Modifier.size(56.dp, 48.dp).placeholder(RoundedCornerShape(8.dp)))
                }
              }
            }
            if (!isFilm) {
                // Seasons and episodes: their slots are there from the start (a series always has them), with shapes in until the list arrives.
                // 56 dp: a season button is 48 dp, and it needs 4 dp over and under it to grow when the remote is on it.
                Row(Modifier.height(56.dp), horizontalArrangement = Arrangement.spacedBy(24.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Episodes", style = Type.RowTitle, modifier = Modifier.padding(start = 8.dp))
                    // Seasons only when there is more than one to choose from. The chosen one is outlined; the main button is the only filled one.
                    if (seasons.size > 1) LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp), contentPadding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                        items(seasons) { s -> AkiButton("S$s", if (s.toInt() == season) Look.Secondary else Look.Text) { season = s.toInt(); kept.season = season } }
                    }
                    // How many are out, when some are not.
                    val out = episodes.count { it.released }
                    if (out < episodes.size) Text("$out of ${episodes.size} out", style = Type.Meta, maxLines = 1)
                }
                Box(Modifier.height(140.dp)) {
                  Crossfade(detail != null, animationSpec = Motion.go(300), label = "episodes") { ready ->
                    if (!ready) Row(horizontalArrangement = Arrangement.spacedBy(14.dp), modifier = Modifier.padding(horizontal = 8.dp, vertical = 10.dp)) { repeat(4) { Box(Modifier.size(200.dp, 120.dp).placeholder(RoundedCornerShape(12.dp))) } }
                    else CompositionLocalProvider(LocalBringIntoViewSpec provides sideways) {
                        LazyRow(state = kept.episodes, horizontalArrangement = Arrangement.spacedBy(14.dp), contentPadding = PaddingValues(horizontal = 8.dp, vertical = 10.dp)) {
                            items(episodes.filter { (it.season ?: 1u).toInt() == season }, key = { it.id }) { episode ->
                                EpisodeCard(episode, watched, autofocus = kept.focused == episode.id, onFocus = { kept.focused = episode.id }) { onPlay(shown, episode.id, episode.label()) }
                            }
                        }
                    }
                  }
                }
            }
        }
    }
}

/** What the main button plays: a film; for a series, the episode last watched if it is not finished, the one after it if it is, else the first. */
private fun whatToPlay(detail: Detail, all: List<Episode>, watched: List<Watched>): Target? {
    detail.playId?.let { return Target(it, null, null) }
    // An episode that has not aired cannot be played.
    val episodes = all.filter { it.released }
    val last = watched.filter { it.titleId == detail.title.id }.maxByOrNull { it.at }
    val episode = if (last == null) episodes.firstOrNull()
    else if (!last.finished()) episodes.firstOrNull { it.id == last.playId }
    else episodes.firstOrNull { it.id == nextEpisode(episodes.map { e -> e.id }, last.playId) } ?: episodes.firstOrNull { it.id == last.playId }
    return episode?.let { Target(it.id, it.label(), it.season) }
}

private class Target(val id: String, val label: String?, val season: UInt?)

/** A wide card for one episode: its number, its name, a check when seen, and how far the viewer got. One that has not aired is dimmed, says when, and does nothing. */
@Composable
private fun EpisodeCard(episode: Episode, watched: List<Watched>, autofocus: Boolean, onFocus: () -> Unit, onClick: () -> Unit) {
    val share = watched.progressOf(episode.id)
    val seen = watched.firstOrNull { it.playId == episode.id }?.finished() == true
    val focus = remember { FocusRequester() }
    if (autofocus) LaunchedEffect(Unit) { focus.requestFocus() }
    Focusable(if (episode.released) onClick else ({}), RoundedCornerShape(12.dp), Ink.Surface, Modifier.size(200.dp, 120.dp).focusRequester(focus).onFocusChanged { if (it.isFocused) onFocus() }) {
        Box(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize().padding(12.dp).alpha(if (episode.released) 1f else 0.5f), verticalArrangement = Arrangement.SpaceBetween) {
                Column {
                    Text("EPISODE", style = Type.Meta)
                    Text(episode.number.toString(), style = Type.Billboard.copy(fontSize = 28.sp, lineHeight = 36.sp))
                }
                if (episode.name != null) Text(episode.name!!, style = Type.Body, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            if (!episode.released) Row(Modifier.align(Alignment.TopEnd).padding(12.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                AkiIcon("clock", size = 16.dp, tint = Ink.TextDim)
                Text(airDay(episode.airdate) ?: "SOON", style = Type.Meta.copy(letterSpacing = 0.sp), maxLines = 1)
            }
            else if (seen) AkiIcon("check-solid", Modifier.align(Alignment.TopEnd).padding(12.dp), size = 20.dp, tint = Ink.Accent)
            if (share != null && !seen) Progress(share, Modifier.align(Alignment.BottomStart))
        }
    }
}

private val Months = listOf("JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC")

/** "2026-12-04" as "4 DEC", or null when the date is not there or does not read. */
fun airDay(airdate: String?): String? {
    val parts = airdate?.take(10)?.split("-") ?: return null
    val month = parts.getOrNull(1)?.toIntOrNull()?.takeIf { it in 1..12 } ?: return null
    val day = parts.getOrNull(2)?.toIntOrNull() ?: return null
    return "$day ${Months[month - 1]}"
}

private fun detailFacts(title: Title, runtimeMinutes: UInt?): String =
    listOfNotNull(
        title.comingSoon ?: title.year?.toString(),
        title.genres.take(3).joinToString(", ").ifEmpty { null },
        runtimeMinutes?.let { "$it MIN" },
        title.score?.let { "$it%" },
    ).joinToString("  ·  ").uppercase()
