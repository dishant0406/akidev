package live.akidev.manga.books

import android.text.format.Formatter
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import live.akidev.manga.app.shortDate
import live.akidev.manga.design.*
import uniffi.akimanga.BookHighlight
import uniffi.akimanga.BookInfo

/** What the person can do with one book: where they are in it, its highlights and what to change about it. */
@Composable
fun BookDetailSheet(
    book: BookInfo,
    marks: List<BookHighlight>,
    onDismiss: () -> Unit,
    read: () -> Unit,
    finish: () -> Unit,
    reset: () -> Unit,
    categories: () -> Unit,
    chooseCover: () -> Unit,
    ownCover: () -> Unit,
    remove: () -> Unit,
) {
    val context = LocalContext.current
    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                AkiImage(book.cover, Modifier.size(width = 104.dp, height = 156.dp).clip(RoundedCornerShape(Aki.thumbRadius)), placeholder = 1.5f, fill = true)
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    AkiText(book.title, AkiTextStyle.Heading, lines = 4)
                    if (book.authors.isNotEmpty()) AkiText(book.authors.joinToString(", "), AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 3)
                    val facts = listOfNotNull(book.language?.uppercase(), Formatter.formatFileSize(context, book.bytes.toLong()), "Added ${shortDate(book.addedMs) ?: ""}")
                    AkiText(facts.joinToString(" · "), AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 3)
                    if (book.openedMs != null) {
                        AkiProgressBar(if (book.finished) 1.0 else book.progress, Modifier.padding(top = 4.dp))
                        AkiText(if (book.finished) "Finished" else "${Math.round(book.progress * 100)}% read", AkiTextStyle.Caption, tone = AkiTone.Muted)
                    }
                }
            }
            AkiButton(
                if (book.openedMs == null) "Start reading" else if (book.finished) "Read again" else "Continue", icon = "play-solid",
                variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large, fullWidth = true, onClick = read,
            )
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                if (!book.finished) AkiButton("Mark as finished", icon = "check", fullWidth = true, onClick = finish)
                if (book.openedMs != null) AkiButton("Mark as not started", icon = "restore", fullWidth = true, onClick = reset)
                AkiButton("Categories", icon = "folder", fullWidth = true, onClick = categories)
                AkiButton("Choose a cover picture", icon = "image", fullWidth = true, onClick = chooseCover)
                if (book.customCover) AkiButton("Use the book's own cover", icon = "restore", fullWidth = true, onClick = ownCover)
            }
            if (marks.isNotEmpty()) {
                AkiSection("Highlights ${marks.size}") {
                    for (mark in marks) {
                        AkiCard(padding = 14.dp) {
                            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                AkiText("“${mark.quote}”", lines = 4)
                                mark.note?.takeIf { it.isNotEmpty() }?.let { AkiText(it, AkiTextStyle.Caption, tone = AkiTone.Muted, lines = 4) }
                            }
                        }
                    }
                }
            }
            AkiButton("Remove from this phone", icon = "trash", variant = AkiButtonVariant.Destructive, fullWidth = true, onClick = remove)
        }
    }
}
