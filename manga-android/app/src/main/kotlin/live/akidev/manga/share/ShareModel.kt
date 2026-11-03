package live.akidev.manga.share

import android.content.Context
import android.net.ConnectivityManager
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.Problem
import live.akidev.manga.app.Sharing
import live.akidev.manga.app.describe
import java.net.Inet4Address
import uniffi.akimanga.Share
import uniffi.akimanga.ShareEvents
import uniffi.akimanga.ShareFile
import uniffi.akimanga.ShareOffered
import uniffi.akimanga.ShareOutcome
import uniffi.akimanga.SharePeer
import uniffi.akimanga.ShareReceived
import uniffi.akimanga.ShareSend

/** What the sheet shows while sending. */
sealed interface SendState {
    data object Choosing : SendState
    class Scanning(val denied: Boolean = false) : SendState
    class Waiting(val to: SharePeer) : SendState
    class Sending(val to: SharePeer, val fraction: Double) : SendState
    class Done(val to: SharePeer) : SendState
    class Failed(val message: String) : SendState
}

/** A send that is coming in, from the question to the last byte. */
class Incoming(val session: String, val from: String, val files: List<ShareOffered>) {
    var accepted by mutableStateOf(false)
    var fraction by mutableStateOf(0.0)
}

/** One time the Share sheet is open: this phone is visible to the others and ready to receive, and may be sending too. */
class ShareModel(private val app: AppModel, private val sharing: Sharing) : ShareEvents {
    private val context: Context = app.appContext
    private var share: Share? = null
    private var nearby: Nearby? = null
    private var lock: WifiManager.MulticastLock? = null
    private var files = emptyList<ShareFile>()
    private var sender: ShareSend? = null
    private var lookup: Job? = null
    private var probing = false
    private var badCode = ""
    private var badAt = 0L
    private val byName = mutableMapOf<String, SharePeer>()

    /** Opened to receive only, with nothing to send. */
    val receiving = sharing.files == null
    val alias: String = Settings.Global.getString(context.contentResolver, "device_name") ?: Build.MODEL
    var peers by mutableStateOf(emptyList<SharePeer>())
        private set
    var state by mutableStateOf<SendState>(SendState.Choosing)
        private set
    var incoming by mutableStateOf<Incoming?>(null)
        private set
    /** What came in and was added, once a receive is over. */
    var received by mutableStateOf<String?>(null)
        private set
    var message by mutableStateOf<Problem?>(null)
    /** What the other phone scans to find this one; none without Wi-Fi. Only true to read once [started]. */
    var code by mutableStateOf<String?>(null)
        private set
    var started by mutableStateOf(false)
        private set
    /** Nobody was found for a while. */
    var lonely by mutableStateOf(false)
        private set
    /** The camera is reading something that is not our code. */
    var wrongCode by mutableStateOf(false)
        private set
    /** False until the files to send are ready (a comic is packed first). */
    var ready by mutableStateOf(sharing.files == null)
        private set

    val label: String? get() = sharing.label

    suspend fun open() {
        val engine = app.engine
        val opened = runCatching { engine.share(alias, Build.MODEL, "mobile", wifiAddress(), this) }.getOrElse { message = describe(it); return }
        share = opened
        sharing.files?.let { make ->
            runCatching { app.io(make) }.onSuccess { files = it; ready = true }.onFailure { message = describe(it) }
        }
        val port = runCatching { opened.start() }.getOrElse { message = describe(it); return }
        code = opened.code()
        started = true
        // Android drops multicast packets to save battery unless an app asks, so the lock is held only while this sheet is open.
        lock = context.applicationContext.getSystemService(WifiManager::class.java).createMulticastLock("akidev").apply { setReferenceCounted(false); acquire() }
        nearby = Nearby(context, { name, ip, found -> app.launch { find(name, ip, found) } }, { name -> app.launch { byName.remove(name)?.let { gone -> peers = peers - gone } } })
            .also { it.start(alias.take(40), port.toInt()) }
        runCatching { opened.listen() }
        // The quick ways found nobody: say so quietly, and look at every address on the Wi-Fi once.
        if (!receiving) lookup = app.launch {
            delay(8000)
            if (peers.isEmpty()) {
                lonely = true
                share?.scan()
            }
        }
    }

    fun close() {
        lookup?.cancel()
        nearby?.stop()
        lock?.release()
        share?.stop()
        sender?.stop()
        app.engine.shareClearOutgoing()
    }

    private suspend fun find(name: String, ip: String, port: Int) {
        val phone = runCatching { share?.probe(ip, port.toUShort(), "https") }.getOrNull() ?: return
        byName[name] = phone
        add(phone)
    }

    private fun add(peer: SharePeer) {
        if (peers.none { it.ip == peer.ip && it.port == peer.port }) peers = peers + peer
    }

    fun startScanning() {
        wrongCode = false
        state = SendState.Scanning()
    }

    fun cameraDenied() {
        state = SendState.Scanning(denied = true)
    }

    /** The camera read a code: if it is ours, the phone it points to gets the send at once. */
    suspend fun codeScanned(text: String) {
        val now = System.currentTimeMillis()
        if (probing || state !is SendState.Scanning || (text == badCode && now - badAt < 2000)) return
        probing = true
        val peer = runCatching { share?.probeCode(text) }.getOrNull()
        probing = false
        if (peer == null) {
            badCode = text
            badAt = now
            wrongCode = true
        } else {
            add(peer)
            send(peer)
        }
    }

    fun send(to: SharePeer) {
        val opened = share ?: return
        val now = opened.sender(to, files).also { sender = it }
        state = SendState.Waiting(to)
        app.launch {
            val poll: Job = app.launch {
                while (true) {
                    delay(200)
                    val progress = now.progress()
                    if (progress.total > 0u) state = SendState.Sending(to, progress.done.toDouble() / progress.total.toDouble())
                }
            }
            val ended = try {
                when (now.run()) {
                    ShareOutcome.SENT -> SendState.Done(to)
                    ShareOutcome.DECLINED -> SendState.Failed("${to.alias} said no, or did not answer in time.")
                    ShareOutcome.BUSY -> SendState.Failed("${to.alias} is busy. Try again in a moment.")
                    ShareOutcome.STOPPED -> SendState.Choosing
                }
            } catch (failure: Exception) {
                SendState.Failed(describe(failure).message)
            }
            poll.cancel()
            state = ended
        }
    }

    fun stopSending() {
        sender?.stop()
    }

    fun again() {
        state = SendState.Choosing
    }

    fun answer(accept: Boolean) {
        val asked = incoming ?: return
        share?.answer(asked.session, accept)
        if (accept) asked.accepted = true else incoming = null
    }

    fun stopReceiving() {
        incoming?.let { share?.stopReceiving(it.session) }
        incoming = null
    }

    // The engine tells these from its own threads.

    override fun asked(session: String, from: String, files: List<ShareOffered>) {
        app.launch { incoming = Incoming(session, from, files) }
    }

    override fun progress(session: String, done: ULong, total: ULong) {
        app.launch { incoming?.takeIf { it.session == session }?.fraction = if (total == 0uL) 0.0 else done.toDouble() / total.toDouble() }
    }

    override fun finished(session: String, files: List<ShareReceived>) {
        app.launch {
            incoming = null
            val said = files.map { app.takeIn(it.path, it.name) }.joinToString("\n")
            share?.clean()
            if (said.isNotBlank()) received = said
        }
    }

    override fun ended(session: String, reason: String) {
        app.launch {
            // Saying no is the person's own doing; the rest is news.
            if (incoming?.session == session && reason != "declined") message = Problem("Stopped", "${incoming?.from} — $reason.")
            if (incoming?.session == session) incoming = null
        }
    }

    override fun seen(peer: SharePeer) {
        app.launch { add(peer) }
    }

    /** This phone's address on the Wi-Fi: the other phone reaches it there, and the QR code carries it. */
    private fun wifiAddress(): String? {
        val connectivity = context.getSystemService(ConnectivityManager::class.java)
        val links = connectivity.getLinkProperties(connectivity.activeNetwork) ?: return null
        return links.linkAddresses.map { it.address }.firstOrNull { it is Inet4Address }?.hostAddress
    }
}
