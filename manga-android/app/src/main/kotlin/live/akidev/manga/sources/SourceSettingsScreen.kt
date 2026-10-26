package live.akidev.manga.sources

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
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
    LaunchedEffect(Unit) { settings = runCatching { model.engine.sourceSettings(source) }.getOrDefault(emptyList()) }

    AkiPage(underTabBar = false, spacing = 16.dp) {
        item { AkiBackHeader(model.name(source), back = router::back, subtitle = "Settings") }
        if (settings.isEmpty()) item { AkiText("This source has no settings.", tone = AkiTone.Muted) }
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
        item { AkiButton("Remove this source", Modifier.padding(top = 8.dp), icon = "trash", variant = AkiButtonVariant.Destructive, fullWidth = true) { removing = true } }
    }
    if (removing) {
        AkiConfirm("Remove ${model.name(source)}?", "Its saved series stay in your library but cannot be read until you install it again.", "Remove", { removing = false }) {
            model.engine.removeSource(source)
            model.refresh()
            router.back()
        }
    }
}
