package live.akidev.tv

import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Text
import uniffi.akitv.Title

@Composable
fun SearchScreen(
    typed: String,
    onTyped: (String) -> Unit,
    asked: String,
    onAsk: () -> Unit,
    results: Load<List<Title>>,
    suggestions: List<String>,
    onPick: (String) -> Unit,
    onOpen: (Title) -> Unit,
) {
    val found = (results as? Load.Done)?.value.orEmpty()
    Column(Modifier.fillMaxSize().background(Ink.Canvas).padding(start = 48.dp, end = 48.dp, top = TopBarHeight + 24.dp), verticalArrangement = Arrangement.spacedBy(24.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Field(typed, onTyped, "Search films, series and anime", Modifier.weight(1f), action = ImeAction.Search, onAction = onAsk, icon = "search")
            AkiButton("Search", Look.Primary, icon = "search") { onAsk() }
        }
        // One slot of fixed height, so the results below do not jump: while the viewer types, suggestions (words only; they sit above the
        // TV keyboard, which covers the lower half of the screen), then the message (searching, failed, nothing found).
        Box(Modifier.height(48.dp)) {
            if (suggestions.isNotEmpty() && typed.trim() != asked) LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp), contentPadding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                items(suggestions, key = { it }) { name -> Suggestion(name) { onPick(name) } }
            } else when (val state = results) {
                Load.Loading -> if (asked.isNotBlank()) Spinner(size = 28.dp)
                is Load.Failed -> Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) { AkiIcon("warning", tint = Ink.Danger); Text(state.reason, style = Type.Body.copy(color = Ink.Danger), maxLines = 1) }
                is Load.Done -> if (state.value.isEmpty() && asked.isNotBlank()) {
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) { AkiIcon("search", tint = Ink.TextDim); Text("Nothing found for \"$asked\". Try another spelling.", style = Type.Body, maxLines = 1) }
                }
            }
        }
        LazyVerticalGrid(
            columns = GridCells.Adaptive(Poster.width),
            // The grid runs to the bottom of the screen and keeps its own space at the end, so rows are not cut off by a hard edge.
            contentPadding = PaddingValues(top = 8.dp, bottom = 48.dp),
            horizontalArrangement = Arrangement.spacedBy(20.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            // When the results arrive the focus goes to the first one: the TV keyboard closes itself on its search key, and the viewer can browse at once.
            // When the results arrive the focus goes to the first one: the TV keyboard closes itself on its search key, and the viewer can browse at once.
            items(found, key = { it.id }) { title -> PosterCard(title, autofocus = title.id == found.first().id) { onOpen(title) } }
        }
    }
}

/** One suggestion: only words. OK puts it in the field and searches for it. */
@Composable
private fun Suggestion(name: String, onClick: () -> Unit) {
    Focusable(onClick, RoundedCornerShape(24.dp), Ink.Raised, Modifier.height(40.dp)) {
        Text(name, style = Type.Body.copy(color = Ink.Text), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 360.dp).padding(horizontal = 20.dp).align(Alignment.Center))
    }
}
