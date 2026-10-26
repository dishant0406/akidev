package live.akidev.manga.design

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog

private val CardShape = AkiShapes.rounded(44.dp)

/** The dialog that is open: what it holds and what closes it. */
@Stable
class AkiShownDialog(val onDismiss: () -> Unit, val content: @Composable () -> Unit)

/** The one dialog that can be open, shown by [AkiDialogLayer]. */
@Stable
class AkiDialogs {
    var shown by mutableStateOf<AkiShownDialog?>(null)
}

/** Dialogs draw in the app's own window, so their glass can show the screen under them; a sheet's window has none and its dialogs use a window of their own. */
val LocalAkiDialogs = compositionLocalOf<AkiDialogs?> { null }

/** Draws the open dialog over the whole app, on a dimmed screen. Put it last in the root of the app, inside the glass scope. */
@Composable
fun AkiDialogLayer(dialogs: AkiDialogs) {
    // Kept after the dialog is dismissed so it can fade out instead of vanishing.
    var last by remember { mutableStateOf<AkiShownDialog?>(null) }
    dialogs.shown?.let { last = it }
    val dialog = last ?: return
    val visible = dialogs.shown != null
    BackHandler(enabled = visible) { dialogs.shown?.onDismiss?.invoke() }
    AnimatedVisibility(visible, enter = fadeIn(tween(150)), exit = fadeOut(tween(120))) {
        Box(
            Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.35f)).clickable(remember { MutableInteractionSource() }, null) { dialogs.shown?.onDismiss?.invoke() }
                .systemBarsPadding().imePadding(),
            contentAlignment = Alignment.Center,
        ) {
            AnimatedVisibility(visible, enter = scaleIn(tween(200), initialScale = 0.92f), exit = scaleOut(tween(120), targetScale = 0.96f)) {
                Box(Modifier.clickable(remember { MutableInteractionSource() }, null) {}) { dialog.content() }
            }
        }
    }
}

/** The glass card every question and message is shown on. */
@Composable
private fun DialogCard(title: String, message: String?, content: @Composable () -> Unit, buttons: @Composable RowScope.() -> Unit) {
    val colors = Aki.colors
    Column(
        Modifier.padding(horizontal = 24.dp).widthIn(max = 340.dp).fillMaxWidth()
            .akiShadow(CardShape, 30.dp, 12.dp)
            .akiGlass(CardShape, tint = colors.secondary.copy(alpha = if (colors.dark) 0.6f else 0.55f))
            .padding(28.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        CompositionLocalProvider(LocalAkiFlat provides true) {
            AkiText(title, AkiTextStyle.Heading)
            if (message != null) AkiText(message, AkiTextStyle.Body, tone = AkiTone.Muted)
            content()
            Row(Modifier.fillMaxWidth().padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) { buttons() }
        }
    }
}

/** Shows the card in the app's window when it has a layer for it, else in a window of its own. */
@Composable
private fun AkiDialogCard(title: String, message: String?, onDismiss: () -> Unit, content: @Composable () -> Unit = {}, buttons: @Composable RowScope.() -> Unit) {
    val dialogs = LocalAkiDialogs.current
    val card = rememberUpdatedState<@Composable () -> Unit> { DialogCard(title, message, content, buttons) }
    if (dialogs != null) {
        DisposableEffect(Unit) {
            val mine = AkiShownDialog(onDismiss) { card.value() }
            dialogs.shown = mine
            onDispose { if (dialogs.shown === mine) dialogs.shown = null }
        }
    } else {
        Dialog(onDismissRequest = onDismiss) { AkiGlassWindow(Aki.colors.background) { card.value() } }
    }
}

/** A message with one button. */
@Composable
fun AkiAlert(title: String, message: String?, onDismiss: () -> Unit) {
    AkiDialogCard(title, message, onDismiss) { AkiButton("OK", Modifier.weight(1f), onClick = onDismiss) }
}

/** A question with a text field: an address, a name. */
@Composable
fun AkiPrompt(title: String, message: String, placeholder: String, confirm: String, onDismiss: () -> Unit, onConfirm: (String) -> Unit, initial: String = "") {
    var text by remember { mutableStateOf(initial) }
    AkiDialogCard(title, message, onDismiss, content = { AkiInput(placeholder, text, { text = it }, isUrl = true, onSubmit = { onConfirm(text) }, autoFocus = true) }) {
        AkiButton("Cancel", Modifier.weight(1f), onClick = onDismiss)
        AkiButton(confirm, Modifier.weight(1f)) { onConfirm(text) }
    }
}

/** A yes or no for something that cannot be undone. */
@Composable
fun AkiConfirm(title: String, message: String, confirm: String, onDismiss: () -> Unit, onConfirm: () -> Unit) {
    AkiDialogCard(title, message, onDismiss) {
        AkiButton("Cancel", Modifier.weight(1f), onClick = onDismiss)
        AkiButton(confirm, Modifier.weight(1f), variant = AkiButtonVariant.Destructive) { onConfirm(); onDismiss() }
    }
}
