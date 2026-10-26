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
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.SeriesRef
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
    var managing by remember { mutableStateOf(false) }
    var assigning by remember { mutableStateOf(emptyList<Pair<String, String>>()) }
    var removing by remember { mutableStateOf(false) }
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
                    AkiHeader("Library", subtitle = if (library.selecting) "${library.pickedShown.size} selected" else "${app.library.size} saved") {
                        AkiMenu("filter", "Sort and filter", library.menu(inMangaTab = part != null) { managing = true })
                    }
                }
                item { MangaPartPills() }
                if (part == null) item { AkiTabs(listOf(AkiChoice(LibraryTab.Saved, "Saved"), AkiChoice(LibraryTab.History, "History")), library.tab, { library.tab = it }) }
                if (library.tab == LibraryTab.Saved) saved(library, columns, inMangaTab = part != null) else history(library)
            }
        }
        if (library.tab == LibraryTab.Saved && library.selecting && library.pickedShown.isNotEmpty()) {
            // What can be done to the series picked: icons alone, round, so three of them fit and sit clear of the tab bar.
            Row(Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 112.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                AkiIconButton("folder", "Categories") { assigning = library.pickedShown.map { it.source to it.id } }
                AkiIconButton("check", "Mark as read") { library.markPickedRead() }
                AkiIconButton("bookmark-off", "Remove from library", tint = Aki.colors.destructive) { removing = true }
            }
        }
    }
    if (managing) CategoriesSheet { managing = false; library.loadCategories(); app.refresh() }
    if (assigning.isNotEmpty()) CategoryPicker(assigning, onDismiss = { assigning = emptyList(); library.loadCategories(); library.picked = emptySet(); library.selecting = false })
    if (removing) {
        AkiConfirm("Remove ${library.pickedShown.size} from your library?", "Their read state stays; you can save them again.", "Remove", { removing = false }) { scope.launch { library.removePicked() } }
    }
}

private fun LazyListScope.saved(library: LibraryModel, columns: Int, inMangaTab: Boolean) {
    if (library.isEmpty) {
        item { AkiEmptyState("Nothing saved yet", "Save a series with the bookmark on its page and it stays here, with how far you have read.", Modifier.heightIn(min = 340.dp)) }
        return
    }
    if (library.categories.isNotEmpty() && !inMangaTab) item { Shelves(library) }
    val shown = library.shown
    if (shown.isEmpty()) {
        item {
            AkiEmptyState("Nothing here", if (library.onlyUnread) "Everything in this shelf is read." else "No series on this shelf yet. Long-press a series to put it on one.", Modifier.heightIn(min = 260.dp))
        }
    }
    akiGrid(shown, columns, key = { it.source + "/" + it.id }) { entry -> Cover(library, entry) }
}

@Composable
private fun Shelves(library: LibraryModel) {
    val shelves = listOf(AkiChoice(-1L, "All")) + library.categories.filter { it.count > 0u || it.books == 0u }.map { AkiChoice(it.id, "${it.name} ${it.count}") } + listOf(AkiChoice(-2L, "No category"))
    AkiChips(shelves, library.category, { library.category = it })
}

@Composable
private fun Cover(library: LibraryModel, entry: LibraryEntry) {
    val router = LocalRouter.current
    val scope = rememberCoroutineScope()
    val subtitle = if (entry.unread > 0u) "${entry.unread} unread" else "Up to date"
    val card = @Composable { AkiCoverCard(entry.title, entry.cover, subtitle = subtitle, badge = entry.unread.toInt()) }
    if (library.selecting) {
        AkiTap({ library.toggle(entry) }) {
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
        item { AkiEmptyState("Nothing read yet", "The chapters you open show up here, newest first.", Modifier.heightIn(min = 340.dp)) }
        return
    }
    items(library.history.size) { index ->
        val entry = library.history[index]
        val router = LocalRouter.current
        AkiTap({ router.push(SeriesRoute(SeriesRef(entry.source, entry.series, entry.title, entry.cover))) }) {
            AkiListCard(entry.cover, if (entry.finished) "Finished" else "Reading", entry.title, badges = listOfNotNull(chapterName(entry.chapterNumber, entry.chapterTitle), shortDate(entry.atMs)))
        }
    }
}
