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
import kotlinx.coroutines.withContext
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
            val missing = if (report.sourcesMissing.isEmpty()) "" else " Could not bring back: ${report.sourcesMissing.joinToString(", ")}."
            message = Problem("Restored", "${report.series} series and ${report.sourcesInstalled.size} sources.$missing")
        } catch (failure: Exception) {
            message = describe(failure)
        }
    }
    val comicImporter = rememberFilePicker(arrayOf("application/zip", "application/vnd.comicbook+zip", "application/x-cbz", "*/*")) { file ->
        message = try {
            app.engine.importFile(file.path)
            app.refresh()
            Problem("Added to your library", "Find it under Saved.")
        } catch (failure: Exception) {
            describe(failure)
        }
    }
    // Turning alerts on asks Android for permission (from Android 13); if the person said no, the switch stays off and says where to change it.
    val askForAlerts = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        app.prefs.notifyUpdates = granted
        app.scheduleBackgroundWork()
        if (!granted) message = Problem("Notifications are off", "Turn them on for Akidev in the Android Settings app, then try again.")
    }
    // Whether the voices repository answers is asked now, when Settings opens, and never at launch.
    LaunchedEffect(Unit) { app.readAloud.check() }
    LaunchedEffect(Unit) {
        downloaded = runCatching { app.engine.downloads().sumOf { it.bytes.toLong() } }.getOrDefault(0)
        categories = runCatching { app.engine.categories() }.getOrDefault(emptyList())
    }

    AkiPage(underTabBar = false, spacing = 22.dp) {
        item { AkiBackHeader("Settings", back = router::back) }
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
            item { AkiText("Manga", AkiTextStyle.Title, Modifier.padding(top = 8.dp)) }
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
        AkiSection("Appearance") { AkiCard(padding = 16.dp) { AkiSelect("Theme", prefs.appearance, { prefs.appearance = it }, Appearance.entries.map { AkiOption(it, it.title) }) } }
    }
    item {
        val prefs = LocalApp.current.prefs
        AkiSection("Glass", footer = "Controls draw what is behind them, bent like glass. Turn it off if this phone feels slow: they become plain shapes.") {
            AkiCard(padding = 16.dp) { AkiSwitch("Reduce glass", prefs.reduceGlass, { prefs.reduceGlass = it }) }
        }
    }
}

private fun LazyListScope.readerSection() {
    item {
        val prefs = LocalApp.current.prefs
        AkiSection("Reader", footer = "Manga and books are both always here. This decides which comes first: the order of the tabs, and the one the app opens on.") {
            AkiCard(padding = 16.dp) {
                AkiSelect("I mostly read", prefs.readerKind ?: ReaderKind.Manga, { prefs.readerKind = it }, ReaderKind.entries.map { AkiOption(it, it.title) })
            }
        }
    }
}

/** How books look and read before one is opened; the reader's own sheet changes the same things for the book in hand. */
private fun LazyListScope.booksSection() {
    item {
        val prefs = LocalApp.current.prefs
        val style = prefs.bookStyle
        AkiSection("Books", footer = "How a book opens. Once you are in a book, its Type and theme sheet changes these for you as you read.") {
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    AkiSelect("Reading", style.scroll, { prefs.bookStyle = style.copy(scroll = it) }, listOf(AkiOption(false, "Pages"), AkiOption(true, "Scroll")))
                    AkiSelect("Theme", style.theme, { prefs.bookStyle = style.copy(theme = it) }, BookTheme.entries.map { AkiOption(it, it.title) })
                    AkiSelect("Font", style.font, { prefs.bookStyle = style.copy(font = it) }, BookFont.entries.map { AkiOption(it, it.title) })
                    AkiSwitch("Keep the screen on", prefs.keepAwake, { prefs.keepAwake = it })
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
            AkiSection("Reading aloud", footer = "Books are read in a voice that runs on this phone. Get a model once; reading then needs no network.") {
                AkiTap({ router.push(VoicesRoute) }) { AkiRow("Voice model", subtitle = app.readAloud.current?.name ?: "None yet: choose one to get", icon = "headphones", showsChevron = true) }
            }
        }
    }
}

@Composable
private fun ContentSection() {
    val app = LocalApp.current
    val prefs = app.prefs
    val languages = app.sources.flatMap { it.languages }.toSortedSet().toList()
    AkiSection("Content", footer = "Sources marked 18+ stay out of Discover and Sources until you turn this on.") {
        AkiCard(padding = 16.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                AkiSwitch("Show 18+ sources", prefs.showAdult, { prefs.showAdult = it })
                if (languages.isNotEmpty()) {
                    AkiSelect("Language", prefs.languages.firstOrNull() ?: "", { prefs.languages = if (it.isEmpty()) emptyList() else listOf(it) }, listOf(AkiOption("", "All")) + languages.map { AkiOption(it, it.uppercase()) })
                }
            }
        }
    }
}

@Composable
private fun LibrarySection(categories: List<CategoryInfo>) {
    val prefs = LocalApp.current.prefs
    val current = if (categories.any { it.id == prefs.saveCategory } || prefs.saveCategory == -1L) prefs.saveCategory else 0L
    AkiSection("Library", footer = "What saving a series does about categories. Make categories from the library's menu.") {
        AkiCard(padding = 16.dp) {
            AkiSelect("When saving", current, { prefs.saveCategory = it }, listOf(AkiOption(0L, "Nothing"), AkiOption(-1L, "Ask which")) + categories.map { AkiOption(it.id, "Put on ${it.name}") })
        }
    }
}

@Composable
private fun ReadingSection() {
    val prefs = LocalApp.current.prefs
    AkiSection("Reading", footer = "A series keeps its own mode once you change it in the reader.") {
        AkiCard(padding = 16.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                AkiSelect("Default mode", prefs.defaultMode, { prefs.defaultMode = it }, ReadingMode.entries.map { AkiOption(it, it.title) })
                AkiSwitch("Swap tap sides", prefs.invertTaps, { prefs.invertTaps = it }, subtitle = "In paged reading the left edge goes forward.")
                AkiSwitch("Keep the screen on", prefs.keepAwake, { prefs.keepAwake = it })
            }
        }
    }
}

@Composable
private fun UpdatesSection(enableAlerts: () -> Unit) {
    val app = LocalApp.current
    val prefs = app.prefs
    AkiSection(
        "Updates",
        footer = "Akidev checks your saved series in the background when Android allows it, usually a few times a day and less if you rarely open the app. Nothing is sent anywhere: the check and the alert happen on this device.",
    ) {
        AkiCard(padding = 16.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                AkiSwitch(
                    "New chapter alerts", prefs.notifyUpdates, { if (it) enableAlerts() else { prefs.notifyUpdates = false; app.scheduleBackgroundWork() } },
                    subtitle = "A notification when a saved series gets a chapter.",
                )
                AkiSwitch("Download new chapters", prefs.autoDownload, { prefs.autoDownload = it; app.scheduleBackgroundWork() }, subtitle = "Keeps them on this device to read offline.")
                if (prefs.autoDownload) AkiSwitch("Only on Wi-Fi", prefs.downloadOnWifiOnly, { prefs.downloadOnWifiOnly = it })
            }
        }
    }
}

@Composable
private fun DownloadsSection(downloaded: Long, scope: kotlinx.coroutines.CoroutineScope) {
    val context = LocalContext.current
    AkiSection("Downloads") {
        AkiCard(padding = 16.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    AkiText("Kept on this device", AkiTextStyle.BodyStrong, Modifier.weight(1f))
                    AkiText(Formatter.formatFileSize(context, downloaded), tone = AkiTone.Muted)
                }
                AkiButton("Clear image cache", icon = "trash", fullWidth = true) { scope.launch(Dispatchers.IO) { AkiImageLoader.clear() } }
            }
        }
    }
}

private fun LazyListScope.dataSection(onBackup: () -> Unit, onRestore: () -> Unit, onComic: () -> Unit) {
    item {
        AkiSection("Your data", footer = "A backup holds your library, what you have read, categories, repositories and source settings. Sources come back from their repositories.") {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                AkiButton("Create a backup", icon = "server", fullWidth = true, onClick = onBackup)
                AkiButton("Restore from a backup", icon = "restore", fullWidth = true, onClick = onRestore)
                AkiButton("Open a comic file (CBZ)", icon = "zip", fullWidth = true, onClick = onComic)
            }
        }
    }
}

private fun LazyListScope.aboutSection() {
    item {
        val app = LocalApp.current
        AkiSection("About") {
            AkiCard(padding = 16.dp) {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                        AkiLogo(size = 44.dp)
                        AkiText("Akidev ${app.engine.version()}", AkiTextStyle.BodyStrong)
                    }
                    AkiText("Icons: Solar by 480 Design, CC BY 4.0. Type: Nunito, SIL Open Font License.", AkiTextStyle.Caption, tone = AkiTone.Muted)
                    AkiText(
                        "A reader with no content of its own. Sources are plugins from repositories you add; the people who make them, and the sites they read, are not affiliated with Akidev.",
                        AkiTextStyle.Caption, tone = AkiTone.Muted,
                    )
                }
            }
        }
    }
}
