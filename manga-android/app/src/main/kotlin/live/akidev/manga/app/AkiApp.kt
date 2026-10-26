package live.akidev.manga.app

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import live.akidev.manga.design.AkiBackdrop
import live.akidev.manga.design.AkiGlassScope
import live.akidev.manga.design.AkiTabBar
import live.akidev.manga.design.AkiTheme
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
    BackHandler(enabled = router.stack.isNotEmpty()) { router.back() }
    Box(Modifier.fillMaxSize()) {
        // The tab the person is on. Changing tab is instant, as on iOS: each tab is its own page.
        when (model.tab) {
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
        AnimatedContent(
            router.top,
            transitionSpec = {
                if (targetState != null) (slideInHorizontally(tween(320)) { it / 3 } + fadeIn(tween(220))) togetherWith fadeOut(tween(160))
                else fadeIn(tween(160)) togetherWith (slideOutHorizontally(tween(260)) { it / 3 } + fadeOut(tween(200)))
            },
            label = "pushed",
        ) { route -> if (route != null) PushedScreen(route) }
    }
}

/** The screen a route opens, over the page behind it. */
@Composable
private fun PushedScreen(route: Route) {
    Box(Modifier.fillMaxSize()) {
        AkiBackdrop()
        when (route) {
            is RepoRoute -> RepoScreen(route.repo)
            is SourceSettingsRoute -> SourceSettingsScreen(route.source)
            else -> Unit
        }
    }
}
