package live.akidev.manga.design

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The Akidev mark, the same picture as the app icon, at any size. */
@Composable
fun AkiLogo(modifier: Modifier = Modifier, size: Dp = 112.dp) {
    Image(painterResource(R.drawable.aki_logo), null, modifier.size(size).clip(RoundedCornerShape(size * 0.2237f)))
}
