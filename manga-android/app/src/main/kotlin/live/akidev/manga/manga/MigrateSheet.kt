package live.akidev.manga.manga

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import live.akidev.manga.app.Problem
import live.akidev.manga.app.SeriesRef
import live.akidev.manga.app.describe
import live.akidev.manga.design.*
import uniffi.akimanga.Card

/**
 * Moves a series to the same series on another source: the library entry, its shelves and the chapters read (matched by chapter number)
 * go with it. It looks for the series by its title as soon as it opens and again whenever another source is chosen, so the usual move
 * is: open, pick the source, tap the match.
 */
@Composable
fun MigrateSheet(ref: SeriesRef, onMoved: () -> Unit, onDismiss: () -> Unit) {
    val app = LocalApp.current
    val scope = rememberCoroutineScope()
    val others = app.visibleSources.filter { it.id != ref.source && it.canSearch }
    var target by remember { mutableStateOf(others.firstOrNull()?.id ?: "") }
    var term by remember { mutableStateOf(ref.title) }
    var results by remember { mutableStateOf(emptyList<Card>()) }
    var searching by remember { mutableStateOf(false) }
    var failure by remember { mutableStateOf<Problem?>(null) }
    var chosen by remember { mutableStateOf<Card?>(null) }
    var problem by remember { mutableStateOf<Problem?>(null) }
    var busy by remember { mutableStateOf(false) }

    suspend fun search() {
        val asked = target
        if (asked.isEmpty()) return
        searching = true
        failure = null
        results = emptyList()
        try {
            val found = app.engine.search(asked, term.trim(), 1u, emptyList()).items
            if (asked == target) results = found
        } catch (error: Exception) {
            if (asked == target) failure = describe(error)
        }
        searching = false
    }
    // Opening the sheet, and choosing another source, each look for the title straight away.
    LaunchedEffect(target) { search() }

    fun move(card: Card) {
        if (busy) return
        busy = true
        scope.launch {
            try {
                // Opening the other series once lets the engine match chapters by number.
                app.engine.details(target, card.id)
                app.engine.chapters(target, card.id)
                app.engine.migrate(ref.source, ref.id, target, card.id)
                app.refresh()
                onMoved()
                onDismiss()
            } catch (error: Exception) {
                problem = describe(error)
            }
            busy = false
        }
    }

    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(18.dp)) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                AkiText("Move to another source", AkiTextStyle.Title)
                AkiText("Find “${ref.title}” on another source. Its shelves and the chapters you read go with it.", AkiTextStyle.Caption, tone = AkiTone.Muted)
            }
            if (others.isEmpty()) {
                AkiEmptyState("No other source", "Install another source that can search, then come back.", Modifier.heightIn(min = 260.dp))
            } else {
                AkiMenuPill(target, { target = it }, others.map { AkiOption(it.id, it.name) }, icon = "sources")
                val scopeForSearch = rememberCoroutineScope()
                AkiInput("Search ${app.name(target)}", term, { term = it }, icon = "search", onSubmit = { scopeForSearch.launch { search() } })
                Results(app.name(target), term, searching, failure, results, onRetry = { scope.launch { search() } }) { chosen = it }
            }
        }
    }
    chosen?.let { card ->
        AkiConfirm("Move to “${card.title}”?", "It leaves ${app.name(ref.source)} and moves to ${app.name(target)}; your shelves and read chapters carry over.", "Move", { chosen = null }) { move(card) }
    }
    problem?.let { AkiAlert(it.title, it.message) { problem = null } }
}

@Composable
private fun Results(source: String, term: String, searching: Boolean, failure: Problem?, results: List<Card>, onRetry: () -> Unit, onChoose: (Card) -> Unit) {
    when {
        searching -> Box(Modifier.fillMaxWidth().heightIn(min = 140.dp), contentAlignment = Alignment.Center) { AkiSpinner() }
        failure != null -> AkiEmptyState(failure.title, failure.message, Modifier.heightIn(min = 240.dp), action = AkiAction("Try again", onRetry))
        results.isEmpty() -> AkiEmptyState("Nothing found", "$source has nothing called “$term”. Try a shorter title, or another source.", Modifier.heightIn(min = 240.dp))
        else -> {
            AkiText("Tap the one that is the same series", AkiTextStyle.CaptionStrong, tone = AkiTone.Muted)
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                for (card in results) {
                    AkiTap({ onChoose(card) }) {
                        AkiListCard(card.cover, source, card.title, badges = listOfNotNull(chaptersLabel(card.chapters))) { AkiIcon("forward", size = 13.dp, tone = AkiTone.Muted) }
                    }
                }
            }
        }
    }
}
