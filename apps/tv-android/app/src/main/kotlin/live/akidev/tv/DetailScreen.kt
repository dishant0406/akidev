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
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import uniffi.akitv.Detail
import uniffi.akitv.Episode
import uniffi.akitv.Title
import uniffi.akitv.detail
import uniffi.akitv.prepareStream

/** "Episode 3", or "S2 E3" from the second season on (and "S0 E1" for a special): what the card and the player call an episode. */
fun Episode.label(): String = if (season != null && season != 1u) "S$season E$number" else "Episode $number"

/**
 * The title page. The name, the facts and the picture show at once from the card the viewer opened; the story, the main button and
 * the episodes come when the title's page has loaded. Each has its own slot of fixed size, with a quiet shape in it until then,
 * so nothing moves when they arrive.
 */
@Composable
fun DetailScreen(title: Title, watched: List<Watched>, onPlay: (Title, playId: String, label: String?) -> Unit) {
    var tries by remember { mutableIntStateOf(0) }
    val loaded by rememberLoad("detail", title.id, tries) { detail(title.id) }
    val detail = (loaded as? Load.Done)?.value
    val shown = detail?.title ?: title
    val isFilm = title.id.startsWith("movie-")
    val episodes = detail?.episodes.orEmpty().sortedWith(compareBy({ it.season ?: 1u }, { it.number }))
    val seasons = episodes.map { it.season ?: 1u }.distinct()
    val target = detail?.let { whatToPlay(it, episodes, watched) }
    var season by remember(detail) { mutableIntStateOf((seasons.firstOrNull { it == (target?.season ?: 1u) } ?: seasons.firstOrNull() ?: 1u).toInt()) }
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
                    Crossfade(if (shown.description != null) 1 else if (loaded is Load.Loading) 0 else 2, animationSpec = Motion.go(300), label = "story") { state ->
                        when (state) {
                            1 -> Text(shown.description.orEmpty(), style = Type.Body, maxLines = 4, overflow = TextOverflow.Ellipsis)
                            0 -> Box(Modifier.fillMaxWidth().height(96.dp).padding(vertical = 8.dp).placeholder(RoundedCornerShape(8.dp)))
                        }
                    }
                }
            }
            // The main button's slot: a shape while loading, the button when ready, or why it could not be loaded.
            Box(Modifier.height(48.dp)) {
              Crossfade(if (target != null) 1 else if (loaded is Load.Failed) 2 else if (detail != null && !isFilm) 3 else 0, animationSpec = Motion.go(300), label = "main") { state ->
                when {
                    state == 1 && target != null -> {
                        // The text says what the button does, and when it can: which episode, and where it resumes.
                        val resumeAt = watched.resumeAt(target.id)
                        val note = listOfNotNull(target.label, if (resumeAt > 0) clock(resumeAt) else null).joinToString("  ·  ")
                        AkiButton(note.ifEmpty { "Play" }, Look.Primary, icon = "play-solid", labelAlways = true, autofocus = true) { onPlay(shown, target.id, target.label) }
                    }
                    state == 2 -> Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
                        AkiButton("Try again", Look.Primary, icon = "refresh", autofocus = true) { tries++ }
                        AkiIcon("warning", tint = Ink.Danger)
                        Text((loaded as Load.Failed).reason, style = Type.Body.copy(color = Ink.Danger), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    state == 3 -> Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) { AkiIcon("list", tint = Ink.TextDim); Text("No episodes were found for this title yet.", style = Type.Body) }
                    else -> Box(Modifier.size(56.dp, 48.dp).placeholder(RoundedCornerShape(8.dp)))
                }
              }
            }
            if (!isFilm) {
                // Seasons and episodes: their slots are there from the start (a series always has them), with shapes in until the list arrives.
                Row(Modifier.height(48.dp), horizontalArrangement = Arrangement.spacedBy(24.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Episodes", style = Type.RowTitle, modifier = Modifier.padding(start = 8.dp))
                    // Seasons only when there is more than one to choose from. The chosen one is outlined; the main button is the only filled one.
                    if (seasons.size > 1) LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        items(seasons) { s -> AkiButton("S$s", if (s.toInt() == season) Look.Secondary else Look.Text) { season = s.toInt() } }
                    }
                }
                Box(Modifier.height(140.dp)) {
                  Crossfade(detail != null, animationSpec = Motion.go(300), label = "episodes") { ready ->
                    if (!ready) Row(horizontalArrangement = Arrangement.spacedBy(14.dp), modifier = Modifier.padding(horizontal = 8.dp, vertical = 10.dp)) { repeat(4) { Box(Modifier.size(200.dp, 120.dp).placeholder(RoundedCornerShape(12.dp))) } }
                    else LazyRow(horizontalArrangement = Arrangement.spacedBy(14.dp), contentPadding = PaddingValues(horizontal = 8.dp, vertical = 10.dp)) {
                        items(episodes.filter { (it.season ?: 1u).toInt() == season }, key = { it.id }) { episode -> EpisodeCard(episode, watched) { onPlay(shown, episode.id, episode.label()) } }
                    }
                  }
                }
            }
        }
    }
}

/** What the main button plays: a film; for a series, the episode last watched if it is not finished, the one after it if it is, else the first. */
private fun whatToPlay(detail: Detail, episodes: List<Episode>, watched: List<Watched>): Target? {
    detail.playId?.let { return Target(it, null, null) }
    val last = watched.filter { it.titleId == detail.title.id }.maxByOrNull { it.at }
    val episode = if (last == null) episodes.firstOrNull()
    else if (!last.finished()) episodes.firstOrNull { it.id == last.playId }
    else episodes.firstOrNull { it.id == nextEpisode(episodes.map { e -> e.id }, last.playId) } ?: episodes.firstOrNull { it.id == last.playId }
    return episode?.let { Target(it.id, it.label(), it.season) }
}

private class Target(val id: String, val label: String?, val season: UInt?)

/** A wide card for one episode: its number, its name, a check when seen, and how far the viewer got. */
@Composable
private fun EpisodeCard(episode: Episode, watched: List<Watched>, onClick: () -> Unit) {
    val share = watched.progressOf(episode.id)
    val seen = watched.firstOrNull { it.playId == episode.id }?.finished() == true
    Focusable(onClick, RoundedCornerShape(12.dp), Ink.Surface, Modifier.size(200.dp, 120.dp)) {
        Box(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize().padding(12.dp), verticalArrangement = Arrangement.SpaceBetween) {
                Column {
                    Text("EPISODE", style = Type.Meta)
                    Text(episode.number.toString(), style = Type.Billboard.copy(fontSize = 28.sp, lineHeight = 36.sp))
                }
                if (episode.name != null) Text(episode.name!!, style = Type.Body, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            if (seen) AkiIcon("check-solid", Modifier.align(Alignment.TopEnd).padding(12.dp), size = 20.dp, tint = Ink.Accent)
            if (share != null && !seen) Progress(share, Modifier.align(Alignment.BottomStart))
        }
    }
}

private fun detailFacts(title: Title, runtimeMinutes: UInt?): String =
    listOfNotNull(
        title.year?.toString(),
        title.genres.take(3).joinToString(", ").ifEmpty { null },
        runtimeMinutes?.let { "$it MIN" },
        title.score?.let { "$it%" },
    ).joinToString("  ·  ").uppercase()
