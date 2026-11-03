package live.akidev.manga.share

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import android.text.format.Formatter
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import kotlinx.coroutines.launch
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.PendingCard
import live.akidev.manga.app.Sharing
import live.akidev.manga.app.describe
import live.akidev.manga.design.*
import uniffi.akimanga.SharePeer

/**
 * Sending to a phone on the same Wi-Fi, and receiving from one, in one sheet. Opening it makes this phone visible and ready to receive;
 * closing it goes quiet. With nothing to send it only receives, and shows the code the other phone scans.
 */
@Composable
fun ShareSheet(sharing: Sharing, onDismiss: () -> Unit) {
    val app = LocalApp.current
    val model = remember { ShareModel(app, sharing) }
    val view = LocalView.current
    // The Wi-Fi must stay up while a file is on its way.
    DisposableEffect(Unit) {
        view.keepScreenOn = true
        onDispose { view.keepScreenOn = false }
    }
    LaunchedEffect(Unit) { model.open() }
    DisposableEffect(Unit) { onDispose { model.close() } }

    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(start = 24.dp, end = 24.dp, top = 12.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
            Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                AkiText(if (model.receiving) "Receive" else "Send", AkiTextStyle.Title)
                model.label?.let { AkiText(if (model.ready) it else "Getting “$it” ready…", AkiTextStyle.Body, tone = AkiTone.Muted, lines = 1) }
            }
            val coming = model.incoming?.takeIf { it.accepted }
            val state = model.state
            when {
                model.received != null -> Finished("check", model.received.orEmpty(), "Done", onDismiss)
                coming != null -> Working("Receiving from ${coming.from}", coming.fraction, null, model::stopReceiving)
                state is SendState.Waiting -> Working("Waiting for ${state.to.alias}…", null, null, model::stopSending)
                state is SendState.Sending -> Working("Sending to ${state.to.alias}", state.fraction, "Keep Akidev open", model::stopSending)
                state is SendState.Done -> Finished("check", "Sent to ${state.to.alias}", "Done", onDismiss)
                state is SendState.Failed -> Finished("warning", state.message, "Try again", model::again, "Close", onDismiss)
                state is SendState.Scanning -> Scan(model, state.denied)
                model.receiving -> Receive(model)
                else -> Choose(model)
            }
        }
    }
    val context = LocalContext.current
    model.incoming?.takeIf { !it.accepted }?.let { asked ->
        val what = asked.files.joinToString("\n") { "${it.name} (${Formatter.formatFileSize(context, it.size.toLong())})" }
        AkiQuestion("${asked.from} wants to send you", what, "Accept", "Decline", { model.answer(true) }, { model.answer(false) })
    }
    model.message?.let { AkiAlert(it.title, it.message) { model.message = null } }
}

/** The code the other phone scans, and who this phone is. */
@Composable
private fun Receive(model: ShareModel) {
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
        val code = model.code
        when {
            code != null -> {
                QrCode(code)
                AkiText("Ask them to scan this, or open Share on their phone.", tone = AkiTone.Muted, align = TextAlign.Center)
                AkiText("Visible as ${model.alias}", AkiTextStyle.Caption, tone = AkiTone.Muted)
            }
            model.started -> AkiText("Connect to Wi-Fi to receive.", tone = AkiTone.Muted, align = TextAlign.Center)
            else -> AkiSpinner()
        }
    }
}

/** Who to send to: the phones found, one tap each. */
@Composable
private fun Choose(model: ShareModel) {
    val context = LocalContext.current
    val asking = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted -> if (granted) model.startScanning() else model.cameraDenied() }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (model.peers.isEmpty()) {
            Column(Modifier.fillMaxWidth().padding(vertical = 20.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                AkiSpinner()
                AkiText("Looking nearby…", tone = AkiTone.Muted)
                if (model.lonely) AkiText("Can't see them? Both phones need the same Wi-Fi.", AkiTextStyle.Caption, tone = AkiTone.Muted, align = TextAlign.Center)
            }
        } else {
            for (peer in model.peers) PersonRow(peer, model.ready) { model.send(peer) }
        }
        AkiButton("Scan their code", icon = "qr", fullWidth = true) {
            val allowed = ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
            if (allowed) model.startScanning() else asking.launch(Manifest.permission.CAMERA)
        }
    }
}

@Composable
private fun PersonRow(peer: SharePeer, enabled: Boolean, onClick: () -> Unit) {
    AkiRow(peer.alias, Modifier.akiPress(label = peer.alias, enabled = enabled, onClick = onClick), icon = "share", showsChevron = true)
}

/** The camera, looking for the other phone's code; or, with the camera not allowed, the way to allow it. */
@Composable
private fun Scan(model: ShareModel, denied: Boolean) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp)) {
        if (denied) {
            AkiText("Allow the camera to scan a code.", tone = AkiTone.Muted, align = TextAlign.Center)
            AkiButton("Open settings", variant = AkiButtonVariant.Primary, fullWidth = true) {
                context.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null)))
            }
        } else {
            QrScanner({ text -> scope.launch { model.codeScanned(text) } }, Modifier.fillMaxWidth().height(210.dp).clip(RoundedCornerShape(28.dp)))
            AkiText(if (model.wrongCode) "That is not an Akidev code" else "Point at the code on their Receive sheet", AkiTextStyle.Caption, tone = if (model.wrongCode) AkiTone.Destructive else AkiTone.Muted, align = TextAlign.Center)
        }
        AkiButton("Cancel", variant = AkiButtonVariant.Ghost, onClick = model::again)
    }
}

/** A send, or a receive, under way: a spinner or a bar, and the way to stop it. */
@Composable
private fun Working(title: String, fraction: Double?, note: String?, cancel: () -> Unit) {
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp)) {
        AkiText(title, AkiTextStyle.BodyStrong, align = TextAlign.Center)
        if (fraction == null) AkiSpinner() else {
            AkiProgressBar(fraction)
            AkiText("${(fraction * 100).toInt()}%", AkiTextStyle.Caption, tone = AkiTone.Muted)
        }
        note?.let { AkiText(it, AkiTextStyle.Caption, tone = AkiTone.Muted) }
        AkiButton("Cancel", variant = AkiButtonVariant.Ghost, onClick = cancel)
    }
}

/** How it ended: a mark, one sentence, one main button and, if it needs one, a quiet second. */
@Composable
private fun Finished(icon: String, text: String, main: String, onMain: () -> Unit, other: String? = null, onOther: () -> Unit = {}) {
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Box(Modifier.size(56.dp).background(Aki.colors.primary, CircleShape), contentAlignment = Alignment.Center) { AkiIcon(icon, size = 24.dp, tone = AkiTone.OnPrimary) }
        AkiText(text, AkiTextStyle.BodyStrong, align = TextAlign.Center)
        AkiButton(main, variant = AkiButtonVariant.Primary, fullWidth = true, onClick = onMain)
        other?.let { AkiButton(it, variant = AkiButtonVariant.Ghost, onClick = onOther) }
    }
}

/**
 * "Add this series?" for a series another phone shared from a source. When the source is new here it is code that decides which site the
 * app contacts, so the person sees its name and where it comes from before anything is kept.
 */
@Composable
fun CardSheet(pending: PendingCard, onDismiss: () -> Unit) {
    val app = LocalApp.current
    val scope = rememberCoroutineScope()
    val card = pending.card
    var adding by remember { mutableStateOf(false) }
    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            AkiText("Add “${card.title}”?", AkiTextStyle.Title)
            AkiText("Another phone shared this. It is read from the source ${card.sourceName}, so nothing is copied but the link to it.", AkiTextStyle.Body, tone = AkiTone.Muted)
            AkiCard(padding = 4.dp) {
                AkiRow(card.sourceName, subtitle = (card.languages.joinToString(", ").uppercase() + if (card.nsfw) " · 18+" else "").trimStart(' ', '·'))
            }
            AkiText(card.repo?.let { "This phone does not have the source yet. It comes from $it." } ?: "This phone does not have the source yet. It comes with the series.", AkiTextStyle.Caption, tone = AkiTone.Muted)
            AkiText("A source decides which site the app contacts. Only add one from someone you trust.", AkiTextStyle.Caption, tone = AkiTone.Muted)
            AkiButton("Add the source and the series", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large, fullWidth = true) {
                if (adding) return@AkiButton
                adding = true
                scope.launch {
                    try {
                        app.engine.shareAddCard(pending.path)
                        app.refresh()
                        app.notice = live.akidev.manga.app.Problem("Added", "“${card.title}” is in your library.")
                    } catch (failure: Exception) {
                        app.notice = describe(failure)
                    }
                    onDismiss()
                }
            }
            AkiButton("Not now", fullWidth = true, onClick = onDismiss)
        }
    }
}
