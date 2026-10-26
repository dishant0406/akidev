package live.akidev.manga.sources

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import live.akidev.manga.app.LocalApp
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
            item { InstalledSection(onAdd = { pastingSource = true }) }
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
