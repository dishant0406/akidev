package live.akidev.manga.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The soft grey card that holds a row. */
@Composable
fun AkiCard(
    modifier: Modifier = Modifier,
    padding: Dp = 12.dp,
    dimmed: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier
            .fillMaxWidth()
            .alpha(if (dimmed) 0.62f else 1f)
            .background(Aki.colors.surface, RoundedCornerShape(Aki.cardRadius))
            .padding(padding),
        content = content,
    )
}
