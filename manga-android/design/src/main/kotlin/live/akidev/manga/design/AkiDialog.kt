package live.akidev.manga.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog

/** The card every question and message is shown on. */
@Composable
private fun AkiDialogCard(title: String, message: String?, onDismiss: () -> Unit, content: @Composable () -> Unit = {}, buttons: @Composable () -> Unit) {
    Dialog(onDismissRequest = onDismiss) {
        AkiGlassWindow(Aki.colors.background) {
            Column(
                Modifier.widthIn(max = 360.dp).background(Aki.colors.background, RoundedCornerShape(Aki.cardRadius)).padding(22.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                AkiText(title, AkiTextStyle.Heading)
                if (message != null) AkiText(message, AkiTextStyle.Body, tone = AkiTone.Muted)
                content()
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.End)) { buttons() }
            }
        }
    }
}

/** A message with one button. */
@Composable
fun AkiAlert(title: String, message: String?, onDismiss: () -> Unit) {
    AkiDialogCard(title, message, onDismiss) { AkiButton("OK", variant = AkiButtonVariant.Primary, onClick = onDismiss) }
}

/** A question with a text field: an address, a name. */
@Composable
fun AkiPrompt(title: String, message: String, placeholder: String, confirm: String, onDismiss: () -> Unit, onConfirm: (String) -> Unit, initial: String = "") {
    var text by remember { mutableStateOf(initial) }
    AkiDialogCard(title, message, onDismiss, content = { AkiInput(placeholder, text, { text = it }, isUrl = true, onSubmit = { onConfirm(text) }) }) {
        AkiButton("Cancel", variant = AkiButtonVariant.Ghost, onClick = onDismiss)
        AkiButton(confirm, variant = AkiButtonVariant.Primary) { onConfirm(text) }
    }
}

/** A yes or no for something that cannot be undone. */
@Composable
fun AkiConfirm(title: String, message: String, confirm: String, onDismiss: () -> Unit, onConfirm: () -> Unit) {
    AkiDialogCard(title, message, onDismiss) {
        AkiButton("Cancel", variant = AkiButtonVariant.Ghost, onClick = onDismiss)
        AkiButton(confirm, variant = AkiButtonVariant.Destructive) { onConfirm(); onDismiss() }
    }
}
