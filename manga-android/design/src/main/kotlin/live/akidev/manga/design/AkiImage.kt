package live.akidev.manga.design

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged

/**
 * A picture from an address, with the headers its source needs, decoded at the size it is shown. Until it arrives (and for ever when
 * there is no address) it is a quiet block of `placeholder` times its width in height; `free` fills whatever space it is given, and
 * `fill` keeps the block's shape and crops the picture to it. `onAspect` is told width over height once known (the reader needs it to
 * restore a position).
 */
@Composable
fun AkiImage(
    url: String?,
    modifier: Modifier = Modifier,
    headers: Map<String, String> = emptyMap(),
    placeholder: Float = 1.5f,
    fill: Boolean = false,
    free: Boolean = false,
    onAspect: ((Float) -> Unit)? = null,
    /** A reader page: a placeholder with its number and a way to retry, instead of the plain block. */
    pageNumber: Int? = null,
) {
    var widthPx by remember { mutableStateOf(0) }
    var failed by remember { mutableStateOf(false) }
    var attempt by remember { mutableStateOf(0) }
    var loaded by remember(url) { mutableStateOf<android.graphics.Bitmap?>(null) }
    // What is already in memory shows in the first frame; the effect below only fetches what is not.
    val bitmap = loaded ?: url?.let { AkiImageLoader.cached(it, widthPx) }

    LaunchedEffect(url, attempt, widthPx) {
        if (url == null || widthPx == 0) return@LaunchedEffect
        failed = false
        val found = AkiImageLoader.cached(url, widthPx) ?: AkiImageLoader.image(url, headers, widthPx)
        if (found == null) failed = true else {
            loaded = found
            onAspect?.invoke(found.width.toFloat() / found.height)
        }
    }

    val shape = when {
        free -> Modifier
        fill -> Modifier.aspectRatio(1f / placeholder)
        bitmap != null -> Modifier.aspectRatio(bitmap.width.toFloat() / bitmap.height)
        else -> Modifier.aspectRatio(1f / placeholder)
    }
    Box(modifier.then(shape).clipToBounds().onSizeChanged { widthPx = it.width }.background(Aki.colors.secondary), contentAlignment = Alignment.Center) {
        if (pageNumber != null && bitmap == null) AkiPagePlaceholder(pageNumber + 1, failed) { attempt += 1 }
        Crossfade(bitmap, label = "picture") { shown ->
            if (shown != null) {
                Image(shown.asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = if (fill || free) ContentScale.Crop else ContentScale.FillWidth)
            } else if (failed && pageNumber == null) {
                AkiIcon("image", tone = AkiTone.Muted)
            }
        }
    }
}
