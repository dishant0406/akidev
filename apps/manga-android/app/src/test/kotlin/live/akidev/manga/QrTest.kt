package live.akidev.manga

import live.akidev.manga.share.qrCells
import live.akidev.manga.share.qrRead
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class QrTest {
    /** The code as a camera sees it: dark squares on white, `scale` pixels a square, a quiet border all round. */
    private fun picture(text: String, scale: Int = 4): Triple<ByteArray, Int, Int> {
        val cells = qrCells(text)
        val side = (cells.size + 8) * scale
        val pixels = ByteArray(side * side) { 255.toByte() }
        cells.forEachIndexed { y, row ->
            row.forEachIndexed { x, dark ->
                if (dark) for (dy in 0 until scale) for (dx in 0 until scale) pixels[((y + 4) * scale + dy) * side + (x + 4) * scale + dx] = 0
            }
        }
        return Triple(pixels, side, side)
    }

    @Test
    fun aCodeReadsBackAsMade() {
        val text = "akidev://share?h=192.168.1.23&p=53317"
        val (pixels, width, height) = picture(text)
        assertEquals(text, qrRead(pixels, width, width, height))
    }

    @Test
    fun aBlankPictureHoldsNoCode() {
        assertNull(qrRead(ByteArray(64 * 64) { 255.toByte() }, 64, 64, 64))
    }
}
