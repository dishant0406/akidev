package live.akidev.manga.listen

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import live.akidev.manga.app.Preferences
import live.akidev.manga.app.Problem
import live.akidev.manga.app.describe
import uniffi.akimanga.Engine
import uniffi.akimanga.VoiceModel
import uniffi.akimanga.VoiceOption
import uniffi.akimanga.VoiceProgress

/**
 * Reading aloud: the voice models the app can get, the ones on the device and the one in use.
 *
 * Models come from a voices repository. Whether it can be reached is asked when Settings is opened (never at launch), and Settings shows
 * reading aloud only if it answered; a model already on the device reads without any network.
 */
class ReadAloudModel(private val engine: Engine, private val prefs: Preferences, private val scope: CoroutineScope) {
    /** The repository answered the last time it was asked. */
    var reachable by mutableStateOf(false)
        private set

    /** What the repository offers. */
    var offered by mutableStateOf(emptyList<VoiceModel>())
        private set
    var installed by mutableStateOf(engine.voicesInstalled())
        private set

    /** How far each model being downloaded has got, 0 to 1. */
    val progress = mutableStateMapOf<String, Double>()
    var failure by mutableStateOf<Problem?>(null)

    /** The voices being fetched now, and the one being heard (empty for the model's own). */
    var fetching by mutableStateOf(emptySet<String>())
        private set
    var hearing by mutableStateOf<String?>(null)
        private set

    /** The model that reads: the one chosen, or the only one there is. */
    val current get() = installed.firstOrNull { it.id == prefs.voiceModel } ?: installed.firstOrNull()

    fun has(model: VoiceModel) = installed.any { it.id == model.id }

    /** The repository offers a newer version of a model that is on the phone. */
    fun hasUpdate(model: VoiceModel) = installed.firstOrNull { it.id == model.id }?.let { it.version < model.version } ?: false

    /** The voice that reads, when one of the model's other voices is chosen and on the phone; otherwise the model's own. */
    val chosenVoice get() = current?.voices?.firstOrNull { it.id == prefs.voiceName && it.downloaded }

    /** The id of the voice that is in use: empty for the model's own. */
    val voiceInUse get() = chosenVoice?.id ?: ""

    /** Every model to show: the repository's, then any on the device the repository no longer lists. */
    val models get() = offered + installed.filter { have -> offered.none { it.id == have.id } }

    /** Asks the repository what it offers. Called each time Settings opens. */
    suspend fun check() {
        val found = runCatching { engine.voicesOffered(REPOSITORY) }.getOrNull()
        reachable = found != null
        offered = found ?: emptyList()
        installed = engine.voicesInstalled()
    }

    suspend fun install(model: VoiceModel) {
        if (progress[model.id] != null) return
        progress[model.id] = 0.0
        try {
            engine.installVoice(REPOSITORY, model.id, object : VoiceProgress {
                override fun update(done: ULong, total: ULong) {
                    val fraction = if (total > 0u) done.toDouble() / total.toDouble() else 0.0
                    scope.launch { if (progress[model.id] != null) progress[model.id] = fraction }
                }
            })
            installed = engine.voicesInstalled()
            if (installed.size == 1) prefs.voiceModel = model.id
        } catch (error: Exception) {
            failure = describe(error)
        }
        progress.remove(model.id)
    }

    fun use(model: VoiceModel) {
        prefs.voiceModel = model.id
        prefs.voiceName = ""
    }

    /** Chooses a voice of the model in use, fetching it first if it is not on the phone yet; nothing is the model's own. */
    suspend fun choose(voice: VoiceOption?) {
        if (voice != null && !fetch(voice)) return
        prefs.voiceName = voice?.id ?: ""
    }

    /** Says a sentence in a voice (the model's own when nothing), fetching the voice first if need be. */
    suspend fun hear(voice: VoiceOption?) {
        val model = current ?: return
        if (voice != null && !fetch(voice)) return
        hearing = voice?.id ?: ""
        val heard = withContext(Dispatchers.Default) {
            runCatching {
                val speaker = engine.openSpeaker(model.id)
                speaker.speak("It is a truth universally acknowledged, that a single man in possession of a good fortune, must be in want of a wife.", voice?.id).toFloatArray() to speaker.sampleRate().toInt()
            }.getOrNull()
        }
        if (heard == null) failure = Problem("That did not work", "The voice could not be played.") else play(heard.first, heard.second)
        hearing = null
    }

    /** Plays a few seconds of speech once: what a voice sounds like, before it is chosen. */
    private suspend fun play(samples: FloatArray, rate: Int) = withContext(Dispatchers.Default) {
        val track = AudioTrack.Builder()
            .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_SPEECH).build())
            .setAudioFormat(AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_FLOAT).setSampleRate(rate).setChannelMask(AudioFormat.CHANNEL_OUT_MONO).build())
            .setBufferSizeInBytes(samples.size * 4).setTransferMode(AudioTrack.MODE_STATIC).build()
        track.write(samples, 0, samples.size, AudioTrack.WRITE_BLOCKING)
        track.setNotificationMarkerPosition(samples.size)
        track.setPlaybackPositionUpdateListener(object : AudioTrack.OnPlaybackPositionUpdateListener {
            override fun onMarkerReached(track: AudioTrack) = track.release()
            override fun onPeriodicNotification(track: AudioTrack) = Unit
        })
        track.play()
    }

    /** Fetches one of the model's voices; whether it is on the phone after. */
    private suspend fun fetch(voice: VoiceOption): Boolean {
        val model = current
        if (voice.downloaded || model == null) return true
        if (voice.id in fetching) return false
        fetching = fetching + voice.id
        try {
            engine.getVoiceChoice(model.id, voice.id)
            installed = engine.voicesInstalled()
            return true
        } catch (error: Exception) {
            failure = describe(error)
            return false
        } finally {
            fetching = fetching - voice.id
        }
    }

    fun remove(model: VoiceModel) {
        try {
            engine.removeVoice(model.id)
            installed = engine.voicesInstalled()
        } catch (error: Exception) {
            failure = describe(error)
        }
    }

    companion object {
        /** Where the models are. */
        const val REPOSITORY = "github.com/dishant0406/akidev-voices"
    }
}
