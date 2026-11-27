package live.akidev.tv

import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.Row
import androidx.compose.ui.Alignment
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Text
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.akitv.TvException
import uniffi.akitv.checkKey

@Composable
fun SettingsScreen(sourcesReady: Boolean, release: AppRelease?, onSaved: () -> Unit) {
    val context = LocalContext.current
    var typed by remember { mutableStateOf(Key.get(context)) }
    val hadKey = remember { typed.isNotEmpty() }
    var status by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()

    Column(Modifier.fillMaxSize().background(Ink.Canvas).padding(start = 48.dp, end = 48.dp, bottom = 48.dp, top = TopBarHeight + 24.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) { AkiIcon("settings-solid", size = 36.dp, tint = Ink.TextDim); Text("Settings", style = Type.Display) }
        Text("Your TorBox API key. It stays on this TV and is sent only to TorBox.", style = Type.Body)
        // The key is hidden as dots; the eye shows it, so a mistyped key can be seen and fixed.
        var hidden by remember { mutableStateOf(true) }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Field(typed, { typed = it }, "Paste or type the key", Modifier.width(720.dp), secret = hidden, icon = "lock")
            AkiButton(if (hidden) "Show the key" else "Hide the key", Look.Text, icon = if (hidden) "eye" else "eye-off") { hidden = !hidden }
        }
        AkiButton("Save and check", Look.Primary, icon = "check-solid", labelAlways = true) {
            status = "Checking"
            scope.launch {
                val result = withContext(Dispatchers.IO) {
                    try {
                        checkKey(typed.trim())
                        null
                    } catch (e: Exception) {
                        (e as? TvException.Failed)?.reason ?: e.message ?: "Something went wrong."
                    }
                }
                status = when {
                    result != null -> result
                    !Key.set(context, typed) -> "This TV cannot keep the key safely, so it was not saved."
                    else -> { onSaved(); "" }
                }
            }
        }
        if (hadKey) AkiButton("Remove the key from this TV", Look.Text, icon = "trash") { Key.clear(context); onSaved() }
        // The result of the check has a slot of its own, so nothing below the button moves when it appears.
        Box(Modifier.height(32.dp)) {
            if (status == "Checking") Spinner(size = 28.dp)
            else if (status.isNotEmpty()) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) { AkiIcon("warning", tint = Ink.Danger); Text(status, style = Type.Body.copy(color = Ink.Danger), maxLines = 1) }
        }
        // The notices (new sources, a new version of the app, a problem with an update) come last, each in a slot of its own, so nothing above them moves when one shows up.
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Box(Modifier.height(28.dp)) {
            if (sourcesReady) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box(Modifier.size(9.dp).background(Ink.Accent, CircleShape))
                Text("New sources are ready. They apply the next time the app opens.", style = Type.Body.copy(color = Ink.Text), maxLines = 1)
            }
        }
        // The app's own update: the button, then the progress in its place.
        var progress by remember { mutableStateOf<Float?>(null) }
        val problem by Updates.problem.collectAsState()
        var refused by remember { mutableStateOf<String?>(null) }
        Box(Modifier.height(52.dp)) {
            if (release != null) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                // A problem with the update takes the place of the sentence (the button stays, to try again).
                val trouble = refused ?: problem
                if (trouble != null) AkiIcon("warning", tint = Ink.Danger) else Box(Modifier.size(9.dp).background(Ink.Accent, CircleShape))
                Text(trouble ?: "Version ${release.version} is ready.", style = Type.Body.copy(color = if (trouble != null) Ink.Danger else Ink.Text), maxLines = 1, modifier = Modifier.widthIn(max = 640.dp))
                AkiButton(progress?.let { "Downloading ${(it * 100).toInt()}%" } ?: "Download and install", Look.Secondary, icon = "download", labelAlways = true) {
                    if (progress == null) scope.launch {
                        progress = 0f
                        refused = Updates.install(context, release) { progress = it }
                        progress = null
                    }
                }
            }
        }
        }
    }
}
