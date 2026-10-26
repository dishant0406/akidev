package live.akidev.manga.app

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlin.properties.ReadWriteProperty
import kotlin.reflect.KProperty

enum class Appearance(val title: String) { System("System"), Light("Light"), Dark("Dark") }

/** What the person mostly reads. Everything is always there; this only decides what comes first. */
enum class ReaderKind(val title: String) { Manga("Manga and comics"), Books("Books") }

enum class ReadingMode(val title: String, val shortTitle: String) {
    Strip("Vertical strip", "Strip"), Paged("Paged, left to right", "Paged"), RightToLeft("Paged, right to left", "Right to left")
}

enum class ReaderBackground(val title: String) { Black("Black"), Grey("Dark grey") }

enum class OrientationLock(val title: String) { Auto("Auto"), Portrait("Portrait"), Landscape("Landscape") }

enum class AutoScrollSpeed(val title: String, val pointsPerSecond: Float) {
    Slow("Slow", 40f), Normal("Normal", 80f), Fast("Fast", 160f)
}

enum class LibrarySort(val title: String) {
    Recent("Last read"), Added("Date added"), Title("Title"), Unread("Unread"),
    Chapters("Total chapters"), Latest("Latest chapter"), Random("Random")
}

/**
 * What the person chose in Settings, kept so it survives launches. Each setting is one line: its name, and what it is until the person
 * says otherwise. Reading one in a screen makes the screen update when it changes.
 */
class Preferences(context: Context) {
    private val store = context.getSharedPreferences("aki", Context.MODE_PRIVATE)

    /** Null until the person has said (a fresh install asks). */
    var readerKind: ReaderKind? by nullableEnum("readerKind")
    var appearance by enum("appearance", Appearance.System)

    /** Sources marked 18+ are hidden until this is on. */
    var showAdult by boolean("showAdult", false)

    /** Languages to keep (empty is all). */
    var languages: List<String> by strings("languages")
    var defaultMode by enum("defaultMode", ReadingMode.Strip)

    /** Swap which screen edge goes forward. */
    var invertTaps by boolean("invertTaps", false)
    var readerBackground by enum("readerBackground", ReaderBackground.Black)
    var orientation by enum("orientation", OrientationLock.Auto)
    var autoScrollSpeed by enum("autoScrollSpeed", AutoScrollSpeed.Normal)
    var librarySort by enum("librarySort", LibrarySort.Recent)
    var libraryReversed by boolean("libraryReversed", false)

    /** What saving a series does about categories: nothing (0), ask which (-1), or put it on that category (its id). */
    var saveCategory by long("saveCategory", 0)
    var keepAwake by boolean("keepAwake", false)

    /** A notification when a saved series gets new chapters. */
    var notifyUpdates by boolean("notifyUpdates", false)

    /** Keep new chapters of saved series on the device without being asked. */
    var autoDownload by boolean("autoDownload", false)

    /** Auto downloads wait for Wi-Fi: on unless the person says otherwise. */
    var downloadOnWifiOnly by boolean("downloadOnWifiOnly", true)

    /** Turns glass off: every control is a plain tinted shape. */
    var reduceGlass by boolean("reduceGlass", false)

    // ---- how the settings are kept -----------------------------------------------------------------------------------------

    private fun boolean(key: String, default: Boolean) = Stored(store, key, store.getBoolean(key, default)) { k, v -> putBoolean(k, v) }

    private fun long(key: String, default: Long) = Stored(store, key, store.getLong(key, default)) { k, v -> putLong(k, v) }

    private fun strings(key: String) = Stored(store, key, store.getStringSet(key, emptySet())!!.sorted()) { k, v -> putStringSet(k, v.toSet()) }

    private inline fun <reified E : Enum<E>> enum(key: String, default: E) =
        Stored(store, key, enumValues<E>().firstOrNull { it.name == store.getString(key, null) } ?: default) { k, v -> putString(k, v.name) }

    private inline fun <reified E : Enum<E>> nullableEnum(key: String) =
        Stored<E?>(store, key, enumValues<E>().firstOrNull { it.name == store.getString(key, null) }) { k, v -> putString(k, v?.name) }
}

/** One stored setting: a value screens can watch, written to the phone whenever it is set. */
private class Stored<T>(
    private val store: SharedPreferences,
    private val key: String,
    first: T,
    private val write: SharedPreferences.Editor.(String, T) -> Unit,
) : ReadWriteProperty<Any?, T> {
    private var value by mutableStateOf(first)

    override fun getValue(thisRef: Any?, property: KProperty<*>) = value

    override fun setValue(thisRef: Any?, property: KProperty<*>, value: T) {
        this.value = value
        store.edit().also { it.write(key, value) }.apply()
    }
}
