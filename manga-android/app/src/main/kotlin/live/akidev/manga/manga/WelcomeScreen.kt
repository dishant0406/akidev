package live.akidev.manga.manga

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import live.akidev.manga.app.LocalApp
import live.akidev.manga.design.*

/** The first screen of a fresh install: the app ships with no source, so the way in is an address. */
@Composable
fun WelcomeScreen() {
    val model = LocalApp.current
    var typing by remember { mutableStateOf(false) }
    Column(
        Modifier.fillMaxWidth().heightIn(min = 560.dp).padding(horizontal = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(22.dp, Alignment.CenterVertically),
    ) {
        AkiLogo(Modifier.shadow(20.dp, AkiShapes.rounded(26.dp), ambientColor = Aki.colors.shadow, spotColor = Aki.colors.shadow), size = 120.dp)
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
            AkiText("Welcome to Akidev", AkiTextStyle.Display, align = TextAlign.Center)
            AkiText(
                "Akidev is a reader. It comes with no manga: you add a repository of sources, a folder usually kept on GitHub, and read what those sources describe.",
                AkiTextStyle.Body, tone = AkiTone.Muted, align = TextAlign.Center,
            )
        }
        Column(Modifier.padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            AkiPasteButton { model.propose(it) }
            AkiButton("Type an address", Modifier.fillMaxWidth(), icon = "keyboard", fullWidth = true) { typing = true }
        }
        AkiText("Only add repositories you trust.", AkiTextStyle.Caption, tone = AkiTone.Muted, align = TextAlign.Center)
    }
    if (typing) {
        AkiPrompt(
            "Add a repository", "The address of a repository's folder on GitHub, or any https folder with an index.json.", "github.com/owner/repository", "Continue",
            onDismiss = { typing = false }, onConfirm = { typing = false; model.propose(it) },
        )
    }
}
