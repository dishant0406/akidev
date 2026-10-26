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
 * Akidev is a renderer: it ships with no source. Sources are plugins that come from repositories the person adds by address (a GitHub
 * link, say); this screen is where that happens.
 */
@Composable
fun SourcesScreen() {
    val model = LocalApp.current
    val router = LocalRouter.current
    var addingRepo by remember { mutableStateOf(false) }
    var pastingSource by remember { mutableStateOf(false) }
    var catalogues by remember { mutableStateOf(loadCatalogues(model)) }
    var addingCatalogue by remember { mutableStateOf(false) }
    var catalogueProblem by remember { mutableStateOf<Problem?>(null) }
    val scope = rememberCoroutineScope()

    AkiRefreshable({ model.updateSources(); model.refresh() }, Modifier.fillMaxSize()) {
        AkiPage {
            item {
                AkiHeader("Sources", subtitle = "${model.sources.size} installed") {
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        AkiIconButton("settings", "Settings") { router.push(SettingsRoute) }
                        AkiIconButton("add", "Add a repository", tint = Aki.colors.primary) { addingRepo = true }
                    }
                }
            }
            if (model.prefs.readerKind == ReaderKind.Books) {
                item { BooksSection(catalogues, onAddCatalogue = { addingCatalogue = true }, onChanged = { catalogues = loadCatalogues(model) }) }
                item { InstalledSection(onAdd = { pastingSource = true }) }
            } else {
                item { InstalledSection(onAdd = { pastingSource = true }) }
                item { BooksSection(catalogues, onAddCatalogue = { addingCatalogue = true }, onChanged = { catalogues = loadCatalogues(model) }) }
            }
            item { RepositoriesSection(onAdd = { addingRepo = true }) }
        }
    }
    if (addingRepo) {
        AkiPrompt(
            "Add a repository", "The address of a repository's folder on GitHub, or any https folder with an index.json.", "github.com/owner/repository", "Continue",
            onDismiss = { addingRepo = false }, onConfirm = { addingRepo = false; model.propose(it) },
        )
    }
    if (pastingSource) SourceAddSheet { pastingSource = false }
    if (addingCatalogue) {
        AkiPrompt(
            "Add a book catalogue", "The address of an OPDS catalogue, for example a Calibre-Web or Standard Ebooks feed.", "example.org/opds", "Add", { addingCatalogue = false },
            onConfirm = { address ->
                addingCatalogue = false
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
    }
    catalogueProblem?.let { AkiAlert(it.title, it.message) { catalogueProblem = null } }
}

private fun loadCatalogues(model: live.akidev.manga.app.AppModel) = runCatching { model.engine.catalogues() }.getOrDefault(emptyList())

/** Books come from files on the phone and from catalogues the person adds by address (OPDS feeds). */
@Composable
private fun BooksSection(catalogues: List<uniffi.akimanga.CatalogueInfo>, onAddCatalogue: () -> Unit, onChanged: () -> Unit) {
    val model = LocalApp.current
    val router = LocalRouter.current
    AkiSection(
        "Books",
        footer = "Add an EPUB from your device, or a catalogue of books by its address (an OPDS feed). Akidev does not include, offer or endorse any catalogue. Books are read in the Books tab and stay on this phone.",
    ) {
        for (catalogue in catalogues) {
            val remove = AkiMenuItem("Remove", "trash", destructive = true) { model.engine.removeCatalogue(catalogue.url); onChanged() }
            AkiTapMenu(listOf(remove), onClick = { router.push(CatalogueRoute(CatalogueRef(catalogue.url, catalogue.name, null))) }) {
                AkiRow(catalogue.name, subtitle = catalogue.url, icon = "book-solid", showsChevron = true)
            }
        }
        AkiButton("Add an EPUB book", icon = "book", fullWidth = true) { model.tab = AppTab.Books; model.addingBook = true }
        AkiButton("Add a book catalogue", icon = "add", fullWidth = true, onClick = onAddCatalogue)
    }
}

@Composable
private fun InstalledSection(onAdd: () -> Unit) {
    val model = LocalApp.current
    val router = LocalRouter.current
    AkiSection("Installed") {
        if (model.sources.isEmpty()) AkiCard(padding = 16.dp) { AkiText("Nothing installed yet", tone = AkiTone.Muted) }
        for (source in model.sources) {
            val problem = model.sourceProblems[source.id]
            val remove = AkiMenuItem("Remove", "trash", destructive = true) { model.engine.removeSource(source.id); model.refresh() }
            AkiTapMenu(listOf(remove), onClick = { router.push(SourceSettingsRoute(source.id)) }) {
                AkiRow(
                    source.name,
                    subtitle = problem?.let { "Not working: $it" } ?: (source.languages.joinToString(", ").uppercase() + " · v${source.version}" + if (source.nsfw) " · 18+" else ""),
                    icon = if (problem == null) "sources-solid" else "warning-solid",
                    showsChevron = true,
                )
            }
        }
        AkiButton("Add a source", icon = "add", fullWidth = true, onClick = onAdd)
    }
}

@Composable
private fun RepositoriesSection(onAdd: () -> Unit) {
    val model = LocalApp.current
    val router = LocalRouter.current
    AkiSection(
        "Repositories",
        footer = "A repository is a folder of source plugins, usually on GitHub: paste its address. Akidev does not include, offer or endorse any. Only add repositories you trust: a plugin decides which sites the app contacts.",
    ) {
        if (model.repos.isEmpty()) AkiCard(padding = 16.dp) { AkiText("No repository added", tone = AkiTone.Muted) }
        for (repo in model.repos) {
            val remove = AkiMenuItem("Remove", "trash", destructive = true) { model.engine.removeRepo(repo.base); model.refresh() }
            AkiTapMenu(listOf(remove), onClick = { router.push(RepoRoute(repo)) }) {
                AkiRow(repo.name, subtitle = if (repo.signed) "Signed" else "Not signed", icon = "server-solid", showsChevron = true)
            }
        }
        AkiButton("Add a repository", icon = "add", fullWidth = true, onClick = onAdd)
    }
}
