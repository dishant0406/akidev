package live.akidev.manga.manga

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.offset
import kotlinx.coroutines.launch
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.ReaderRoute
import live.akidev.manga.app.SeriesRef
import live.akidev.manga.app.SeriesRoute
import live.akidev.manga.app.rememberPhotoPicker
import live.akidev.manga.app.shortDate
import live.akidev.manga.design.*
import uniffi.akimanga.ChapterEntry

private enum class SeriesTab { Chapters, Bookmarks, Details }

/** A series: a full-width picture with its title, and a sheet that rises over it with the chapters. */
@Composable
fun SeriesScreen(ref: SeriesRef) {
    val app = LocalApp.current
    val router = LocalRouter.current
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current
    val series = remember(ref) { SeriesModel(app, ref) }
    var tab by remember { mutableStateOf(SeriesTab.Chapters) }
    var forgetting by remember { mutableStateOf(false) }
    var moving by remember { mutableStateOf(false) }
    var choosingCategories by remember { mutableStateOf(false) }
    val chooseCover = rememberPhotoPicker { series.setCover(it) }
    val open = { target: ReaderTarget -> router.push(ReaderRoute(ref, series.chapters, target)) }

    LaunchedEffect(Unit) { series.load() }
    // Back from the reader the page finds where the person got to.
    LaunchedEffect(router.top) { if ((router.top as? SeriesRoute)?.ref === ref) series.refreshProgress() }

    val list = rememberLazyListState()
    // The picture is under the status bar until the page has scrolled past it.
    val onPicture by remember { derivedStateOf { list.firstVisibleItemIndex == 0 && list.firstVisibleItemScrollOffset < 200 } }
    AkiLightStatusBar(on = (router.top as? SeriesRoute)?.ref === ref && onPicture)

    Box(Modifier.fillMaxSize()) {
        AkiRefreshable({ series.load(force = true) }, Modifier.fillMaxSize()) {
            LazyColumn(Modifier.fillMaxSize().akiGlassSource(), list, contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 0.dp)) {
                item { Hero(series) }
                sheet(series, tab, { tab = it }, open, scope)
            }
        }
        // Pinned over the scrolling page, so back is always there and always takes the tap.
        Controls(series, onSaved = {
            scope.launch {
                if (series.toggleSaved()) {
                    haptic.performHapticFeedback(HapticFeedbackType.Confirm)
                    if (series.putOnSaveCategory()) choosingCategories = true
                }
            }
        }, onCategories = { choosingCategories = true }, onCover = chooseCover, onMove = { moving = true }, onForget = { forgetting = true }, scope = scope)
        AnimatedVisibility(
            series.resumable != null, Modifier.align(Alignment.BottomCenter), enter = slideInVertically { it } + fadeIn(), exit = slideOutVertically { it } + fadeOut(),
        ) {
            series.resumable?.let { next ->
                AkiButton(next.label, Modifier.navigationBarsPadding().padding(horizontal = 24.dp, vertical = 12.dp), icon = "play-solid", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large) { open(next.target) }
            }
        }
    }

    series.problem?.let { AkiAlert("Download failed", it) { series.problem = null } }
    if (moving) MigrateSheet(ref, onMoved = { router.back() }) { moving = false }
    if (choosingCategories) CategoryPicker(listOf(ref.source to ref.id), onDismiss = { choosingCategories = false })
    if (forgetting) {
        AkiConfirm("Forget ${ref.title}?", "It leaves your library and its reading progress and downloads are deleted.", "Forget", { forgetting = false }) {
            series.forget()
            router.back()
        }
    }
}

private val HeroHeight = 470.dp

@Composable
private fun Hero(series: SeriesModel) {
    val app = LocalApp.current
    val colors = Aki.colors
    Box(Modifier.fillMaxWidth().height(HeroHeight)) {
        // The picture is a glass source of its own, so the badges on it show the picture (see AkiBillboard).
        Box(Modifier.fillMaxSize().akiGlassSource(layer = -0.5f)) {
            AkiImage(series.cover, Modifier.fillMaxSize(), placeholder = 1.4f, free = true)
            Box(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(colors.scrimTop, Color.Transparent, colors.scrimBottom))))
        }
        Column(Modifier.align(Alignment.BottomStart).widthIn(max = 720.dp).padding(start = 20.dp, end = 20.dp, bottom = 46.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (series.chapters.isNotEmpty()) AkiBadge("${series.chapters.size} chapters", icon = "list", style = AkiBadgeStyle.Glass)
                series.statusText?.let { AkiBadge(it, icon = "clock", style = AkiBadgeStyle.Glass) }
                AkiBadge(app.name(series.ref.source), icon = "sources", style = AkiBadgeStyle.Glass)
            }
            AkiText(series.detail?.title ?: series.ref.title, AkiTextStyle.Display, tone = AkiTone.OnMedia, lines = 3)
            val authors = series.detail?.authors.orEmpty()
            if (authors.isNotEmpty()) AkiText(authors.joinToString(", "), AkiTextStyle.Caption, tone = AkiTone.OnMedia, lines = 1)
        }
    }
}

/** Back, save and more: glass circles over the picture. */
@Composable
private fun Controls(
    series: SeriesModel,
    onSaved: () -> Unit,
    onCategories: () -> Unit,
    onCover: () -> Unit,
    onMove: () -> Unit,
    onForget: () -> Unit,
    scope: kotlinx.coroutines.CoroutineScope,
) {
    val router = LocalRouter.current
    val chapters = series.chapters
    Row(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        AkiIconButton("back", "Back", onMedia = true) { router.back() }
        Box(Modifier.weight(1f))
        AkiIconButton(
            if (series.saved) "bookmark-solid" else "bookmark", if (series.saved) "Remove from library" else "Save to library",
            tint = if (series.saved) Aki.colors.primary else null, onMedia = true, onClick = onSaved,
        )
        AkiMenu(
            "more", "More", onMedia = true, items = listOf(
                AkiMenuItem("Mark all as read", "check") { series.markAll(read = true) },
                AkiMenuItem("Mark all as unread", "check") { series.markAll(read = false) },
                AkiMenuItem("Download unread", "download") { scope.launch { series.download(chapters.filter { !it.finished && it.id !in series.kept }) } },
                AkiMenuItem("Refresh", "refresh") { scope.launch { series.load(force = true) } },
                AkiMenuItem("Categories", "folder", onClick = onCategories),
                AkiMenuItem("Choose a cover", "image", onClick = onCover),
                AkiMenuItem("Reset cover", "refresh") { series.clearCover() },
                AkiMenuItem("Move to another source", "swap", onClick = onMove),
                AkiMenuItem("Forget this series", "trash", destructive = true, onClick = onForget),
            ),
        )
    }
}

// ---- the sheet over the picture -----------------------------------------------------------------------------------------------

/** The sheet's rows are items of the page, each on the page colour; the first has the rounded top that rises over the picture. */
private fun LazyListScope.sheet(series: SeriesModel, tab: SeriesTab, onTab: (SeriesTab) -> Unit, open: (ReaderTarget) -> Unit, scope: kotlinx.coroutines.CoroutineScope) {
    item {
        Box(Modifier.fillMaxWidth().offset(y = (-30).dp), contentAlignment = Alignment.TopCenter) {
            // The page colour is a glass source between the picture and the page, so the tabs show it and not the picture behind the sheet.
            Box(Modifier.matchParentSize().akiGlassSource(layer = -0.25f).background(Aki.colors.background, RoundedCornerShape(topStart = Aki.sheetRadius, topEnd = Aki.sheetRadius)))
            AkiTabs(
                listOf(AkiChoice(SeriesTab.Chapters, "Chapters"), AkiChoice(SeriesTab.Bookmarks, "Bookmarks"), AkiChoice(SeriesTab.Details, "Details")), tab, onTab,
                Modifier.widthIn(max = 720.dp).padding(start = 20.dp, end = 20.dp, top = 22.dp, bottom = 18.dp),
            )
        }
    }
    val error = series.error
    when {
        tab == SeriesTab.Bookmarks -> bookmarkRows(series, open)
        error != null && series.chapters.isEmpty() -> sheetItem {
            AkiEmptyState(error.title, error.message, Modifier.heightIn(min = 260.dp), action = AkiAction("Try again") { scope.launch { series.load(force = true) } })
        }
        tab == SeriesTab.Chapters && series.chapters.isEmpty() && !series.loading -> sheetItem {
            val name = LocalApp.current.name(series.ref.source)
            AkiEmptyState(
                "Nothing to read here",
                "$name lists this series but has no chapters it can show in your language. Licensed series are often only linked to the publisher's own site. Try another source, or another language in the source's settings.",
                Modifier.heightIn(min = 280.dp),
            )
        }
        tab == SeriesTab.Chapters -> chapterRows(series, open, scope)
        else -> sheetItem { Details(series) }
    }
    item { Box(Modifier.fillMaxWidth().height(120.dp).offset(y = (-30).dp).background(Aki.colors.background)) }
}

/** One row of the sheet: on the page colour, at the page's width, pulled up under the rounded top. */
@Composable
private fun SheetRow(content: @Composable () -> Unit) {
    Box(Modifier.fillMaxWidth().offset(y = (-30).dp).background(Aki.colors.background), contentAlignment = Alignment.TopCenter) {
        Box(Modifier.widthIn(max = 720.dp).padding(horizontal = 20.dp, vertical = 6.dp)) { content() }
    }
}

private fun LazyListScope.sheetItem(content: @Composable () -> Unit) {
    item { SheetRow(content) }
}

private fun LazyListScope.chapterRows(series: SeriesModel, open: (ReaderTarget) -> Unit, scope: kotlinx.coroutines.CoroutineScope) {
    if (series.chapters.isEmpty() && series.loading) sheetItem { Box(Modifier.fillMaxWidth().padding(40.dp), contentAlignment = Alignment.Center) { AkiSpinner() } }
    items(series.chapters.size, key = { series.chapters[it].id }) { index ->
        val chapter = series.chapters[index]
        val download = if (chapter.id in series.kept) AkiMenuItem("Delete download", "trash", destructive = true) { series.removeDownload(chapter) }
        else AkiMenuItem("Download", "download") { scope.launch { series.download(listOf(chapter)) } }
        SheetRow {
            AkiTapMenu(listOf(download), onClick = { open(ReaderTarget(chapter.id, (chapter.page ?: 0u).toInt())) }) {
                AkiListCard(
                    series.cover, series.label(chapter), chapterName(chapter.number, chapter.title),
                    badges = listOfNotNull(shortDate(chapter.dateMs), chapter.scanlator), dimmed = chapter.finished,
                ) { ReadMark(chapter, series) }
            }
        }
    }
}

@Composable
private fun ReadMark(chapter: ChapterEntry, series: SeriesModel) {
    Box(Modifier.size(44.dp).akiPress(label = if (chapter.finished) "Mark as unread" else "Mark as read") { series.toggleRead(chapter) }, contentAlignment = Alignment.Center) {
        AkiIcon(if (chapter.finished) "check-solid" else "check", size = 22.dp, tone = if (chapter.finished) AkiTone.Text else AkiTone.Muted)
    }
}

private fun LazyListScope.bookmarkRows(series: SeriesModel, open: (ReaderTarget) -> Unit) {
    if (series.bookmarks.isEmpty()) sheetItem { AkiEmptyState("No bookmarks yet", "Tap the bookmark button while you read to mark a page, and it will be here.", Modifier.heightIn(min = 260.dp)) }
    items(series.bookmarks.size) { index ->
        val mark = series.bookmarks[index]
        SheetRow {
            AkiTap({ open(ReaderTarget(mark.chapter, mark.page.toInt(), mark.offset)) }) {
                AkiListCard(series.cover, "Page ${mark.page + 1u}", chapterName(mark.chapterNumber, mark.chapterTitle), badges = listOfNotNull(shortDate(mark.createdMs))) {
                    Box(Modifier.size(44.dp).akiPress(label = "Remove bookmark") { series.removeBookmark(mark) }, contentAlignment = Alignment.Center) { AkiIcon("trash", size = 22.dp, tone = AkiTone.Muted) }
                }
            }
        }
    }
}

@Composable
private fun Details(series: SeriesModel) {
    val app = LocalApp.current
    var expanded by remember { mutableStateOf(false) }
    val detail = series.detail
    Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
        detail?.description?.let { description ->
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    AkiText(description, AkiTextStyle.Body, lines = if (expanded) null else 6)
                    AkiButton(if (expanded) "Show less" else "Read more", variant = AkiButtonVariant.Ghost, size = AkiButtonSize.Small) { expanded = !expanded }
                }
            }
        }
        AkiCard(padding = 4.dp) {
            Info("Status", series.statusText ?: "Unknown")
            Info("Author", detail?.authors?.takeIf { it.isNotEmpty() }?.joinToString(", ") ?: "—")
            Info("Source", app.name(series.ref.source))
            Info("Chapters", "${series.chapters.size}")
        }
        if (detail != null && detail.tags.isNotEmpty()) FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) { for (tag in detail.tags) AkiBadge(tag) }
    }
}

@Composable
private fun Info(name: String, value: String) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        AkiText(name, AkiTextStyle.Caption, Modifier.weight(1f), tone = AkiTone.Muted)
        AkiText(value, AkiTextStyle.BodyStrong, align = androidx.compose.ui.text.style.TextAlign.End)
    }
}
