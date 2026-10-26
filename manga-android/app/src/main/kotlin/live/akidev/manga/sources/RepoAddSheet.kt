package live.akidev.manga.sources

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import live.akidev.manga.app.AppTab
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.PendingRepo
import live.akidev.manga.app.Problem
import live.akidev.manga.app.describe
import live.akidev.manga.design.*
import uniffi.akimanga.RepoView

/**
 * "Add this repository?": shows what an address offers before anything is kept. A repository is code that decides which sites the app
 * contacts, so the person sees its name, whether it is signed and what it lists.
 */
@Composable
fun RepoAddSheet(pending: PendingRepo, onDismiss: () -> Unit) {
    val model = LocalApp.current
    val scope = rememberCoroutineScope()
    var view by remember { mutableStateOf<RepoView?>(null) }
    var error by remember { mutableStateOf<Problem?>(null) }
    var adding by remember { mutableStateOf(false) }
    var attempt by remember { mutableStateOf(0) }

    LaunchedEffect(attempt) {
        error = null
        try { view = model.engine.previewRepo(pending.address) } catch (failure: Exception) { error = describe(failure) }
    }

    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                AkiLogo(size = 36.dp)
                AkiText("Add a repository", AkiTextStyle.Title)
            }
            val problem = error
            val loaded = view
            if (problem != null) {
                AkiEmptyState(problem.title, problem.message, Modifier.heightIn(min = 240.dp), action = AkiAction("Try again") { attempt += 1 })
            } else if (loaded != null) {
                AkiText(loaded.info.name, AkiTextStyle.Heading)
                AkiText(if (loaded.info.signed) "Signed, so the app notices if it ever changes hands." else "Not signed: you are trusting this address.", AkiTextStyle.Caption, tone = AkiTone.Muted)
                AkiCard(padding = 4.dp) {
                    for (source in loaded.sources) {
                        AkiRow(source.name, subtitle = source.languages.joinToString(", ").uppercase() + if (source.nsfw) " · 18+" else "")
                    }
                }
                AkiText("Akidev does not include, offer or endorse any repository. Only add one you trust.", AkiTextStyle.Caption, tone = AkiTone.Muted)
                AkiButton("Add repository", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large, fullWidth = true) {
                    if (adding) return@AkiButton
                    adding = true
                    scope.launch {
                        try {
                            model.engine.addRepo(pending.address)
                            model.refresh()
                            model.tab = AppTab.Sources
                            onDismiss()
                        } catch (failure: Exception) {
                            error = describe(failure)
                        }
                        adding = false
                    }
                }
            } else {
                Box(Modifier.fillMaxWidth().heightIn(min = 160.dp), contentAlignment = Alignment.Center) { AkiSpinner() }
            }
            AkiButton("Cancel", variant = AkiButtonVariant.Ghost, fullWidth = true, onClick = onDismiss)
        }
    }
}
