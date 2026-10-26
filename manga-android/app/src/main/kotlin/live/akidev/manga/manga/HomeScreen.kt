package live.akidev.manga.manga

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
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
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.AppTab
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.SeriesRef
import live.akidev.manga.app.SeriesRoute
import live.akidev.manga.design.*
import uniffi.akimanga.Card

/** Discover: the sources you installed, what is popular or new in one, and search. */
@Composable
fun HomeScreen() {
    val model = LocalApp.current
    val router = LocalRouter.current
    val discover = model.discover
    val scope = rememberCoroutineScope()
    var filtering by remember { mutableStateOf(false) }
    val columns = akiColumns()
    val open = { source: String, card: Card -> router.push(SeriesRoute(SeriesRef(source, card.id, card.title, card.cover))) }

    LaunchedEffect(model.sourcesKey) { discover.sourcesChanged() }

    AkiRefreshable({ discover.reload() }, Modifier.fillMaxSize()) {
        AkiPage(narrow = false, state = discover.scroll) {
            item { AkiHeader("Discover") }
            item { MangaPartPills() }
            if (model.sources.isEmpty() && model.repos.isEmpty()) {
                item { WelcomeScreen() }
            } else if (model.visibleSources.isEmpty()) {
                item { NothingVisible(model) }
            } else {
                item { SearchRow(onFilters = { filtering = true }) }
                item { SourcePills() }
                if (discover.isGlobal) everySource(model, open) else oneSource(model, columns, open)
            }
        }
    }
    if (filtering) {
        FiltersSheet(discover.otherFilters, discover.choices, { discover.choices = it }, { scope.launch { discover.reload() } }) { filtering = false }
    }
}

@Composable
private fun NothingVisible(model: AppModel) {
    AkiEmptyState(
        if (model.sources.isEmpty()) "No sources installed" else "Nothing to show",
        if (model.sources.isEmpty()) "Install a source from your repository in Sources to start reading." else "Your content settings hide every installed source.",
        Modifier.heightIn(min = 420.dp), action = AkiAction("Open Sources") { model.tab = AppTab.Sources },
    )
}

@Composable
private fun SearchRow(onFilters: () -> Unit) {
    val discover = LocalApp.current.discover
    val scope = rememberCoroutineScope()
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
        AkiInput(
            "Search ${if (discover.isGlobal) "every source" else discover.info?.name ?: ""}", discover.term, { discover.term = it }, Modifier.weight(1f), icon = "search",
            onSubmit = { discover.submitted = discover.term; scope.launch { discover.reload() } },
            onClear = { discover.submitted = ""; scope.launch { discover.reload() } },
        )
        if (discover.otherFilters.isNotEmpty() && !discover.isGlobal) {
            AkiIconButton("sliders", "Filters", tint = if (discover.choices.isEmpty()) null else Aki.colors.primary, onClick = onFilters)
        }
    }
}

@Composable
private fun SourcePills() {
    val model = LocalApp.current
    val discover = model.discover
    val scope = rememberCoroutineScope()
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        if (model.visibleSources.size > 1) {
            AkiMenuPill(
                discover.source, { discover.choose(it); scope.launch { discover.reload() } },
                listOf(AkiOption(DiscoverModel.EVERY_SOURCE, "All sources")) + model.visibleSources.map { AkiOption(it.id, it.name) }, icon = "sources",
            )
        }
        val category = discover.category
        if (category != null && !discover.isGlobal) {
            val all = if (category.options.any { it.value.isEmpty() }) emptyList() else listOf(AkiOption("", "All"))
            AkiMenuPill(
                discover.choices[category.id] ?: "",
                { value -> discover.choices = if (value.isEmpty()) discover.choices - category.id else discover.choices + (category.id to value); scope.launch { discover.reload() } },
                all + category.options.map { AkiOption(it.value, it.name) }, icon = "filter", placeholder = category.name,
            )
        }
    }
}

// ---- every source at once -----------------------------------------------------------------------------------------------------

/** Every source at once: one big title, what you were reading, then a popular and a new shelf per source. */
private fun LazyListScope.everySource(model: AppModel, open: (String, Card) -> Unit) {
    val feed = model.discover.feed
    if (model.discover.submitted.isNotEmpty()) {
        searchResults(model.discover.global, open)
    } else if (feed.shelves.isEmpty() && feed.loading) {
        item { Box(Modifier.fillMaxWidth().heightIn(min = 280.dp), contentAlignment = Alignment.Center) { AkiSpinner() } }
    } else if (feed.shelves.isEmpty()) {
        item { AkiEmptyState("Nothing to show", "None of your sources gave any titles. Pull down to try again.", Modifier.heightIn(min = 320.dp)) }
    } else {
        val popular = feed.shelves.firstOrNull { it.id.endsWith("popular") }
        val lead = popular?.items?.firstOrNull()
        if (popular != null && lead != null) item { Lead(lead, model.name(popular.source)) { open(popular.source, lead) } }
        continueReading(model, open)
        for (shelf in feed.shelves) {
            item(key = shelf.id) {
                AkiShelf(shelf.title, subtitle = shelf.subtitle) {
                    items(shelf.items.size, key = { shelf.items[it].id }) { index ->
                        val card = shelf.items[index]
                        AkiTap({ open(shelf.source, card) }) { AkiPoster(card.title, card.cover, caption = chaptersLabel(card.chapters)) }
                    }
                }
            }
        }
    }
}

/** One search sent to every source: a row of covers for each, as each answers. */
private fun LazyListScope.searchResults(search: GlobalSearchModel, open: (String, Card) -> Unit) {
    items(search.groups.size, key = { search.groups[it].source.id }) { index ->
        val group = search.groups[index]
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            AkiText(group.source.name, AkiTextStyle.Heading)
            when (val state = group.state) {
                SearchState.Loading -> Box(Modifier.fillMaxWidth().heightIn(min = 60.dp), contentAlignment = Alignment.Center) { AkiSpinner() }
                is SearchState.Failed -> AkiCard(padding = 14.dp) { AkiText(state.message, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 3) }
                is SearchState.Done -> if (state.cards.isEmpty()) {
                    AkiText("No results here", AkiTextStyle.Caption, tone = AkiTone.Muted)
                } else {
                    LazyRow(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        items(state.cards.size, key = { state.cards[it].id }) { cardIndex ->
                            val card = state.cards[cardIndex]
                            AkiTap({ open(group.source.id, card) }, Modifier.width(130.dp)) { AkiCoverCard(card.title, card.cover) }
                        }
                    }
                }
            }
        }
    }
}

// ---- one source ---------------------------------------------------------------------------------------------------------------

private fun LazyListScope.oneSource(model: AppModel, columns: Int, open: (String, Card) -> Unit) {
    val discover = model.discover
    val browse = discover.browse
    val error = browse.error
    if (error != null && browse.items.isEmpty()) {
        item { Retry(error.title, error.message) }
    } else if (browse.items.isEmpty() && !browse.loading) {
        item {
            AkiEmptyState(
                if (discover.submitted.isEmpty()) "Nothing to show" else "No results",
                if (discover.submitted.isEmpty()) "This source gave no titles." else "Nothing matched “${discover.submitted}” in ${discover.info?.name ?: discover.source}.",
                Modifier.heightIn(min = 320.dp),
            )
        }
    } else if (browse.items.isEmpty()) {
        item { Box(Modifier.fillMaxWidth().heightIn(min = 240.dp), contentAlignment = Alignment.Center) { AkiSpinner() } }
    } else if (discover.submitted.isEmpty() && discover.choices.isEmpty()) {
        sourceFeed(model, columns, open)
    } else {
        coverGrid(model, browse.items, columns, open)
        showMore(model)
    }
}

/** One source: its leading title, what you were reading, its top ten, what is new, then everything else. */
private fun LazyListScope.sourceFeed(model: AppModel, columns: Int, open: (String, Card) -> Unit) {
    val discover = model.discover
    val items = discover.browse.items
    val source = discover.source
    item { Lead(items.first(), "Popular on ${discover.info?.name ?: "this source"}") { open(source, items.first()) } }
    continueReading(model, open)
    item {
        AkiShelf("Top 10", subtitle = discover.info?.name) {
            items(minOf(items.size, 10), key = { items[it].id }) { index ->
                val card = items[index]
                AkiTap({ open(source, card) }) { AkiPoster(card.title, card.cover, caption = chaptersLabel(card.chapters), rank = index + 1, width = 116.dp) }
            }
        }
    }
    val latest = discover.feed.latest
    if (latest.isNotEmpty()) {
        item {
            AkiShelf("New releases") {
                items(latest.size, key = { latest[it].id }) { index ->
                    val card = latest[index]
                    AkiTap({ open(source, card) }) { AkiPoster(card.title, card.cover, caption = chaptersLabel(card.chapters)) }
                }
            }
        }
    }
    if (items.size > 10) {
        item { AkiText("Browse all", AkiTextStyle.Heading) }
        coverGrid(model, items.drop(10), columns, open)
    }
    showMore(model)
}

/** Covers in rows of `columns`; each row is an item of the page, so only the rows on screen are made. */
private fun LazyListScope.coverGrid(model: AppModel, cards: List<Card>, columns: Int, open: (String, Card) -> Unit) {
    val discover = model.discover
    akiGrid(cards, columns, key = { it.id }) { card ->
        AkiTap({ open(discover.source, card) }) { AkiCoverCard(card.title, card.cover, subtitle = chaptersLabel(card.chapters) ?: discover.info?.name) }
    }
}

private fun LazyListScope.showMore(model: AppModel) {
    val discover = model.discover
    if (!discover.browse.hasNext) return
    item {
        val scope = rememberCoroutineScope()
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            AkiButton(if (discover.browse.loading) "Loading…" else "Show more") { scope.launch { discover.load(reset = false) } }
        }
    }
}

private fun LazyListScope.continueReading(model: AppModel, open: (String, Card) -> Unit) {
    val entries = model.discover.continueReading
    if (entries.isEmpty()) return
    item {
        AkiShelf("Continue reading") {
            items(entries.size, key = { entries[it].source + "/" + entries[it].series }) { index ->
                val entry = entries[index]
                AkiTap({ open(entry.source, Card(entry.series, entry.title, entry.cover, null)) }) {
                    AkiPoster(entry.title, entry.cover, caption = chapterName(entry.chapterNumber, entry.chapterTitle))
                }
            }
        }
    }
}

@Composable
private fun Lead(card: Card, tag: String, onClick: () -> Unit) {
    AkiTap(onClick, squeeze = 0.98f) { AkiBillboard(card.title, card.cover, tag) }
}

@Composable
private fun Retry(title: String, message: String) {
    val scope = rememberCoroutineScope()
    val discover = LocalApp.current.discover
    AkiEmptyState(title, message, Modifier.heightIn(min = 320.dp), action = AkiAction("Try again") { scope.launch { discover.reload() } })
}

fun chaptersLabel(count: UInt?) = when {
    count == null || count == 0u -> null
    count == 1u -> "1 chapter"
    else -> "$count chapters"
}

/** A chapter's name: its title, else "Chapter 12". */
fun chapterName(number: Double?, title: String?): String = when {
    !title.isNullOrEmpty() -> title
    number != null -> "Chapter " + if (number == number.toLong().toDouble()) number.toLong().toString() else "%.2f".format(number).trimEnd('0').trimEnd('.')
    else -> "Chapter"
}
