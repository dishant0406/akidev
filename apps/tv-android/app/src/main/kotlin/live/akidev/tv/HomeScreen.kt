@file:OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)

package live.akidev.tv

import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.launch
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.Dispatchers
import android.os.SystemClock
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Text
import coil3.compose.AsyncImage
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import uniffi.akitv.Title
import uniffi.akitv.Row as HomeRow
import uniffi.akitv.TvException
import uniffi.akitv.homeRow

/** What Home keeps while another screen is on top: where each list and row (of each tab) was scrolled to, and which card the remote was on. */
class HomeMemory {
    val rows = mutableMapOf<String, LazyListState>()
    var focused: String? = null
}

private const val WAITING = 0
private const val FAILED = 1
private const val READY = 2

/** How long the remote must rest on a title before the hero shows it. */
private const val HERO_DELAY_MS = 220L

/** The hero has four slots of fixed height (name, facts, story, buttons): whatever is missing leaves its slot empty, nothing moves. */
private val HeroHeight = 148.dp
private val RowTitleHeight = 32.dp
/** The room the top bar takes at the top (16 dp above it, 44 dp for an item, 6 dp of padding over and under). */
val TopBarHeight = 72.dp
private val ListTopPadding = 8.dp

/**
 * The home screen: the glass bar on top, one hero that shows the title the remote is on, and the rows under it.
 * Everything has a fixed place and size, and what is still loading is drawn as a quiet shape of the same size, so nothing moves when it arrives.
 * The hero changes only once the remote has rested on a title for a moment: a picture swapped on every step of a quick scroll made
 * scrolling slow on the TV (see docs/design.md).
 */
@Composable
fun HomeScreen(
    rows: Load<HomeRows>,
    memory: HomeMemory,
    tab: BarTab,
    onRetry: () -> Unit,
    onOpen: (Title) -> Unit,
    onViewAll: (BrowseTarget) -> Unit,
    watched: List<Watched>,
    onResume: (Watched) -> Unit,
) {
    Box(Modifier.fillMaxSize().background(Ink.Canvas)) {
        // The shapes fade into the rows when they arrive.
        // Only the change between loading, failed and ready is a cross-fade; rows that arrive one by one join the list as they come.
        Crossfade(targetState = if (rows is Load.Done) READY else if (rows is Load.Failed) FAILED else WAITING, animationSpec = Motion.go(360), label = "rows") { phase ->
        when (phase) {
            WAITING -> Column(Modifier.fillMaxSize()) {
                Spacer(Modifier.height(TopBarHeight))
                Column(Modifier.fillMaxWidth().height(HeroHeight).padding(horizontal = 48.dp, vertical = 4.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Box(Modifier.height(50.dp).width(420.dp).padding(vertical = 6.dp).placeholder(RoundedCornerShape(8.dp)))
                    Box(Modifier.height(22.dp).width(300.dp).padding(vertical = 4.dp).placeholder(RoundedCornerShape(6.dp)))
                    Box(Modifier.height(52.dp).width(600.dp).padding(vertical = 8.dp).placeholder(RoundedCornerShape(6.dp)))
                }
                repeat(2) { SkeletonRow() }
            }
            FAILED -> StateCard("The catalogues did not load", (rows as? Load.Failed)?.reason, icon = "warning") { AkiButton("Try again", Look.Primary, icon = "refresh", autofocus = true) { onRetry() } }
            else -> (rows as? Load.Done)?.value?.let { Loaded(it.rows, it.more, memory, tab, onOpen, onViewAll, watched, onResume) }
        }
        }
    }
}

@Composable
private fun SkeletonRow() {
    Column(Modifier.padding(horizontal = 40.dp, vertical = 0.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(Modifier.padding(start = 8.dp).size(180.dp, RowTitleHeight).padding(vertical = 4.dp).placeholder(RoundedCornerShape(8.dp)))
        Row(Modifier.padding(horizontal = 8.dp, vertical = 10.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            repeat(7) { Box(Modifier.size(Poster.width, Poster.height).placeholder(RoundedCornerShape(12.dp))) }
        }
    }
}

@Composable
private fun Loaded(
    all: List<HomeRow>,
    more: Boolean,
    memory: HomeMemory,
    tab: BarTab,
    onOpen: (Title) -> Unit,
    onViewAll: (BrowseTarget) -> Unit,
    watched: List<Watched>,
    onResume: (Watched) -> Unit,
) {
    Box(Modifier.fillMaxSize()) {
        // Switching tab cross-fades the hero and the rows (the bar stays), instead of cutting from one to the other.
        Crossfade(targetState = tab, animationSpec = Motion.go(340), label = "tab") { shownTab -> TabContent(all, more, shownTab, memory, onOpen, onViewAll, watched, onResume) }
    }
}

/** The hero and the rows of one tab, under the top bar (which is drawn over it). */
@Composable
private fun TabContent(
    all: List<HomeRow>,
    more: Boolean,
    tab: BarTab,
    memory: HomeMemory,
    onOpen: (Title) -> Unit,
    onViewAll: (BrowseTarget) -> Unit,
    watched: List<Watched>,
    onResume: (Watched) -> Unit,
) {
    val shown = all.filter { tab.kind == null || it.kind == tab.kind }
    // Only the first card gets the first focus, once: a row scrolled back into view must not take the focus from the viewer.
    var focusPlaced by remember { mutableStateOf(false) }
    val keepWatching = if (tab.kind == null) watched.continueWatching() else emptyList()
    val first = keepWatching.firstOrNull()?.asTitle() ?: shown.firstOrNull()?.titles?.firstOrNull()
    // The card that gets the first focus: the one the remote was on when the viewer left (if it is still here), else the first.
    val keys = keepWatching.map { "continue/${it.titleId}" } + shown.flatMap { row -> row.titles.map { "${row.name}/${it.id}" } + "${row.name}/all" }
    val wanted = memory.focused?.takeIf { it in keys } ?: keys.firstOrNull()
    // `onHero` only writes `heroFocus`; only the hero reads `heroShown`. So moving the remote does not compose this screen again.
    val heroFocus = remember(tab) { mutableStateOf<Title?>(null) }
    val heroShown = remember(tab) { mutableStateOf<Title?>(null) }
    LaunchedEffect(tab) { snapshotFlow { heroFocus.value }.collectLatest { delay(HERO_DELAY_MS); heroShown.value = it } }
    val onHero = remember(heroFocus) { { title: Title -> heroFocus.value = title } }

    val listState = memory.rows.getOrPut("list/${tab.label}") { LazyListState() }
    val sideways = rememberLeastToShow()

    HeroBackdrop(heroShown, first)
    Column(Modifier.fillMaxSize()) {
        Spacer(Modifier.height(TopBarHeight))
        Hero(heroShown, first)
        if (shown.isEmpty() && !more) {
            Row(Modifier.padding(horizontal = 48.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) { AkiIcon("list", tint = Ink.TextDim); Text("Nothing here yet.", style = Type.Body) }
        } else CompositionLocalProvider(LocalBringIntoViewSpec provides RowAtTop(with(LocalDensity.current) { ListTopPadding.toPx() })) { LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            // The big space under the list lets the last row also scroll up to the top, like every other row.
            contentPadding = PaddingValues(start = 40.dp, end = 40.dp, top = ListTopPadding, bottom = 96.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            if (tab.kind == null && keepWatching.isNotEmpty()) item(key = "continue") {
                Column(Modifier.wholeRowInView(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Continue watching", style = Type.RowTitle, modifier = Modifier.padding(start = 8.dp))
                    CompositionLocalProvider(LocalBringIntoViewSpec provides sideways) {
                        LazyRow(state = memory.rows.getOrPut("${tab.label}/continue") { LazyListState() }, horizontalArrangement = Arrangement.spacedBy(14.dp), contentPadding = PaddingValues(horizontal = 8.dp, vertical = 10.dp)) {
                            items(keepWatching, key = { it.titleId }) { entry ->
                                val key = "continue/${entry.titleId}"
                                ContinueCard(entry, autofocus = !focusPlaced && key == wanted, onFocus = { focusPlaced = true; memory.focused = key; onHero(entry.asTitle()) }) { onResume(entry) }
                            }
                        }
                    }
                }
            }
            // The rows still on their way: one quiet shape under the last, so the end of the list does not look final.
            items(shown, key = { it.name }) { row ->
                Column(Modifier.wholeRowInView(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(row.name, style = Type.RowTitle, modifier = Modifier.padding(start = 8.dp))
                    CompositionLocalProvider(LocalBringIntoViewSpec provides sideways) {
                        LazyRow(state = memory.rows.getOrPut("${tab.label}/${row.name}") { LazyListState() }, horizontalArrangement = Arrangement.spacedBy(14.dp), contentPadding = PaddingValues(horizontal = 8.dp, vertical = 10.dp)) {
                            itemsIndexed(row.titles, key = { _, title -> title.id }) { index, title ->
                                val key = "${row.name}/${title.id}"
                                val autofocus = !focusPlaced && key == wanted
                                val onFocus = { focusPlaced = true; memory.focused = key; onHero(title) }
                                if (row.ranked) RankedCard(index + 1, title, autofocus, onFocus) { onOpen(title) }
                                else PosterCard(title, showName = false, autofocus = autofocus, onFocus = onFocus) { onOpen(title) }
                            }
                            // The end of the row: the whole list, with the row's own order and genre to start from.
                            item(key = "all") {
                                val key = "${row.name}/all"
                                ViewAllCard(autofocus = !focusPlaced && key == wanted, onFocus = { focusPlaced = true; memory.focused = key }) { onViewAll(BrowseTarget(row.name, row.kind, row.sort, row.genre)) }
                            }
                        }
                    }
                }
            }
            if (more) item(key = "more") { SkeletonRow() }
        } }
    }
}

/** The picture of the title in focus. Reads the hero's state itself, so only this part composes when it changes. */
@Composable
private fun HeroBackdrop(shown: State<Title?>, first: Title?) {
    val title = shown.value ?: first
    Backdrop(title?.banner ?: title?.poster, fallback = title?.poster)
}

/** Name, the line of facts and two lines of story, each in a slot of fixed height. It only shows what is in focus: pressing OK on the card opens it. */
@Composable
private fun Hero(shown: State<Title?>, first: Title?) {
    Box(Modifier.fillMaxWidth().height(HeroHeight).padding(horizontal = 48.dp, vertical = 4.dp)) {
        val t = shown.value ?: first ?: return@Box
        Crossfade(targetState = t, animationSpec = Motion.go(280), label = "hero") { title ->
            // The story comes later: asked for once the remote has rested on the title (and no faster than the catalogue allows).
            var story by remember(title.id) { mutableStateOf(title.description) }
            LaunchedEffect(title.id) { if (story == null) story = Story.of(title.id) }
            Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Box(Modifier.height(50.dp)) { Text(title.name, style = Type.Billboard, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                Box(Modifier.height(22.dp)) { Text(facts(title), style = Type.Meta, maxLines = 1) }
                Box(Modifier.height(52.dp).width(640.dp)) { story?.let { Text(it, style = Type.Body, maxLines = 2, overflow = TextOverflow.Ellipsis) } }
            }
        }
    }
}

/** The last card of a row: opens the whole list. Same size as a poster. */
@Composable
private fun ViewAllCard(autofocus: Boolean, onFocus: () -> Unit, onClick: () -> Unit) {
    val focus = remember { FocusRequester() }
    if (autofocus) LaunchedEffect(Unit) { focus.requestFocus() }
    Focusable(onClick, RoundedCornerShape(12.dp), Ink.Surface, Modifier.size(Poster.width, Poster.height).focusRequester(focus).onFocusChanged { if (it.isFocused) onFocus() }) {
        Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterVertically)) {
            AkiIcon("more", size = 32.dp, tint = Ink.Text)
            Text("View all", style = Type.Label, maxLines = 1)
        }
    }
}

/** A wide card for something half watched: the picture, the name, and how much is left. */
@Composable
private fun ContinueCard(entry: Watched, autofocus: Boolean, onFocus: () -> Unit, onClick: () -> Unit) {
    val focus = remember { FocusRequester() }
    if (autofocus) LaunchedEffect(Unit) { focus.requestFocus() }
    val left = ((entry.length - entry.position) / 60_000).coerceAtLeast(1)
    Focusable(onClick, RoundedCornerShape(12.dp), Ink.Surface, Modifier.size(224.dp, 126.dp).focusRequester(focus).onFocusChanged { if (it.isFocused) onFocus() }) {
        Box(Modifier.fillMaxSize()) {
            AsyncImage(entry.banner ?: entry.poster, null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize().clip(RoundedCornerShape(12.dp)))
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.15f to Color.Transparent, 0.85f to Color(0xF20C0B0A))))
            Column(Modifier.align(Alignment.BottomStart).padding(start = 12.dp, end = 12.dp, bottom = 10.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(entry.title, style = Type.Label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    AkiIcon("clock", size = 16.dp, tint = Ink.TextDim)
                    Text(listOfNotNull(entry.label, "$left min").joinToString("  ·  "), style = Type.Meta.copy(letterSpacing = 0.sp, color = Ink.TextDim), maxLines = 1)
                }
            }
            Progress(entry.position.toFloat() / entry.length, Modifier.align(Alignment.BottomStart))
        }
    }
}

/** What the hero needs of something watched, which keeps only a little about its title. */
fun Watched.asTitle() = Title(titleId, title, null, poster, null, banner, null, emptyList(), null)

/** "2010  ·  ADVENTURE, SCI-FI  ·  88%" in the small capitals of the design plan. A title that is not out says when it comes in place of the year. */
fun facts(title: Title): String =
    listOfNotNull(
        title.comingSoon ?: title.year?.toString(),
        title.genres.take(3).joinToString(", ").ifEmpty { null },
        title.score?.let { "$it%" },
    ).joinToString("  ·  ").uppercase()

/** The rows that have come so far (in order, a failed one left out), and whether more are still on their way. */
class HomeRows(val rows: List<HomeRow>, val more: Boolean)

/**
 * The home rows as one state: a row is shown only when every row before it has an answer, so a row that arrives late never pushes the
 * ones already on the screen down. Nothing yet is Loading; all rows failed is Failed.
 */
fun homeRowsOf(states: List<Load<HomeRow>>): Load<HomeRows> {
    val answered = states.takeWhile { it !is Load.Loading }
    val rows = answered.mapNotNull { (it as? Load.Done)?.value }
    val more = answered.size < states.size
    return when {
        rows.isNotEmpty() -> Load.Done(HomeRows(rows, more))
        more -> Load.Loading
        else -> Load.Failed("Could not load the catalogues. Check the network.")
    }
}

/** How many rows are asked for at the same time: the first ones come sooner when the TV's three cores are not busy with ten requests. */
private const val ROWS_AT_ONCE = 3

/** Asks for the rows in order, a few at a time, and puts each answer in its place in `states` as it arrives. */
suspend fun loadHomeRows(states: SnapshotStateList<Load<HomeRow>>) {
    val started = SystemClock.elapsedRealtime()
    val turns = Semaphore(ROWS_AT_ONCE)
    coroutineScope {
        states.indices.forEach { index ->
            launch(Dispatchers.IO) {
                turns.withPermit {
                    states[index] = try {
                        Load.Done(homeRow(index.toUInt()))
                    } catch (e: TvException.Failed) {
                        Load.Failed(e.reason)
                    } catch (e: Exception) {
                        Load.Failed(e.message ?: "Something went wrong.")
                    }
                    Perf.log("home row $index", SystemClock.elapsedRealtime() - started)
                }
            }
        }
    }
}
