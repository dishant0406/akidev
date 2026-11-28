@file:OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class, androidx.compose.ui.ExperimentalComposeUiApi::class)

package live.akidev.tv

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Text
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.akitv.Title
import uniffi.akitv.TvException
import uniffi.akitv.browse
import uniffi.akitv.browseOptions

/** What a "View all" list holds: the heading, the catalogue (`null` for all of them) and how it starts out. */
data class BrowseTarget(val heading: String, val kind: String?, val sort: String?, val genre: String?)

/**
 * What a browse screen keeps while a title page or the player is on top of it: the filters, the titles loaded so far, the scroll and the
 * title the remote was on. It lives in the `Screen` on the stack, so Back lands in the same place.
 */
class BrowseState(target: BrowseTarget) {
    var sort by mutableStateOf(target.sort)
    var genre by mutableStateOf(target.genre)
    var titles by mutableStateOf<List<Title>>(emptyList())
    var more by mutableStateOf(false)
    var page by mutableIntStateOf(0)
    /** The first page: waiting, failed, or done. The next pages show on the button instead. */
    var first by mutableStateOf<Load<Unit>>(Load.Loading)
    var loadingMore by mutableStateOf(false)
    var moreFailed by mutableStateOf<String?>(null)
    var tries by mutableIntStateOf(0)
    var focused: String? = null
    val grid = LazyGridState()
    /** The filters the titles belong to, so coming back does not ask again. */
    var loadedFor: List<Any?>? = null
}

/** One page of the catalogue, off the main thread. */
private suspend fun page(target: BrowseTarget, sort: String, genre: String?, number: Int) = withContext(Dispatchers.IO) { browse(target.kind, sort, genre, number.toUInt()) }

private fun reason(e: Exception) = if (e is TvException.Failed) e.reason else e.message ?: "Something went wrong."

/**
 * The full list of a catalogue (or of all of them), as on the website. The glass bar at the top is the one of the first screens, with
 * the ways to order the list as icons and one more icon that opens the genres. Under it: the name of the catalogue, what is chosen in
 * words, the grid, and "Show more" under the last title. Pressing OK on a title opens it.
 */
@Composable
fun BrowseScreen(target: BrowseTarget, state: BrowseState, onOpen: (Title) -> Unit) {
    val scope = rememberCoroutineScope()
    val options by rememberLoad("browse options", target.kind) { browseOptions(target.kind) }
    val ready = (options as? Load.Done)?.value
    val sort = state.sort ?: ready?.sorts?.firstOrNull()?.sort
    val sorts = ready?.sorts.orEmpty()
    val sideways = rememberLeastToShow()
    // The first of the titles "Show more" brought: the remote goes there, because the button itself moves down, out of sight.
    var jumpTo by remember { mutableStateOf<String?>(null) }
    // The bar of this screen: the ways to order the list, then the genres (a panel, because there are many of them).
    val bar = remember(sorts) { sorts.map { BarTab(it.label, sortIcon(it.sort), null) } + BarTab("Genre", "filters", null) }
    val barFocus = remember(bar.size) { List(bar.size) { FocusRequester() } }
    var genresOpen by remember { mutableStateOf(false) }
    fun closeGenres() {
        genresOpen = false
        barFocus.last().requestFocus()
    }
    BackHandler(genresOpen) { closeGenres() }
    // Only one card takes the first focus, once for each visit of the screen: a card scrolled back into view, or the list coming
    // again after a filter was changed, must not take the focus from the viewer.
    var focusPlaced by remember { mutableStateOf(false) }

    // The first page, again each time a filter changes (and not when the viewer comes back from a title).
    LaunchedEffect(sort, state.genre, state.tries) {
        val now = listOf(sort, state.genre, state.tries)
        if (sort == null || state.loadedFor == now) return@LaunchedEffect
        state.first = Load.Loading
        state.titles = emptyList()
        state.moreFailed = null
        state.focused = null
        try {
            val got = page(target, sort, state.genre, 1)
            state.titles = got.titles.distinctBy { it.id }
            state.more = got.more
            state.page = 1
            state.loadedFor = now
            state.first = Load.Done(Unit)
        } catch (e: Exception) {
            state.first = Load.Failed(reason(e))
        }
    }

    fun showMore() {
        val picked = sort ?: return
        state.loadingMore = true
        state.moreFailed = null
        scope.launch {
            try {
                val got = page(target, picked, state.genre, state.page + 1)
                // A title can come on two pages (the order shifts between asks): each shows once.
                val all = (state.titles + got.titles).distinctBy { it.id }
                jumpTo = all.getOrNull(state.titles.size)?.id
                state.titles = all
                state.more = got.more
                state.page += 1
            } catch (e: Exception) {
                state.moreFailed = reason(e)
            }
            state.loadingMore = false
        }
    }

    Box(Modifier.fillMaxSize().background(Ink.Canvas)) {
        Column(Modifier.fillMaxSize().padding(horizontal = 48.dp)) {
            // The band of the bar: the name of the catalogue at the left, what is chosen at the right, in words (the icons alone do not
            // say it when the remote is in the grid).
            Row(Modifier.fillMaxWidth().padding(top = 16.dp).height(56.dp).padding(horizontal = 8.dp), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Text(target.heading, style = Type.Billboard, maxLines = 1)
                Column(horizontalAlignment = Alignment.End) {
                    Text(sorts.firstOrNull { it.sort == sort }?.label.orEmpty().uppercase(), style = Type.Meta.copy(color = Ink.TextDim), maxLines = 1)
                    Text((state.genre ?: "All genres").uppercase(), style = Type.Meta, maxLines = 1)
                }
            }
            Box(Modifier.fillMaxSize()) {
                when {
                    options is Load.Failed -> StateCard("The filters did not load", (options as Load.Failed).reason, icon = "warning")
                    state.first is Load.Failed -> StateCard("The list did not load", (state.first as Load.Failed).reason, icon = "warning") { AkiButton("Try again", Look.Primary, icon = "refresh", autofocus = true) { state.tries++ } }
                    state.first is Load.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Spinner() }
                    state.titles.isEmpty() -> StateCard("Nothing here", "No title matches this genre. Choose another one from the bar.", icon = "list")
                    else -> Box { CompositionLocalProvider(LocalBringIntoViewSpec provides sideways) {
                        LazyVerticalGrid(
                            columns = GridCells.Fixed(6),
                            state = state.grid,
                            // Room at the edges for the card that grows, and under the last row.
                            contentPadding = PaddingValues(start = 8.dp, end = 8.dp, top = 20.dp, bottom = 48.dp),
                            horizontalArrangement = Arrangement.spacedBy(16.dp),
                            verticalArrangement = Arrangement.spacedBy(24.dp),
                            modifier = Modifier.fillMaxSize(),
                        ) {
                            // Coming back from a title, the remote is on that title; the first time it is on the first one.
                            val wanted = state.focused?.takeIf { id -> state.titles.any { it.id == id } } ?: state.titles.firstOrNull()?.id
                            items(state.titles, key = { it.id }) { title ->
                                PosterCard(title, autofocus = (!focusPlaced && title.id == wanted) || title.id == jumpTo, onFocus = { focusPlaced = true; jumpTo = null; state.focused = title.id }) { onOpen(title) }
                            }
                            // As on the website: a hairline with how many are shown, and the button under it.
                            item(key = "more", span = { GridItemSpan(maxLineSpan) }) {
                                Column(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
                                    Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
                                        Box(Modifier.size(200.dp, 1.dp).background(Ink.Line))
                                        Text("${state.titles.size} SHOWN", style = Type.Meta)
                                        Box(Modifier.size(200.dp, 1.dp).background(Ink.Line))
                                    }
                                    // The button stays while the next page loads (and does nothing then): taking it away would drop the remote's focus.
                                    if (state.more || state.moreFailed != null) AkiButton(if (state.loadingMore) "Loading" else "Show more", Look.Secondary, icon = if (state.loadingMore) "refresh" else "down", labelAlways = true) { if (!state.loadingMore) showMore() }
                                    state.moreFailed?.let { Text(it, style = Type.Body.copy(color = Ink.Danger), maxLines = 1) }
                                }
                            }
                        }
                        // The posters fade out under the bar, so they do not end in a hard line.
                        Box(Modifier.fillMaxWidth().height(20.dp).background(Brush.verticalGradient(listOf(Ink.Canvas, Color.Transparent))))
                    }
                    }
                }
            }
        }
        // Drawn after the list, so the name that floats under an icon is over the posters.
        if (ready != null) TopBar(
            bar,
            selected = sorts.indexOfFirst { it.sort == sort }.coerceAtLeast(0),
            onPick = { index -> if (index == bar.lastIndex) genresOpen = true else state.sort = sorts[index].sort },
            // The amber dot says a genre is chosen (amber is for state).
            badge = bar.lastIndex.takeIf { state.genre != null },
            actions = setOf(bar.lastIndex),
            requesters = barFocus,
        )
        AnimatedVisibility(genresOpen, enter = fadeIn(Motion.go(200)), exit = fadeOut(Motion.go(150))) {
            Box(Modifier.fillMaxSize().background(Ink.Canvas.copy(alpha = 0.6f)), contentAlignment = Alignment.CenterEnd) {
                GenrePanel(ready?.genres.orEmpty(), state.genre, open = { genresOpen }) { state.genre = it; closeGenres() }
            }
        }
    }
}

/** The genres, in the panel the player uses for its tracks: "All genres" first, a tick on the chosen one. The remote stays inside it. */
@Composable
private fun GenrePanel(genres: List<String>, chosen: String?, open: () -> Boolean, onPick: (String?) -> Unit) {
    val all = listOf<String?>(null) + genres
    // The list opens at the chosen genre, so that line is on the screen and takes the focus.
    // The genre that was chosen when the panel opened. A pick must not move the focus to the new one: the panel is on its way out then.
    val first = remember { chosen }
    val list = rememberLazyListState(all.indexOf(first).coerceAtLeast(0))
    // That line takes the focus once: not again each time the list scrolls it back into view.
    var placed by remember { mutableStateOf(false) }
    Column(
        Modifier.fillMaxHeight().width(400.dp).background(Ink.Surface.copy(alpha = 0.96f), RoundedCornerShape(topStart = 16.dp, bottomStart = 16.dp)).padding(vertical = 24.dp, horizontal = 14.dp)
            // While it is open the remote cannot leave it; once it closes, the focus is free to go back to the bar.
            .focusProperties { exit = { if (open()) FocusRequester.Cancel else FocusRequester.Default } }.focusGroup().onFocusChanged { if (it.hasFocus) placed = true },
    ) {
        PanelHeading("filters", "GENRE", Modifier.padding(start = 10.dp, bottom = 8.dp))
        LazyColumn(state = list, verticalArrangement = Arrangement.spacedBy(8.dp), contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp)) {
            items(all, key = { it ?: "" }) { genre -> PanelLine(Choice(genre ?: "All genres", genre == chosen) { onPick(genre) }, autofocus = !placed && genre == first) }
        }
    }
}
