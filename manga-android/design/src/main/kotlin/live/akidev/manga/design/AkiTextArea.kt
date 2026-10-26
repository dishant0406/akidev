package live.akidev.manga.design

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** A glass box for text that runs over many lines: a pasted definition (monospaced), or words of your own (`prose`). */
@Composable
fun AkiTextArea(text: String, onChange: (String) -> Unit, modifier: Modifier = Modifier, height: Dp = 160.dp, prose: Boolean = false) {
    val style = if (prose) akiTextStyle(AkiTextStyle.Body, AkiTone.Text.color) else akiTextStyle(AkiTextStyle.Caption, AkiTone.Text.color).copy(fontFamily = FontFamily.Monospace, fontSize = 13.sp)
    // A fixed height: a long paste scrolls inside the box instead of pushing the buttons below it off the screen.
    BasicTextField(
        value = text,
        onValueChange = onChange,
        modifier = modifier.fillMaxWidth().height(height).akiGlass(AkiShapes.rounded(18.dp)).padding(14.dp).verticalScroll(rememberScrollState()),
        textStyle = style,
        cursorBrush = SolidColor(Aki.colors.text),
        keyboardOptions = KeyboardOptions(capitalization = if (prose) KeyboardCapitalization.Sentences else KeyboardCapitalization.None, autoCorrectEnabled = prose),
    )
}
