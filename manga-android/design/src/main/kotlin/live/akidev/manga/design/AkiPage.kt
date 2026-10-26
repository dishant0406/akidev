package live.akidev.manga.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * A screen's scrolling content: the side margins and top room every screen has, room at the bottom for the floating tab bar, and on a
 * wide screen (a tablet) a column in the middle instead of lines running the whole width. Lists of cards take a narrower column than
 * a grid of covers. What scrolls is what the glass bars above and below show.
 */
@Composable
fun AkiPage(
    modifier: Modifier = Modifier,
    narrow: Boolean = true,
    underTabBar: Boolean = true,
    spacing: Dp = 18.dp,
    state: LazyListState = rememberLazyListState(),
    content: LazyListScope.() -> Unit,
) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        LazyColumn(
            Modifier.fillMaxHeight().widthIn(max = if (narrow) 640.dp else 720.dp).akiGlassSource().windowInsetsPadding(androidx.compose.foundation.layout.WindowInsets.statusBars),
            state = state,
            contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 8.dp, bottom = if (underTabBar) 112.dp else 24.dp),
            verticalArrangement = Arrangement.spacedBy(spacing),
            content = content,
        )
    }
}
