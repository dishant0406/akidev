package live.akidev.manga.settings

import android.Manifest
import android.os.Build
import android.text.format.Formatter
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import live.akidev.manga.app.Appearance
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.Problem
import live.akidev.manga.app.ReaderKind
import live.akidev.manga.app.VoicesRoute
import live.akidev.manga.books.BookFont
import live.akidev.manga.books.BookTheme
import live.akidev.manga.app.ReadingMode
import live.akidev.manga.app.describe
import live.akidev.manga.app.rememberFilePicker
import live.akidev.manga.app.shareFile
import live.akidev.manga.design.*
import live.akidev.manga.updates.UpdateAlerts
import uniffi.akimanga.CategoryInfo
import java.time.LocalDate

/** Everything the person can change about the app, and where their data goes in and out. */
@Composable
fun SettingsScreen() {
    val app = LocalApp.current
    val router = LocalRouter.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var message by remember { mutableStateOf<Problem?>(null) }
    var downloaded by remember { mutableStateOf(0L) }
    var categories by remember { mutableStateOf(emptyList<CategoryInfo>()) }
    val importer = rememberFilePicker(arrayOf("application/json", "text/plain", "*/*")) { file ->
        try {
            val report = app.engine.restore(file.readText())
            app.refresh()
            val sources = report.sourcesInstalled.size
            val missing = if (report.sourcesMissing.isEmpty()) "" else " " + Copy.Settings.couldNotBringBack(report.sourcesMissing.joinToString(", "))
            message = Problem(Copy.Settings.backupRestored, Copy.Settings.restoreSummary("${report.series}", Copy.Settings.restoredSourceCount(sources)) + missing)
        } catch (failure: Exception) {
            message = describe(failure)
        }
    }
    val comicImporter = rememberFilePicker(arrayOf("application/zip", "application/vnd.comicbook+zip", "application/x-cbz", "*/*")) { file ->
        message = try {
            app.engine.importFile(file.path)
            app.refresh()
            Problem(Copy.Settings.addedToLibrary, Copy.Settings.findInLibrary)
        } catch (failure: Exception) {
            describe(failure)
        }
    }
    // Turning alerts on asks Android for permission (from Android 13); if the person said no, the switch stays off and says where to change it.
    val askForAlerts = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        app.prefs.notifyUpdates = granted
        app.scheduleBackgroundWork()
        if (!granted) message = Problem(Copy.Settings.notificationsOffTitle, Copy.Settings.notificationsOffMessage)
    }
    // Whether the voices repository answers is asked now, when Settings opens, and never at launch.
    LaunchedEffect(Unit) { app.readAloud.check() }
    LaunchedEffect(Unit) {
        downloaded = runCatching { app.engine.downloads().sumOf { it.bytes.toLong() } }.getOrDefault(0)
        categories = runCatching { app.engine.categories() }.getOrDefault(emptyList())
    }

    AkiPage(underTabBar = false, spacing = 22.dp) {
        item { AkiBackHeader(Copy.Settings.title, back = router::back) }
        if (app.prefs.readerKind == ReaderKind.Books) {
            readerSection()
            booksSection()
            readAloudSection()
            appearanceSection()
            dataSection(onBackup = {
                runCatching { app.engine.backup() }.fold(
                    { shareFile(context, "AkiManga-${LocalDate.now()}.json", it, "application/json") },
                    { message = describe(it) },
                )
            }, onRestore = importer, onComic = comicImporter)
            item { AkiText(Copy.Home.manga, AkiTextStyle.Title, Modifier.padding(top = 8.dp), isHeading = true) }
            item { ContentSection() }
            item { LibrarySection(categories) }
            item { ReadingSection() }
            item { UpdatesSection { enableAlerts(app, context, askForAlerts) } }
            item { DownloadsSection(downloaded, scope) }
            aboutSection()
        } else {
            appearanceSection()
            readerSection()
            item { ContentSection() }
            item { LibrarySection(categories) }
            item { ReadingSection() }
            booksSection()
            readAloudSection()
            item { UpdatesSection { enableAlerts(app, context, askForAlerts) } }
            item { DownloadsSection(downloaded, scope) }
            dataSection(onBackup = {
                runCatching { app.engine.backup() }.fold(
                    { shareFile(context, "AkiManga-${LocalDate.now()}.json", it, "application/json") },
                    { message = describe(it) },
                )
            }, onRestore = importer, onComic = comicImporter)
            aboutSection()
        }
    }
    message?.let { AkiAlert(it.title, it.message) { message = null } }
}

private fun enableAlerts(app: live.akidev.manga.app.AppModel, context: android.content.Context, ask: androidx.activity.result.ActivityResultLauncher<String>) {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !UpdateAlerts.allowed(context)) {
        ask.launch(Manifest.permission.POST_NOTIFICATIONS)
    } else {
        app.prefs.notifyUpdates = true
        app.scheduleBackgroundWork()
    }
}

private fun LazyListScope.appearanceSection() {
    item {
        val prefs = LocalApp.current.prefs
        AkiSection(Copy.Settings.appearance) { AkiCard(padding = 16.dp) { AkiSelect(Copy.Settings.theme, prefs.appearance, { prefs.appearance = it }, Appearance.entries.map { AkiOption(it, it.title) }) } }
    }
    item {
        val prefs = LocalApp.current.prefs
        AkiSection(Copy.Settings.glass, footer = Copy.Settings.glassFooter) {
            AkiCard(padding = 16.dp) { AkiSwitch(Copy.Settings.reduceGlass, prefs.reduceGlass, { prefs.reduceGlass = it }) }
        }
    }
}

private fun LazyListScope.readerSection() {
    item {
        val prefs = LocalApp.current.prefs
        AkiSection(Copy.Settings.reader, footer = Copy.Settings.readerFooter) {
            AkiCard(padding = 16.dp) {
                AkiSelect(Copy.Settings.readerKindLabel, prefs.readerKind ?: ReaderKind.Manga, { prefs.readerKind = it }, ReaderKind.entries.map { AkiOption(it, it.title) })
            }
        }
    }
}

/** How books look and read before one is opened; the reader's own sheet changes the same things for the book in hand. */
private fun LazyListScope.booksSection() {
    item {
        val prefs = LocalApp.current.prefs
        val style = prefs.bookStyle
        AkiSection(Copy.Home.books, footer = Copy.Settings.booksFooter) {
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    AkiSelect(Copy.Settings.bookReadingStyle, style.scroll, { prefs.bookStyle = style.copy(scroll = it) }, listOf(AkiOption(false, Copy.Books.pages), AkiOption(true, Copy.Books.scroll)))
                    AkiSelect(Copy.Settings.theme, style.theme, { prefs.bookStyle = style.copy(theme = it) }, BookTheme.entries.map { AkiOption(it, it.title) })
                    AkiSelect(Copy.Books.font, style.font, { prefs.bookStyle = style.copy(font = it) }, BookFont.entries.map { AkiOption(it, it.title) })
                    AkiSwitch(Copy.Reader.keepScreenOn, prefs.keepAwake, { prefs.keepAwake = it })
                }
            }
        }
    }
}

/** Reading aloud is offered only when the voices repository answered when this screen opened. */
private fun LazyListScope.readAloudSection() {
    item {
        val app = LocalApp.current
        val router = LocalRouter.current
        if (app.readAloud.reachable) {
            AkiSection(Copy.Settings.readAloud, footer = Copy.Settings.readAloudFooter) {
                AkiTap({ router.push(VoicesRoute) }) { AkiRow(Copy.Settings.voiceModel, subtitle = app.readAloud.current?.name ?: Copy.Settings.noVoiceModelYet, icon = "headphones", showsChevron = true) }
            }
        }
    }
}

@Composable
private fun ContentSection() {
    val app = LocalApp.current
    val prefs = app.prefs
    val languages = app.sources.flatMap { it.languages }.toSortedSet().toList()
    AkiSection(Copy.Settings.content, footer = Copy.Settings.contentFooterWithSwitch) {
        AkiCard(padding = 16.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                AkiSwitch(Copy.Settings.showAdultSources, prefs.showAdult, { prefs.showAdult = it })
                if (languages.isNotEmpty()) {
                    AkiSelect(Copy.Settings.language, prefs.languages.firstOrNull() ?: "", { prefs.languages = if (it.isEmpty()) emptyList() else listOf(it) }, listOf(AkiOption("", Copy.Home.all)) + languages.map { AkiOption(it, it.uppercase()) })
                }
            }
        }
    }
}

@Composable
private fun LibrarySection(categories: List<CategoryInfo>) {
    val prefs = LocalApp.current.prefs
    val current = if (categories.any { it.id == prefs.saveCategory } || prefs.saveCategory == -1L) prefs.saveCategory else 0L
    AkiSection(Copy.Home.library, footer = Copy.Settings.libraryFooter) {
        AkiCard(padding = 16.dp) {
            AkiSelect(Copy.Settings.whenAddingToLibrary, current, { prefs.saveCategory = it }, listOf(AkiOption(0L, Copy.Library.noCategory), AkiOption(-1L, Copy.Settings.askEachTime)) + categories.map { AkiOption(it.id, Copy.Settings.putInCategory(it.name)) })
        }
    }
}

@Composable
private fun ReadingSection() {
    val prefs = LocalApp.current.prefs
    AkiSection(Copy.Settings.reading, footer = Copy.Settings.readingFooter) {
        AkiCard(padding = 16.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                AkiSelect(Copy.Settings.defaultMode, prefs.defaultMode, { prefs.defaultMode = it }, ReadingMode.entries.map { AkiOption(it, it.title) })
                AkiSwitch(Copy.Settings.leftEdgeForward, prefs.invertTaps, { prefs.invertTaps = it }, subtitle = Copy.Settings.leftEdgeForwardHint)
                AkiSwitch(Copy.Reader.keepScreenOn, prefs.keepAwake, { prefs.keepAwake = it })
            }
        }
    }
}

@Composable
private fun UpdatesSection(enableAlerts: () -> Unit) {
    val app = LocalApp.current
    val prefs = app.prefs
    AkiSection(
        Copy.Home.updates,
        footer = Copy.Settings.updatesFooter,
    ) {
        AkiCard(padding = 16.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                AkiSwitch(
                    Copy.Settings.newChapterAlerts, prefs.notifyUpdates, { if (it) enableAlerts() else { prefs.notifyUpdates = false; app.scheduleBackgroundWork() } },
                    subtitle = Copy.Settings.newChapterAlertsHint,
                )
                AkiSwitch(Copy.Settings.downloadNewChapters, prefs.autoDownload, { prefs.autoDownload = it; app.scheduleBackgroundWork() }, subtitle = Copy.Settings.downloadNewChaptersHint)
                if (prefs.autoDownload) AkiSwitch(Copy.Settings.downloadOnWifiOnly, prefs.downloadOnWifiOnly, { prefs.downloadOnWifiOnly = it })
            }
        }
    }
}

@Composable
private fun DownloadsSection(downloaded: Long, scope: kotlinx.coroutines.CoroutineScope) {
    val context = LocalContext.current
    val view = androidx.compose.ui.platform.LocalView.current
    AkiSection(Copy.Settings.downloads) {
        AkiCard(padding = 16.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    AkiText(Copy.Settings.keptOnThisDevice, AkiTextStyle.BodyStrong, Modifier.weight(1f))
                    AkiText(Formatter.formatFileSize(context, downloaded), tone = AkiTone.Muted, tabular = true)
                }
                AkiButton(Copy.Settings.clearImageCache, icon = "trash", fullWidth = true) { scope.launch(Dispatchers.IO) { AkiImageLoader.clear() }; view.announceForAccessibility(Copy.Settings.imageCacheCleared) }
            }
        }
    }
}

private fun LazyListScope.dataSection(onBackup: () -> Unit, onRestore: () -> Unit, onComic: () -> Unit) {
    item {
        AkiSection(Copy.Settings.yourData, footer = Copy.Settings.yourDataFooter) {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                AkiButton(Copy.Settings.createBackup, icon = "server", fullWidth = true, onClick = onBackup)
                AkiButton(Copy.Settings.restoreFromBackup, icon = "restore", fullWidth = true, onClick = onRestore)
                AkiButton(Copy.Settings.openCbzComic, icon = "zip", fullWidth = true, onClick = onComic)
            }
        }
    }
}

private fun LazyListScope.aboutSection() {
    item {
        val app = LocalApp.current
        AkiSection(Copy.Books.about) {
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                        AkiLogo(size = 44.dp)
                        AkiText(Copy.Settings.akidevVersion(app.engine.version()), AkiTextStyle.BodyStrong, tabular = true)
                    }
                    AkiText(Copy.Settings.aboutCredits + " " + Copy.Settings.aboutFontCredit, AkiTextStyle.Caption, tone = AkiTone.Muted)
                    AkiText(
                        Copy.Settings.aboutDescription,
                        AkiTextStyle.Caption, tone = AkiTone.Muted,
                    )
                }
            }
        }
    }
}
