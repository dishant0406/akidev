package live.akidev.manga.books

import android.content.Context
import android.graphics.Typeface
import androidx.compose.ui.graphics.Color
import androidx.core.content.res.ResourcesCompat
import live.akidev.manga.R
import org.json.JSONObject
import java.util.Calendar

/** The reading surface's fonts. Lora and Atkinson Hyperlegible are carried in the app; the others are the phone's own. */
enum class BookFont(val title: String) {
    Lora("Lora"), Serif("Serif"), Sans("Sans"), Atkinson("Atkinson Hyperlegible"), Rounded("Rounded");

    /** The face for a style of it: the weight is 400 or 700, and the italic is the font's own where it has one. */
    fun typeface(context: Context, bold: Boolean, italic: Boolean, mono: Boolean): Typeface {
        if (mono) return Typeface.create(Typeface.MONOSPACE, if (bold) Typeface.BOLD else Typeface.NORMAL)
        val base = when (this) {
            Lora -> ResourcesCompat.getFont(context, R.font.lora)
            Atkinson -> ResourcesCompat.getFont(context, R.font.atkinson)
            Rounded -> ResourcesCompat.getFont(context, live.akidev.manga.design.R.font.nunito)
            Serif -> Typeface.SERIF
            Sans -> Typeface.SANS_SERIF
        } ?: Typeface.DEFAULT
        return Typeface.create(base, if (bold) 700 else 400, italic)
    }
}

enum class BookMargin(val title: String, val dp: Int) { Narrow("Narrow", 20), Normal("Normal", 32), Wide("Wide", 44) }

/** The reading surface: paper and ink. Not the app's tokens, which are for the app's own screens. */
enum class BookTheme(val title: String, val paper: Color, val ink: Color) {
    /** White by day, night by night: whichever the phone is in. */
    Auto("Auto", Color(0xFFFAFAF8), Color(0xFF1A1A1A)),
    Paper("Paper", Color(0xFFFAFAF8), Color(0xFF1A1A1A)),
    Sepia("Sepia", Color(0xFFF3EAD7), Color(0xFF4B3B2A)),
    Ink("Ink", Color(0xFFDADAD6), Color(0xFF262626)),
    Night("Night", Color(0xFF1C1C1E), Color(0xFFD6D6DA)),
    Black("Black", Color(0xFF000000), Color(0xFFBDBDC2));

    /** The theme itself, with `Auto` settled by the phone's appearance. */
    fun resolved(dark: Boolean) = if (this == Auto) (if (dark) Night else Paper) else this

    /** Chapter name and page number under the text. */
    val quiet get() = ink.copy(alpha = 0.5f)
    val isDark get() = this == Night || this == Black
}

/** When the warm tint is on. */
enum class BookWarmth(val title: String) {
    Off("Off"), Always("Always"), Night("At night");

    /** Night is a clock window, so no location is asked for. */
    fun isOn(hour: Int = Calendar.getInstance().get(Calendar.HOUR_OF_DAY)) = when (this) {
        Off -> false
        Always -> true
        Night -> hour >= 20 || hour < 7
    }
}

/** How a book looks to its reader: the choices in the reader's settings. Saved as one value, so a new choice is one new field. */
data class BookStyle(
    val font: BookFont = BookFont.Lora,
    /** Text size in dp, 14 to 34. */
    val size: Double = 20.0,
    /** How tall a line is, as a multiple of the font's own: 1.1 to 1.9. */
    val lineSpacing: Double = 1.35,
    val margin: BookMargin = BookMargin.Normal,
    val justify: Boolean = true,
    val hyphenate: Boolean = true,
    val theme: BookTheme = BookTheme.Auto,
    /** How bright the screen is while reading, 0 to 1, or null to leave the phone's own. Below the lowest the phone goes, a dark layer over the page takes it further. */
    val brightness: Double? = null,
    val warmth: BookWarmth = BookWarmth.Off,
    /** How strong the warm tint is, 0 to 1. */
    val warmthAmount: Double = 0.35,
    /** A paper-like display: grey, still, no page-turn motion. */
    val ink: Boolean = false,
    /** The book is read by scrolling, one chapter running into the next, instead of in pages. */
    val scroll: Boolean = false,
    /** Headings and the like are set in the fonts the book carries, when it carries them. */
    val bookFonts: Boolean = true,
) {
    /** The choices that change where words fall on a page; the others only change how the page is lit or coloured. */
    val affectsType get() = listOf(font, size, lineSpacing, margin, justify, hyphenate, bookFonts)

    fun toJson(): String = JSONObject()
        .put("font", font.name).put("size", size).put("lineSpacing", lineSpacing).put("margin", margin.name).put("justify", justify).put("hyphenate", hyphenate)
        .put("theme", theme.name).put("warmth", warmth.name).put("warmthAmount", warmthAmount).put("ink", ink).put("scroll", scroll).put("bookFonts", bookFonts)
        .also { if (brightness != null) it.put("brightness", brightness) }.toString()

    companion object {
        val sizes = 14.0..34.0
        val spacings = 1.1..1.9

        /** What is missing is the default, so a setting added later does not reset the ones already chosen. */
        fun fromJson(text: String?, tablet: Boolean): BookStyle {
            val first = BookStyle(size = if (tablet) 24.0 else 20.0)
            if (text == null) return first
            val json = runCatching { JSONObject(text) }.getOrNull() ?: return first
            return BookStyle(
                font = enumOf(json, "font", first.font), size = json.optDouble("size", first.size), lineSpacing = json.optDouble("lineSpacing", first.lineSpacing),
                margin = enumOf(json, "margin", first.margin), justify = json.optBoolean("justify", first.justify), hyphenate = json.optBoolean("hyphenate", first.hyphenate),
                theme = enumOf(json, "theme", first.theme), brightness = if (json.has("brightness")) json.getDouble("brightness") else null,
                warmth = enumOf(json, "warmth", first.warmth), warmthAmount = json.optDouble("warmthAmount", first.warmthAmount), ink = json.optBoolean("ink", first.ink),
                scroll = json.optBoolean("scroll", first.scroll), bookFonts = json.optBoolean("bookFonts", first.bookFonts),
            )
        }

        private inline fun <reified E : Enum<E>> enumOf(json: JSONObject, key: String, default: E) = enumValues<E>().firstOrNull { it.name == json.optString(key) } ?: default
    }
}

/** The four highlight colours. */
object HighlightColour {
    val names = listOf("Yellow", "Green", "Blue", "Pink")
    val colours = listOf(Color(0xFFFFD21F), Color(0xFF6CCB5F), Color(0xFF5AA9FF), Color(0xFFFF7BAC))

    /** The colour of a selection and its handles. */
    val selection = Color(0xFF4C8DFF)

    fun colour(index: Int) = colours[index.coerceIn(0, colours.lastIndex)]
    fun fill(index: Int) = colour(index).copy(alpha = 0.42f)
}
