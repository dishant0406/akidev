package live.akidev.manga.app

import uniffi.akimanga.RepoInfo

/** A repository's page: what it offers, with install and update. */
class RepoRoute(val repo: RepoInfo) : Route

/** The choices one source offers (a language, what content to show). */
class SourceSettingsRoute(val source: String) : Route

/** A series' page. */
class SeriesRoute(val ref: SeriesRef) : Route

/** The reader, over everything: a series' chapters, opened at a place in one. */
class ReaderRoute(val ref: SeriesRef, val chapters: List<uniffi.akimanga.ChapterEntry>, val target: live.akidev.manga.manga.ReaderTarget) : Route

/** Where a series is: the source it comes from, its id, and what the card showed. */
class SeriesRef(val source: String, val id: String, val title: String, val cover: String?)

/** The settings page. */
object SettingsRoute : Route

/** A page of a book catalogue to open: its address, its name, and how to search the catalogue it belongs to. */
class CatalogueRef(val url: String, val name: String, val search: String?)

class CatalogueRoute(val ref: CatalogueRef) : Route

/** Everything highlighted in every book; `open` takes the person to one. */
class HighlightsRoute(val open: (uniffi.akimanga.BookHighlightIn) -> Unit) : Route

/** A book, being read, over everything. */
class BookReaderRoute(val book: uniffi.akimanga.BookInfo) : Route

/** The voice models for reading aloud. */
object VoicesRoute : Route

/** The voices of the model that reads. */
object VoiceChoiceRoute : Route
