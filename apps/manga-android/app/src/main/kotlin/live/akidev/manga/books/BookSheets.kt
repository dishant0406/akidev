package live.akidev.manga.books

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import live.akidev.manga.app.shareFile
import live.akidev.manga.design.*
import live.akidev.manga.design.Copy
import uniffi.akimanga.BookEntry
import uniffi.akimanga.BookHit
import uniffi.akimanga.BookSection
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription

/** What can be changed about how the book looks: theme, face, size, spacing, margins. Every change shows on the page at once. */
@Composable
fun BookSettingsSheet(model: BookModel, onDismiss: () -> Unit) {
    val style = model.style
    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(22.dp)) {
            AkiText(Copy.Books.textAndTheme, AkiTextStyle.Title, isHeading = true)
            ThemePicker(style.theme) { model.changeStyle(style.copy(theme = it)) }
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AkiText(Copy.Library.reading, AkiTextStyle.CaptionStrong, tone = AkiTone.Muted, isHeading = true)
                AkiTabs(listOf(AkiChoice(false, Copy.Books.pages), AkiChoice(true, Copy.Books.scroll)), style.scroll, { model.changeStyle(style.copy(scroll = it)) })
            }
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
                    AkiSelect(Copy.Books.font, style.font, { model.changeStyle(style.copy(font = it)) }, BookFont.entries.map { AkiOption(it, it.title) })
                    Stepper(Copy.Books.textSize, style.size, BookStyle.sizes, Copy.Books.textSizeSpoken(Math.round(style.size).toInt()), "A", "A") { model.changeStyle(style.copy(size = it)) }
                    Stepper(Copy.Books.lineSpacing, style.lineSpacing, BookStyle.spacings, "%.2f".format(style.lineSpacing), "−", "+") { model.changeStyle(style.copy(lineSpacing = it)) }
                }
            }
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AkiText(Copy.Books.margins, AkiTextStyle.CaptionStrong, tone = AkiTone.Muted, isHeading = true)
                AkiTabs(BookMargin.entries.map { AkiChoice(it, it.title) }, style.margin, { model.changeStyle(style.copy(margin = it)) })
            }
            Light(model)
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    AkiSwitch(Copy.Books.justifyText, style.justify, { model.changeStyle(style.copy(justify = it)) })
                    AkiSwitch(Copy.Books.hyphenateTitle, style.hyphenate, { model.changeStyle(style.copy(hyphenate = it)) }, subtitle = Copy.Books.hyphenateSubtitle)
                    AkiSwitch(Copy.Books.bookFontsTitle, style.bookFonts, { model.changeStyle(style.copy(bookFonts = it)) }, subtitle = Copy.Books.bookFontsSubtitle)
                }
            }
        }
    }
}

/** Brightness, warmth and the paper-like display. */
@Composable
private fun Light(model: BookModel) {
    val style = model.style
    AkiCard(padding = 16.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
            AkiSwitch(
                Copy.Books.brightnessHereTitle, style.brightness != null, { model.changeStyle(style.copy(brightness = if (it) 0.6 else null)) },
                subtitle = Copy.Books.brightnessHereSubtitle,
            )
            style.brightness?.let { Stepper(Copy.Books.brightness, it, 0.0..1.0, "${Math.round(it * 100)}%", "−", "+") { value -> model.changeStyle(style.copy(brightness = value)) } }
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AkiText(Copy.Books.warmTint, AkiTextStyle.BodyStrong, isHeading = true)
                AkiTabs(BookWarmth.entries.map { AkiChoice(it, it.title) }, style.warmth, { model.changeStyle(style.copy(warmth = it)) })
            }
            if (style.warmth != BookWarmth.Off) Stepper(Copy.Books.warmth, style.warmthAmount, 0.1..0.8, "${Math.round(style.warmthAmount * 100)}%", "−", "+") { model.changeStyle(style.copy(warmthAmount = it)) }
            AkiSwitch(Copy.Books.inkTitle, style.ink, { model.changeStyle(style.copy(ink = it)) }, subtitle = Copy.Books.inkSubtitle)
        }
    }
}

@Composable
private fun Stepper(title: String, value: Double, range: ClosedFloatingPointRange<Double>, spoken: String, small: String, large: String, onChange: (Double) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        // The slider carries the name and the value for TalkBack, so the words around it are not read again.
        AkiText(title, AkiTextStyle.BodyStrong, Modifier.clearAndSetSemantics {})
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            AkiText(small, AkiTextStyle.Caption, Modifier.clearAndSetSemantics {}, tone = AkiTone.Muted)
            AkiSlider(value, onChange, Modifier.weight(1f).spokenValue(spoken), range, label = title)
            AkiText(large, AkiTextStyle.Heading, Modifier.clearAndSetSemantics {}, tone = AkiTone.Muted)
        }
    }
}

/** What TalkBack says as the slider's value ("12 points"), in place of a bare percentage. */
internal fun Modifier.spokenValue(spoken: String) = semantics { stateDescription = spoken }

/** The reading themes as small round swatches, the chosen one ringed in yellow. */
@Composable
private fun ThemePicker(selection: BookTheme, onSelect: (BookTheme) -> Unit) {
    val dark = androidx.compose.foundation.isSystemInDarkTheme()
    // Wraps onto a second line when the font is large, so no swatch is cut off.
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        for (theme in BookTheme.entries) {
            val shown = theme.resolved(dark)
            val chosen = theme == selection
            AkiTap({ onSelect(theme) }, Modifier.semantics { selected = chosen }, label = theme.title) {
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Box(
                        Modifier.size(46.dp).background(shown.paper, CircleShape).border(if (chosen) 3.dp else 1.dp, if (chosen) Aki.colors.primary else Aki.colors.secondary, CircleShape),
                        contentAlignment = Alignment.Center,
                    ) { androidx.compose.foundation.text.BasicText("Aa", Modifier.clearAndSetSemantics {}, style = androidx.compose.ui.text.TextStyle(color = shown.ink, fontSize = androidx.compose.ui.unit.TextUnit(15f, androidx.compose.ui.unit.TextUnitType.Sp), fontFamily = androidx.compose.ui.text.font.FontFamily.Serif, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)) }
                    AkiText(theme.title, AkiTextStyle.Caption, tone = if (chosen) AkiTone.Text else AkiTone.Muted, lines = 1)
                }
            }
        }
    }
}

// ---- contents ----------------------------------------------------------------------------------------------------------------

private enum class ContentsTab { Contents, Marks, About }

private enum class Group { Front, Body, Back }

/** The book's table of contents, and what you highlighted: tap an entry to go there. The part being read is yellow. */
@Composable
fun BookContentsSheet(model: BookModel, onClose: () -> Unit) {
    var tab by remember { mutableStateOf(ContentsTab.Contents) }

    /** Which part of the book an entry is in: the part of the chapter it opens. The contents page itself and the cover are not listed. */
    fun group(entry: BookEntry): Group? = when (model.sections.getOrElse(entry.chapter.toInt()) { BookSection.BODY }) {
        BookSection.COVER, BookSection.CONTENTS -> null
        BookSection.FRONT -> if (entry.chapter.toInt() < model.start) Group.Front else Group.Back
        BookSection.BODY -> Group.Body
        BookSection.BACK -> Group.Back
    }
    val chapter = model.place?.chapter ?: 0
    val current = model.contents.indexOfLast { it.chapter.toInt() <= chapter }.coerceAtLeast(0)
    val currentGroup = model.contents.getOrNull(current)?.let(::group)
    var showFront by remember { mutableStateOf(currentGroup == Group.Front) }
    var showBack by remember { mutableStateOf(currentGroup == Group.Back) }
    fun indices(wanted: Group) = model.contents.indices.filter { group(model.contents[it]) == wanted }
    val list = rememberLazyListState()
    LaunchedEffect(Unit) { if (tab == ContentsTab.Contents) list.scrollToItem(maxOf(current - 3, 0)) }

    AkiSheet(onClose, full = true) {
        Column(Modifier.padding(start = 20.dp, end = 20.dp, top = 16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                AkiText(when (tab) { ContentsTab.Contents -> Copy.Books.contents; ContentsTab.Marks -> Copy.Books.highlights; ContentsTab.About -> Copy.Books.about }, AkiTextStyle.Title, isHeading = true)
                AkiText(model.info.title, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
            }
            AkiTabs(listOf(AkiChoice(ContentsTab.Contents, Copy.Books.contents), AkiChoice(ContentsTab.Marks, Copy.Books.highlights), AkiChoice(ContentsTab.About, Copy.Books.about)), tab, { tab = it })
        }
        when (tab) {
            ContentsTab.Contents -> LazyColumn(Modifier.weight(1f), list, contentPadding = androidx.compose.foundation.layout.PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                val front = indices(Group.Front)
                val back = indices(Group.Back)
                if (front.isNotEmpty()) item(key = "front") { Fold(Copy.Books.beforeTheBook, front.size, showFront) { showFront = !showFront } }
                if (showFront) items(front.size, key = { "e${front[it]}" }) { EntryRow(model, front[it], current, onClose) }
                val body = indices(Group.Body)
                items(body.size, key = { "e${body[it]}" }) { EntryRow(model, body[it], current, onClose) }
                if (back.isNotEmpty()) item(key = "back") { Fold(Copy.Books.afterTheBook, back.size, showBack) { showBack = !showBack } }
                if (showBack) items(back.size, key = { "e${back[it]}" }) { EntryRow(model, back[it], current, onClose) }
            }
            ContentsTab.Marks -> Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) { Marks(model, onClose) }
            ContentsTab.About -> Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(20.dp)) { About(model, onClose) }
        }
    }
}

/** A quiet line that opens or folds the front or back matter. */
@Composable
private fun Fold(title: String, count: Int, open: Boolean, toggle: () -> Unit) {
    AkiTap(toggle, Modifier.semantics { stateDescription = if (open) Copy.Books.expanded else Copy.Books.collapsed }) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            AkiText(Copy.Books.sectionTitleCount(title, count), AkiTextStyle.CaptionStrong, Modifier.weight(1f), tone = AkiTone.Muted, tabular = true)
            AkiIcon(if (open) "down" else "forward", size = 12.dp, tone = AkiTone.Muted)
        }
    }
}

@Composable
private fun EntryRow(model: BookModel, index: Int, current: Int, onClose: () -> Unit) {
    val entry = model.contents[index]
    val isCurrent = index == current
    AkiTap({ model.open(entry); onClose() }, Modifier.semantics { selected = isCurrent }) {
        Row(
            Modifier.fillMaxWidth().background(if (isCurrent) Aki.colors.primary else Aki.colors.surface, RoundedCornerShape(22.dp))
                .padding(start = 18.dp + 16.dp * minOf(entry.depth.toInt(), 3), end = 18.dp, top = 14.dp, bottom = 14.dp),
        ) {
            AkiText(entry.title, if (entry.depth == 0u) AkiTextStyle.BodyStrong else AkiTextStyle.Body, tone = if (isCurrent) AkiTone.OnPrimary else AkiTone.Text, lines = 2)
        }
    }
}

@Composable
private fun Marks(model: BookModel, onClose: () -> Unit) {
    val context = LocalContext.current
    if (model.marks.isEmpty()) {
        AkiEmptyState(Copy.Books.noHighlightsTitle, Copy.Books.noHighlightsInBookMessage, Modifier.heightIn(min = 240.dp))
        return
    }
    for (mark in model.marks) {
        AkiTapMenu(listOf(AkiMenuItem(Copy.Books.removeHighlight, "trash", destructive = true) { model.remove(mark) }), onClick = { model.open(mark); onClose() }) {
            Row(Modifier.fillMaxWidth().background(Aki.colors.surface, RoundedCornerShape(22.dp)).padding(horizontal = 18.dp, vertical = 14.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.padding(top = 5.dp).size(14.dp).background(HighlightColour.colour(mark.colour.toInt()), CircleShape))
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    AkiText(mark.quote, AkiTextStyle.Body, lines = 4)
                    mark.note?.let { AkiText(it, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 4) }
                    AkiText(model.title(mark.chapter.toInt()), AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
                }
            }
        }
    }
    model.markdown()?.let { text -> AkiButton(Copy.Books.shareAsMarkdown, icon = "share", variant = AkiButtonVariant.Primary) { shareFile(context, "${model.info.title.replace("/", "-")} highlights.md", text, "text/markdown") } }
}

/**
 * What the book says about itself and the pages around it: who wrote it, how big it is, and the copyright page, dedication,
 * acknowledgements, notes and colophon, each a card to read here or to open in the book.
 */
@Composable
private fun About(model: BookModel, onClose: () -> Unit) {
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    // The text of each page, read when the card is opened.
    var texts by remember { mutableStateOf(emptyMap<Int, String>()) }
    var opened by remember { mutableStateOf(emptySet<Int>()) }
    var coverNote by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) { for (chapter in model.aboutPages) if (texts[chapter] == null) texts = texts + (chapter to model.plainText(chapter)) }

    Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            AkiImage(model.info.cover, Modifier.size(width = 84.dp, height = 126.dp).clip(RoundedCornerShape(Aki.thumbRadius)), placeholder = 1.5f, fill = true)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                AkiText(model.info.title, AkiTextStyle.Heading, lines = 4)
                if (model.info.authors.isNotEmpty()) AkiText(model.info.authors.joinToString(", "), AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 3)
                val facts = listOfNotNull(model.info.language?.uppercase(), android.text.format.Formatter.formatFileSize(context, model.info.bytes.toLong()), Copy.Books.partCount(model.chapterCount))
                AkiText(facts.joinToString(" · "), AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 3)
            }
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            AkiButton(Copy.Reader.useCoverPage, icon = "image", size = AkiButtonSize.Small) { scope.launch { coverNote = if (model.useCurrentPageAsCover()) null else Copy.Books.pageNotCover } }
            if (model.info.customCover) AkiButton(Copy.Books.useOriginalCover, size = AkiButtonSize.Small) { model.useOwnCover() }
        }
        coverNote?.let { AkiText(it, AkiTextStyle.Caption, Modifier.semantics { liveRegion = LiveRegionMode.Polite }, tone = AkiTone.Destructive) }
        if (model.aboutPages.isEmpty()) AkiText(Copy.Books.noAboutPages, AkiTextStyle.Caption, tone = AkiTone.Muted)
        // A page that is only a picture (a title page) has nothing to read here.
        for (chapter in model.aboutPages.filter { texts[it]?.isEmpty() != true }) {
            val text = texts[chapter]
            val open = chapter in opened
            Column(Modifier.fillMaxWidth().background(Aki.colors.surface, RoundedCornerShape(22.dp)).padding(horizontal = 18.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AkiTap({ opened = if (open) opened - chapter else opened + chapter }, Modifier.semantics { stateDescription = if (open) Copy.Books.expanded else Copy.Books.collapsed }) {
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        AkiText(text?.let { model.pageTitle(chapter, it) } ?: model.contents.firstOrNull { it.chapter.toInt() == chapter }?.title ?: Copy.Books.chapterFallbackTitleAndroid(chapter + 1), AkiTextStyle.BodyStrong, Modifier.weight(1f), lines = 2)
                        AkiIcon(if (open) "down" else "forward", size = 12.dp, tone = AkiTone.Muted)
                    }
                }
                if (open) {
                    if (text != null) {
                        // A long page (notes) shows its start: the rest is in the book.
                        AkiText(text.take(1800) + if (text.length > 1800) "…" else "", tone = AkiTone.Muted)
                        AkiButton(Copy.Books.openInBook, icon = "forward", size = AkiButtonSize.Small) { model.open(BookEntry("", 0u, chapter.toUInt(), null)); onClose() }
                    } else AkiSpinner()
                }
            }
        }
    }
}

// ---- search --------------------------------------------------------------------------------------------------------------------

/** Words in the book, and where they are. */
@Composable
fun BookSearchSheet(model: BookModel, onClose: () -> Unit) {
    val scope = rememberCoroutineScope()
    var query by remember { mutableStateOf("") }
    var asked by remember { mutableStateOf("") }
    var hits by remember { mutableStateOf(emptyList<BookHit>()) }
    var searching by remember { mutableStateOf(false) }
    var task by remember { mutableStateOf<Job?>(null) }

    fun run() {
        val words = query.trim()
        task?.cancel()
        hits = emptyList()
        asked = words
        if (words.isEmpty()) return
        searching = true
        task = scope.launch {
            val found = model.search(words)
            hits = found
            searching = false
        }
    }

    AkiSheet(onClose, full = true) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            AkiText(Copy.Books.search, AkiTextStyle.Title, isHeading = true)
            AkiInput(Copy.Books.searchThisBook, query, { query = it }, icon = "search", onSubmit = ::run, onClear = { task?.cancel(); hits = emptyList(); asked = ""; searching = false })
            if (searching && hits.isEmpty()) {
                Box(Modifier.fillMaxWidth().heightIn(min = 80.dp), contentAlignment = Alignment.Center) { AkiSpinner(label = Copy.Books.searching) }
            } else if (asked.isNotEmpty() && hits.isEmpty()) {
                AkiText(Copy.Books.noResultsFor(asked), modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }, tone = AkiTone.Muted)
            } else if (hits.firstOrNull()?.similar == true) {
                AkiText(Copy.Books.noExactMatch, AkiTextStyle.Caption, Modifier.semantics { liveRegion = LiveRegionMode.Polite }, tone = AkiTone.Muted)
            }
        }
        LazyColumn(Modifier.weight(1f), contentPadding = androidx.compose.foundation.layout.PaddingValues(24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            itemsIndexed(hits) { _, hit ->
                AkiTap({ model.open(hit); onClose() }) {
                    Column(Modifier.fillMaxWidth().background(Aki.colors.surface, RoundedCornerShape(22.dp)).padding(horizontal = 18.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        AkiText(hit.snippet, lines = 3)
                        AkiText(model.title(hit.chapter.toInt()), AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
                    }
                }
            }
        }
    }
}

// ---- a note and a footnote --------------------------------------------------------------------------------------------------

/** A note on a highlight. */
@Composable
fun BookNoteSheet(model: BookModel, draft: NoteDraft, onClose: () -> Unit) {
    var text by remember { mutableStateOf(model.marks.firstOrNull { it.id == draft.id }?.note ?: "") }
    AkiSheet(onClose) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            AkiText(Copy.Books.note, AkiTextStyle.Title, isHeading = true)
            AkiText(draft.quote, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 4)
            AkiTextArea(text, { text = it }, height = 140.dp, prose = true, label = Copy.Books.note)
            AkiButton(Copy.Books.saveNote, variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large, fullWidth = true) { model.saveNote(draft, text); onClose() }
        }
    }
}

/** A footnote, in a card over the page, with a way to go to it. */
@Composable
fun BookFootnoteSheet(model: BookModel, note: Footnote, onClose: () -> Unit) {
    val scope = rememberCoroutineScope()
    AkiSheet(onClose) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            AkiText(Copy.Books.footnote, AkiTextStyle.Title, isHeading = true)
            AkiText(note.text)
            AkiButton(Copy.Books.goToFootnote, icon = "forward", fullWidth = true) {
                onClose()
                scope.launch { model.followInBook(note.link) }
            }
        }
    }
}
