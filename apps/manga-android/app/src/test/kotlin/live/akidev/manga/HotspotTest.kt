package live.akidev.manga

import live.akidev.manga.share.hotspotAddress
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class HotspotTest {
    @Test
    fun theHotspotIsOnAWifiInterfaceThatIsNotTheOneThePhoneJoined() {
        val found = listOf("rmnet0" to "10.20.30.40", "wlan0" to "192.168.1.7", "wlan1" to "192.168.43.1")
        assertEquals("192.168.43.1", hotspotAddress(found, client = "192.168.1.7"))
    }

    @Test
    fun withNoWifiConnectionTheHotspotMayUseTheFirstWifiInterface() {
        assertEquals("192.168.43.1", hotspotAddress(listOf("lo" to "127.0.0.1", "wlan0" to "192.168.43.1"), client = null))
        assertEquals("192.168.49.1", hotspotAddress(listOf("swlan0" to "192.168.49.1"), client = null))
    }

    @Test
    fun noWifiInterfaceMeansNoAddress() {
        assertNull(hotspotAddress(listOf("rmnet0" to "10.20.30.40"), client = null))
        assertNull(hotspotAddress(listOf("wlan0" to "192.168.1.7"), client = "192.168.1.7"))
    }
}
