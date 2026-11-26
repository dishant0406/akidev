package live.akidev.manga

import live.akidev.manga.books.BookFont
import live.akidev.manga.books.BookMargin
import live.akidev.manga.books.BookStyle
import live.akidev.manga.books.BookTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class BookStyleTest {
    @Test
    fun savedStyleComesBackTheSame() {
        val style = BookStyle(font = BookFont.Atkinson, size = 26.0, margin = BookMargin.Wide, theme = BookTheme.Night, brightness = 0.4, scroll = true)
        assertEquals(style, BookStyle.fromJson(style.toJson(), tablet = false))
    }

    @Test
    fun nothingSavedStartsWithThePhoneOrTabletSize() {
        assertEquals(20.0, BookStyle.fromJson(null, tablet = false).size, 0.0)
        assertEquals(24.0, BookStyle.fromJson(null, tablet = true).size, 0.0)
    }

    @Test
    fun aChoiceAddedLaterDoesNotResetTheOnesAlreadyMade() {
        val style = BookStyle.fromJson("""{"size": 30.0, "justify": false}""", tablet = false)
        assertEquals(30.0, style.size, 0.0)
        assertEquals(false, style.justify)
        assertEquals(BookStyle().font, style.font)
        assertNull(style.brightness)
    }

    @Test
    fun brokenTextFallsBackToTheStart() {
        assertEquals(BookStyle(), BookStyle.fromJson("not json", tablet = false))
    }

    @Test
    fun anUnknownNameFallsBackToTheDefault() {
        assertEquals(BookStyle().font, BookStyle.fromJson("""{"font": "Comic"}""", tablet = false).font)
    }
}
