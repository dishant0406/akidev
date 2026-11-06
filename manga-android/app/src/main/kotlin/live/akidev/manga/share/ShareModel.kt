package live.akidev.manga.share

import android.content.Context
import android.net.ConnectivityManager
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.PendingCard
import live.akidev.manga.app.Problem
import live.akidev.manga.app.Sharing
import live.akidev.manga.app.describe
import java.net.Inet4Address
import uniffi.akimanga.Share
import uniffi.akimanga.ShareCode
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
    /** Joining the other phone's private Wi-Fi, after a code that has one. */
    data object Joining : SendState
    /** On the same network as the sender, waiting for it to start sending (this phone is receiving). */
    class Connected(val to: SharePeer) : SendState
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
    private var scanJob: Job? = null
    private var badCode = ""
    private var badAt = 0L
    private val hotspot = Hotspot(context)
    private val joiner = WifiJoin(context)
    private var limit: Job? = null
    /** The code for the Wi-Fi both phones are on; [code] is this, or the private Wi-Fi's code while there is one. */
    private var wifiCode: String? = null
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
    /** The series that came from a source this phone does not have: the person is asked before the source is added. */
    var newSource by mutableStateOf<PendingCard?>(null)
        private set
    private val waiting = mutableListOf<PendingCard>()
    private var retry: PendingCard? = null
    /** What the other phone scans to find this one; none without Wi-Fi. Only true to read once [started]. */
    var code by mutableStateOf<String?>(null)
        private set
    var started by mutableStateOf(false)
        private set
    /** Nobody was found for a while. */
    var lonely by mutableStateOf(false)
        private set
    /** What the scanner says under the camera when the last code could not be used. */
    var scanNote by mutableStateOf<String?>(null)
        private set
    /** The private Wi-Fi this phone made, while it is up. */
    var network by mutableStateOf<HotspotInfo?>(null)
        private set
    /** The private Wi-Fi is being made. */
    var making by mutableStateOf(false)
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
        wifiCode = opened.makeCode(null, null, null, null)
        code = wifiCode
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
        endNetwork()
        scanJob?.cancel()
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
        // On the private Wi-Fi this phone made, the one phone that joins is who the send is for.
        if (network != null && !receiving && state is SendState.Choosing) send(peer)
    }

    // The private Wi-Fi, for when the phones share none.

    /** Makes the private Wi-Fi and shows its code in place of the Wi-Fi one. The permission it needs was asked for already. */
    suspend fun makeNetwork() {
        val opened = share ?: return
        if (network != null || making) return
        making = true
        try {
            val info = hotspot.start(wifiAddress())
            val text = opened.makeCode(info.address, info.ssid, info.password, null)
            if (text == null) {
                hotspot.stop()
                message = Problem("Temporary Wi-Fi", "This phone could not make a code for the temporary Wi-Fi.")
            } else {
                network = info
                code = text
                // A private Wi-Fi is for one send: it does not stay up for ever.
                limit = app.launch {
                    delay(5 * 60_000L)
                    endNetwork()
                }
            }
        } catch (failure: NetworkProblem) {
            message = Problem("Temporary Wi-Fi", failure.message.orEmpty())
        }
        making = false
    }

    fun networkDenied() {
        val which = if (Build.VERSION.SDK_INT >= 33) "Nearby devices" else "Location"
        message = Problem("Temporary Wi-Fi", "Allow $which for Akidev in Settings to make a temporary Wi-Fi.")
    }

    /** Takes down the Wi-Fi this phone made, or leaves the one it joined, and goes back to the Wi-Fi code. */
    fun endNetwork() {
        limit?.cancel()
        limit = null
        hotspot.stop()
        joiner.leave()
        if (network != null) {
            network = null
            code = wifiCode
        }
    }

    fun startScanning() {
        scanNote = null
        state = SendState.Scanning()
    }

    fun cameraDenied() {
        state = SendState.Scanning(denied = true)
    }

    /** The camera read a code (it reads the same one over and over): if it is ours, the phone it points to is reached at once. */
    fun codeScanned(text: String) {
        app.launch {
            val now = System.currentTimeMillis()
            if (probing || state !is SendState.Scanning || (text == badCode && now - badAt < 2000)) return@launch
            probing = true
            scanJob = currentCoroutineContext()[Job]
            try {
                val code = runCatching { share?.readCode(text) }.getOrNull()
                if (code == null) {
                    badCode = text
                    badAt = now
                    scanNote = "That is not an Akidev code"
                } else {
                    reach(code, text)
                }
            } finally {
                probing = false
            }
        }
    }

    /** Joins the private Wi-Fi the code has, finds the phone it points to, then sends to it, or waits for it to send. */
    private suspend fun reach(code: ShareCode, text: String) {
        val ssid = code.ssid
        val password = code.password
        if (ssid != null && password != null) {
            state = SendState.Joining
            try {
                joiner.join(ssid, password)
            } catch (failure: NetworkProblem) {
                state = SendState.Failed(failure.message.orEmpty())
                return
            }
        }
        // A new Wi-Fi can take a while to give this phone an address: ten tries, about 15 seconds.
        val tries = if (ssid != null) 10 else 1
        var peer: SharePeer? = null
        if (code.ip != null) {
            for (attempt in 1..tries) {
                peer = try {
                    share?.probeCode(text)
                } catch (failure: CancellationException) {
                    throw failure
                } catch (failure: Exception) {
                    null
                }
                if (peer != null) break
                if (attempt < tries) delay(1500)
            }
        }
        if (peer == null) {
            joiner.leave()
            state = SendState.Failed(
                if (code.peer != null) "That phone has no shared Wi-Fi with you. Tap “No shared Wi-Fi?” on this phone and ask them to scan your code."
                else "Could not reach that phone.",
            )
        } else {
            add(peer)
            if (receiving) state = SendState.Connected(peer) else send(peer)
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
                    ShareOutcome.SENT -> SendState.Done(to).also { endNetwork() }
                    ShareOutcome.DECLINED -> SendState.Failed("${to.alias} said no.")
                    ShareOutcome.BUSY -> SendState.Failed("${to.alias} is busy right now.")
                    ShareOutcome.STOPPED -> SendState.Choosing
                }
            } catch (failure: Exception) {
                SendState.Failed(describe(failure).message)
            }
            poll.cancel()
            state = ended
        }
    }

    /** Stops a send, or leaves a Wi-Fi joined to wait for one. */
    fun stopSending() {
        sender?.stop()
        if (state is SendState.Connected || state is SendState.Joining) {
            scanJob?.cancel()
            endNetwork()
            state = SendState.Choosing
        }
    }

    fun again() {
        state = SendState.Choosing
        // The phone that joined this phone's private Wi-Fi is still there: send to it again.
        if (network != null && !receiving) peers.lastOrNull()?.let(::send)
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
            // The "got it" reply to the other phone is still on its way: taking the Wi-Fi down now would cut it.
            app.launch { delay(3_000); endNetwork() }
            val said = files.map { app.takeIn(it.path, it.name) { card -> waiting.add(card) } }.filter { it.isNotBlank() }.joinToString("\n")
            share?.clean()
            if (said.isNotBlank()) received = said
            askNextSource()
        }
    }

    private fun askNextSource() {
        if (newSource == null && waiting.isNotEmpty()) newSource = waiting.removeAt(0)
    }

    /** The person's answer to "Add the source?". */
    fun answerSource(accept: Boolean) {
        val pending = newSource ?: return
        newSource = null
        app.launch {
            if (accept) {
                try {
                    app.engine.shareAddCard(pending.path)
                    app.refresh()
                    received = "\"${pending.card.title}\" is in your library."
                } catch (failure: Exception) {
                    val problem = describe(failure)
                    if (problem.message.contains("could not reach")) {
                        // Nothing was added, and the card is kept: a source is downloaded, so this phone needs the internet.
                        message = Problem("Share", "Could not add the source: this phone needs the internet to download it. Connect to Wi-Fi or mobile data, then tap Add again.")
                        retry = pending
                        return@launch
                    }
                    message = problem
                }
            }
            java.io.File(pending.path).delete()
            askNextSource()
        }
    }

    /** The person closed a message. When a source could not be added for want of the internet, its question comes back. */
    fun closeMessage() {
        message = null
        val again = retry ?: return
        retry = null
        newSource = again
    }

    override fun ended(session: String, reason: String) {
        app.launch {
            // Saying no is the person's own doing; the rest is news.
            if (incoming?.session == session && reason != "declined" && reason != "stopped on this phone") message = Problem("Share", "The send stopped: $reason.")
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
