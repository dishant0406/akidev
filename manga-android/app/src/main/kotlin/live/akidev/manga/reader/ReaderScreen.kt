package live.akidev.manga.reader

import android.app.Activity
import android.content.pm.ActivityInfo
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import kotlinx.coroutines.launch
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.LocalRouter
import live.akidev.manga.app.OrientationLock
import live.akidev.manga.app.ReaderBackground
import live.akidev.manga.app.ReaderRoute
import live.akidev.manga.app.ReadingMode
import live.akidev.manga.design.*

/**
 * The reader: a long strip, or pages turned left to right or right to left, with pinch zoom. Chrome is glass and hides on tap. Where the
 * person is (chapter, page, how far down it) is saved as they read.
 */
@Composable
fun ReaderScreen(route: ReaderRoute) {
    val app = LocalApp.current
    val scope = rememberCoroutineScope()
    val view = LocalView.current
    val reader = remember(route) { ReaderModel(app, route.ref, route.chapters, route.target, scope) }
    var settings by remember { mutableStateOf(false) }
    var chapters by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) { reader.open(reader.current, reader.page, reader.restore?.offset ?: 0.0) }
    // The status and navigation bars go away with the controls.
    LaunchedEffect(reader.chrome) {
        val window = (view.context as Activity).window
        val bars = WindowCompat.getInsetsController(window, view)
        bars.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        if (reader.chrome) bars.show(WindowInsetsCompat.Type.systemBars()) else bars.hide(WindowInsetsCompat.Type.systemBars())
    }
    DisposableEffect(app.prefs.keepAwake || reader.autoScrolling) {
        view.keepScreenOn = app.prefs.keepAwake || reader.autoScrolling
        onDispose { view.keepScreenOn = false }
    }
    DisposableEffect(app.prefs.orientation) {
        val activity = view.context as Activity
        activity.requestedOrientation = when (app.prefs.orientation) {
            OrientationLock.Auto -> ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
            OrientationLock.Portrait -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            OrientationLock.Landscape -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        }
        onDispose { activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED }
    }
    DisposableEffect(Unit) {
        onDispose {
            reader.flush()
            val window = (view.context as Activity).window
            WindowCompat.getInsetsController(window, view).show(WindowInsetsCompat.Type.systemBars())
        }
    }

    AkiTheme(dark = true) {
        Box(Modifier.fillMaxSize().background(if (app.prefs.readerBackground == ReaderBackground.Black) Aki.colors.stage else Aki.colors.stageAlt)) {
            Box(Modifier.fillMaxSize().akiGlassSource()) {
                val error = reader.error
                if (reader.segments.isNotEmpty()) {
                    reader.screenWidthPx = view.width
                    reader.screenHeightPx = view.height
                    if (reader.mode == ReadingMode.Strip) StripReader(reader) else PagedReader(reader)
                } else if (error != null) {
                    AkiEmptyState(error.title, error.message, action = AkiAction("Try again") { scope.launch { reader.open(reader.current, reader.page, 0.0) } })
                }
            }
            // One loader for the whole opening: while the chapter's pages are fetched, and while the strip is put at your place.
            if (reader.error == null && (reader.segments.isEmpty() || (reader.mode == ReadingMode.Strip && reader.restoring))) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { AkiOpeningLoader(reader.chapter(reader.current)?.let { reader.label(it) }) }
            }
            AnimatedVisibility(reader.chrome, Modifier.align(Alignment.TopCenter), enter = fadeIn(), exit = fadeOut()) { TopBar(reader, onChapters = { chapters = true }, onSettings = { settings = true }) }
            AnimatedVisibility(reader.chrome && reader.currentSegment != null, Modifier.align(Alignment.BottomCenter), enter = fadeIn(), exit = fadeOut()) { BottomBar(reader, onChapters = { chapters = true }) }
        }
        if (settings) ReaderSettingsSheet(reader) { settings = false }
        if (chapters) {
            ReaderChaptersSheet(reader, onOpen = { chapter ->
                chapters = false
                // Opens where the person was in it, or at its start.
                scope.launch { reader.open(chapter.id, (chapter.page ?: 0u).toInt(), 0.0) }
            }) { chapters = false }
        }
    }
}

@Composable
private fun TopBar(reader: ReaderModel, onChapters: () -> Unit, onSettings: () -> Unit) {
    val router = LocalRouter.current
    val scrim = Aki.colors.scrimStrong
    Row(
        Modifier.fillMaxWidth().widthIn(max = 640.dp).statusBarsPadding().padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically,
    ) {
        AkiIconButton("back", "Back", onMedia = true, scrim = scrim) { router.back() }
        // The title takes the room that is left and is as wide as its words.
        Box(Modifier.weight(1f)) {
            Box(Modifier.akiGlass(AkiShapes.pill(48.dp), tint = scrim).akiPress(label = "Chapters", onClick = onChapters).padding(horizontal = 16.dp, vertical = 8.dp)) {
                Column {
                    AkiText(reader.ref.title, AkiTextStyle.CaptionStrong, tone = AkiTone.OnMedia, lines = 1)
                    reader.chapter(reader.current)?.let { AkiText(reader.label(it), AkiTextStyle.Caption, tone = AkiTone.OnMedia, lines = 1) }
                }
            }
        }
        if (reader.mode == ReadingMode.Strip) {
            AkiIconButton(if (reader.autoScrolling) "pause" else "play", if (reader.autoScrolling) "Stop auto-scroll" else "Auto-scroll", onMedia = true, scrim = scrim) { reader.autoScrolling = !reader.autoScrolling }
        }
        AkiIconButton("sliders", "Reader settings", onMedia = true, scrim = scrim, onClick = onSettings)
    }
}

@Composable
private fun BottomBar(reader: ReaderModel, onChapters: () -> Unit) {
    val scope = rememberCoroutineScope()
    val scrim = Aki.colors.scrimStrong
    val count = reader.currentSegment?.pages?.images?.size ?: 0
    Row(
        Modifier.fillMaxWidth().widthIn(max = 640.dp).navigationBarsPadding().padding(bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally), verticalAlignment = Alignment.CenterVertically,
    ) {
        reader.older(reader.current)?.let { older -> AkiIconButton("previous", "Previous chapter", onMedia = true, scrim = scrim) { scope.launch { reader.open(older.id, 0, 0.0) } } }
        AkiIconButton("list", "Chapters", onMedia = true, scrim = scrim, onClick = onChapters)
        AkiText("${minOf(reader.page + 1, maxOf(count, 1))} / $count", AkiTextStyle.Caption, Modifier.akiGlass(AkiShapes.pill(44.dp), tint = scrim).padding(horizontal = 18.dp, vertical = 12.dp), tone = AkiTone.OnMedia)
        AkiIconButton(
            if (reader.isBookmarked) "bookmark-solid" else "bookmark", if (reader.isBookmarked) "Remove bookmark" else "Bookmark this page",
            tint = if (reader.isBookmarked) Aki.colors.primary else null, onMedia = true, scrim = scrim,
        ) { reader.toggleBookmark() }
        reader.newer(reader.current)?.let { newer -> AkiIconButton("next", "Next chapter", onMedia = true, scrim = scrim) { scope.launch { reader.open(newer.id, 0, 0.0) } } }
    }
}
