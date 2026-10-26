package live.akidev.manga.app

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import live.akidev.manga.design.AkiBackdrop
import live.akidev.manga.design.AkiGlassScope
import live.akidev.manga.design.AkiTabBar
import live.akidev.manga.design.AkiTheme
import live.akidev.manga.books.BookReaderScreen
import live.akidev.manga.books.BooksBrowseScreen
import live.akidev.manga.books.BooksHomeScreen
import live.akidev.manga.books.BooksScreen
import live.akidev.manga.books.CatalogueScreen
import live.akidev.manga.books.HighlightsScreen
import live.akidev.manga.listen.VoiceChoiceScreen
import live.akidev.manga.listen.VoicesScreen
import live.akidev.manga.manga.HomeScreen
import live.akidev.manga.manga.LibraryScreen
import live.akidev.manga.manga.MangaHubScreen
import live.akidev.manga.manga.UpdatesScreen
import live.akidev.manga.manga.SeriesScreen
import live.akidev.manga.reader.ReaderScreen
import live.akidev.manga.settings.SettingsScreen
import live.akidev.manga.sources.RepoAddSheet
import live.akidev.manga.sources.RepoScreen
import live.akidev.manga.sources.SourceSettingsScreen
import live.akidev.manga.sources.SourcesScreen

/** The whole app: the tabs with the floating bar, the screens pushed over them, and the first-run question. */
@Composable
fun AkiApp(model: AppModel) {
    val router = remember { Router() }
    val dark = when (model.prefs.appearance) {
        Appearance.System -> isSystemInDarkTheme()
        Appearance.Light -> false
        Appearance.Dark -> true
    }
    CompositionLocalProvider(LocalApp provides model, LocalRouter provides router) {
        AkiTheme(dark) {
            AkiGlassScope(reduce = model.prefs.reduceGlass) {
                Box(Modifier.fillMaxSize()) {
                    AkiBackdrop()
                    Shell(model, router)
                    model.pendingRepo?.let { pending -> RepoAddSheet(pending) { model.pendingRepo = null } }
                    AnimatedVisibility(model.prefs.readerKind == null, enter = fadeIn(), exit = fadeOut()) { OnboardingScreen() }
                }
            }
        }
    }
}

@Composable
private fun Shell(model: AppModel, router: Router) {
    BackHandler(enabled = router.top != null) { router.back() }
    Box(Modifier.fillMaxSize()) {
        // The tab the person is on. Each tab is its own page; what a tab remembers (its titles, its scroll place) lives in the model.
        when (model.tab) {
            AppTab.Home -> if (model.prefs.readerKind == ReaderKind.Books) BooksHomeScreen() else HomeScreen()
            AppTab.Books -> BooksScreen()
            AppTab.Browse -> BooksBrowseScreen()
            AppTab.Library -> LibraryScreen()
            AppTab.Manga -> MangaHubScreen()
            AppTab.Updates -> UpdatesScreen()
            AppTab.Sources -> SourcesScreen()
            else -> Unit
        }
        val top = router.top
        AnimatedVisibility(top == null || !top.hidesTabBar, Modifier.align(Alignment.BottomCenter), enter = slideInVertically { it } + fadeIn(), exit = slideOutVertically { it } + fadeOut()) {
            val kind = model.prefs.readerKind
            AkiTabBar(
                model.tabs.map { it.item(kind, if (it == AppTab.Updates || it == AppTab.Manga) model.unreadTotal else 0) },
                model.tab, { model.tab = it }, Modifier.navigationBarsPadding().padding(bottom = 6.dp),
            )
        }
        // Every pushed screen stays on the stack under the one on top, so going back finds each as it was.
        for (pushed in router.stack) key(pushed) { PushedScreen(pushed, router, isTop = pushed.route === top) }
    }
}

/** A pushed screen: it slides in from the right, over the page behind it, and slides out when it is taken off the stack. */
@Composable
private fun PushedScreen(pushed: Pushed, router: Router, isTop: Boolean) {
    val slide = remember { Animatable(1f) }
    LaunchedEffect(Unit) { slide.animateTo(0f, tween(320, easing = FastOutSlowInEasing)) }
    LaunchedEffect(pushed.leaving) {
        if (pushed.leaving) {
            slide.animateTo(1f, tween(260, easing = FastOutSlowInEasing))
            router.remove(pushed)
        }
    }
    Box(
        Modifier.fillMaxSize()
            .graphicsLayer { translationX = slide.value * size.width * 0.35f; alpha = 1f - slide.value }
            // A screen under the top one is not drawn at all: it only keeps its state.
            .then(if (isTop || pushed.leaving) Modifier else Modifier.drawWithContent {}),
    ) {
        AkiBackdrop()
        when (val route = pushed.route) {
            is RepoRoute -> RepoScreen(route.repo)
            is SourceSettingsRoute -> SourceSettingsScreen(route.source)
            is SeriesRoute -> SeriesScreen(route.ref)
            is ReaderRoute -> ReaderScreen(route)
            SettingsRoute -> SettingsScreen()
            is CatalogueRoute -> CatalogueScreen(route.ref)
            VoicesRoute -> VoicesScreen()
            VoiceChoiceRoute -> VoiceChoiceScreen()
            is HighlightsRoute -> HighlightsScreen(route.open)
            is BookReaderRoute -> BookReaderScreen(route)
            else -> Unit
        }
    }
}
