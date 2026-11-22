package live.akidev.manga.sources

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.describe
import live.akidev.manga.design.*

/**
 * "Sign in to a source": the fields its definition asks for, drawn here, and where they go. Only the site's answer is kept (in the
 * Keystore); the password is sent once and forgotten.
 */
@Composable
fun SignInSheet(source: String, onSignedIn: () -> Unit = {}, onDismiss: () -> Unit) {
    val model = LocalApp.current
    val scope = rememberCoroutineScope()
    val form = remember(source) { runCatching { model.engine.signInForm(source) }.getOrNull() }
    val values = remember { mutableStateMapOf<String, String>() }
    var working by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf<String?>(null) }
    val ready = form != null && form.fields.all { !values[it.id].isNullOrEmpty() }

    fun signIn() {
        if (working || !ready) return
        working = true
        scope.launch {
            try {
                model.engine.signIn(source, values.toMap())
                model.refresh()
                onSignedIn()
                onDismiss()
            } catch (failure: Exception) {
                problem = describe(failure).message
            }
            working = false
        }
    }

    AkiSheet(onDismiss) {
        Column(Modifier.imePadding().verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                AkiLogo(size = 36.dp)
                AkiText("Sign in to ${model.name(source)}", AkiTextStyle.Title, Modifier.semantics { heading() })
            }
            if (form != null) {
                form.fields.forEach { field ->
                    AkiField(field.name, values[field.id].orEmpty(), { values[field.id] = it; problem = null }, secure = field.secret, email = field.id.contains("email"), onSubmit = ::signIn)
                }
                problem?.let { AkiText(it, AkiTextStyle.Caption, Modifier.semantics { liveRegion = LiveRegionMode.Assertive }, tone = AkiTone.Destructive) }
                AkiButton(if (working) "Signing in…" else "Sign in", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large, fullWidth = true, enabled = ready && !working) { signIn() }
                AkiText("Your details go to ${form.host} only. Akidev keeps a session, not your password.", AkiTextStyle.Caption, tone = AkiTone.Muted)
            } else {
                AkiText("Could not load the sign-in form. Close this and try again.", AkiTextStyle.Caption, Modifier.semantics { liveRegion = LiveRegionMode.Assertive }, tone = AkiTone.Destructive)
            }
            AkiButton("Cancel", variant = AkiButtonVariant.Ghost, fullWidth = true) { onDismiss() }
        }
    }
}
