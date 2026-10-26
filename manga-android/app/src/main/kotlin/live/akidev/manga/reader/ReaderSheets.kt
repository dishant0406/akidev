package live.akidev.manga.reader

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import live.akidev.manga.app.AutoScrollSpeed
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.OrientationLock
import live.akidev.manga.app.ReaderBackground
import live.akidev.manga.app.ReadingMode
import live.akidev.manga.app.shortDate
import live.akidev.manga.design.*
import live.akidev.manga.manga.chapterName
import uniffi.akimanga.ChapterEntry

/**
 * The series' chapters, from inside the reader: tap one to read it, without going back to the series page. The chapter being read is
 * yellow and the list opens on it; read chapters are dimmed, and the ones kept on the device say so.
 */
@Composable
fun ReaderChaptersSheet(reader: ReaderModel, onOpen: (ChapterEntry) -> Unit, onDismiss: () -> Unit) {
    val app = LocalApp.current
    // Read state as it is now: the reader's own list was made when it opened.
    val chapters = remember { runCatching { app.engine.cachedChapters(reader.ref.source, reader.ref.id) }.getOrDefault(emptyList()).ifEmpty { reader.chapters } }
    val kept = remember {
        runCatching { app.engine.downloads() }.getOrDefault(emptyList()).filter { it.source == reader.ref.source && it.series == reader.ref.id && it.state == "done" }.map { it.chapter }.toSet()
    }
    // Opened on the chapter being read, a little way down the list.
    val list = rememberLazyListState(maxOf(chapters.indexOfFirst { it.id == reader.current } - 3, 0))
    AkiSheet(onDismiss, full = true) {
        Column(Modifier.padding(start = 20.dp, end = 20.dp, top = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            AkiText("Chapters", AkiTextStyle.Title)
            AkiText("${reader.ref.title} · ${chapters.size}", AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
        }
        LazyColumn(Modifier.weight(1f).fillMaxWidth(), list, contentPadding = androidx.compose.foundation.layout.PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            items(chapters, key = { it.id }) { chapter -> ChapterRow(chapter, reading = chapter.id == reader.current, kept = chapter.id in kept) { onOpen(chapter) } }
        }
    }
}

@Composable
private fun ChapterRow(chapter: ChapterEntry, reading: Boolean, kept: Boolean, onClick: () -> Unit) {
    val colors = Aki.colors
    val tone = if (reading) AkiTone.OnPrimary else AkiTone.Text
    AkiTap(onClick) {
        Row(
            Modifier.fillMaxWidth().background(if (reading) colors.primary else colors.surface, RoundedCornerShape(22.dp)).padding(horizontal = 18.dp, vertical = 14.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                AkiText(chapterName(chapter.number, chapter.title), AkiTextStyle.BodyStrong, tone = if (reading) tone else if (chapter.finished) AkiTone.Muted else tone, lines = 1)
                shortDate(chapter.dateMs)?.let { AkiText(it, AkiTextStyle.Caption, Modifier.alpha(if (reading) 0.7f else 1f), tone = if (reading) tone else AkiTone.Muted, lines = 1) }
            }
            // What is special about a chapter, as icons on the right; a chapter not yet opened has none. Fixed sizes, so nothing moves.
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
                if (kept) AkiIcon("download", size = 18.dp, tone = if (reading) tone else AkiTone.Muted)
                when {
                    reading -> Row(Modifier.size(36.dp).background(colors.onPrimary, CircleShape), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) { AkiIcon("play-solid", size = 14.dp, tone = AkiTone.OnMedia) }
                    chapter.finished -> AkiIcon("check-solid", size = 20.dp, tone = AkiTone.Muted)
                    chapter.page != null -> AkiIcon("play", size = 18.dp)
                }
            }
        }
    }
}

/**
 * What can be changed while reading: how pages are laid out, the backdrop, which way the screen turns. Each choice is a row of pills,
 * one tap, nothing hidden in a menu.
 */
@Composable
fun ReaderSettingsSheet(reader: ReaderModel, onDismiss: () -> Unit) {
    val app = LocalApp.current
    val prefs = app.prefs
    val scope = rememberCoroutineScope()
    var coverSet by remember { mutableStateOf<Boolean?>(null) }
    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
            AkiText("Reader", AkiTextStyle.Title)
            Choice("Reading mode, this series") { AkiTabs(ReadingMode.entries.map { AkiChoice(it, it.shortTitle) }, reader.mode, { reader.changeMode(it) }) }
            Choice("Auto-scroll speed") { AkiTabs(AutoScrollSpeed.entries.map { AkiChoice(it, it.title) }, prefs.autoScrollSpeed, { prefs.autoScrollSpeed = it }) }
            Choice("Background") { AkiTabs(ReaderBackground.entries.map { AkiChoice(it, it.title) }, prefs.readerBackground, { prefs.readerBackground = it }) }
            Choice("Orientation") { AkiTabs(OrientationLock.entries.map { AkiChoice(it, it.title) }, prefs.orientation, { prefs.orientation = it }) }
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    AkiSwitch("Swap tap sides", prefs.invertTaps, { prefs.invertTaps = it }, subtitle = "In paged reading the left edge goes forward.")
                    AkiSwitch("Keep the screen on", prefs.keepAwake, { prefs.keepAwake = it })
                }
            }
            AkiButton(
                when (coverSet) { true -> "Cover changed"; false -> "Could not use this page"; null -> "Use what is on screen as the cover" }, icon = "image", fullWidth = true,
            ) { scope.launch { coverSet = reader.useCurrentPageAsCover(); app.refresh() } }
        }
    }
}

@Composable
private fun Choice(title: String, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        AkiText(title, AkiTextStyle.CaptionStrong, tone = AkiTone.Muted)
        content()
    }
}
