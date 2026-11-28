package live.akidev.tv

import android.net.Uri
import android.os.SystemClock
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.core.tween
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.util.UnstableApi
import androidx.media3.common.Tracks
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.core.content.res.ResourcesCompat
import androidx.media3.ui.CaptionStyleCompat
import androidx.media3.ui.PlayerView
import androidx.tv.material3.Text
import java.io.File
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.akitv.Episode
import uniffi.akitv.Stream
import uniffi.akitv.Subtitle
import uniffi.akitv.Title
import uniffi.akitv.detail
import uniffi.akitv.Source
import uniffi.akitv.findStream
import uniffi.akitv.findStreamOf
import uniffi.akitv.sources
import uniffi.akitv.subtitleFile
import uniffi.akitv.subtitles

private const val SEEK_MS = 10_000L
private const val HIDE_AFTER_MS = 3_500L
private const val SAVE_EVERY_MS = 5_000L
private const val MOST_EXTERNAL_SUBTITLES = 6

/**
 * Finds a copy of the film or episode, then plays it. If a copy fails, "Try another copy" asks again and leaves out the ones that failed.
 * `label` is "Episode 3" for an episode, null for a film. `onNext` replaces this screen with the next episode.
 */
@Composable
fun PlayerScreen(title: Title, playId: String, label: String?, key: String, onNext: (Episode) -> Unit, onBack: () -> Unit) {
    var tries by remember { mutableIntStateOf(0) }
    val failed = remember(playId) { mutableStateListOf<String>() }
    val opened = remember { SystemClock.elapsedRealtime() }
    // The copy the viewer picked in the panel (its hash); null leaves the choice to the engine.
    var chosen by remember(playId) { mutableStateOf<String?>(null) }
    val found by rememberLoad("stream", playId, tries, chosen) { chosen?.let { findStreamOf(playId, key, it) } ?: findStream(playId, key, failed.toList()) }
    // The list of copies is asked for once the viewer opens the panel, not before: it costs calls to TorBox.
    var sourcesWanted by remember(playId) { mutableStateOf(false) }
    val copies by rememberLoad("sources", playId, sourcesWanted) { if (sourcesWanted) sources(playId, key) else emptyList() }
    val name = if (label == null) title.name else "${title.name}  ·  $label"
    when (val state = found) {
        Load.Loading -> StateCard("Finding a copy", name)
        is Load.Failed -> StateCard("This could not be played", state.reason, icon = "warning") {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                AkiButton("Try again", Look.Primary, icon = "refresh", autofocus = true) { tries++ }
                AkiButton("Back", Look.Secondary, icon = "back") { onBack() }
            }
        }
        is Load.Done -> Playing(state.value, title, playId, label, name, opened, onNext, onBack, onFailed = { failed.add(state.value.hash); tries++ }, copies = if (sourcesWanted) copies else null, onSources = { sourcesWanted = true }, onSource = { chosen = it })
    }
}

@androidx.annotation.OptIn(UnstableApi::class) // the player view's caption style is a Media3 call that is not yet marked stable
@Composable
private fun Playing(
    stream: Stream, title: Title, playId: String, label: String?, name: String, opened: Long, onNext: (Episode) -> Unit, onBack: () -> Unit, onFailed: () -> Unit,
    copies: Load<List<Source>>?, onSources: () -> Unit, onSource: (String) -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val player = remember { val t = SystemClock.elapsedRealtime(); ExoPlayer.Builder(context).setMediaSourceFactory(DefaultMediaSourceFactory(OkHttpDataSource.Factory(HttpClient.video))).build().also { Perf.log("player built", SystemClock.elapsedRealtime() - t) }.apply { trackSelectionParameters = trackSelectionParameters.buildUpon().setPreferredTextLanguage(Locale.getDefault().language).build() } }
    val next = rememberNextEpisode(title.id, playId, label)
    // The player's listener lives as long as the stream, but the list of episodes may arrive after it started.
    val latestNext by rememberUpdatedState(next)
    val outside by rememberLoad("subtitles", playId) { subtitles(playId).take(MOST_EXTERNAL_SUBTITLES) }

    var error by remember { mutableStateOf<String?>(null) }
    var buffering by remember { mutableStateOf(true) }
    var playing by remember { mutableStateOf(true) }
    var tracks by remember { mutableStateOf(Tracks.EMPTY) }
    var position by remember { mutableLongStateOf(0) }
    var length by remember { mutableLongStateOf(0) }
    var shownAt by remember { mutableLongStateOf(System.currentTimeMillis()) }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    var panelOpen by remember { mutableStateOf(false) }
    var controlsFocused by remember { mutableStateOf(false) }
    var chosenOutside by remember { mutableStateOf<Subtitle?>(null) }
    var subtitlesNote by remember { mutableStateOf<String?>(null) }
    val screen = remember { FocusRequester() }
    val controls = remember { FocusRequester() }

    fun save(at: Long = player.currentPosition) {
        val total = player.duration
        if (total > 0 && at > 0) History.save(context, Watched(playId, title.id, title.name, title.poster, title.banner, label, at, total, System.currentTimeMillis()))
    }

    DisposableEffect(stream.url) {
        val listener = object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                buffering = state == Player.STATE_BUFFERING
                if (state == Player.STATE_READY) Perf.log("player ready after open", SystemClock.elapsedRealtime() - opened)
                if (state == Player.STATE_ENDED) {
                    save(player.duration)
                    latestNext?.let(onNext) ?: onBack()
                }
            }
            override fun onIsPlayingChanged(isPlaying: Boolean) { playing = isPlaying }
            override fun onRenderedFirstFrame() { Perf.log("player first frame after open", SystemClock.elapsedRealtime() - opened) }
            override fun onTracksChanged(newTracks: Tracks) { tracks = newTracks }
            override fun onPlayerError(e: PlaybackException) { panelOpen = false; error = "The player stopped: ${e.errorCodeName}" }
        }
        player.addListener(listener)
        // Home, or the TV going to sleep: stop the sound and keep the place.
        val lifecycle = (context as ComponentActivity).lifecycle
        val watcher = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_STOP) { save(); player.pause() } }
        lifecycle.addObserver(watcher)
        player.setMediaItem(MediaItem.fromUri(stream.url), History.load(context).resumeAt(playId))
        player.prepare()
        player.playWhenReady = true
        Perf.log("player prepared after open", SystemClock.elapsedRealtime() - opened)
        onDispose {
            save()
            lifecycle.removeObserver(watcher)
            player.removeListener(listener)
            player.release()
        }
    }
    // Twice a second: the progress bar and when to hide it. Every 5 seconds: where the viewer is, kept for next time.
    LaunchedEffect(player) {
        var lastSave = 0L
        while (true) {
            position = player.currentPosition
            length = player.duration.coerceAtLeast(0)
            now = System.currentTimeMillis()
            if (player.isPlaying && now - lastSave >= SAVE_EVERY_MS) { save(); lastSave = now }
            delay(500)
        }
    }
    LaunchedEffect(Unit) { screen.requestFocus() }
    BackHandler(panelOpen) { panelOpen = false; screen.requestFocus() }

    error?.let {
        StateCard("This could not be played", it, icon = "warning") {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                AkiButton("Try another copy", Look.Primary, icon = "refresh", autofocus = true) { onFailed() }
                AkiButton("Back", Look.Secondary, icon = "back") { onBack() }
            }
        }
        return
    }

    fun pickTrack(group: Tracks.Group) {
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(group.type, false)
            .setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, 0)).build()
    }
    fun subtitlesOff() {
        chosenOutside = null
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon().setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true).build()
    }
    // A subtitle from outside the video is fetched, saved in the cache and added by loading the same video again at the same place.
    fun pickOutside(subtitle: Subtitle) {
        subtitlesNote = "Loading the subtitle"
        scope.launch {
            val file = withContext(Dispatchers.IO) {
                runCatching { File(context.cacheDir, "subtitle.vtt").apply { writeText(subtitleFile(subtitle.url)) } }.getOrNull()
            }
            if (file == null) { subtitlesNote = "That subtitle could not be loaded."; return@launch }
            if (error != null) return@launch
            val config = MediaItem.SubtitleConfiguration.Builder(Uri.fromFile(file)).setMimeType(MimeTypes.TEXT_VTT)
                .setLanguage(subtitle.language).setSelectionFlags(C.SELECTION_FLAG_DEFAULT).build()
            val keepPlaying = player.playWhenReady
            player.trackSelectionParameters = player.trackSelectionParameters.buildUpon().setTrackTypeDisabled(C.TRACK_TYPE_TEXT, false).build()
            player.setMediaItem(MediaItem.Builder().setUri(stream.url).setSubtitleConfigurations(listOf(config)).build(), player.currentPosition)
            player.prepare()
            player.playWhenReady = keepPlaying
            chosenOutside = subtitle
            subtitlesNote = null
            panelOpen = false
            screen.requestFocus()
        }
    }

    val outsideList = (outside as? Load.Done)?.value ?: emptyList()
    val audio = tracks.groups.filter { it.type == C.TRACK_TYPE_AUDIO }.mapIndexed { i, g ->
        val f = g.getTrackFormat(0)
        Choice(trackName(f.language, f.label, i), g.isSelected) { pickTrack(g) }
    }
    val embedded = tracks.groups.filter { it.type == C.TRACK_TYPE_TEXT }
    val textShown = tracks.groups.any { it.type == C.TRACK_TYPE_TEXT && it.isSelected }
    val subtitleChoices = buildList {
        add(Choice("Off", !textShown) { subtitlesOff() })
        embedded.forEachIndexed { i, g ->
            val f = g.getTrackFormat(0)
            add(Choice(trackName(f.language, f.label, i), g.isSelected && chosenOutside == null) { chosenOutside = null; pickTrack(g) })
        }
        // Only the language: a release's file name says nothing the viewer needs. A second one in the same language gets a number.
        outsideList.forEachIndexed { i, s ->
            val same = outsideList.take(i).count { it.language == s.language }
            add(Choice(if (same == 0) s.language else "${s.language}  ·  ${same + 1}", chosenOutside == s) { pickOutside(s) })
        }
    }

    // Opening the panel asks for the copies; picking one saves where the viewer is and plays that copy from there (the screen resumes from the saved place).
    LaunchedEffect(panelOpen) { if (panelOpen) onSources() }
    val sourceList = (copies as? Load.Done)?.value.orEmpty()
    val sourceChoices = sourceList.map { Choice(it.label, it.hash == stream.hash) { save(); onSource(it.hash) } }
    val sourcesNote = when {
        copies == null || copies is Load.Loading -> "Looking for copies"
        copies is Load.Failed -> copies.reason
        sourceList.isEmpty() -> "No other copies found."
        else -> null
    }

    val barShown = !playing || controlsFocused || panelOpen || now - shownAt < HIDE_AFTER_MS
    Box(
        Modifier.fillMaxSize().background(Color.Black).focusRequester(screen).focusable().onPreviewKeyEvent { event ->
            if (panelOpen || event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
            shownAt = System.currentTimeMillis()
            val code = event.key.nativeKeyCode
            if (controlsFocused) {
                if (code == KeyEvent.KEYCODE_DPAD_UP) { screen.requestFocus(); return@onPreviewKeyEvent true }
                return@onPreviewKeyEvent false
            }
            when (code) {
                KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE -> { if (player.isPlaying) player.pause() else player.play(); true }
                KeyEvent.KEYCODE_MEDIA_PLAY -> { player.play(); true }
                KeyEvent.KEYCODE_MEDIA_PAUSE -> { player.pause(); true }
                KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> { player.seekTo(player.currentPosition + SEEK_MS); true }
                KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_MEDIA_REWIND -> { player.seekTo((player.currentPosition - SEEK_MS).coerceAtLeast(0)); true }
                // The first Down shows the bar; the next one moves the remote onto its buttons.
                KeyEvent.KEYCODE_DPAD_DOWN -> { if (barShown) controls.requestFocus(); true }
                KeyEvent.KEYCODE_DPAD_UP -> true
                else -> false
            }
        },
    ) {
        AndroidView(
            factory = {
                PlayerView(it).apply {
                    useController = false
                    keepScreenOn = true
                    this.player = player
                    // Our own caption look, not the TV's setting or the file's styles: white with a black edge, in the app's font.
                    subtitleView?.setApplyEmbeddedStyles(false)
                    subtitleView?.setApplyEmbeddedFontSizes(false)
                    subtitleView?.setStyle(CaptionStyleCompat(android.graphics.Color.WHITE, android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT, CaptionStyleCompat.EDGE_TYPE_OUTLINE, android.graphics.Color.BLACK, ResourcesCompat.getFont(it, R.font.hanken_medium)))
                    subtitleView?.setFixedTextSize(android.util.TypedValue.COMPLEX_UNIT_SP, 22f)
                }
            },
            modifier = Modifier.fillMaxSize(),
        )
        if (buffering) Spinner(Modifier.align(Alignment.Center), size = 56.dp)
        // Paused: a pause icon fades in at the middle, so it is clear the picture is held and not stuck.
        AnimatedVisibility(!playing && !buffering, Modifier.align(Alignment.Center), enter = fadeIn(), exit = fadeOut()) { AkiIcon("pause", size = 72.dp, tint = Ink.Text.copy(alpha = 0.85f)) }
        AnimatedVisibility(barShown, Modifier.align(Alignment.TopCenter), enter = fadeIn(Motion.go(260)), exit = fadeOut(Motion.go(220))) {
            Text(
                name, style = Type.RowTitle,
                modifier = Modifier.fillMaxWidth().background(Brush.verticalGradient(listOf(Color(0xCC000000), Color.Transparent))).padding(horizontal = 48.dp, vertical = 24.dp),
            )
        }
        AnimatedVisibility(barShown, Modifier.align(Alignment.BottomCenter), enter = fadeIn(Motion.go(260)) + slideInVertically(Motion.go(320)) { it / 4 }, exit = fadeOut(Motion.go(220))) {
            Column(
                Modifier.fillMaxWidth().background(Brush.verticalGradient(listOf(Color.Transparent, Color(0xCC000000)))).padding(horizontal = 48.dp, vertical = 24.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Box(Modifier.fillMaxWidth().height(4.dp).background(Ink.Line)) {
                    val share = if (length > 0) (position.toFloat() / length).coerceIn(0f, 1f) else 0f
                    Box(Modifier.fillMaxWidth(share).height(4.dp).background(Ink.Accent))
                }
                Text("${clock(position)} / ${clock(length)}", style = Type.Meta.copy(color = Ink.TextDim))
                Row(Modifier.onFocusChanged { controlsFocused = it.hasFocus }, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    AkiButton("Options", Look.Secondary, Modifier.focusRequester(controls), icon = "subtitles", tipAbove = true) { panelOpen = true }
                    next?.let { AkiButton("Next episode", Look.Secondary, icon = "next", tipAbove = true) { save(); onNext(it) } }
                }
            }
        }
        AnimatedVisibility(panelOpen, Modifier.align(Alignment.CenterEnd), enter = slideInHorizontally(Motion.go(400)) { it } + fadeIn(Motion.go(300)), exit = slideOutHorizontally(Motion.go(240)) { it } + fadeOut(Motion.go(200))) {
            TracksPanel(
                audio, subtitleChoices, subtitlesNote ?: if (outside is Load.Done && outsideList.isEmpty()) "No more subtitles were found." else null,
                sourceChoices, sourcesNote,
            )
        }
    }
}

/** The episode after this one, from the title's episode list; null for a film or the last episode. */
@Composable
private fun rememberNextEpisode(titleId: String, playId: String, label: String?): Episode? {
    val loaded by rememberLoad("episodes", titleId, label) { if (label == null) emptyList() else detail(titleId).episodes }
    val episodes = (loaded as? Load.Done)?.value ?: return null
    return nextEpisode(episodes.filter { it.released }.sortedWith(compareBy({ it.season ?: 1u }, { it.number })).map { it.id }, playId)?.let { id -> episodes.first { it.id == id } }
}

/** The id after `playId` in `ids`, or null. */
fun nextEpisode(ids: List<String>, playId: String): String? {
    val at = ids.indexOf(playId)
    return if (at < 0) null else ids.getOrNull(at + 1)
}

fun clock(ms: Long): String {
    val total = ms / 1000
    val (h, m, s) = Triple(total / 3600, total / 60 % 60, total % 60)
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
}
