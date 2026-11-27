package live.akidev.tv

import kotlinx.coroutines.launch
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.Lifecycle
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.DisposableEffect
import android.os.Bundle
import android.os.Process
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.delay
import coil3.ImageLoader
import coil3.SingletonImageLoader
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import coil3.request.crossfade
import uniffi.akitv.Title
import uniffi.akitv.Row as HomeRow
import uniffi.akitv.homeRows
import uniffi.akitv.search
import uniffi.akitv.checkDefinitions
import uniffi.akitv.definitionsWaiting
import uniffi.akitv.sourcesVersion
import uniffi.akitv.tabs
import uniffi.akitv.start

/** Where the viewer is. The Back button on the remote takes the top one off the stack. */
sealed interface Screen {
    data object Home : Screen
    data object Search : Screen
    data object Settings : Screen
    data class Detail(val title: Title) : Screen
    /** `label` is "Episode 3" for an episode and null for a film. */
    data class Play(val title: Title, val playId: String, val label: String?) : Screen
}

/** How often the app looks whether newer definitions have been saved (a read of a number in memory, no network). */
private const val UPDATE_CHECK_MS = 5_000L

/** How long typing must pause before suggestions are asked for, the fewest letters that get any, and how many show. */
private const val SUGGEST_PAUSE_MS = 600L
private const val MIN_LETTERS_TO_SUGGEST = 3
private const val MOST_SUGGESTIONS = 6

class MainActivity : ComponentActivity() {
    override fun onResume() {
        super.onResume()
        Motion.refresh(this)
        // Coming back to the app (also from standby): look for newer definitions now, not at the next check of the loop.
        checkDefinitions()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        Motion.refresh(this)
        Perf.once("activity created")
        super.onCreate(savedInstanceState)
        start(filesDir.absolutePath)
        // Every poster comes from one host and the library allows 5 downloads per host at a time: the big backdrop of the hero waited
        // behind about 30 posters (30 seconds on the TV). `HttpClient.images` allows more, and pictures fade in when they arrive.
        SingletonImageLoader.setSafe { context ->
            ImageLoader.Builder(context)
                .components {
                    add(OkHttpNetworkFetcherFactory(callFactory = { HttpClient.images }))
                    add(UpgradeToHttps())
                    add(RetryOnce())
                }
                .crossfade(Motion.ms(280))
                .build()
        }
        setContent { App() }
    }
}

@Composable
private fun App() {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val stack = remember { mutableStateListOf<Screen>(Screen.Home) }
    val key = remember { mutableStateOf(Key.get(context)) }
    fun back() { stack.removeAt(stack.lastIndex) }

    // Kept here, not inside the screens, so Back returns to the same rows, scroll position and search.
    var homeTries by remember { mutableIntStateOf(0) }
    // Stale while revalidating: this run uses the definitions it started with (the saved copy, or the built-in one), so the tabs and rows never
    // change under the viewer. Newer ones are fetched in the background and saved; the next start uses them. When one is saved, a small dot
    // shows on Settings (and Settings says so) until then.
    val definitions = remember { sourcesVersion() }
    val sourcesReady by produceState(false) {
        while (!value) {
            value = definitionsWaiting()
            delay(UPDATE_CHECK_MS)
        }
    }
    // The app itself: what an earlier check found shows at once; the check (at the start and when the app comes back, at most every 30
    // minutes) runs in the background, together with the one for the sources. Settings offers the download.
    var appRelease by remember { mutableStateOf(Updates.cached(context)) }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle) {
        val watcher = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_RESUME) scope.launch { appRelease = Updates.check(context) } }
        lifecycle.addObserver(watcher)
        onDispose { lifecycle.removeObserver(watcher) }
    }
    val updateReady = sourcesReady || appRelease != null
    val tabs = remember(definitions) { barTabs(tabs()) }
    // The rows come one by one (see `loadHomeRows`): the names are in the definitions, the titles are asked for in order, a few at a time.
    val rowStates = remember(definitions, homeTries) { mutableStateListOf<Load<HomeRow>>().apply { repeat(homeRows().size) { add(Load.Loading) } } }
    LaunchedEffect(rowStates) { loadHomeRows(rowStates) }
    val rows = homeRowsOf(rowStates)
    LaunchedEffect(rows is Load.Done) { if (rows is Load.Done) Perf.once("home rows on screen") }
    val homeMemory = remember { HomeMemory() }
    // The bar lists the tabs, then Search and Settings. Picking one swaps the first screen of the stack (it cross-fades) and keeps the bar.
    val barItems = remember(tabs) { tabs + BarTab("Search", "search", null) + BarTab("Settings", "settings", null) }
    var barIndex by remember { mutableIntStateOf(0) }
    val picked = barIndex.coerceIn(0, barItems.lastIndex)
    val tab = tabs[picked.coerceAtMost(tabs.lastIndex)]
    fun pick(index: Int) {
        barIndex = index
        stack[0] = when (index) { tabs.size -> Screen.Search; tabs.size + 1 -> Screen.Settings; else -> Screen.Home }
    }
    // Back on Search or Settings returns to Home; Back on Home leaves the app.
    BackHandler(enabled = stack.size > 1 || stack[0] != Screen.Home) { if (stack.size > 1) back() else pick(0) }
    var typed by remember { mutableStateOf("") }
    // The search runs when the viewer presses Search on the keyboard or the button, not on every letter.
    var asked by remember { mutableStateOf("") }
    var searches by remember { mutableIntStateOf(0) }
    // Suggestions while typing, as on the desktop: a pause after the last letter, and the newest word wins (a new word restarts this effect,
    // and the screen reads only the answer to the word it asks now). Only names, no results: the results change when the viewer sends the
    // search. Short words are not asked: the catalogue of anime allows 30 requests a minute.
    var suggestFor by remember { mutableStateOf("") }
    LaunchedEffect(typed) {
        val term = typed.trim()
        if (term.length < MIN_LETTERS_TO_SUGGEST) suggestFor = ""
        else { delay(SUGGEST_PAUSE_MS); suggestFor = term }
    }
    val suggestions by rememberLoad("suggest", suggestFor) {
        if (suggestFor.isBlank()) emptyList()
        else {
            // At background priority: the keyboard is another app on the same three slow cores, and it stuttered while this ran.
            Process.setThreadPriority(Process.THREAD_PRIORITY_BACKGROUND)
            try { search(suggestFor).map { it.name }.distinct().take(MOST_SUGGESTIONS) } finally { Process.setThreadPriority(Process.THREAD_PRIORITY_DEFAULT) }
        }
    }
    val results by rememberLoad("search", asked, searches) { if (asked.isBlank()) emptyList() else search(asked) }
    // What was watched is read again each time the viewer comes back from another screen.
    var watched by remember { mutableStateOf(History.load(context)) }
    LaunchedEffect(stack.size) { watched = History.load(context) }

    fun play(title: Title, playId: String, label: String?) {
        if (key.value.isEmpty()) stack.add(Screen.Settings) else stack.add(Screen.Play(title, playId, label))
    }

    // A screen opened slides in a little from the right while the old one fades; Back does the reverse. The slide is short and small:
    // it says "this came from there" without hiding the screen.
    val previousDepth = remember { intArrayOf(stack.size) }
    val deeper = stack.size >= previousDepth[0]
    SideEffect { previousDepth[0] = stack.size }
    Box(Modifier.fillMaxSize()) {
    AnimatedContent(
        targetState = stack.last(),
        transitionSpec = {
            if (stack.size == 1 && previousDepth[0] == 1) fadeIn(Motion.go(340)) togetherWith fadeOut(Motion.go(200))
            else if (deeper) (fadeIn(Motion.go(320, delay = 80)) + slideInHorizontally(Motion.go(420)) { it / 12 }) togetherWith fadeOut(Motion.go(200))
            else fadeIn(Motion.go(280)) togetherWith (fadeOut(Motion.go(240)) + slideOutHorizontally(Motion.go(300)) { it / 12 })
        },
        label = "screen",
    ) { screen ->
    when (screen) {
        Screen.Home -> HomeScreen(
            rows = rows,
            memory = homeMemory,
            tab = tab,
            watched = watched,
            onRetry = { homeTries++ },
            onOpen = { stack.add(Screen.Detail(it)) },
            onResume = { play(it.asTitle(), it.playId, it.label) },
        )
        Screen.Search -> SearchScreen(
            typed, { typed = it }, asked, { asked = typed.trim(); searches++ }, results, (suggestions as? Load.Done)?.value.orEmpty(),
            onPick = { name -> typed = name; asked = name; searches++ }, onOpen = { stack.add(Screen.Detail(it)) },
        )
        Screen.Settings -> SettingsScreen(sourcesReady, appRelease, onSaved = { key.value = Key.get(context); if (stack.size > 1) back() else pick(0) })
        is Screen.Detail -> DetailScreen(screen.title, watched, onPlay = ::play)
        is Screen.Play -> PlayerScreen(
            screen.title, screen.playId, screen.label, key.value,
            onNext = { next -> stack[stack.lastIndex] = Screen.Play(screen.title, next.id, next.label()) },
            onBack = ::back,
        )
    }
    }
    AnimatedVisibility(stack.size == 1, enter = fadeIn(Motion.go(320, delay = 80)), exit = fadeOut(Motion.go(200))) { TopBar(barItems, picked, ::pick, badgeOnLast = updateReady) }
    }
}
