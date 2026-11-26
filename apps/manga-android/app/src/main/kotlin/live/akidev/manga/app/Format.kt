package live.akidev.manga.app

import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale


/** A short date in the person's own language and order ("Oct 20, 2026" in US English) for a time in milliseconds. */
fun shortDate(ms: Long?): String? = ms?.let { DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(Locale.getDefault()).withZone(ZoneId.systemDefault()).format(Instant.ofEpochMilli(it)) }
