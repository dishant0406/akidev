package live.akidev.manga.books

import android.app.Activity
import android.content.Intent
import androidx.core.net.toUri
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import live.akidev.manga.app.BookReaderRoute
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.describe
import live.akidev.manga.design.*
import live.akidev.manga.listen.ListenBar

/** Opens the book and shows it, or says why it cannot be shown. */
@Composable
fun BookReaderScreen(route: BookReaderRoute) {
    val app = LocalApp.current
    val router = LocalRouter.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var model by remember { mutableStateOf<BookModel?>(null) }
    var failure by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        try {
            model = BookModel.open(app, context, route.book, scope)
        } catch (error: Exception) {
            failure = describe(error).message
        }
    }
    val opened = model
    when {
        opened != null -> BookReaderView(opened)
        failure != null -> AkiEmptyState("This book cannot be opened", failure!!, action = AkiAction("Close") { router.back() })
        else -> Box(Modifier.fillMaxSize().background(Aki.colors.background), contentAlignment = Alignment.Center) { AkiOpeningLoader(route.book.title) }
    }
}

/** The reader for a book: the page fills the screen; the bars are glass and come and go with a tap in the middle. */
@Composable
private fun BookReaderView(model: BookModel) {
    val app = LocalApp.current
    val router = LocalRouter.current
    val view = LocalView.current
    val dark = isSystemInDarkTheme()
    var settings by remember { mutableStateOf(false) }
    var contents by remember { mutableStateOf(false) }
    var searching by remember { mutableStateOf(false) }

    model.systemDark = dark
    // The page is laid out for the screen, and for the room the system's bars take whether or not they are showing.
    LaunchedEffect(view.width, view.height) {
        val bars = ViewCompat.getRootWindowInsets(view)?.getInsetsIgnoringVisibility(WindowInsetsCompat.Type.systemBars())
        model.resize(view.width, view.height, view.resources.displayMetrics.density, bars?.top ?: 0, bars?.bottom ?: 0)
    }
    LaunchedEffect(model.chromeVisible) {
        val bars = WindowCompat.getInsetsController((view.context as Activity).window, view)
        bars.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        if (model.chromeVisible) bars.show(WindowInsetsCompat.Type.systemBars()) else bars.hide(WindowInsetsCompat.Type.systemBars())
    }
    DisposableEffect(app.prefs.keepAwake) {
        view.keepScreenOn = app.prefs.keepAwake
        onDispose { view.keepScreenOn = false }
    }
    DisposableEffect(Unit) {
        onDispose {
            model.close()
            WindowCompat.getInsetsController((view.context as Activity).window, view).show(WindowInsetsCompat.Type.systemBars())
        }
    }

    AkiTheme(dark = model.theme.isDark) {
        Box(Modifier.fillMaxSize().background(model.theme.paper)) {
            // The page and the bars: the bars show the page under them.
            Box(Modifier.fillMaxSize().akiGlassSource()) {
                Box(Modifier.fillMaxSize().then(if (model.style.ink) Modifier.grey() else Modifier)) {
                    if (model.style.scroll) BookScroller(model) else BookPager(model)
                }
                BookComfortOverlay(model.style)
                if (model.place == null) Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { AkiOpeningLoader(model.info.title) }
            }
            AnimatedVisibility(model.chromeVisible, Modifier.align(Alignment.TopCenter), enter = fadeIn(), exit = fadeOut()) { TopBar(model, onContents = { contents = true }, onSearch = { searching = true }, onSettings = { settings = true }) }
            Column(Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (model.trail.isNotEmpty()) AkiButton("Back to where you were", icon = "back", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Small) { model.goBack() }
                model.listener?.let { ListenBar(it) }
                AnimatedVisibility(model.chromeVisible, enter = fadeIn(), exit = fadeOut()) { BottomBar(model, onContents = { contents = true }) }
            }
        }
        if (settings) BookSettingsSheet(model) { settings = false }
        if (contents) BookContentsSheet(model) { contents = false }
        if (searching) BookSearchSheet(model) { searching = false }
        model.noteDraft?.let { draft -> BookNoteSheet(model, draft) { model.noteDraft = null } }
        model.footnote?.let { note -> BookFootnoteSheet(model, note) { model.footnote = null } }
        model.failure?.let { AkiAlert(it.title, it.message) { model.failure = null } }
        model.externalLink?.let { link ->
            AkiConfirm("Open this link?", link, "Open", { model.externalLink = null }) { view.context.startActivity(Intent(Intent.ACTION_VIEW, link.toUri())) }
        }
    }
}

/** A grey page: the paper-like display has no colour. */
private fun Modifier.grey() = graphicsLayer { colorFilter = ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0f) }) }

@Composable
private fun TopBar(model: BookModel, onContents: () -> Unit, onSearch: () -> Unit, onSettings: () -> Unit) {
    val router = LocalRouter.current
    val app = LocalApp.current
    AkiReaderTopBar {
        AkiIconButton("back", "Back") { router.back() }
        AkiBarTitle(model.info.title, model.info.authors.firstOrNull(), "Contents", onContents)
        if (model.listener == null && app.readAloud.current != null) AkiIconButton("headphones", "Read aloud") { model.listen() }
        AkiIconButton("search", "Search in this book", onClick = onSearch)
        AkiIconButton("sliders", "Type and theme", onClick = onSettings)
    }
}

@Composable
private fun BottomBar(model: BookModel, onContents: () -> Unit) {
    Row(Modifier.fillMaxWidth().widthIn(max = 640.dp).padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        AkiIconButton("list", "Contents", onClick = onContents)
        Row(
            Modifier.weight(1f).height(44.dp).akiGlass(AkiShapes.pill(44.dp)).padding(horizontal = 18.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically,
        ) {
            AkiSlider(model.progress, { value -> model.progress = value }, Modifier.weight(1f), onEnd = { model.goToProgress(model.progress) })
            AkiText("${Math.round(model.progress * 100)}%", AkiTextStyle.Caption, Modifier.widthIn(min = 40.dp), lines = 1, align = androidx.compose.ui.text.style.TextAlign.End)
        }
    }
}

