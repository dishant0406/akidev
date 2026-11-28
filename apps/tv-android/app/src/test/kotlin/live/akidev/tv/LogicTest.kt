package live.akidev.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class LogicTest {
    private fun watched(playId: String, titleId: String = "series-1", position: Long, length: Long = 1000, at: Long = 0) =
        Watched(playId, titleId, "Show", null, null, null, position, length, at)

    @Test
    fun past_ninety_percent_counts_as_seen() {
        assertFalse(watched("a", position = 899).finished())
        assertTrue(watched("a", position = 900).finished())
        assertFalse(watched("a", position = 5, length = 0).finished())
    }

    @Test
    fun resume_starts_where_the_viewer_stopped_unless_it_was_seen() {
        val list = listOf(watched("half", position = 400), watched("seen", position = 950))
        assertEquals(400L, list.resumeAt("half"))
        assertEquals(0L, list.resumeAt("seen"))
        assertEquals(0L, list.resumeAt("never"))
    }

    @Test
    fun progress_is_a_share_or_null() {
        val list = listOf(watched("half", position = 500))
        assertEquals(0.5f, list.progressOf("half")!!, 0.001f)
        assertNull(list.progressOf("never"))
    }

    @Test
    fun continue_watching_keeps_the_last_entry_of_each_title_and_drops_seen_ones() {
        val list = listOf(
            watched("e1", "series-1", position = 100, at = 1),
            watched("e2", "series-1", position = 200, at = 5),
            watched("film", "movie-2", position = 950, at = 9),
            watched("other", "series-3", position = 100, at = 3),
        )
        assertEquals(listOf("e2", "other"), list.continueWatching().map { it.playId })
        assertEquals(1, list.continueWatching(most = 1).size)
    }

    @Test
    fun the_next_episode_is_the_one_after_in_the_list() {
        val ids = listOf("e1", "e2", "e3")
        assertEquals("e2", nextEpisode(ids, "e1"))
        assertNull(nextEpisode(ids, "e3"))
        assertNull(nextEpisode(ids, "missing"))
    }

    @Test
    fun the_clock_shows_hours_only_when_there_are_some() {
        assertEquals("0:23", clock(23_000))
        assertEquals("26:00", clock(26 * 60_000))
        assertEquals("2:28:07", clock(((2 * 60 + 28) * 60 + 7) * 1000L))
    }

    @Test
    fun a_track_is_named_by_language_then_label_or_by_number() {
        assertEquals("English", trackName("en", null, 0))
        assertEquals("Japanese  ·  Commentary", trackName("ja", "Commentary", 1))
        assertEquals("Track 3", trackName("und", null, 2))
        assertEquals("Signs", trackName(null, "Signs", 0))
    }

    @Test
    fun the_tabs_follow_the_definitions_after_home() {
        val tabs = barTabs(listOf(uniffi.akitv.TabInfo("anime", "Anime"), uniffi.akitv.TabInfo("movie", "Films"), uniffi.akitv.TabInfo("music", "Music")))
        assertEquals(listOf("Home", "Anime", "Films", "Music"), tabs.map { it.label })
        assertNull(tabs.first().kind)
        // A kind that a plugin brought has no icon of its own: it gets a neutral one.
        assertEquals(listOf("home", "sparkles", "film", "list"), tabs.map { it.icon })
    }

    @Test
    fun a_plain_http_picture_address_is_asked_for_over_https() {
        assertEquals("https://img.example.com/a.jpg", httpsOf("http://img.example.com/a.jpg"))
        assertEquals("https://img.example.com/a.jpg", httpsOf("https://img.example.com/a.jpg"))
    }

    private fun row(name: String) = uniffi.akitv.Row(name, "movie", "popularity", null, false, emptyList())

    @Test
    fun rows_show_in_order_and_only_after_the_rows_before_them_have_an_answer() {
        assertEquals(Load.Loading, homeRowsOf(listOf(Load.Loading, Load.Loading)))
        // Row 1 came first, but row 0 has not: nothing is shown yet, so nothing is pushed down later.
        assertEquals(Load.Loading, homeRowsOf(listOf(Load.Loading, Load.Done(row("b")))))
        val shown = (homeRowsOf(listOf(Load.Done(row("a")), Load.Loading, Load.Done(row("c")))) as Load.Done).value
        assertEquals(listOf("a"), shown.rows.map { it.name })
        assertTrue(shown.more)
    }

    @Test
    fun a_row_that_failed_is_left_out_and_all_failed_is_an_error() {
        val shown = (homeRowsOf(listOf(Load.Done(row("a")), Load.Failed("x"), Load.Done(row("c")))) as Load.Done).value
        assertEquals(listOf("a", "c"), shown.rows.map { it.name })
        assertFalse(shown.more)
        assertTrue(homeRowsOf(listOf(Load.Failed("x"), Load.Failed("y"))) is Load.Failed)
    }

    @Test
    fun versions_compare_as_numbers() {
        assertTrue(Updates.isNewer("1.0.1", "1.0.0"))
        assertTrue(Updates.isNewer("1.10.0", "1.9.0"))
        assertTrue(Updates.isNewer("2.0", "1.9.9"))
        assertFalse(Updates.isNewer("1.0", "1.0.0"))
        assertFalse(Updates.isNewer("1.0.0", "1.0.1"))
        assertFalse(Updates.isNewer("beta", "1.0.0"))
    }

    private fun release(tag: String, apk: Boolean = true, draft: Boolean = false, prerelease: Boolean = false) =
        """{"tag_name":"$tag","draft":$draft,"prerelease":$prerelease,"body":"notes of $tag","assets":[""" +
            (if (apk) """{"name":"AkidevTV.apk","size":123,"browser_download_url":"https://example.test/$tag.apk","digest":"sha256:abc"}""" else """{"name":"notes.txt"}""") + "]}"

    @Test
    fun the_newest_published_release_of_this_app_with_an_apk_is_picked() {
        val json = "[" + listOf(
            release("desktop-mac-v9.9.9"),
            release("tv-android-v1.2.0", draft = true),
            release("tv-android-v1.3.0", prerelease = true),
            release("tv-android-v1.1.0", apk = false),
            release("tv-android-v1.0.9"),
            release("tv-android-v1.0.10"),
        ).joinToString(",") + "]"
        val picked = Updates.pick(json)!!
        assertEquals("1.0.10", picked.version)
        assertEquals("https://example.test/tv-android-v1.0.10.apk", picked.apkUrl)
        assertEquals("abc", picked.sha256)
        assertEquals(123L, picked.size)
        assertNull(Updates.pick("[" + release("desktop-mac-v1.0.0") + "]"))
    }

    @Test
    fun an_air_date_reads_as_a_day_and_a_month() {
        assertEquals("4 DEC", airDay("2026-12-04"))
        assertEquals("10 FEB", airDay("2008-02-10T00:00:00Z"))
        assertNull(airDay(null))
        assertNull(airDay("soon"))
        assertNull(airDay("2026-13-04"))
    }

    @Test
    fun a_row_keeps_the_card_clear_of_its_edge() {
        val rule = LeastToShow(margin = 24f)
        // Already in view with room to spare: no scroll.
        assertEquals(0f, rule.calculateScrollDistance(offset = 100f, size = 256f, containerSize = 1000f), 0f)
        // Flush with the right edge: it moves 24 px in, so the card that grows still shows in full.
        assertEquals(24f, rule.calculateScrollDistance(offset = 744f, size = 256f, containerSize = 1000f), 0f)
        // Past the left edge: it moves in by the distance and the margin.
        assertEquals(-124f, rule.calculateScrollDistance(offset = -100f, size = 256f, containerSize = 1000f), 0f)
    }

    @Test
    fun the_facts_line_says_when_a_title_comes_and_the_year_once_it_is_out() {
        val out = uniffi.akitv.Title("a", "A", 2020, null, 85u, null, null, listOf("Drama"), null)
        assertEquals("2020  ·  DRAMA  ·  85%", facts(out))
        val coming = out.copy(year = null, comingSoon = "Coming 9 January 2027")
        assertEquals("COMING 9 JANUARY 2027  ·  DRAMA  ·  85%", facts(coming))
        // Not out and with no date at all: still the words, not a missing year.
        assertEquals("COMING SOON  ·  DRAMA  ·  85%", facts(coming.copy(comingSoon = "Coming soon")))
    }
}
