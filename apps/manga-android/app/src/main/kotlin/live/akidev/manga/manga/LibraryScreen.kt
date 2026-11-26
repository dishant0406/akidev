package live.akidev.manga.manga

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.SeriesRef
import live.akidev.manga.app.plural
import live.akidev.manga.app.SeriesRoute
import live.akidev.manga.app.shortDate
import live.akidev.manga.design.*
import uniffi.akimanga.LibraryEntry

/** What you saved, and what you read last. */
@Composable
fun LibraryScreen() {
    val app = LocalApp.current
    val library = app.libraryScreen
    val part = LocalMangaPart.current
    val scope = rememberCoroutineScope()
    var sheet by remember { mutableStateOf<LibrarySheet?>(null) }
    val columns = akiColumns()

    LaunchedEffect(Unit) { app.refresh(); library.loadHistory(); library.loadCategories() }
    // The counts on the shelves are read from the same place as the library: they change when it does, wherever the change came from.
    LaunchedEffect(app.library) { library.loadCategories() }
    // Inside the Manga tab the two lists are chosen in its row of choices.
    LaunchedEffect(part?.part) {
        when (part?.part) {
            MangaPart.History -> library.tab = LibraryTab.History
            MangaPart.Saved -> library.tab = LibraryTab.Saved
            else -> Unit
        }
    }

    Box(Modifier.fillMaxSize()) {
        AkiRefreshable({ app.refreshLibrary(force = true); library.loadHistory(); library.loadCategories() }, Modifier.fillMaxSize()) {
            AkiPage(narrow = false, state = library.scroll) {
                item {
                    AkiHeader(Copy.Home.library, subtitle = if (library.selecting) Copy.Library.selectedCount(library.pickedShown.size) else Copy.Library.seriesCountAndroid(app.library.size)) {
                        AkiMenu("filter", Copy.Library.sortAndFilter, library.menu(inMangaTab = part != null) { sheet = LibrarySheet.Managing })
                    }
                }
                item { MangaPartPills() }
                if (part == null) item { AkiTabs(listOf(AkiChoice(LibraryTab.Saved, Copy.Home.library), AkiChoice(LibraryTab.History, Copy.Home.history)), library.tab, { library.tab = it }) }
                if (library.tab == LibraryTab.Saved) saved(library, columns, inMangaTab = part != null) else history(library)
            }
        }
        if (library.tab == LibraryTab.Saved && library.selecting && library.pickedShown.isNotEmpty()) {
            // What can be done to the series picked: icons alone, round, so three of them fit and sit clear of the tab bar.
            Row(Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 112.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                AkiIconButton("folder", Copy.Library.changeCategories) { sheet = LibrarySheet.Assigning(library.pickedShown.map { it.source to it.id }) }
                AkiIconButton("check", Copy.Library.markAsRead) { library.markPickedRead() }
                AkiIconButton("bookmark-off", Copy.Library.removeFromLibrary, tint = Aki.colors.destructive) { sheet = LibrarySheet.Removing }
            }
        }
    }
    when (val current = sheet) {
        LibrarySheet.Managing -> CategoriesSheet { sheet = null; library.loadCategories(); app.refresh() }
        is LibrarySheet.Assigning -> CategoryPicker(current.series, onDismiss = { sheet = null; library.loadCategories(); library.picked = emptySet(); library.selecting = false })
        LibrarySheet.Removing -> AkiConfirm(Copy.Library.removeSeriesQuestion(library.pickedShown.size), Copy.Library.removeSeriesHint, Copy.Library.removeSeries, { sheet = null }) { scope.launch { library.removePicked() } }
        null -> {}
    }
}

/** The sheet or dialog over the library: only one is open at a time. */
private sealed interface LibrarySheet {
    data object Managing : LibrarySheet
    data class Assigning(val series: List<Pair<String, String>>) : LibrarySheet
    data object Removing : LibrarySheet
}

private fun LazyListScope.saved(library: LibraryModel, columns: Int, inMangaTab: Boolean) {
    if (library.isEmpty) {
        item { AkiEmptyState(Copy.Library.libraryEmpty, Copy.Library.libraryEmptyHint, Modifier.heightIn(min = 340.dp)) }
        return
    }
    if (library.categories.isNotEmpty() && !inMangaTab) item { Shelves(library) }
    val shown = library.shown
    if (shown.isEmpty()) {
        item {
            AkiEmptyState(if (library.onlyUnread) Copy.Home.allCaughtUp else Copy.Library.nothingHere, if (library.onlyUnread) Copy.Library.readEverythingInCategory else Copy.Library.noSeriesInCategory, Modifier.heightIn(min = 260.dp))
        }
    }
    akiGrid(shown, columns, key = { it.source + "/" + it.id }) { entry -> Cover(library, entry) }
}

@Composable
private fun Shelves(library: LibraryModel) {
    val shelves = listOf(AkiChoice(-1L, Copy.Home.all)) + library.categories.filter { it.count > 0u || it.books == 0u }.map { AkiChoice(it.id, "${it.name} ${it.count}") } + listOf(AkiChoice(-2L, Copy.Library.noCategory))
    AkiChips(shelves, library.category, { library.category = it })
}

@Composable
private fun Cover(library: LibraryModel, entry: LibraryEntry) {
    val router = LocalRouter.current
    val scope = rememberCoroutineScope()
    val subtitle = if (entry.unread > 0u) Copy.Library.unreadCount(entry.unread.toInt()) else Copy.Home.upToDate
    val card = @Composable { AkiCoverCard(entry.title, entry.cover, subtitle = subtitle, badge = entry.unread.toInt()) }
    if (library.selecting) {
        AkiTap({ library.toggle(entry) }, Modifier.semantics { selected = entry.source + "/" + entry.id in library.picked }) {
            Box {
                card()
                Tick(entry.source + "/" + entry.id in library.picked, Modifier.align(Alignment.TopStart))
            }
        }
    } else {
        AkiTapMenu(library.menuOf(entry, scope), onClick = { router.push(SeriesRoute(SeriesRef(entry.source, entry.id, entry.title, entry.cover))) }) { card() }
    }
}

@Composable
private fun Tick(on: Boolean, modifier: Modifier) {
    Box(modifier.padding(8.dp).background(if (on) Aki.colors.primary else Aki.colors.scrim, CircleShape).padding(6.dp)) {
        AkiIcon(if (on) "check-solid" else "check", size = 22.dp, tone = if (on) AkiTone.OnPrimary else AkiTone.OnMedia)
    }
}

private fun LazyListScope.history(library: LibraryModel) {
    if (library.history.isEmpty()) {
        item { AkiEmptyState(Copy.Library.nothingReadYet, Copy.Library.chaptersOpenedHint, Modifier.heightIn(min = 340.dp)) }
        return
    }
    items(library.history.size) { index ->
        val entry = library.history[index]
        val router = LocalRouter.current
        AkiTapMenu(library.historyMenu(entry), onClick = { router.push(SeriesRoute(SeriesRef(entry.source, entry.series, entry.title, entry.cover))) }) {
            AkiListCard(entry.cover, if (entry.finished) Copy.Library.finished else Copy.Library.reading, entry.title, badges = listOfNotNull(chapterName(entry.chapterNumber, entry.chapterTitle), shortDate(entry.atMs)))
        }
    }
}
