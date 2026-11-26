package live.akidev.manga.app

import live.akidev.manga.design.AkiTabItem
import live.akidev.manga.design.Copy

enum class AppTab(val icon: String, val iconSolid: String) {
    Home("home", "home-solid"),
    Library("library", "library-solid"),
    Books("book", "book-solid"),
    Updates("inbox", "inbox-solid"),
    Sources("sources", "sources-solid"),

    /** For someone who mostly reads books: finding books, and the manga tabs folded into one. */
    Browse("search", "search"),
    Manga("library", "library-solid");

    /** What the tab is called: for someone who reads books, the first tab is their home and the manga tabs say what they are. */
    fun title(kind: ReaderKind?) = when (this) {
        Home -> if (kind == ReaderKind.Books) Copy.Home.home else Copy.Home.discover
        Library -> Copy.Home.library
        Books -> if (kind == ReaderKind.Books) Copy.Home.library else Copy.Home.books
        Updates -> Copy.Home.updates
        Browse -> Copy.Home.browse
        Manga -> Copy.Home.manga
        Sources -> Copy.Home.sources
    }

    fun item(kind: ReaderKind?, badge: Int = 0) = AkiTabItem(this, icon, iconSolid, title(kind), badge)

    companion object {
        /**
         * The tabs in the order the person's kind of reading wants them: what they read most is first, and where they start. Sources
         * is always last: it is set up once, not used every day.
         */
        fun order(kind: ReaderKind?) =
            if (kind == ReaderKind.Books) listOf(Home, Books, Browse, Manga, Sources) else listOf(Home, Library, Books, Updates, Sources)
    }
}
