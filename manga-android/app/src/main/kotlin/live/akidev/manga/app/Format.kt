package live.akidev.manga.app

import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale


/** "Oct 20, 2026" for a time in milliseconds. */
fun shortDate(ms: Long?): String? = ms?.let { DateTimeFormatter.ofPattern("MMM d, yyyy", Locale.getDefault()).withZone(ZoneId.systemDefault()).format(Instant.ofEpochMilli(it)) }
