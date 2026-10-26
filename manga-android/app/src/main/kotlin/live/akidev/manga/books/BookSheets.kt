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
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.shareFile
import live.akidev.manga.design.*
import uniffi.akimanga.BookEntry
import uniffi.akimanga.BookHit
import uniffi.akimanga.BookSection

/** What can be changed about how the book looks: theme, face, size, spacing, margins. Every change shows on the page at once. */
@Composable
fun BookSettingsSheet(model: BookModel, onDismiss: () -> Unit) {
    val style = model.style
    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(22.dp)) {
            AkiText("Type and theme", AkiTextStyle.Title)
            ThemePicker(style.theme) { model.changeStyle(style.copy(theme = it)) }
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AkiText("Reading", AkiTextStyle.CaptionStrong, tone = AkiTone.Muted)
                AkiTabs(listOf(AkiChoice(false, "Pages"), AkiChoice(true, "Scroll")), style.scroll, { model.changeStyle(style.copy(scroll = it)) })
            }
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(18.dp)) {
                    AkiSelect("Font", style.font, { model.changeStyle(style.copy(font = it)) }, BookFont.entries.map { AkiOption(it, it.title) })
                    Stepper("Text size", style.size, BookStyle.sizes, "A", "A") { model.changeStyle(style.copy(size = it)) }
                    Stepper("Line spacing", style.lineSpacing, BookStyle.spacings, "−", "+") { model.changeStyle(style.copy(lineSpacing = it)) }
                }
            }
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AkiText("Margins", AkiTextStyle.CaptionStrong, tone = AkiTone.Muted)
                AkiTabs(BookMargin.entries.map { AkiChoice(it, it.title) }, style.margin, { model.changeStyle(style.copy(margin = it)) })
            }
            Light(model)
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    AkiSwitch("Justify text", style.justify, { model.changeStyle(style.copy(justify = it)) })
                    AkiSwitch("Hyphenate long words", style.hyphenate, { model.changeStyle(style.copy(hyphenate = it)) }, subtitle = "Fewer gaps in justified lines.")
                    AkiSwitch("Use the book's fonts", style.bookFonts, { model.changeStyle(style.copy(bookFonts = it)) }, subtitle = "For headings and special text, when the book carries its own.")
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
                "Set the brightness here", style.brightness != null, { model.changeStyle(style.copy(brightness = if (it) 0.6 else null)) },
                subtitle = "Goes lower than the phone's lowest, and is given back when you close the book.",
            )
            style.brightness?.let { Stepper("Brightness", it, 0.0..1.0, "−", "+") { value -> model.changeStyle(style.copy(brightness = value)) } }
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AkiText("Warm tint", AkiTextStyle.BodyStrong)
                AkiTabs(BookWarmth.entries.map { AkiChoice(it, it.title) }, style.warmth, { model.changeStyle(style.copy(warmth = it)) })
            }
            if (style.warmth != BookWarmth.Off) Stepper("Warmth", style.warmthAmount, 0.1..0.8, "−", "+") { model.changeStyle(style.copy(warmthAmount = it)) }
            AkiSwitch("Ink display", style.ink, { model.changeStyle(style.copy(ink = it)) }, subtitle = "Grey, still and quiet like paper: no page-turn motion.")
        }
    }
}

@Composable
private fun Stepper(title: String, value: Double, range: ClosedFloatingPointRange<Double>, small: String, large: String, onChange: (Double) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        AkiText(title, AkiTextStyle.BodyStrong)
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            AkiText(small, AkiTextStyle.Caption, tone = AkiTone.Muted)
            AkiSlider(value, onChange, Modifier.weight(1f), range)
            AkiText(large, AkiTextStyle.Heading, tone = AkiTone.Muted)
        }
    }
}

/** The reading themes as small round swatches, the chosen one ringed in yellow. */
@Composable
private fun ThemePicker(selection: BookTheme, onSelect: (BookTheme) -> Unit) {
    val dark = androidx.compose.foundation.isSystemInDarkTheme()
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        for (theme in BookTheme.entries) {
            val shown = theme.resolved(dark)
            val chosen = theme == selection
            AkiTap({ onSelect(theme) }, label = theme.title) {
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Box(
                        Modifier.size(46.dp).background(shown.paper, CircleShape).border(if (chosen) 3.dp else 1.dp, if (chosen) Aki.colors.primary else Aki.colors.secondary, CircleShape),
                        contentAlignment = Alignment.Center,
                    ) { androidx.compose.foundation.text.BasicText("Aa", style = androidx.compose.ui.text.TextStyle(color = shown.ink, fontSize = androidx.compose.ui.unit.TextUnit(15f, androidx.compose.ui.unit.TextUnitType.Sp), fontFamily = androidx.compose.ui.text.font.FontFamily.Serif, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)) }
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
                AkiText(when (tab) { ContentsTab.Contents -> "Contents"; ContentsTab.Marks -> "Highlights"; ContentsTab.About -> "About" }, AkiTextStyle.Title)
                AkiText(model.info.title, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 1)
            }
            AkiTabs(listOf(AkiChoice(ContentsTab.Contents, "Contents"), AkiChoice(ContentsTab.Marks, "Marks ${model.marks.size}"), AkiChoice(ContentsTab.About, "About")), tab, { tab = it })
        }
        when (tab) {
            ContentsTab.Contents -> LazyColumn(Modifier.weight(1f), list, contentPadding = androidx.compose.foundation.layout.PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                val front = indices(Group.Front)
                val back = indices(Group.Back)
                if (front.isNotEmpty()) item(key = "front") { Fold("Before the book", front.size, showFront) { showFront = !showFront } }
                if (showFront) items(front.size, key = { "e${front[it]}" }) { EntryRow(model, front[it], current, onClose) }
                val body = indices(Group.Body)
                items(body.size, key = { "e${body[it]}" }) { EntryRow(model, body[it], current, onClose) }
                if (back.isNotEmpty()) item(key = "back") { Fold("After the book", back.size, showBack) { showBack = !showBack } }
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
    AkiTap(toggle) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            AkiText("$title · $count", AkiTextStyle.CaptionStrong, Modifier.weight(1f), tone = AkiTone.Muted)
            AkiIcon(if (open) "down" else "forward", size = 12.dp, tone = AkiTone.Muted)
        }
    }
}

@Composable
private fun EntryRow(model: BookModel, index: Int, current: Int, onClose: () -> Unit) {
    val entry = model.contents[index]
    val isCurrent = index == current
    AkiTap({ model.open(entry); onClose() }) {
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
        AkiEmptyState("No highlights yet", "Press and hold a word in the book, then drag the handles to choose what to highlight.", Modifier.heightIn(min = 240.dp))
        return
    }
    for (mark in model.marks) {
        AkiTapMenu(listOf(AkiMenuItem("Remove", "trash", destructive = true) { model.remove(mark) }), onClick = { model.open(mark); onClose() }) {
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
    model.markdown()?.let { text -> AkiButton("Share as Markdown", icon = "share", variant = AkiButtonVariant.Primary) { shareFile(context, "${model.info.title.replace("/", "-")} highlights.md", text, "text/markdown") } }
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
                val facts = listOfNotNull(model.info.language?.uppercase(), android.text.format.Formatter.formatFileSize(context, model.info.bytes.toLong()), "${model.chapterCount} parts")
                AkiText(facts.joinToString(" · "), AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 3)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            AkiButton("Use this page as cover", icon = "image", size = AkiButtonSize.Small) { scope.launch { coverNote = if (model.useCurrentPageAsCover()) null else "This page could not be made into a cover." } }
            if (model.info.customCover) AkiButton("Original", size = AkiButtonSize.Small) { model.useOwnCover() }
        }
        coverNote?.let { AkiText(it, AkiTextStyle.Caption, tone = AkiTone.Destructive) }
        if (model.aboutPages.isEmpty()) AkiText("This book has no pages besides the book itself.", AkiTextStyle.Caption, tone = AkiTone.Muted)
        // A page that is only a picture (a title page) has nothing to read here.
        for (chapter in model.aboutPages.filter { texts[it]?.isEmpty() != true }) {
            val text = texts[chapter]
            val open = chapter in opened
            Column(Modifier.fillMaxWidth().background(Aki.colors.surface, RoundedCornerShape(22.dp)).padding(horizontal = 18.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AkiTap({ opened = if (open) opened - chapter else opened + chapter }) {
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        AkiText(text?.let { model.pageTitle(chapter, it) } ?: model.contents.firstOrNull { it.chapter.toInt() == chapter }?.title ?: "Page ${chapter + 1}", AkiTextStyle.BodyStrong, Modifier.weight(1f), lines = 2)
                        AkiIcon(if (open) "down" else "forward", size = 12.dp, tone = AkiTone.Muted)
                    }
                }
                if (open) {
                    if (text != null) {
                        // A long page (notes) shows its start: the rest is in the book.
                        AkiText(text.take(1800) + if (text.length > 1800) "…" else "", tone = AkiTone.Muted)
                        AkiButton("Open in the book", icon = "forward", size = AkiButtonSize.Small) { model.open(BookEntry("", 0u, chapter.toUInt(), null)); onClose() }
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
            AkiText("Search", AkiTextStyle.Title)
            AkiInput("Words in this book", query, { query = it }, icon = "search", onSubmit = ::run, onClear = { task?.cancel(); hits = emptyList(); asked = ""; searching = false })
            if (searching && hits.isEmpty()) {
                Box(Modifier.fillMaxWidth().heightIn(min = 80.dp), contentAlignment = Alignment.Center) { AkiSpinner() }
            } else if (asked.isNotEmpty() && hits.isEmpty()) {
                AkiText("Nothing for “$asked”.", tone = AkiTone.Muted)
            } else if (hits.firstOrNull()?.similar == true) {
                AkiText("Nothing is written exactly like that. These are the nearest words.", AkiTextStyle.Caption, tone = AkiTone.Muted)
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
            AkiText("Note", AkiTextStyle.Title)
            AkiText(draft.quote, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 4)
            AkiTextArea(text, { text = it }, height = 140.dp, prose = true)
            AkiButton("Save", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large, fullWidth = true) { model.saveNote(draft, text); onClose() }
        }
    }
}

/** A footnote, in a card over the page, with a way to go to it. */
@Composable
fun BookFootnoteSheet(model: BookModel, note: Footnote, onClose: () -> Unit) {
    val scope = rememberCoroutineScope()
    AkiSheet(onClose) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            AkiText("Note", AkiTextStyle.Title)
            AkiText(note.text)
            AkiButton("Go to the note", icon = "forward", fullWidth = true) {
                onClose()
                scope.launch { model.followInBook(note.link) }
            }
        }
    }
}
