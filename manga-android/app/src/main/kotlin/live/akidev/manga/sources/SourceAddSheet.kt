package live.akidev.manga.sources

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
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
import live.akidev.manga.app.Problem
import live.akidev.manga.app.describe
import live.akidev.manga.design.*
import uniffi.akimanga.SourceInfo

/** Where the pasted definition is: not checked, being checked, found to be this source, or not good. */
private sealed interface PasteStep {
    data object Idle : PasteStep
    data object Checking : PasteStep
    data class Found(val source: SourceInfo) : PasteStep
    data class Failed(val problem: Problem) : PasteStep
}

/**
 * "Add a source?": a definition pasted in by hand, shown before anything is kept. Most sources come from a repository; this is for one
 * written on its own, which usually extends a theme.
 */
@Composable
fun SourceAddSheet(onDismiss: () -> Unit) {
    val model = LocalApp.current
    val scope = rememberCoroutineScope()
    var text by remember { mutableStateOf("") }
    var step by remember { mutableStateOf<PasteStep>(PasteStep.Idle) }
    var installing by remember { mutableStateOf(false) }
    val json = text.trim()

    fun check() {
        if (step == PasteStep.Checking || json.isEmpty()) return
        step = PasteStep.Checking
        scope.launch {
            step = try { PasteStep.Found(model.engine.previewSource(json)) } catch (failure: Exception) { PasteStep.Failed(describe(failure)) }
        }
    }

    fun install() {
        if (installing || json.isEmpty()) return
        installing = true
        scope.launch {
            try {
                model.engine.installSource(json)
                model.refresh()
                model.tab = AppTab.Sources
                onDismiss()
            } catch (failure: Exception) {
                step = PasteStep.Failed(describe(failure))
            }
            installing = false
        }
    }

    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                AkiLogo(size = 36.dp)
                AkiText("Add a source", AkiTextStyle.Title)
            }
            AkiText("Paste a source definition. One that extends a theme has the theme merged in, so it works on its own.", AkiTextStyle.Caption, tone = AkiTone.Muted)
            AkiPasteButton { pasted -> text = pasted; step = PasteStep.Idle }
            AkiTextArea(text, { text = it; step = PasteStep.Idle })

            val state = step
            if (state is PasteStep.Failed) {
                AkiEmptyState(state.problem.title, state.problem.message, Modifier.heightIn(min = 240.dp), action = AkiAction("Check again") { check() })
            } else if (state is PasteStep.Found) {
                val found = state.source
                AkiCard(padding = 16.dp) {
                    AkiText(found.name, AkiTextStyle.Heading)
                    AkiText(found.languages.joinToString(", ").uppercase() + " · v${found.version}" + if (found.nsfw) " · 18+" else "", AkiTextStyle.Caption, tone = AkiTone.Muted)
                }
                model.engine.installedOrigin(found.id)?.let { origin ->
                    AkiText("This replaces the ${found.name} installed from $origin. A pasted source does not update itself, and the repository stops updating it.", AkiTextStyle.Caption, tone = AkiTone.Muted)
                }
                AkiButton(if (installing) "Installing…" else "Install", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large, fullWidth = true, enabled = !installing) { install() }
            } else {
                AkiButton(if (state == PasteStep.Checking) "Checking…" else "Check", size = AkiButtonSize.Large, fullWidth = true, enabled = state != PasteStep.Checking && json.isNotEmpty()) { check() }
            }
            AkiText("Akidev does not include, offer or endorse any source. Only add one you trust: it decides which sites the app contacts.", AkiTextStyle.Caption, tone = AkiTone.Muted)
            AkiButton("Cancel", variant = AkiButtonVariant.Ghost, fullWidth = true, onClick = onDismiss)
        }
    }
}
