package live.akidev.manga.manga

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.SeriesRef
import live.akidev.manga.app.SeriesRoute
import live.akidev.manga.app.shortDate
import live.akidev.manga.design.*

enum class UpdatesTab { Unread, Recent }

/** The tab chosen on the Updates screen, kept as long as the app. */
class UpdatesModel {
    var tab by mutableStateOf(UpdatesTab.Unread)
    val scroll = androidx.compose.foundation.lazy.LazyListState()
}

/** New chapters across your library, as a list of cards. */
@Composable
fun UpdatesScreen() {
    val app = LocalApp.current
    val updates = app.updatesScreen
    val part = LocalMangaPart.current
    val scope = rememberCoroutineScope()

    LaunchedEffect(Unit) { app.refresh() }
    LaunchedEffect(part?.part) {
        when (part?.part) {
            MangaPart.Recent -> updates.tab = UpdatesTab.Recent
            MangaPart.Unread -> updates.tab = UpdatesTab.Unread
            else -> Unit
        }
    }
    val refresh: suspend () -> Unit = { app.refreshLibrary(force = true) }

    AkiRefreshable(refresh, Modifier.fillMaxSize()) {
        AkiPage(narrow = true, state = updates.scroll) {
            item {
                AkiHeader("Updates", subtitle = if (app.unreadTotal > 0) "${app.unreadTotal} unread chapters" else "All caught up") {
                    if (app.refreshing) AkiSpinner(Modifier.size(44.dp)) else AkiIconButton("refresh", "Check for new chapters") { scope.launch { refresh() } }
                }
            }
            item { MangaPartPills() }
            if (part == null) item { AkiTabs(listOf(AkiChoice(UpdatesTab.Unread, "Unread"), AkiChoice(UpdatesTab.Recent, "Recent")), updates.tab, { updates.tab = it }) }
            if (app.updateFailures.isNotEmpty()) item { Failures(app) }
            rows(app, updates.tab) { scope.launch { refresh() } }
        }
    }
}

private fun LazyListScope.rows(app: AppModel, tab: UpdatesTab, check: () -> Unit) {
    val list = if (tab == UpdatesTab.Unread) app.updates.filter { it.unread > 0u } else app.updates
    if (list.isEmpty()) {
        item {
            AkiEmptyState(
                if (tab == UpdatesTab.Unread) "All caught up" else "No series saved",
                if (tab == UpdatesTab.Unread) "New chapters of the series you saved appear here." else "Save a series and its chapters show up here.",
                Modifier.heightIn(min = 340.dp), action = if (tab == UpdatesTab.Unread) AkiAction("Check for new chapters", check) else null,
            )
        }
        return
    }
    items(list.size, key = { list[it].source + "/" + list[it].id }) { index ->
        val row = list[index]
        val router = LocalRouter.current
        val latest = if (row.latestNumber == null && row.latestTitle == null) null else chapterName(row.latestNumber, row.latestTitle)
        AkiTap({ router.push(SeriesRoute(SeriesRef(row.source, row.id, row.title, row.cover))) }) {
            AkiListCard(
                row.cover, if (row.unread > 0u) "${row.unread} new chapter${if (row.unread == 1u) "" else "s"}" else "Up to date", row.title,
                badges = listOfNotNull(latest, shortDate(row.latestDateMs)), dimmed = row.unread == 0u,
            )
        }
    }
}

@Composable
private fun Failures(app: AppModel) {
    AkiCard(padding = 16.dp) {
        Column(verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp)) {
            AkiText("Could not check ${app.updateFailures.size} series", AkiTextStyle.BodyStrong)
            for (failure in app.updateFailures) AkiText("${failure.title} (${failure.source}): ${failure.message}", AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 2)
        }
    }
}
