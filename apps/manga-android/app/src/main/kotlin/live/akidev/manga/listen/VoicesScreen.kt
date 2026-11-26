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
        item { AkiBackHeader("Voice models", back = router::back, subtitle = "Reading aloud happens on this phone") }
        if (model.models.isEmpty()) item { AkiEmptyState("No voice models yet", "The voices repository has none to offer. Check your connection and try again later.", Modifier.heightIn(min = 260.dp)) }
        items(model.models.size, key = { model.models[it].id }) { index ->
            val voice = model.models[index]
            val parts = listOf(voice.languages.joinToString(", ").uppercase(), Formatter.formatFileSize(context, voice.bytes.toLong()), voice.licence) + if (voice.commercial) emptyList() else listOf("not for commercial use")
            AkiRow(voice.name, subtitle = parts.joinToString(" · ")) { Control(voice) { scope.launch { model.install(voice) } } }
        }
        val current = model.current
        if (current != null && current.voices.isNotEmpty()) {
            item {
                AkiSection("Voice") {
                    AkiTap({ router.push(VoiceChoiceRoute) }) { AkiRow("Voice of ${current.name}", subtitle = model.chosenVoice?.name ?: "Standard", icon = "headphones", showsChevron = true) }
                }
            }
        }
        // Underdog Woof's licence asks every app that offers it to credit it, with a link.
        if (model.models.any { it.id == "woof-en" }) {
            item { AkiTap({ uriHandler.openUri("https://underdog.ai/woof") }) { AkiText("Voice by Underdog Woof, underdog.ai. Free for non-commercial use.", AkiTextStyle.Caption, tone = AkiTone.Muted) } }
        }
        item { AkiText("A model is downloaded once, checked, and then reads books with no network. Nothing you read leaves your phone.", AkiTextStyle.Caption, tone = AkiTone.Muted) }
    }
    model.failure?.let { AkiAlert(it.title, it.message) { model.failure = null } }
}

@Composable
private fun Control(voice: VoiceModel, install: () -> Unit) {
    val model = LocalApp.current.readAloud
    val fraction = model.progress[voice.id]
    when {
        fraction != null -> AkiProgressBar(fraction, Modifier.width(64.dp), label = "Downloading ${voice.name}")
        model.has(voice) -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (model.hasUpdate(voice)) AkiButton("Update", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Small, description = "Update ${voice.name}", onClick = install)
            else if (model.current?.id == voice.id) AkiText("In use", AkiTextStyle.CaptionStrong, tone = AkiTone.Muted)
            else AkiButton("Use", size = AkiButtonSize.Small, description = "Use ${voice.name}") { model.use(voice) }
            AkiIconButton("trash", "Delete ${voice.name}") { model.remove(voice) }
        }
        voice.usable -> AkiButton("Download", icon = "download", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Small, description = "Download ${voice.name}", onClick = install)
        else -> AkiText("Update Akidev to use this voice", AkiTextStyle.Caption, tone = AkiTone.Muted)
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
    val groups = listOf("en-us" to "American English", "en-gb" to "British English", "" to "Characters").map { (key, title) -> title to voices.filter { (it.accent ?: "") == key } }.filter { it.second.isNotEmpty() }

    AkiPage(underTabBar = false, spacing = 22.dp) {
        item { AkiBackHeader("Voice", back = router::back, subtitle = model.current?.let { "Voices of ${it.name}" }) }
        item { AkiSection("Standard") { VoiceCard("", "${model.current?.name ?: "Model"} voice", null, "The model’s own voice.", null) } }
        for ((title, group) in groups) {
            item(key = title) {
                AkiSection(title) { for (voice in group) VoiceCard(voice.id, voice.name, detail(voice), voice.description, voice) }
            }
        }
        item { AkiText("These voices differ in pace and in how much they lift and drop. Hear a few before choosing: the model reads the words, the voice sets the character.", AkiTextStyle.Caption, tone = AkiTone.Muted) }
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
                    busy -> Box(Modifier.size(44.dp), contentAlignment = Alignment.Center) { AkiSpinner(label = "Getting $name") }
                    model.playing == id -> AkiIconButton("pause", "Stop sample of $name") { model.stopHearing() }
                    else -> AkiIconButton("play", "Play sample of $name") { scope.launch { model.hear(voice) } }
                }
                if (model.voiceInUse == id) AkiText("In use", AkiTextStyle.CaptionStrong, tone = AkiTone.Muted)
                else AkiButton("Use", size = AkiButtonSize.Small, description = "Use $name") { scope.launch { model.choose(voice) } }
            }
        }
    }
}
