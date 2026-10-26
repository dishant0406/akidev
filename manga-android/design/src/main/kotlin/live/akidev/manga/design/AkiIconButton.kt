package live.akidev.manga.design

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** A round glass button with one symbol: back, save, refresh. */
@Composable
fun AkiIconButton(
    symbol: String,
    label: String,
    modifier: Modifier = Modifier,
    tint: Color? = null,
    /** Over a picture the symbol is white. */
    onMedia: Boolean = false,
    /** How dark the glass is over a picture: the reader turns it up, since a white page would otherwise make the glass pale. */
    scrim: Color = Aki.colors.scrim,
    size: Dp = 44.dp,
    onClick: () -> Unit,
) {
    Box(
        modifier.size(size).akiPress(label = label, onClick = onClick).akiGlass(AkiShapes.pill(size), tint = tint ?: if (onMedia) scrim else null),
        contentAlignment = Alignment.Center,
    ) {
        AkiIconFace(symbol, size, tinted = tint != null, onMedia = onMedia)
    }
}

/** The symbol alone, for a menu that supplies its own button. */
@Composable
fun AkiIconFace(symbol: String, size: Dp = 44.dp, tinted: Boolean = false, onMedia: Boolean = false) {
    AkiIcon(symbol, size = size * 0.5f, tone = if (tinted) AkiTone.OnPrimary else if (onMedia) AkiTone.OnMedia else AkiTone.Text)
}
