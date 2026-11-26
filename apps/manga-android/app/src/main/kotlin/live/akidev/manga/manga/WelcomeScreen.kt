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
            AkiText(Copy.Welcome.title, AkiTextStyle.Display, align = TextAlign.Center, isHeading = true)
            AkiText(Copy.Welcome.body, AkiTextStyle.Body, tone = AkiTone.Muted, align = TextAlign.Center)
        }
        Column(Modifier.padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            AkiPasteButton { model.propose(it) }
            AkiButton(Copy.Welcome.typeAddressButton, Modifier.fillMaxWidth(), icon = "keyboard", fullWidth = true) { typing = true }
        }
        AkiText(Copy.Welcome.caption, AkiTextStyle.Caption, tone = AkiTone.Muted, align = TextAlign.Center)
    }
    if (typing) {
        AkiPrompt(
            Copy.Welcome.AddRepository.title, Copy.Welcome.AddRepository.message, Copy.Welcome.AddRepository.placeholder, Copy.Welcome.AddRepository.confirm,
            onDismiss = { typing = false }, onConfirm = { typing = false; model.propose(it) },
        )
    }
}
