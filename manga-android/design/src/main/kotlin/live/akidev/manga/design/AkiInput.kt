package live.akidev.manga.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.autofill.ContentType
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.contentType
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics

/**
 * A glass text field with an icon and a clear button. [secure] hides what is typed (a password) and [email] asks for the email keyboard;
 * both are for a form, so the keyboard's action moves on instead of searching.
 */
@Composable
fun AkiInput(
    placeholder: String,
    text: String,
    onChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    icon: String? = null,
    isUrl: Boolean = false,
    secure: Boolean = false,
    email: Boolean = false,
    onSubmit: () -> Unit = {},
    onClear: () -> Unit = {},
    /** Takes the keyboard as soon as it appears, as the field in an iOS alert does. */
    autoFocus: Boolean = false,
    /** What a screen reader calls the field. The placeholder, when there is no better name. */
    label: String = placeholder,
    /** What is wrong with the field. A screen reader says it with the field. */
    errorMessage: String? = null,
) {
    val focus = LocalFocusManager.current
    val requester = remember { FocusRequester() }
    if (autoFocus) LaunchedEffect(Unit) { requester.requestFocus() }
    var focused by remember { mutableStateOf(false) }
    Row(
        modifier.fillMaxWidth().heightIn(min = 52.dp).akiGlass(AkiShapes.pill(52.dp)).akiPress { requester.requestFocus() }.padding(horizontal = 18.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) AkiIcon(icon, size = 16.dp, tone = AkiTone.Muted)
        Box(Modifier.weight(1f)) {
            if (text.isEmpty()) AkiText(placeholder, tone = AkiTone.Muted, lines = 1, modifier = Modifier.clearAndSetSemantics {})
            BasicTextField(
                value = text,
                onValueChange = onChange,
                modifier = Modifier.fillMaxWidth().focusRequester(requester).onFocusChanged { focused = it.isFocused }.semantics {
                    contentDescription = label
                    if (secure) contentType = ContentType.Password else if (email) contentType = ContentType.EmailAddress
                    if (errorMessage != null) error(errorMessage)
                },
                singleLine = true,
                visualTransformation = if (secure) PasswordVisualTransformation() else VisualTransformation.None,
                textStyle = akiTextStyle(AkiTextStyle.Body, AkiTone.Text.color),
                cursorBrush = SolidColor(Aki.colors.text),
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false,
                    keyboardType = if (secure) KeyboardType.Password else if (email) KeyboardType.Email else if (isUrl) KeyboardType.Uri else KeyboardType.Text,
                    imeAction = if (secure || isUrl) ImeAction.Go else if (email) ImeAction.Done else ImeAction.Search,
                ),
                keyboardActions = KeyboardActions(onAny = { focus.clearFocus(); onSubmit() }),
            )
        }
        if (text.isNotEmpty() && !secure) {
            Box(Modifier.akiPress(label = "Clear text") { onChange(""); onClear() }.akiTouchTarget()) { AkiIcon("close-solid", size = 16.dp, tone = AkiTone.Muted) }
        }
        // While typing, a way out that does not need a search: closes the keyboard.
        if (focused) AkiText("Done", AkiTextStyle.BodyStrong, Modifier.akiPress { focus.clearFocus() }.akiTouchTarget())
    }
}

/** A form field: its name above, the field below. The name stays when the field fills, which a placeholder does not. */
@Composable
fun AkiField(
    label: String,
    text: String,
    onChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String = "",
    secure: Boolean = false,
    email: Boolean = false,
    onSubmit: () -> Unit = {},
    /** What is wrong with the field, in words that say what to do ("Enter an address that starts with https://"). Shown under it. */
    errorMessage: String? = null,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        AkiText(label, AkiTextStyle.CaptionStrong, Modifier.padding(start = 18.dp), tone = AkiTone.Muted)
        AkiInput(placeholder, text, onChange, secure = secure, email = email, onSubmit = onSubmit, label = label, errorMessage = errorMessage)
        if (errorMessage != null) AkiText(errorMessage, AkiTextStyle.Caption, Modifier.padding(start = 18.dp), tone = AkiTone.Destructive)
    }
}
