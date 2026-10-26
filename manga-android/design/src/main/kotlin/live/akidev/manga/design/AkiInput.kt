package live.akidev.manga.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.unit.dp

/** A glass text field with an icon and a clear button. */
@Composable
fun AkiInput(
    placeholder: String,
    text: String,
    onChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    icon: String? = null,
    isUrl: Boolean = false,
    onSubmit: () -> Unit = {},
    onClear: () -> Unit = {},
) {
    val focus = LocalFocusManager.current
    val requester = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    Row(
        modifier.fillMaxWidth().height(52.dp).akiGlass(AkiShapes.pill(52.dp)).akiPress { requester.requestFocus() }.padding(horizontal = 18.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) AkiIcon(icon, size = 16.dp, tone = AkiTone.Muted)
        Box(Modifier.weight(1f)) {
            if (text.isEmpty()) AkiText(placeholder, tone = AkiTone.Muted, lines = 1)
            BasicTextField(
                value = text,
                onValueChange = onChange,
                modifier = Modifier.fillMaxWidth().focusRequester(requester).onFocusChanged { focused = it.isFocused },
                singleLine = true,
                textStyle = akiTextStyle(AkiTextStyle.Body, AkiTone.Text.color),
                cursorBrush = SolidColor(Aki.colors.text),
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false,
                    keyboardType = if (isUrl) KeyboardType.Uri else KeyboardType.Text, imeAction = if (isUrl) ImeAction.Go else ImeAction.Search,
                ),
                keyboardActions = KeyboardActions(onAny = { focus.clearFocus(); onSubmit() }),
            )
        }
        if (text.isNotEmpty()) {
            Box(Modifier.akiPress(label = "Clear") { onChange(""); onClear() }) { AkiIcon("close-solid", size = 16.dp, tone = AkiTone.Muted) }
        }
        // While typing, a way out that does not need a search: closes the keyboard.
        if (focused) AkiText("Done", AkiTextStyle.BodyStrong, Modifier.akiPress { focus.clearFocus() })
    }
}
