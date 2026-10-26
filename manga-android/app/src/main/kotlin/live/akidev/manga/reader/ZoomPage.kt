package live.akidev.manga.reader

import android.graphics.Bitmap
import androidx.compose.animation.core.animate
import androidx.compose.foundation.Image
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.layout.ContentScale
import kotlinx.coroutines.launch
import live.akidev.manga.design.AkiImageLoader
import live.akidev.manga.design.AkiPagePlaceholder

/**
 * One page of the paged reader: the picture fitted to the screen, pinch to zoom up to 4×, double tap to zoom in on a point and out
 * again, drag to move around when zoomed. A single tap reports where on the screen it landed (0 to 1 across), so the reader can turn the
 * page or show its controls. The dark placeholder stays until the picture is in, and comes back (tappable) when it failed.
 */
@Composable
fun ZoomPage(number: Int, url: String, headers: Map<String, String>, onTap: (Float) -> Unit, onZoomed: (Boolean) -> Unit) {
    var attempt by remember { mutableStateOf(0) }
    var failed by remember { mutableStateOf(false) }
    var picture by remember(url) { mutableStateOf<Bitmap?>(null) }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val widthPx = constraints.maxWidth
        LaunchedEffect(url, attempt) {
            failed = false
            val found = AkiImageLoader.image(url, headers, widthPx * 2)
            if (found == null) failed = true else picture = found
        }
        val shown = picture
        when {
            shown == null -> AkiPagePlaceholder(number + 1, failed) { attempt += 1 }
            // A page much taller than the screen (a webtoon strip shown in page mode) is fitted to the width and scrolled down.
            shown.width.toFloat() / shown.height < 0.5f -> TallPage(shown, onTap)
            else -> ZoomablePicture(shown, onTap, onZoomed)
        }
    }
}

@Composable
private fun TallPage(picture: Bitmap, onTap: (Float) -> Unit) {
    Box(Modifier.fillMaxSize().pointerInput(Unit) { detectTapGestures { onTap(it.x / size.width) } }.verticalScroll(rememberScrollState())) {
        Image(picture.asImageBitmap(), null, Modifier.fillMaxWidth(), contentScale = ContentScale.FillWidth)
    }
}

@Composable
private fun ZoomablePicture(picture: Bitmap, onTap: (Float) -> Unit, onZoomed: (Boolean) -> Unit) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(scale > 1.01f) { onZoomed(scale > 1.01f) }

    // The picture may be moved no further than its edge meeting the screen's.
    fun limited(moved: Offset, at: Float, width: Float, height: Float) = Offset(
        moved.x.coerceIn(-(width * (at - 1) / 2), width * (at - 1) / 2),
        moved.y.coerceIn(-(height * (at - 1) / 2), height * (at - 1) / 2),
    )

    Box(
        Modifier.fillMaxSize()
            .pointerInput(Unit) {
                detectTapGestures(
                    onTap = { onTap(it.x / size.width) },
                    onDoubleTap = { point ->
                        val from = scale
                        val fromOffset = offset
                        val to = if (from > 1.01f) 1f else 2.5f
                        val centre = Offset(size.width / 2f, size.height / 2f)
                        // Zooming in keeps the point that was tapped where it is under the finger.
                        val toOffset = if (to == 1f) Offset.Zero else limited((point - centre) * (1 - to), to, size.width.toFloat(), size.height.toFloat())
                        scope.launch {
                            animate(0f, 1f) { progress, _ ->
                                scale = from + (to - from) * progress
                                offset = fromOffset + (toOffset - fromOffset) * progress
                            }
                        }
                    },
                )
            }
            // Pinching always zooms; one finger moves the picture only while it is zoomed, so at rest the swipe turns the page.
            .pointerInput(Unit) {
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    do {
                        val event = awaitPointerEvent(PointerEventPass.Initial)
                        if (event.changes.size > 1 || scale > 1.01f) {
                            val newScale = (scale * event.calculateZoom()).coerceIn(1f, 4f)
                            val centre = Offset(size.width / 2f, size.height / 2f)
                            val moved = offset + event.calculatePan() + (event.calculateCentroid() - centre - offset) * (1 - newScale / scale)
                            scale = newScale
                            offset = limited(moved, newScale, size.width.toFloat(), size.height.toFloat())
                            event.changes.forEach { if (it.positionChanged()) it.consume() }
                        }
                    } while (event.changes.any { it.pressed })
                    if (scale < 1.05f) { scale = 1f; offset = Offset.Zero }
                }
            },
    ) {
        Image(
            picture.asImageBitmap(), null,
            Modifier.fillMaxSize().graphicsLayer { scaleX = scale; scaleY = scale; translationX = offset.x; translationY = offset.y },
            contentScale = ContentScale.Fit,
        )
    }
}
