package live.akidev.manga.sources

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import live.akidev.manga.app.AppTab
import live.akidev.manga.app.CatalogueRef
import live.akidev.manga.app.CatalogueRoute
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.Problem
import live.akidev.manga.app.describe
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.ReaderKind
import live.akidev.manga.app.RepoRoute
import live.akidev.manga.app.SettingsRoute
import live.akidev.manga.app.SourceSettingsRoute
import live.akidev.manga.design.*

/**
 * Akidev is a renderer: it ships with no source. Sources come from repositories the person adds by address (a GitHub link, say); this
 * screen is where that happens.
 */
@Composable
fun SourcesScreen() {
    val model = LocalApp.current
    val router = LocalRouter.current
    var sheet by remember { mutableStateOf<SourcesSheet?>(null) }
    var catalogues by remember { mutableStateOf(loadCatalogues(model)) }
    var catalogueProblem by remember { mutableStateOf<Problem?>(null) }
    val scope = rememberCoroutineScope()

    AkiRefreshable({ model.updateSources(); model.refresh() }, Modifier.fillMaxSize()) {
        AkiPage {
            item {
                AkiHeader(Copy.Home.sources, subtitle = Copy.Sources.installedCount(model.sources.size)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        AkiIconButton("settings", Copy.Settings.title) { router.push(SettingsRoute) }
                        AkiIconButton("add", Copy.Sources.addRepository, tint = Aki.colors.primary) { sheet = SourcesSheet.Repository }
                    }
                }
            }
            if (model.prefs.readerKind == ReaderKind.Books) {
                item { BooksSection(catalogues, onAddCatalogue = { sheet = SourcesSheet.Catalogue }, onChanged = { catalogues = loadCatalogues(model) }) }
                item { InstalledSection(onAdd = { sheet = SourcesSheet.Source }) }
            } else {
                item { InstalledSection(onAdd = { sheet = SourcesSheet.Source }) }
                item { BooksSection(catalogues, onAddCatalogue = { sheet = SourcesSheet.Catalogue }, onChanged = { catalogues = loadCatalogues(model) }) }
            }
            item { RepositoriesSection(onAdd = { sheet = SourcesSheet.Repository }) }
        }
    }
    when (sheet) {
        SourcesSheet.Repository -> AkiPrompt(
            Copy.Sources.addRepository, Copy.Sources.addRepositoryMessage, Copy.Welcome.AddRepository.placeholder, Copy.Sources.previewRepository,
            onDismiss = { sheet = null }, onConfirm = { sheet = null; model.propose(it) },
        )
        SourcesSheet.Source -> SourceAddSheet { sheet = null }
        SourcesSheet.Catalogue -> AkiPrompt(
            Copy.Books.addBookCatalogue, Copy.Sources.addCatalogueMessage, Copy.Books.catalogueAddressPlaceholder, Copy.Books.addCatalogueConfirm, { sheet = null },
            onConfirm = { address ->
                sheet = null
                scope.launch {
                    try {
                        model.engine.addCatalogue(address)
                        catalogues = loadCatalogues(model)
                    } catch (failure: Exception) {
                        catalogueProblem = describe(failure)
                    }
                }
            },
        )
        null -> {}
    }
    catalogueProblem?.let { AkiAlert(it.title, it.message) { catalogueProblem = null } }
}

/** The sheet or prompt over the sources: only one is open at a time. */
private enum class SourcesSheet { Repository, Source, Catalogue }

private fun loadCatalogues(model: live.akidev.manga.app.AppModel) = runCatching { model.engine.catalogues() }.getOrDefault(emptyList())

/** Books come from files on the phone and from catalogues the person adds by address (OPDS feeds). */
@Composable
private fun BooksSection(catalogues: List<uniffi.akimanga.CatalogueInfo>, onAddCatalogue: () -> Unit, onChanged: () -> Unit) {
    val model = LocalApp.current
    val router = LocalRouter.current
    AkiSection(
        Copy.Home.books,
        footer = Copy.Sources.booksFooter,
    ) {
        for (catalogue in catalogues) {
            val remove = AkiMenuItem(Copy.Books.removeCatalogue, "trash", destructive = true) { model.engine.removeCatalogue(catalogue.url); onChanged() }
            AkiTapMenu(listOf(remove), onClick = { router.push(CatalogueRoute(CatalogueRef(catalogue.url, catalogue.name, null))) }) {
                AkiRow(catalogue.name, subtitle = catalogue.url, icon = "book-solid", showsChevron = true)
            }
        }
        AkiButton(Copy.Sources.addEpub, icon = "book", fullWidth = true) { model.tab = AppTab.Books; model.addingBook = true }
        AkiButton(Copy.Books.addBookCatalogue, icon = "add", fullWidth = true, onClick = onAddCatalogue)
    }
}

@Composable
private fun InstalledSection(onAdd: () -> Unit) {
    val model = LocalApp.current
    val router = LocalRouter.current
    AkiSection(Copy.Sources.installed) {
        if (model.sources.isEmpty()) AkiCard(padding = 16.dp) { AkiText(Copy.Sources.noSourcesYet, tone = AkiTone.Muted) }
        for (source in model.sources) {
            val problem = model.sourceProblems[source.id]
            val remove = AkiMenuItem(Copy.Sources.removeSource, "trash", destructive = true) { model.engine.removeSource(source.id); model.refresh() }
            AkiTapMenu(listOf(remove), onClick = { router.push(SourceSettingsRoute(source.id)) }) {
                AkiRow(
                    source.name,
                    subtitle = problem?.let { Copy.Sources.notWorking(it) } ?: if (source.nsfw) Copy.Sources.sourceLineAdult(source.languages.joinToString(", ").uppercase(), "${source.version}") else Copy.Sources.sourceLine(source.languages.joinToString(", ").uppercase(), "${source.version}"),
                    icon = if (problem == null) "sources-solid" else "warning-solid",
                    showsChevron = true,
                )
            }
        }
        AkiButton(Copy.Sources.addSource, icon = "add", fullWidth = true, onClick = onAdd)
    }
}

@Composable
private fun RepositoriesSection(onAdd: () -> Unit) {
    val model = LocalApp.current
    val router = LocalRouter.current
    AkiSection(
        Copy.Sources.repositories,
        footer = Copy.Sources.repositoriesFooter,
    ) {
        if (model.repos.isEmpty()) AkiCard(padding = 16.dp) { AkiText(Copy.Sources.noRepositoriesYet, tone = AkiTone.Muted) }
        for (repo in model.repos) {
            val remove = AkiMenuItem(Copy.Sources.removeRepository, "trash", destructive = true) { model.engine.removeRepo(repo.base); model.refresh() }
            AkiTapMenu(listOf(remove), onClick = { router.push(RepoRoute(repo)) }) {
                AkiRow(repo.name, subtitle = if (repo.signed) Copy.Sources.signed else Copy.Sources.notSigned, icon = "server-solid", showsChevron = true)
            }
        }
        AkiButton(Copy.Sources.addRepository, icon = "add", fullWidth = true, onClick = onAdd)
    }
}
