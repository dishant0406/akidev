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
    var found by remember { mutableStateOf<Found>(Found.Cards(emptyList())) }
    var chosen by remember { mutableStateOf<Card?>(null) }
    var problem by remember { mutableStateOf<Problem?>(null) }
    var busy by remember { mutableStateOf(false) }

    suspend fun search() {
        val asked = target
        if (asked.isEmpty()) return
        found = Found.Searching
        try {
            val cards = app.engine.search(asked, term.trim(), 1u, emptyList()).items
            if (asked == target) found = Found.Cards(cards)
        } catch (error: Exception) {
            if (asked == target) found = Found.Failed(describe(error, Copy.Library.couldNotSearchSource))
        }
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
                problem = describe(error, Copy.Library.couldNotMoveSeries)
            }
            busy = false
        }
    }

    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(18.dp)) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                AkiText(Copy.Library.moveToAnotherSource, AkiTextStyle.Title, isHeading = true)
                AkiText(Copy.Library.findOnAnotherSource(ref.title), AkiTextStyle.Caption, tone = AkiTone.Muted)
            }
            if (others.isEmpty()) {
                AkiEmptyState(Copy.Library.noOtherSource, Copy.Library.installAnotherSource, Modifier.heightIn(min = 260.dp))
            } else {
                AkiMenuPill(target, { target = it }, others.map { AkiOption(it.id, it.name) }, icon = "sources")
                val scopeForSearch = rememberCoroutineScope()
                AkiInput(Copy.Home.searchSource(app.name(target)), term, { term = it }, icon = "search", onSubmit = { scopeForSearch.launch { search() } })
                Results(app.name(target), term, found, onRetry = { scope.launch { search() } }) { chosen = it }
            }
        }
    }
    chosen?.let { card ->
        AkiConfirm(Copy.Library.moveSeriesQuestion(ref.title, app.name(target)), Copy.Library.seriesLeavesSource(app.name(ref.source), app.name(target)), Copy.Library.moveSeries, { chosen = null }) { move(card) }
    }
    problem?.let { AkiAlert(it.title, it.message) { problem = null } }
}

/** What the search for the series on the other source has: waiting, an error, or the cards found (maybe none). */
private sealed interface Found {
    data object Searching : Found
    data class Failed(val problem: Problem) : Found
    data class Cards(val cards: List<Card>) : Found
}

@Composable
private fun Results(source: String, term: String, found: Found, onRetry: () -> Unit, onChoose: (Card) -> Unit) {
    when (found) {
        Found.Searching -> Box(Modifier.fillMaxWidth().heightIn(min = 140.dp), contentAlignment = Alignment.Center) { AkiSpinner(label = Copy.Home.searchingSource(source)) }
        is Found.Failed -> AkiEmptyState(found.problem.title, found.problem.message, Modifier.heightIn(min = 240.dp), action = AkiAction(Copy.Home.tryAgain, onRetry))
        is Found.Cards -> if (found.cards.isEmpty()) {
            AkiEmptyState(Copy.Library.nothingFound, Copy.Library.hasNothingCalled(source, term), Modifier.heightIn(min = 240.dp))
        } else {
            AkiText(Copy.Library.chooseMatchingSeries, AkiTextStyle.CaptionStrong, tone = AkiTone.Muted, isHeading = true)
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                for (card in found.cards) {
                    AkiTap({ onChoose(card) }) {
                        AkiListCard(card.cover, source, card.title, badges = listOfNotNull(chaptersLabel(card.chapters))) { AkiIcon("forward", size = 13.dp, tone = AkiTone.Muted) }
                    }
                }
            }
        }
    }
}
