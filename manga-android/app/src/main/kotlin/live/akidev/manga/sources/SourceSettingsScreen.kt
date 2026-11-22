package live.akidev.manga.sources

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import kotlinx.coroutines.launch
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.design.*
import uniffi.akimanga.SettingInfo

/** The choices a source offers (a language, what content to show). */
@Composable
fun SourceSettingsScreen(source: String) {
    val model = LocalApp.current
    val router = LocalRouter.current
    var settings by remember { mutableStateOf(emptyList<SettingInfo>()) }
    var removing by remember { mutableStateOf(false) }
    var signingIn by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val info = model.sources.firstOrNull { it.id == source }
    var signedInAs by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(info?.signedIn) { signedInAs = runCatching { model.engine.signedInAs(source) }.getOrNull() }
    LaunchedEffect(Unit) { settings = runCatching { model.engine.sourceSettings(source) }.getOrDefault(emptyList()) }

    AkiPage(underTabBar = false, spacing = 16.dp) {
        item { AkiBackHeader(model.name(source), back = router::back, subtitle = "Settings") }
        if (info?.hasSignIn == true) item {
            if (info.signedIn) {
                AkiRow(signedInAs ?: "Account", subtitle = "Signed in") {
                    AkiButton("Sign out", size = AkiButtonSize.Small) { scope.launch { runCatching { model.engine.signOut(source) }; model.refresh() } }
                }
            } else {
                AkiRow("Account", subtitle = "Not signed in") {
                    AkiButton("Sign in", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Small) { signingIn = true }
                }
            }
        }
        if (settings.isEmpty() && info?.hasSignIn != true) item { AkiText("This source has no settings.", tone = AkiTone.Muted) }
        items(settings.size) { index ->
            val setting = settings[index]
            AkiCard(padding = 16.dp) {
                AkiSelect(setting.name, setting.value, { value ->
                    model.engine.setSetting(source, setting.id, value)
                    settings = runCatching { model.engine.sourceSettings(source) }.getOrDefault(settings)
                    model.sourceSettingsChanged()
                }, setting.options.map { AkiOption(it.value, it.name) })
            }
        }
        item { AkiButton("Remove source", Modifier.padding(top = 8.dp), icon = "trash", variant = AkiButtonVariant.Destructive, fullWidth = true) { removing = true } }
    }
    if (signingIn) SignInSheet(source) { signingIn = false }
    if (removing) {
        AkiConfirm("Remove “${model.name(source)}”?", "Its series stay in your library but cannot be read until you install it again.", "Remove source", { removing = false }) {
            model.engine.removeSource(source)
            model.refresh()
            router.back()
        }
    }
}
