package live.akidev.manga.app

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import live.akidev.manga.design.*

/** The first screen of a fresh install: what does the person read? Both stay in the app; the answer only decides what comes first. */
@Composable
fun OnboardingScreen() {
    val model = LocalApp.current
    var chosen by remember { mutableStateOf<ReaderKind?>(null) }
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { shown = true }
    LaunchedEffect(chosen) {
        val kind = chosen ?: return@LaunchedEffect
        // The card takes the yellow and the other steps back, and then the app opens on what was chosen.
        delay(450)
        model.prefs.readerKind = kind
        model.tab = model.tabs.first()
    }

    Box(Modifier.fillMaxSize()) {
        AkiAurora()
        Column(
            Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().padding(horizontal = 20.dp).widthIn(max = 560.dp).align(Alignment.Center),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(28.dp, Alignment.CenterVertically),
        ) {
            Reveal(shown, 0) {
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(18.dp)) {
                    Box(Modifier.shadow(18.dp, AkiShapes.rounded(34.dp), ambientColor = Aki.colors.shadow, spotColor = Aki.colors.shadow).akiGlass(AkiShapes.rounded(34.dp)).padding(10.dp)) { AkiLogo(size = 76.dp) }
                    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        AkiText("What do you read?", AkiTextStyle.Display, align = TextAlign.Center)
                        AkiText("Akidev reads manga and books, both always here. Pick what comes first.", AkiTextStyle.Body, tone = AkiTone.Muted, align = TextAlign.Center)
                    }
                }
            }
            Row(Modifier.height(IntrinsicSize.Max), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Reveal(shown, 120, Modifier.weight(1f).fillMaxHeight()) { Choice(ReaderKind.Manga, "Follow series from your sources, with new chapter alerts.", chosen) { chosen = it } }
                Reveal(shown, 220, Modifier.weight(1f).fillMaxHeight()) { Choice(ReaderKind.Books, "EPUB books from your files or a catalogue, with your place kept.", chosen) { chosen = it } }
            }
            Reveal(shown, 320) { AkiText("You can change this any time in Settings.", AkiTextStyle.Caption, tone = AkiTone.Muted, align = TextAlign.Center) }
        }
    }
}

@Composable
private fun Choice(kind: ReaderKind, detail: String, chosen: ReaderKind?, onChoose: (ReaderKind) -> Unit) {
    val state = when (chosen) { null -> AkiChoiceState.Rest; kind -> AkiChoiceState.Chosen; else -> AkiChoiceState.Passed }
    AkiChoiceCard(kind.title, detail, state, { if (chosen == null) onChoose(kind) }, Modifier.fillMaxSize()) {
        if (kind == ReaderKind.Manga) AkiMangaArt() else AkiBooksArt()
    }
}

/** Comes up from below and fades in, a moment after the one before. */
@Composable
private fun Reveal(shown: Boolean, delayMs: Int, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    AnimatedVisibility(shown, modifier, enter = fadeIn(tween(600, delayMs)) + slideInVertically(tween(700, delayMs)) { 60 }) { content() }
}
