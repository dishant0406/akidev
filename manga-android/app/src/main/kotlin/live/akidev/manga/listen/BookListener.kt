package live.akidev.manga.listen

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import android.media.PlaybackParams
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import live.akidev.manga.app.Problem
import live.akidev.manga.app.describe
import live.akidev.manga.books.BookSpan
import uniffi.akimanga.BookReader
import uniffi.akimanga.BookSection
import uniffi.akimanga.BookSpeech
import uniffi.akimanga.Engine
import uniffi.akimanga.VoiceSpeaker

/**
 * Reads a book aloud, one sentence at a time, from the page the person is on.
 *
 * The voice model makes the speech on this phone a few sentences ahead of the one being heard; each is written to the audio track as it
 * is ready, and the sentence heard is what the page marks. It carries on through the chapters of the book and stops at the back matter.
 */
class BookListener(
    private val context: Context,
    private val scope: CoroutineScope,
    private val reader: BookReader,
    private val engine: Engine,
    /** What the lock screen shows: the book's title and author. */
    private val title: String,
    private val author: String?,
    private val model: String,
    /** The voice of the model that reads, or null for the model's own. */
    private val voice: String?,
    private val sections: List<BookSection>,
    rate: Double,
    private val saveRate: (Double) -> Unit,
    /** Told each time another sentence starts, to move the page to it. */
    private val follow: (BookSpan) -> Unit,
    /** Told when reading ends, of itself (null) or because of a failure (what to tell the person). */
    private val ended: (Problem?) -> Unit,
) : ListenControls {
    enum class State { Loading, Playing, Paused, Stopped }

    var state by mutableStateOf(State.Loading)
        private set

    /** The sentence being heard, for the page to mark. */
    var current by mutableStateOf<BookSpan?>(null)
        private set

    /** How fast it is read, as a multiple of the voice's own pace. */
    var rate by mutableStateOf(rate)
        private set

    private class Queued(val span: BookSpan, val endFrame: Long)

    private var speaker: VoiceSpeaker? = null
    private var track: AudioTrack? = null
    private var sampleRate = 24_000

    /** The sentences written to the track and not yet heard to the end, the first being heard now. */
    private val queued = ArrayDeque<Queued>()
    private var written = 0L
    private var finishedQueueing = false

    /** Bumped by anything that changes where reading is, so what the old position still has in flight is ignored. */
    private var generation = 0
    private var producing: Job? = null
    private var watching: Job? = null
    private var idle: Job? = null
    private var pausedByFocus = false
    private val sentences = HashMap<Int, List<BookSpeech>>()
    private val audio = context.getSystemService(AudioManager::class.java)
    private var focus: AudioFocusRequest? = null
    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) = pause()
    }

    /** How many sentences the queue holds, counting the one being heard. */
    private val queueLimit = 3

    // ---- controls --------------------------------------------------------------------------------------------------------------

    /** Starts reading at `offset` in `chapter`. */
    suspend fun start(chapter: Int, offset: Int) {
        val mine = generation
        try {
            val opened = withContext(Dispatchers.Default) { engine.openSpeaker(model) }
            // Stopped while the model was loading: nothing may start behind the person's back.
            if (mine != generation) return
            speaker = opened
            sampleRate = opened.sampleRate().toInt()
            begin()
        } catch (error: Exception) {
            return stop(describe(error))
        }
        run(chapter, offset)
    }

    override fun pause() {
        if (state != State.Playing) return
        track?.pause()
        state = State.Paused
        ListenSession.update(context, false)
        // A paused voice is not kept in memory for long.
        idle = scope.launch {
            delay(180_000)
            stop()
        }
    }

    override fun play() {
        if (state != State.Paused) return
        pausedByFocus = false
        idle?.cancel()
        // A call or a change of headphones can take the audio away: carry on from the sentence that was being heard.
        track?.apply { playbackParams = PlaybackParams().setSpeed(rate.toFloat()).setPitch(1f) }?.play()
        state = State.Playing
        ListenSession.update(context, true)
    }

    fun toggle() = if (state == State.Paused) play() else pause()

    /** Sets the pace; it is kept for the next book too. */
    fun changeRate(value: Double) {
        rate = value
        saveRate(value)
        if (state == State.Playing) track?.playbackParams = PlaybackParams().setSpeed(value.toFloat()).setPitch(1f)
    }

    /** Reads on from `offset` in `chapter`, wherever it was. */
    fun jump(chapter: Int, offset: Int) {
        if (state == State.Stopped || speaker == null) return
        run(chapter, offset)
    }

    /** Goes to the sentence after or before the one being heard. */
    private fun skip(forward: Boolean) {
        val here = current ?: return
        scope.launch {
            val list = sentences(here.chapter) ?: return@launch
            val index = list.indexOfFirst { it.start.toInt() == here.start }
            if (index < 0) return@launch
            if (forward && index + 1 >= list.size) {
                val next = chapter(after = here.chapter) ?: return@launch stop()
                return@launch run(next, 0)
            }
            run(here.chapter, list[if (forward) index + 1 else maxOf(index - 1, 0)].start.toInt())
        }
    }

    override fun next() = skip(forward = true)
    override fun previous() = skip(forward = false)

    override fun stop() = stop(null)

    private fun stop(failure: Problem?) {
        if (state == State.Stopped) return
        generation += 1
        idle?.cancel()
        watching?.cancel()
        track?.apply { runCatching { pause(); flush() }; release() }
        track = null
        speaker?.close()
        speaker = null
        queued.clear()
        current = null
        state = State.Stopped
        runCatching { context.unregisterReceiver(noisy) }
        focus?.let { audio.abandonAudioFocusRequest(it) }
        ListenSession.stop(context)
        ended(failure)
    }

    // ---- audio -----------------------------------------------------------------------------------------------------------------

    private fun begin() {
        val attributes = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_SPEECH).build()
        // Room for twenty seconds, so writing a sentence never waits for the one before it to be heard.
        val made = AudioTrack.Builder()
            .setAudioAttributes(attributes)
            .setAudioFormat(AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_FLOAT).setSampleRate(sampleRate).setChannelMask(AudioFormat.CHANNEL_OUT_MONO).build())
            .setBufferSizeInBytes(sampleRate * 4 * 20).setTransferMode(AudioTrack.MODE_STREAM).build()
        track = made
        // Another app's sound takes the voice away (a call, an alarm, other music), and unplugging the headphones pauses it.
        val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN).setAudioAttributes(attributes)
            .setOnAudioFocusChangeListener(::focusChanged).build()
        focus = request
        audio.requestAudioFocus(request)
        context.registerReceiver(noisy, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY))
        ListenSession.start(context, title, author, this)
        watching = scope.launch { watch() }
    }

    /** Another app's sound pauses the voice; when a short interruption (a call, an alarm) is over, it goes on by itself. */
    private fun focusChanged(change: Int) {
        when (change) {
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> if (state == State.Playing) { pause(); pausedByFocus = true }
            AudioManager.AUDIOFOCUS_LOSS -> pause()
            AudioManager.AUDIOFOCUS_GAIN -> if (pausedByFocus) play()
        }
    }

    /** Follows what the track has played, to say which sentence is being heard. */
    private suspend fun watch() {
        while (state != State.Stopped) {
            delay(40)
            val played = track?.playbackHeadPosition?.toLong()?.and(0xFFFFFFFFL) ?: continue
            while (queued.isNotEmpty() && queued.first().endFrame <= played) queued.removeFirst()
            val front = queued.firstOrNull()
            if (front != null) {
                if (current != front.span) heard(front.span)
            } else if (finishedQueueing && state == State.Playing) stop()
        }
    }

    private fun heard(span: BookSpan) {
        current = span
        follow(span)
    }

    // ---- reading ---------------------------------------------------------------------------------------------------------------

    /** Reads from `offset` in `chapter`, dropping whatever was queued for any other place. */
    private fun run(chapter: Int, offset: Int) {
        generation += 1
        val mine = generation
        track?.apply { pause(); flush() }
        queued.clear()
        written = 0
        current = null
        finishedQueueing = false
        idle?.cancel()
        state = State.Loading
        val before = producing
        producing = scope.launch {
            before?.join()
            produce(chapter, offset, mine)
        }
    }

    /** Makes the speech of each sentence in turn and writes it, never more than `queueLimit` sentences ahead. */
    private suspend fun produce(first: Int, offset: Int, mine: Int) {
        var chapter = first
        var from = offset
        var failures = 0
        while (mine == generation) {
            val list = sentences(chapter) ?: return
            val start = list.indexOfFirst { it.start.toInt() + it.length.toInt() > from }.let { if (it < 0) list.size else it }
            for (sentence in list.drop(start)) {
                while (queued.size >= queueLimit && mine == generation) delay(100)
                val voiceOfModel = speaker
                if (mine != generation || voiceOfModel == null) return
                val samples = try {
                    withContext(Dispatchers.Default) { voiceOfModel.speak(sentence.text, voice).toFloatArray() }
                } catch (error: Exception) {
                    failures += 1
                    if (failures >= 3 && mine == generation) stop(describe(error))
                    continue
                }
                failures = 0
                if (mine != generation) return
                write(samples, pause = sentence.heading, BookSpan(chapter, sentence.start.toInt(), (sentence.start + sentence.length).toInt()), mine)
            }
            chapter = chapter(after = chapter) ?: break
            from = 0
        }
        if (mine != generation) return
        finishedQueueing = true
        if (queued.isEmpty()) stop()
    }

    private suspend fun write(samples: FloatArray, pause: Boolean, span: BookSpan, mine: Int) {
        val target = track ?: return
        // A heading is followed by a pause, as a reader would leave one.
        val silence = if (pause) (sampleRate * 0.6).toInt() else 0
        val frames = samples.copyOf(samples.size + silence)
        withContext(Dispatchers.IO) { target.write(frames, 0, frames.size, AudioTrack.WRITE_BLOCKING) }
        if (mine != generation) return
        written += frames.size
        queued.addLast(Queued(span, written))
        if (state == State.Loading) {
            target.playbackParams = PlaybackParams().setSpeed(rate.toFloat()).setPitch(1f)
            target.play()
            state = State.Playing
            ListenSession.update(context, true)
        }
    }

    /** A chapter's sentences, read once; reading stops, saying why, if the chapter cannot be read. */
    private suspend fun sentences(chapter: Int): List<BookSpeech>? {
        sentences[chapter]?.let { return it }
        return try {
            val found = withContext(Dispatchers.Default) { reader.speech(chapter.toUInt()) }
            // Only the chapters near the one being read are kept.
            sentences.keys.filter { kotlin.math.abs(it - chapter) > 1 }.forEach { sentences.remove(it) }
            sentences[chapter] = found
            found
        } catch (error: Exception) {
            stop(describe(error))
            null
        }
    }

    /** The chapter reading goes on to: the next one that is not a cover or a contents page; none at the back matter. */
    private fun chapter(after: Int): Int? {
        var next = after + 1
        while (next < sections.size && (sections[next] == BookSection.COVER || sections[next] == BookSection.CONTENTS)) next += 1
        return if (next < sections.size && sections[next] != BookSection.BACK) next else null
    }
}
