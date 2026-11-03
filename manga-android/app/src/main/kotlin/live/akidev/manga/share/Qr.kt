package live.akidev.manga.share

import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.EncodeHintType
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.ReaderException
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import live.akidev.manga.design.Aki
import kotlin.math.floor
import java.util.concurrent.Executors

/** The squares of a QR code for [text], row by row; true is a dark one. */
fun qrCells(text: String): List<BooleanArray> {
    val hints = mapOf(EncodeHintType.MARGIN to 0, EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M)
    val matrix = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, 0, 0, hints)
    return List(matrix.height) { y -> BooleanArray(matrix.width) { x -> matrix[x, y] } }
}

/** The text of a QR code in a picture of brightness values (one byte a pixel, `rowWidth` bytes a row), or null when there is none. */
fun qrRead(brightness: ByteArray, rowWidth: Int, width: Int, height: Int): String? = try {
    QRCodeReader().decode(BinaryBitmap(HybridBinarizer(PlanarYUVLuminanceSource(brightness, rowWidth, height, 0, 0, width, height, false)))).text
} catch (_: ReaderException) {
    null
}

/** A code on white, the way every camera reads it, whatever the theme. */
@Composable
fun QrCode(text: String) {
    val cells = remember(text) { qrCells(text) }
    val ink = Aki.colors.onPrimary
    Box(Modifier.size(240.dp).background(Color.White, RoundedCornerShape(28.dp)).padding(18.dp), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(204.dp)) {
            // Whole pixels a square, so the edges are sharp; what is left over goes round the code.
            val cell = floor(size.width / cells.size)
            val start = floor((size.width - cell * cells.size) / 2)
            cells.forEachIndexed { y, row ->
                row.forEachIndexed { x, dark -> if (dark) drawRect(ink, Offset(start + x * cell, start + y * cell), Size(cell, cell)) }
            }
        }
    }
}

/** The camera, looking for a code. Each time it reads one, [onCode] is told (from another thread), over and over while it stays in view. */
@Composable
fun QrScanner(onCode: (String) -> Unit, modifier: Modifier = Modifier) {
    val owner = LocalLifecycleOwner.current
    val told by rememberUpdatedState(onCode)
    val executor = remember { Executors.newSingleThreadExecutor() }
    val camera = remember { arrayOfNulls<ProcessCameraProvider>(1) }
    val gone = remember { booleanArrayOf(false) }
    DisposableEffect(Unit) {
        onDispose {
            gone[0] = true
            camera[0]?.unbindAll()
            executor.shutdown()
        }
    }
    AndroidView(
        modifier = modifier,
        factory = { context ->
            val view = PreviewView(context).apply { scaleType = PreviewView.ScaleType.FILL_CENTER }
            val future = ProcessCameraProvider.getInstance(context)
            future.addListener({
                if (gone[0]) return@addListener
                val provider = future.get().also { camera[0] = it }
                val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
                val reading = ImageAnalysis.Builder().setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
                reading.setAnalyzer(executor) { image -> image.use { frame -> frame.qrText()?.let(told) } }
                provider.unbindAll()
                provider.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, reading)
            }, ContextCompat.getMainExecutor(context))
            view
        },
    )
}

private fun ImageProxy.qrText(): String? {
    val plane = planes[0]
    val bytes = ByteArray(plane.buffer.remaining()).also { plane.buffer.get(it) }
    return qrRead(bytes, plane.rowStride, width, height)
}
