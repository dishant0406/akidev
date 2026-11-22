package live.akidev.manga.app

import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale


/** A short date in the person's own language and order ("Oct 20, 2026" in US English) for a time in milliseconds. */
fun shortDate(ms: Long?): String? = ms?.let { DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(Locale.getDefault()).withZone(ZoneId.systemDefault()).format(Instant.ofEpochMilli(it)) }

/** "1 source", "2 sources": the count with the right word, joined by a no-break space so they never part at the end of a line. */
fun plural(count: Int, one: String, many: String) = if (count == 1) "1\u00A0$one" else "$count\u00A0$many"
