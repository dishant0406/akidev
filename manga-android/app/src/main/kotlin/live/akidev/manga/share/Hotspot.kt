package live.akidev.manga.share

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSpecifier
import android.os.Build
import android.os.Handler
import android.os.Looper
import kotlinx.coroutines.delay
import kotlinx.coroutines.suspendCancellableCoroutine
import java.net.Inet4Address
import java.net.NetworkInterface
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** A private Wi-Fi this phone made: its name, its password, and this phone's address on it. */
class HotspotInfo(val ssid: String, val password: String, val address: String)

/** A phone that cannot make or join the private Wi-Fi; [message] is for the person. */
class NetworkProblem(message: String) : Exception(message)

/**
 * The address of this phone on the private Wi-Fi it made. [found] is every address of the interfaces that are up (name, address);
 * the hotspot is on a Wi-Fi interface, and it is not the one the phone's own Wi-Fi connection uses ([client], when it has one).
 */
fun hotspotAddress(found: List<Pair<String, String>>, client: String?): String? =
    found.firstOrNull { (name, address) -> address != client && listOf("wlan", "ap", "swlan", "softap").any(name::startsWith) }?.second

/** Makes a Wi-Fi with no internet, for one send, and takes it down after. Android picks its name and password. */
class Hotspot(private val context: Context) {
    private var reservation: WifiManager.LocalOnlyHotspotReservation? = null

    /** [client] is this phone's address on the Wi-Fi it is connected to, if any. */
    suspend fun start(client: String?): HotspotInfo {
        try {
            val started = begin()
            val (ssid, password) = credentials(started)
            if (ssid == null || password == null) throw NetworkProblem(CANNOT)
            // The Wi-Fi is on before it has an address: wait for it, up to 5 seconds.
            repeat(25) {
                val address = hotspotAddress(addresses(), client)
                if (address != null) return HotspotInfo(ssid, password, address)
                delay(200)
            }
            throw NetworkProblem(CANNOT)
        } catch (failure: Throwable) {
            stop()
            throw failure
        }
    }

    private suspend fun begin(): WifiManager.LocalOnlyHotspotReservation = suspendCancellableCoroutine { go ->
        val manager = context.getSystemService(WifiManager::class.java)
        val callback = object : WifiManager.LocalOnlyHotspotCallback() {
            override fun onStarted(started: WifiManager.LocalOnlyHotspotReservation) {
                reservation = started
                if (go.isActive) go.resume(started) else stop()
            }

            override fun onFailed(reason: Int) {
                if (go.isActive) go.resumeWithException(NetworkProblem(CANNOT))
            }
        }
        try {
            manager.startLocalOnlyHotspot(callback, Handler(Looper.getMainLooper()))
        } catch (failure: SecurityException) {
            // The permission was taken away, or Location is off (older Android).
            go.resumeWithException(NetworkProblem(CANNOT))
        } catch (failure: IllegalStateException) {
            go.resumeWithException(NetworkProblem(CANNOT))
        }
    }

    fun stop() {
        reservation?.close()
        reservation = null
    }

    @Suppress("DEPRECATION")
    private fun credentials(started: WifiManager.LocalOnlyHotspotReservation): Pair<String?, String?> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val config = started.softApConfiguration
            config.ssid to config.passphrase
        } else {
            val config = started.wifiConfiguration
            config?.SSID?.trim('"') to config?.preSharedKey?.trim('"')
        }

    private fun addresses(): List<Pair<String, String>> = runCatching {
        NetworkInterface.getNetworkInterfaces().toList().filter { it.isUp && !it.isLoopback }.flatMap { found ->
            found.inetAddresses.toList().filterIsInstance<Inet4Address>().mapNotNull { address -> address.hostAddress?.let { found.name to it } }
        }
    }.getOrDefault(emptyList())

    private companion object {
        const val CANNOT = "This phone cannot make a temporary Wi-Fi. Use the same Wi-Fi on both phones, or turn on this phone's hotspot and join it from the other one."
    }
}

/** Joins another phone's private Wi-Fi for one send, and leaves it after. While joined, everything this app sends goes over it. */
class WifiJoin(context: Context) {
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)
    private var callback: ConnectivityManager.NetworkCallback? = null

    /** The system asks the person once. Returns when the Wi-Fi is up. */
    suspend fun join(ssid: String, password: String) = suspendCancellableCoroutine { go ->
        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .removeCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .setNetworkSpecifier(WifiNetworkSpecifier.Builder().setSsid(ssid).setWpa2Passphrase(password).build())
            .build()
        val joining = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                connectivity.bindProcessToNetwork(network)
                if (go.isActive) go.resume(Unit)
            }

            override fun onUnavailable() {
                if (go.isActive) go.resumeWithException(NetworkProblem("Could not join their Wi-Fi."))
            }
        }
        callback = joining
        try {
            connectivity.requestNetwork(request, joining, 30_000)
        } catch (failure: RuntimeException) {
            go.resumeWithException(NetworkProblem("Could not join their Wi-Fi."))
        }
        go.invokeOnCancellation { leave() }
    }

    fun leave() {
        callback?.let {
            connectivity.bindProcessToNetwork(null)
            runCatching { connectivity.unregisterNetworkCallback(it) }
        }
        callback = null
    }
}
