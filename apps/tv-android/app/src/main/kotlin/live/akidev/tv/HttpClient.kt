package live.akidev.tv

import coil3.intercept.Interceptor
import coil3.request.ErrorResult
import coil3.request.ImageResult
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.delay
import java.net.Inet6Address
import java.net.InetAddress
import okhttp3.Dispatcher
import okhttp3.Dns
import okhttp3.OkHttpClient

/**
 * Tries a host's IPv4 addresses before its IPv6 ones. On this kind of home network the IPv6 address of a host is given but does not
 * work, and the client waited for the whole connect timeout (15 s) on it before trying the IPv4 one: every picture from those hosts took 15 s.
 */
private val Ipv4First = object : Dns {
    override fun lookup(hostname: String): List<InetAddress> = Dns.SYSTEM.lookup(hostname).sortedBy { it is Inet6Address }
}

/** The app's web clients: one for pictures (many at once, from one host) and one for the video (one long download). */
object HttpClient {
    val images: OkHttpClient = OkHttpClient.Builder()
        .dispatcher(Dispatcher().apply { maxRequests = 32; maxRequestsPerHost = 16 })
        .dns(Ipv4First)
        .connectTimeout(8, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .build()

    val video: OkHttpClient = OkHttpClient.Builder()
        .dns(Ipv4First)
        .connectTimeout(8, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .build()
}

/** A picture that failed (a dropped connection, a busy host) is asked for once more after a moment, before the tile gives up. */
class RetryOnce : Interceptor {
    override suspend fun intercept(chain: Interceptor.Chain): ImageResult {
        val first = chain.proceed()
        if (first !is ErrorResult) return first
        delay(1_500)
        return chain.proceed()
    }
}

/** A picture address a provider gives with `http://` is asked for with `https://`: Android does not let an app use plain HTTP. */
fun httpsOf(address: String): String = if (address.startsWith("http://")) "https://" + address.removePrefix("http://") else address

class UpgradeToHttps : Interceptor {
    override suspend fun intercept(chain: Interceptor.Chain): ImageResult {
        val address = chain.request.data as? String ?: return chain.proceed()
        if (httpsOf(address) == address) return chain.proceed()
        return chain.withRequest(chain.request.newBuilder().data(httpsOf(address)).build()).proceed()
    }
}
