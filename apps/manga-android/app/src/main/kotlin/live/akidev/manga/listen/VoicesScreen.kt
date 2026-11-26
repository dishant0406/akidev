package live.akidev.manga.listen

import android.text.format.Formatter
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.VoiceChoiceRoute
import live.akidev.manga.design.*
import live.akidev.manga.design.Copy
import uniffi.akimanga.VoiceModel
import uniffi.akimanga.VoiceOption
import live.akidev.manga.books.percent

/** The voice models for reading aloud: get one, choose which reads, remove it. */
@Composable
fun VoicesScreen() {
    val app = LocalApp.current
    val router = LocalRouter.current
    val model = app.readAloud
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val uriHandler = LocalUriHandler.current
    AkiPage(underTabBar = false, spacing = 16.dp) {
        item { AkiBackHeader(Copy.Listen.voiceModels, back = router::back, subtitle = Copy.Listen.readingAloudOnPhone) }
        if (model.models.isEmpty()) item { AkiEmptyState(Copy.Listen.noVoiceModels, Copy.Listen.noVoiceModelsHint, Modifier.heightIn(min = 260.dp)) }
        items(model.models.size, key = { model.models[it].id }) { index ->
            val voice = model.models[index]
            val parts = listOf(voice.languages.joinToString(", ").uppercase(), Formatter.formatFileSize(context, voice.bytes.toLong()), voice.licence) + if (voice.commercial) emptyList() else listOf(Copy.Listen.notForCommercialUse)
            AkiRow(voice.name, subtitle = parts.joinToString(" · ")) { Control(voice) { scope.launch { model.install(voice) } } }
        }
        val current = model.current
        if (current != null && current.voices.isNotEmpty()) {
            item {
                AkiSection(Copy.Listen.voice) {
                    AkiTap({ router.push(VoiceChoiceRoute) }) { AkiRow(Copy.Listen.voiceOf(current.name), subtitle = model.chosenVoice?.name ?: Copy.Listen.standard, icon = "headphones", showsChevron = true) }
                }
            }
        }
        // Underdog Woof's licence asks every app that offers it to credit it, with a link.
        if (model.models.any { it.id == "woof-en" }) {
            item { AkiTap({ uriHandler.openUri("https://underdog.ai/woof") }) { AkiText(Copy.Listen.underdogCredit, AkiTextStyle.Caption, tone = AkiTone.Muted) } }
        }
        item { AkiText(Copy.Listen.modelDownloadedOnce, AkiTextStyle.Caption, tone = AkiTone.Muted) }
    }
    model.failure?.let { AkiAlert(it.title, it.message) { model.failure = null } }
}

@Composable
private fun Control(voice: VoiceModel, install: () -> Unit) {
    val model = LocalApp.current.readAloud
    val fraction = model.progress[voice.id]
    when {
        fraction != null -> AkiProgressBar(fraction, Modifier.width(64.dp), label = Copy.Listen.downloadingVoice(voice.name))
        model.has(voice) -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (model.hasUpdate(voice)) AkiButton(Copy.Listen.update, variant = AkiButtonVariant.Primary, size = AkiButtonSize.Small, description = Copy.Listen.updateVoice(voice.name), onClick = install)
            else if (model.current?.id == voice.id) AkiText(Copy.Listen.inUse, AkiTextStyle.CaptionStrong, tone = AkiTone.Muted)
            else AkiButton(Copy.Listen.use, size = AkiButtonSize.Small, description = Copy.Listen.useVoice(voice.name)) { model.use(voice) }
            AkiIconButton("trash", Copy.Listen.deleteVoice(voice.name)) { model.remove(voice) }
        }
        voice.usable -> AkiButton(Copy.Series.download, icon = "download", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Small, description = Copy.Listen.downloadVoice(voice.name), onClick = install)
        else -> AkiText(Copy.Listen.updateAkidevToUseVoice, AkiTextStyle.Caption, tone = AkiTone.Muted)
    }
}

/** The voices of the model that reads: hear one, choose one. A voice is fetched (a few hundred kilobytes) when first heard or chosen. */
@Composable
fun VoiceChoiceScreen() {
    val app = LocalApp.current
    val router = LocalRouter.current
    val model = app.readAloud
    // A sample stops when the person leaves.
    DisposableEffect(Unit) { onDispose { model.stopHearing() } }

    /** The voices grouped by where they come from, in the order the model lists them. */
    val voices = model.current?.voices ?: emptyList()
    val groups = listOf("en-us" to Copy.Listen.americanEnglish, "en-gb" to Copy.Listen.britishEnglish, "" to Copy.Listen.characters).map { (key, title) -> title to voices.filter { (it.accent ?: "") == key } }.filter { it.second.isNotEmpty() }

    AkiPage(underTabBar = false, spacing = 22.dp) {
        item { AkiBackHeader(Copy.Listen.voice, back = router::back, subtitle = model.current?.let { Copy.Listen.voicesOf(it.name) }) }
        item { AkiSection(Copy.Listen.standard) { VoiceCard("", Copy.Listen.voiceName(model.current?.name ?: Copy.Listen.modelFallback), null, Copy.Listen.modelsOwnVoice, null) } }
        for ((title, group) in groups) {
            item(key = title) {
                AkiSection(title) { for (voice in group) VoiceCard(voice.id, voice.name, detail(voice), voice.description, voice) }
            }
        }
        item { AkiText(Copy.Listen.voicesDifferHint, AkiTextStyle.Caption, tone = AkiTone.Muted) }
    }
    model.failure?.let { AkiAlert(it.title, it.message) { model.failure = null } }
}

private fun detail(voice: VoiceOption): String? {
    val parts = listOfNotNull(voice.gender, voice.age).map { it.replaceFirstChar { first -> first.uppercase() } }
    return if (parts.isEmpty()) null else parts.joinToString(" · ")
}

@Composable
private fun VoiceCard(id: String, name: String, detail: String?, description: String, voice: VoiceOption?) {
    val model = LocalApp.current.readAloud
    val scope = rememberCoroutineScope()
    val busy = id in model.fetching || model.hearing == id
    AkiCard(padding = 16.dp) {
        androidx.compose.foundation.layout.Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                AkiText(name, AkiTextStyle.Heading, isHeading = true)
                if (detail != null) AkiText(detail, AkiTextStyle.Caption, tone = AkiTone.Muted)
            }
            AkiText(description, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 4)
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
                when {
                    busy -> Box(Modifier.size(44.dp), contentAlignment = Alignment.Center) { AkiSpinner(label = Copy.Listen.gettingVoice(name)) }
                    model.playing == id -> AkiIconButton("pause", Copy.Listen.stopSampleOf(name)) { model.stopHearing() }
                    else -> AkiIconButton("play", Copy.Listen.playSampleOf(name)) { scope.launch { model.hear(voice) } }
                }
                if (model.voiceInUse == id) AkiText(Copy.Listen.inUse, AkiTextStyle.CaptionStrong, tone = AkiTone.Muted)
                else AkiButton(Copy.Listen.use, size = AkiButtonSize.Small, description = Copy.Listen.useVoice(name)) { scope.launch { model.choose(voice) } }
            }
        }
    }
}
