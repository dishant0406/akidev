package live.akidev.manga.design

import androidx.compose.foundation.Image
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.foundation.layout.size
import androidx.compose.ui.unit.dp

/** An icon from the Solar set (see manga-app/scripts/fetch-icons.py): `name` is the outline, `name-solid` the filled one. */
@Composable
fun AkiIcon(name: String, modifier: Modifier = Modifier, size: Dp = 18.dp, tone: AkiTone = AkiTone.Text) {
    val resource = akiIconSet[name] ?: error("No icon called $name")
    Image(
        painter = painterResource(resource),
        contentDescription = null,
        modifier = modifier.size(size),
        colorFilter = ColorFilter.tint(tone.color),
    )
}
