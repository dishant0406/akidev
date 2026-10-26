package live.akidev.manga.app

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.UUID

/** Asks for one file from the device and hands over a copy the app may read (deleted when `onPick` returns). Returns what opens the picker. */
@Composable
fun rememberFilePicker(types: Array<String> = arrayOf("*/*"), onPick: suspend (File) -> Unit): () -> Unit {
    val pick = rememberFilesPicker(types) { files -> files.firstOrNull()?.let { onPick(it) } }
    return pick
}

/** Asks for one or more files and hands over copies, all together; each copy is in a folder of its own, so two files with one name do not meet. */
@Composable
fun rememberFilesPicker(types: Array<String> = arrayOf("*/*"), onPick: suspend (List<File>) -> Unit): () -> Unit {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val latest = rememberUpdatedState(onPick)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        scope.launch {
            val copies = withContext(Dispatchers.IO) { uris.mapNotNull { copyToCache(context, it) } }
            latest.value(copies)
            withContext(Dispatchers.IO) { copies.forEach { it.parentFile?.deleteRecursively() } }
        }
    }
    return { launcher.launch(types) }
}

private fun copyToCache(context: Context, uri: Uri): File? = runCatching {
    val name = context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { if (it.moveToFirst()) it.getString(0) else null } ?: "file"
    val folder = File(context.cacheDir, "picked-" + UUID.randomUUID()).apply { mkdirs() }
    File(folder, name).also { copy -> context.contentResolver.openInputStream(uri)!!.use { input -> copy.outputStream().use { input.copyTo(it) } } }
}.getOrNull()

/**
 * Asks for a picture from the photo library (the system shows its own picker, so no permission is needed) and hands it over as a JPEG no
 * wider than `width` pixels, small enough to keep as a cover. Returns what opens the picker.
 */
@Composable
fun rememberPhotoPicker(width: Int = 900, onPick: (ByteArray) -> Unit): () -> Unit {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val latest = rememberUpdatedState(onPick)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            val jpeg = withContext(Dispatchers.IO) { runCatching { coverJpeg(decodeNoWiderThan(context, uri, width), width) }.getOrNull() }
            if (jpeg != null) latest.value(jpeg)
        }
    }
    return { launcher.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) }
}

private fun decodeNoWiderThan(context: Context, uri: Uri, width: Int): Bitmap =
    ImageDecoder.decodeBitmap(ImageDecoder.createSource(context.contentResolver, uri)) { decoder, info, _ ->
        decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
        if (info.size.width > width) decoder.setTargetSize(width, info.size.height * width / info.size.width)
    }

/**
 * A picture as a JPEG for a cover: at most `width` pixels wide (never enlarged) and no taller than a book cover is. A tall picture (a
 * webtoon strip) is cut to that height around `centre`, how far down it the wanted part's middle is (0 to 1; 0 keeps the top).
 */
fun coverJpeg(bitmap: Bitmap, width: Int, centre: Float = 0f): ByteArray {
    // A picture the loader decoded for drawing cannot be changed; a copy can.
    val picture = if (bitmap.config == Bitmap.Config.HARDWARE) bitmap.copy(Bitmap.Config.ARGB_8888, false) else bitmap
    val scale = minOf(1f, width.toFloat() / picture.width)
    val drawnWidth = (picture.width * scale).toInt()
    val drawnHeight = (picture.height * scale).toInt()
    val height = minOf(drawnHeight, (drawnWidth * 1.6f).toInt())
    val top = (centre * drawnHeight - height / 2f).toInt().coerceIn(0, drawnHeight - height)
    val scaled = Bitmap.createScaledBitmap(picture, drawnWidth, drawnHeight, true)
    val cut = Bitmap.createBitmap(scaled, 0, top, drawnWidth, height)
    return ByteArrayOutputStream().also { cut.compress(Bitmap.CompressFormat.JPEG, 85, it) }.toByteArray()
}

/** Hands a file to the share sheet (save to Files, Drive, a message). The file is kept in the cache folder until the system clears it. */
fun shareFile(context: Context, name: String, text: String, mime: String) {
    val folder = File(context.cacheDir, "share").apply { mkdirs() }
    val file = File(folder, name).also { it.writeText(text) }
    val uri = androidx.core.content.FileProvider.getUriForFile(context, "${context.packageName}.files", file)
    val send = android.content.Intent(android.content.Intent.ACTION_SEND).setType(mime).putExtra(android.content.Intent.EXTRA_STREAM, uri).addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
    context.startActivity(android.content.Intent.createChooser(send, name))
}
