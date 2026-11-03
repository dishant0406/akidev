package live.akidev.manga.share

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import java.net.Inet4Address

private const val SERVICE = "_akidev._tcp"

/**
 * Android's own way of finding phones on the Wi-Fi: it tells the network this phone is here, on `port`, and listens for others. It is
 * the same Bonjour the iPhone uses, so the two see each other. Each phone found is handed over as an address.
 */
class Nearby(context: Context, private val found: (name: String, ip: String, port: Int) -> Unit, private val lost: (name: String) -> Unit) {
    private val manager = context.getSystemService(NsdManager::class.java)
    private val waiting = ArrayDeque<NsdServiceInfo>()
    private var resolving = false
    private var ownName = ""

    private val registration = object : NsdManager.RegistrationListener {
        override fun onServiceRegistered(info: NsdServiceInfo) { ownName = info.serviceName }
        override fun onRegistrationFailed(info: NsdServiceInfo, error: Int) {}
        override fun onServiceUnregistered(info: NsdServiceInfo) {}
        override fun onUnregistrationFailed(info: NsdServiceInfo, error: Int) {}
    }

    private val discovery = object : NsdManager.DiscoveryListener {
        override fun onServiceFound(info: NsdServiceInfo) {
            if (info.serviceName == ownName) return
            synchronized(this@Nearby) { waiting.add(info) }
            resolveNext()
        }
        override fun onServiceLost(info: NsdServiceInfo) = lost(info.serviceName)
        override fun onDiscoveryStarted(type: String) {}
        override fun onDiscoveryStopped(type: String) {}
        override fun onStartDiscoveryFailed(type: String, error: Int) {}
        override fun onStopDiscoveryFailed(type: String, error: Int) {}
    }

    fun start(name: String, port: Int) {
        manager.registerService(NsdServiceInfo().apply { serviceName = name; serviceType = SERVICE; this.port = port }, NsdManager.PROTOCOL_DNS_SD, registration)
        manager.discoverServices(SERVICE, NsdManager.PROTOCOL_DNS_SD, discovery)
    }

    fun stop() {
        runCatching { manager.unregisterService(registration) }
        runCatching { manager.stopServiceDiscovery(discovery) }
    }

    /** Android resolves one service at a time, so the ones found wait their turn. */
    @Suppress("DEPRECATION")
    private fun resolveNext() {
        val next = synchronized(this) { if (resolving) null else waiting.removeFirstOrNull()?.also { resolving = true } } ?: return
        manager.resolveService(next, object : NsdManager.ResolveListener {
            override fun onServiceResolved(info: NsdServiceInfo) {
                // The engine speaks IPv4 only, as the LocalSend app does.
                (info.host as? Inet4Address)?.hostAddress?.let { found(info.serviceName, it, info.port) }
                resolved()
            }
            override fun onResolveFailed(info: NsdServiceInfo, error: Int) = resolved()
        })
    }

    private fun resolved() {
        synchronized(this) { resolving = false }
        resolveNext()
    }
}
