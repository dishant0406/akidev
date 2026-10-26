package live.akidev.manga.sources

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
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
import live.akidev.manga.app.Problem
import live.akidev.manga.app.describe
import live.akidev.manga.design.*
import uniffi.akimanga.RepoInfo
import uniffi.akimanga.RepoSource
import uniffi.akimanga.RepoView
import uniffi.akimanga.SourceState

/** What a repository offers, with install and update. */
@Composable
fun RepoScreen(repo: RepoInfo) {
    val model = LocalApp.current
    val router = LocalRouter.current
    val scope = rememberCoroutineScope()
    var view by remember { mutableStateOf<RepoView?>(null) }
    var error by remember { mutableStateOf<Problem?>(null) }
    var working by remember { mutableStateOf(setOf<String>()) }

    suspend fun load() {
        error = null
        try { view = model.engine.repoSources(repo.base) } catch (failure: Exception) { error = describe(failure) }
    }
    LaunchedEffect(Unit) { load() }

    fun install(source: RepoSource) {
        working = working + source.id
        scope.launch {
            try {
                model.engine.installFromRepo(repo.base, source.id)
                model.refresh()
                load()
            } catch (failure: Exception) {
                error = describe(failure)
            }
            working = working - source.id
        }
    }

    AkiRefreshable({ load() }, Modifier.fillMaxSize()) {
        AkiPage(underTabBar = false) {
            val loaded = view
            val problem = error
            item { AkiBackHeader(repo.name, back = router::back, subtitle = if (loaded?.info?.signed == true) "Signed repository" else "Not signed") }
            if (problem != null) {
                item { AkiEmptyState(problem.title, problem.message, Modifier.heightIn(min = 260.dp), action = AkiAction("Try again") { scope.launch { load() } }) }
            } else if (loaded == null) {
                item { Box(Modifier.fillMaxWidth().heightIn(min = 200.dp), contentAlignment = Alignment.Center) { AkiSpinner() } }
            } else {
                items(loaded.sources.size) { index ->
                    val source = loaded.sources[index]
                    AkiRow(source.name, subtitle = source.languages.joinToString(", ").uppercase() + " · v${source.version}" + if (source.nsfw) " · 18+" else "") {
                        RepoAction(source, installing = source.id in working) { install(source) }
                    }
                }
                item {
                    AkiText(
                        if (loaded.info.signed) "This repository signs its index, so the app notices if it ever changes hands." else "This repository is not signed. You are trusting its address.",
                        AkiTextStyle.Caption, tone = AkiTone.Muted,
                    )
                }
            }
        }
    }
}

@Composable
private fun RepoAction(source: RepoSource, installing: Boolean, onInstall: () -> Unit) {
    if (installing) {
        AkiSpinner()
        return
    }
    when (source.state) {
        SourceState.NOT_INSTALLED -> AkiButton("Install", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Small, onClick = onInstall)
        SourceState.UPDATE_AVAILABLE -> AkiButton("Update", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Small, onClick = onInstall)
        SourceState.INSTALLED -> AkiBadge("Installed", icon = "tick")
        SourceState.NEEDS_NEWER_APP -> AkiBadge("Needs a newer app")
    }
}
