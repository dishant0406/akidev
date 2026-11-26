package live.akidev.manga.books

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.design.*
import live.akidev.manga.design.Copy
import uniffi.akimanga.BookHighlightIn

/** Everything the person highlighted or wrote a note on, in every book, newest first. Tapping one opens the book there. */
@Composable
fun HighlightsScreen(open: (BookHighlightIn) -> Unit) {
    val app = LocalApp.current
    val router = LocalRouter.current
    var found by remember { mutableStateOf(emptyList<BookHighlightIn>()) }
    LaunchedEffect(Unit) { found = withContext(Dispatchers.Default) { runCatching { app.engine.recentHighlights(500u) }.getOrDefault(emptyList()) } }

    AkiPage(underTabBar = false, spacing = 14.dp) {
        item { AkiBackHeader(Copy.Books.highlights, back = router::back, subtitle = Copy.Books.highlightCount(found.size)) }
        if (found.isEmpty()) item { AkiEmptyState(Copy.Books.noHighlightsTitle, Copy.Books.noHighlightsMessageAndroid, Modifier.heightIn(min = 300.dp)) }
        items(found.size, key = { found[it].highlight.id }) { index ->
            val item = found[index]
            AkiTap({ router.back(); open(item) }) {
                AkiCard(padding = 14.dp) {
                    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        AkiText("“${item.highlight.quote}”", lines = 5)
                        item.highlight.note?.takeIf { it.isNotEmpty() }?.let { AkiText(it, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 4) }
                        AkiText(item.title, AkiTextStyle.CaptionStrong, tone = AkiTone.Muted, lines = 1)
                    }
                }
            }
        }
    }
}
