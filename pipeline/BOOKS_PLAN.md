# Books in Akidev: an EPUB reader we draw ourselves

Written 2026-10-24. A plan, not a build. Nothing in the app has changed for it.

## 1. What we are making

A second kind of content in Akidev: **books**. EPUB files are added from Files or from an open catalogue the person adds (OPDS),
live in their own tab, and are read in a reader we draw ourselves: native text, no web view, no reading library, glass chrome,
calm and fast.

Principles, carried over from the manga side:

1. **A renderer with no content of its own.** The app ships no books and no catalogue. People add files, or add a catalogue by its address.
2. **Native.** The engine (Rust, shared with the desktop app) reads the file; Swift lays out and draws it. No `WKWebView`, no epub.js, no Readium toolkit.
3. **Light.** Budget: the app grows by **no more than 0.5 MB** (now 8.2 MB), launch time does not change, no bundled fonts.
4. **Quiet.** The reader is mostly text. Everything else (bars, sheets) is glass and hides.
5. **Same look.** `manga-app/DESIGN.md` (bright, one yellow, glass chrome). The reading surface has its own themes.

## 2. What readers want (research)

Two passes. The first (web search) was mostly vendor listicles. The second, added 2026-10-24, read the real Reddit threads
(through the TinyFish search and fetch APIs; the in-app browser blocks reddit): r/ebooks, r/ereader, r/apple, r/macapps, r/koreader,
r/readest, r/readwise, about 25 threads from 2022 to 2026. X gave almost nothing useful (company accounts and promotion), so it is not
used. Reddit is self-selected: loud complainers and power users are over-represented, and several threads are developers advertising
their own app. Counts below are "threads where it came up", not statistics.

### 2.1 What people actually say (Reddit)

| Theme | What they wrote (paraphrased) | Threads | What it means for us |
| --- | --- | --- | --- |
| **Books vanish** | Apple Books offloads books to iCloud, "even the book you are reading", even on a plane or with poor signal; no "download all" button | 5+ (r/apple, r/ebooks, r/macapps, r/zlibrary) | **Our biggest promise: a book you added stays on the device until you remove it.** No offloading, ever. Say so in the app |
| **Subscription for local features** | Hiding basic local functionality behind a subscription; limits like 10 files per import; people say they would pay once | 4 | No subscription, no import limit. Anything we charge for is one-time, if at all (decision for you) |
| **Getting a big library in** | Dozens of GB of books in folders on a Mac; iOS apps make this hard; Eboox crashes with several GB; PC upload over Wi-Fi in MapleRead is praised; Google Drive "Open in" works | 3 | Import whole folders from Files, many files at once, a Wi-Fi upload page later; library must stay fast at thousands of books (test at 2,000) |
| **Slow open, laggy, stale screen** | Readwise Reader took ~2 s even for a 200 KB book and fixed it only after a user measured; Readest on iOS shows a frozen frame after returning to the app (reported for 137+ days); Apple Books "hiccups", pages do not render until force quit | 4 | Open time and resume-from-background are features. Measure both; a cached chapter must show at once; redraw on becoming active |
| **True black** | Many read in dark rooms; OLED users want black, not grey (a 2017 thread says iBooks' darkest was grey); some cannot use light mode at all (floaters, light sensitivity) | 5 | Keep **True black** and auto switch; also accessibility: high contrast, no pure white flash when opening |
| **Typography they miss** | No custom fonts or text colour; cannot tap the page to turn it in Apple Books; cannot adjust margins; layout and custom-layout settings not remembered | 4 | Margins, tap zones, per-book memory of settings are cheap wins. Custom fonts (import a font file) rises from "later" to phase 5 |
| **Paging vs scrolling** | Both are wanted: "choose your own touch zones, no animations, a quick non-descript transition"; some do not like turning pages at all | 3 | Both modes, **configurable tap zones**, short transition, option for none |
| **Sync that is trusted** | KOReader and Readest progress sync is celebrated; Google Play Books notes "glitchy"; a Moon+ user lost highlights on hundreds of books after a restore; "I just want to sort by percent read" | 6 | Progress sync is the most requested extra and also where trust is lost: never overwrite, merge by time, and **a plain-file export/import of highlights** as the safety net |
| **Highlights and notes that come out** | Export as Markdown or plain text; "notes section inside each book"; Readwise-style review | 4 | Highlights export to Markdown in phase 5, not "later" |
| **Text to speech** | Voice Dream, Speechify and ElevenReader time limits and paywalls pushed people to look for free ones; word-level highlight while reading aloud is the praised feature; offline narration | 3 | System speech (AVSpeechSynthesizer) with sentence highlight is free to offer and valued; stays in phase 6 |
| **Broken EPUBs and footnotes** | Footnote pop-up empty or in the wrong font; formatting "completely messed up" when the font size changes; embedded fonts ignored in most apps (KOReader honours them) | 5 | Footnote popover and **embedded fonts honoured** are differentiators; test corpus must include odd books |
| **Looks** | An independent app was called "basic", its icon "looks like AI" and not Apple-like; people want "Apple Books feel, Readest/KOReader openness" | 2 | Our glass design is a real asset; avoid generic look |
| **Privacy** | Separate threads asking for a reader with no tracking and no account | 4 | State it plainly: no account, nothing leaves the device |
| **Library features** | Sort by percent read, shelves/collections, Calibre or OPDS access, comics in the same app | 4 | Sort by progress, collections (reuse categories), OPDS in phase 4 |
| **Annotations alongside PDFs** | One request is for PDF annotation and a knowledge organiser, not only EPUB | 2 | Out of scope, said honestly |

Which apps people name, so we know the bar: Apple Books (default, "smooth", but offloads and hiccups), Readest (modern, KOReader sync, iOS lag),
KOReader (loved, hard to set up, no iOS store app), Yomu (one-time purchase, accepts everything), ReadEra (free, many features), MapleRead,
KyBook 3 and Marvin (old, no longer updated), BookFusion and Readwise Reader (subscriptions), Moon+ (Android).

### 2.2 What changes in the plan because of this

1. **Promise on the first screen of the Books tab:** books stay on this device. No cloud offloading. (Replaces the generic "no accounts" line as headline.)
2. **Phase order moves:** resume-from-background correctness and open time become phase 3 gates, not phase 6 polish.
3. **Pulled forward:** configurable tap zones and margins (phase 3), highlights export as Markdown and custom font import (phase 5).
4. **Sync:** still not in version 1, but the highlight export/import file and a merge-never-overwrite rule go into the schema from phase 1 so sync can be added without losing anyone's notes. KOReader-compatible progress is the first sync to build.
5. **Library scale test:** 2,000 books, folder import, many files at once.
6. **Pricing stance** (decision for you, section 9): free with no limits, or one-time.



**What they value**

- **Typography control:** font, size, line and paragraph spacing, margins, justification, hyphenation, themes (white, sepia, black), per-book
  memory of those choices. Moon+, Kindle, Voice Dream, ReadEra all lead with this.
- **Pagination or scroll, their choice,** plus tap zones and a progress scrubber.
- **Highlights and notes** that can be exported, a **dictionary** in one tap, **search in the book**.
- **Sync** of position, highlights and notes between devices (Kindle's strongest feature; the one Apple Books is faulted on).
- **Text to speech** with the sentence highlighted (Voice Dream; Readest reviewers call it a lifesaver).
- **Open files, any source:** import from Files, OPDS catalogues, no store lock-in.

**What they complain about**

- Apple Books: syncing that fails or lags, sideloaded EPUBs treated as second class, customisation spread over many taps, themes that
  do not sync, a library that slows as it grows (Apple Community threads, a TidBITS comment; anecdotes, dated 2017 to 2022; confirmed and extended by the Reddit pass in 2.1).
- Readest reviewers: a book that is slow to open and sticks on a spinner; no true black for OLED screens; no web search next to the dictionary.
- Reading systems differ on CSS support, footnotes, fixed layout, right-to-left and vertical text, and **broken EPUBs** (W3C digital
  publishing notes, Project Gutenberg's CSS wiki, forum threads). "Anything but Books on iOS is a crapshoot" for fixed layout.
- A third-party app's listing sells **true black OLED theme** and **page curl**: gaps people look elsewhere to fill.

**Where we can be better, and cheaply**

| Opportunity | Why it is ours |
| --- | --- |
| Opens a book instantly | We cache the parsed and laid-out chapter; Readest's slow open is a known complaint |
| True black and a real theme set | A colour table, no cost |
| Glass chrome, calm reader | Our whole design language |
| Open catalogues without a store | We already have "add by address" and trust screens |
| No accounts, nothing leaves the device | Privacy page already says so |
| Fast and small | 8 MB app, measured launch |

Not our fight at first: Kindle/Kobo store integration (DRM), PDF, MOBI/AZW3 (Readest and ReadEra cover these), cloud sync (see section 9).

## 3. The EPUB format, and what we must read

An EPUB is a **zip** (the OCF container). The parts that matter (W3C EPUB 3.3; also read EPUB 2 files):

| Part | What it is | We need |
| --- | --- | --- |
| `mimetype`, `META-INF/container.xml` | Says where the package document is | yes |
| Package document (`.opf`) | Metadata (title, authors, language, cover, direction), **manifest** (every file), **spine** (reading order), rendition (reflowable or fixed layout) | yes |
| Navigation document (`nav.xhtml`, EPUB 3) or `toc.ncx` (EPUB 2) | Table of contents and landmarks | yes |
| Content documents | XHTML (and SVG) with CSS, images, fonts, sometimes audio, video, script | XHTML, CSS, images; ignore script |
| `META-INF/encryption.xml`, `rights.xml` | DRM or font obfuscation | detect: refuse DRM clearly; undo the standard font obfuscation |

We do **not** need a full browser engine. Real books use a small, known subset: paragraphs, headings, lists, block quotes, emphasis,
`<span>` styling, images, simple tables, footnote links, page breaks, a few hundred CSS properties of which perhaps thirty carry
the look (font family and size, weight and style, colour, alignment, text-indent, margins, line-height, `page-break-*`,
`font-variant: small-caps`, `float` for drop caps and images).

## 4. Architecture

```
 file or OPDS download
        |
   [ Rust engine: crate akidev-manga, module `book` ]
        |   zip  ->  container -> OPF -> spine, manifest, toc
        |   XHTML -> html5ever (forgiving, handles broken markup)
        |   CSS   -> cssparser + selectors (already in the app), cascade for the subset
        v
   "styled document" per spine item: blocks and runs with computed style
        |   (compact, serialised; cached on disk beside the book)
        v
 [ Swift ]  layout (paginate)  ->  pages  ->  draw  ->  glass chrome
```

### 4.1 Engine (Rust): parse and style

- **Why Rust:** it already holds the zip, the HTML parser and the CSS parser, so this costs almost no size; it is testable without a simulator; the desktop app can share it later.
- **Dependencies already linked on iOS:** `zip`, `html5ever`/`scraper`, `cssparser`, `selectors`. Probably one tiny XML reader is added (`roxmltree`, no dependencies); measure, and fall back to the HTML parser in XML-ish mode if it costs more than 60 KB.
- **Output, per chapter:** a flat list of blocks (paragraph, heading, list item, quote, image, table row group, rule, page break), each with runs of text carrying computed style (font traits, size in em, colour, link target, footnote role) and a **text map** (character offset to source node) for positions and highlights.
- **Tolerance:** malformed XHTML parses through html5ever; a bad file should open degraded, not fail. A corpus test enforces it.
- **Limits:** file size, entries, and expanded size, as `local.rs` already does for comic archives (zip bombs).
- **Cache:** the styled document of each chapter is written once to disk, keyed by the book's hash, so the second open is a read, not a parse.

### 4.2 Swift: layout, paint, interaction

The honest decision, to be settled by a short spike (phase 0) with numbers:

| | Core Text (`CTFramesetter`) | TextKit 2 (`NSTextLayoutManager`) |
| --- | --- | --- |
| Control of page breaks | Full: we ask for one page's frame at a time, from a character offset | Viewport based; pagination is not its design, multiple containers unsupported (forum reports) |
| Speed | Layout only the page needed, cache neighbours; known approach in native readers | Reports of stutter on very large text in one view |
| Selection, VoiceOver, dictionary, Dynamic Type | We build them | Mostly free in `UITextView` |
| Precedent | Yuedu-style readers; Core Text readers on GitHub | A native TextKit 2 EPUB viewer exists (EPUBLib PR 4): one `UITextView` per page, text map to DOM positions |

Working recommendation, to be confirmed or reversed by the spike: **our own pipeline into `NSAttributedString`, paginated with Core Text on a background queue, painted by one cheap custom view per page**, with selection and highlights built from `CTLine` geometry. If the spike shows selection and accessibility are too costly to rebuild, the fallback is the TextKit 2 per-page `UITextView` route, with our styled document still feeding it. Either way the **parser, styling and position model are ours** and identical.

Pagination rules: pages break on whole lines; honour `page-break-before/after`, keep headings with the next paragraph, avoid widows and orphans, hyphenate (paragraph style), justify when asked. Paginate per chapter, per (font, size, margins, width) setting; cache the page ranges; prefetch the next and previous page; invalidate only on a setting change.

### 4.3 Where a person is: the locator

A reflowable book has no stable page numbers. Store, per book: **spine index + character offset in the chapter's text map + chapter progression (0 to 1)**, and the book's **content hash**. The offset restores exactly after any font change; the progression is the fallback if the offset no longer fits. Export a standard `epubcfi` later for KOReader-style sync (the CFI spec's goal is exactly cross-reader portability).

### 4.4 Storage (schema 7)

```
book            (id, title, authors, language, direction, cover, added_at, opened_at, file_hash, bytes, kind)
book_progress   (book, spine, offset, progression, updated_at)
book_mark       (book, spine, start, end, kind[highlight|bookmark], colour, note, created_at)
book_setting    (book, key, value)       -- per-book typography, like the manga per-series mode
```

Original files stay as the person gave them (read from the zip on demand; no unpacked copy), under `Application Support/AkiManga/books/`.
Covers go through the existing cover pipeline. Backup includes the library rows, progress and marks, and (as for covers) not the files, unless the person chooses to include them: files can be large; the restore says what it could not bring back. Everything follows the manga rules (migrations in one transaction, tests for an older library opening).

### 4.5 Getting books in

1. **Files / Open in / share sheet:** an `.epub` is copied in and parsed. This is phase 1 and needs no network.
2. **A book source = an OPDS catalogue address** (open standard for catalogues: Standard Ebooks, Project Gutenberg and the Internet Archive publish feeds). "Add a source" gets a second choice: *Manga source* or *Book catalogue*. The app ships no catalogue; the docs list suggestions, as the manga repositories do. Browse, search, open a book's page, **Get** downloads into the library.
3. **Not in scope:** DRM'd books (Kindle, Kobo, Adobe ADEPT, LCP). The file is detected and refused with a plain message. The review notes use the same stance as the manga side: a renderer with no content.

## 5. User flow

```
First run (no books)                    Settings
  Books tab hidden  ----------------->  "Show the Books tab"  (also appears after the first book is added)

Books tab
  Empty: [Add from Files]  [Add a catalogue]
  Shelf: Continue reading card (cover, progress ring)   | grid of covers | sort: recent, title, author, progress
         long-press: Details, Mark finished, Remove (and its file)
         Select mode: remove, mark, move to a shelf

Add from Files ---> picker ---> parse ---> book appears (cover first), errors: DRM, damaged, too big
Add a catalogue --> paste address --> trust screen (as repositories) --> browse / search --> Get

Book detail (sheet)  cover, title, author, language, length (hours at your speed), [Read] [Share] [Delete]

Reader
  tap centre: bars show/hide       tap left/right edge: turn page     swipe: turn page (scroll mode: scroll)
  top bar:  back | chapter title | search | settings
  bottom:   scrubber (with chapter ticks, "page x of y in chapter", time left) | contents | highlights
  select text: menu = Highlight (colours), Note, Copy, Look up, Search, Share
  tap a footnote: popover with the note (no leaving the page)     tap an image: full-screen, zoom
  Settings sheet: theme (white, sepia, grey, black, true black) | font | size | line height | margins
                  | justify | hyphenate | page turn (slide, none) | scroll or pages | brightness | keep awake
  Contents sheet: chapters (like the manga chapter sheet), bookmarks, highlights, notes (tabs)
```

States to design: opening (one calm loader, like the manga reader), chapter is large (stream the first page, finish in background), no cover (generated plain cover from the title), search with no match, a broken image (a quiet placeholder), right-to-left and vertical books (see risks).

## 6. Reader design (glass, calm, fast)

- **Reading surface:** flat colour from the theme, generous margins, no chrome while reading. The status bar hides in the reader.
- **Chrome:** glass capsules like the manga reader: back, title, search, settings on top; a scrubber and two round buttons below. Dark tint like the manga bars so they stay readable on white paper.
- **Themes:** White, Sepia, Grey, Black, **True black** (OLED). Text colours chosen for contrast, not pure white on pure black. Auto switch with the system appearance, optional.
- **Type:** system serif (New York) and system sans (SF Rounded is the app's face) with **no bundled fonts** (size); the person's own fonts via the Files picker later.
- **Page turn:** a short horizontal slide with the next page already drawn (prefetched); no curl at first. Reduce Motion respected.
- **Feel:** page turn must land in one frame at the display's rate (target 120 Hz on ProMotion); opening a book you have read shows the page at once from the cache.
- **Accessibility:** Dynamic Type for the UI, a text size of its own for the book, VoiceOver reading by paragraph, high contrast themes.

## 7. Phases

| # | Goal | Done when |
| --- | --- | --- |
| 0 | **Spike (2 days).** Parse one real EPUB; lay out 40 pages with Core Text and with TextKit 2; measure open time, page turn time, memory, and how hard selection and VoiceOver are | A written choice with numbers; budget table filled in |
| 1 | **Engine: read a book (4 days).** `book` module: container, OPF, spine, TOC, metadata, cover, font de-obfuscation, DRM detection, limits; styled document output; schema 7; backup | Parses a corpus of 60 real EPUBs (20 Standard Ebooks, 20 Gutenberg, 10 modern commercial-style DRM-free, 10 broken or odd) with zero crashes; tests; no size regression beyond budget |
| 2 | **Layout and paint (5 days).** Styled document to attributed text; CSS subset; images; pagination and cache; text map | A chapter paginates under 100 ms on a mid iPhone; pages match the corpus's expectations by eye on 10 books |
| 3 | **Reader (5 days).** Paged and scroll modes, chrome, scrubber, settings and themes, position save and restore | Open to first page under 300 ms on a cached book; reflow keeps the position within one paragraph |
| 4 | **Library and getting books (4 days).** Books tab, import from Files and Open in, shelf, detail, OPDS source (browse, search, get), trust screen | Add by file and by catalogue; remove deletes the file |
| 5 | **Reading tools (5 days).** Contents, highlights, notes, bookmarks, search, dictionary, footnote popovers, image viewer | Marks survive a font change and a restore |
| 6 | **Speech and polish (4 days).** Text to speech with the sentence highlighted (check the launch cost of the audio framework before linking: PhotosUI cost ~830 ms in the simulator), accessibility pass, empty and error states, performance and size audit | Launch time unchanged, app size within budget, VoiceOver walk-through |
| later | Fixed-layout pages, vertical and right-to-left writing, KOReader/OPDS progress sync, custom fonts, iCloud, PDF | Each decided after phase 6 |

Each phase ends with: tests, a simulator recording of the real flow, an independent review of the diff (as on the manga side), a TestFlight build.

## 8. Quality gates

| Gate | Target |
| --- | --- |
| App size | +0.5 MB at most over 8.2 MB, measured as with the last size work (archive, app folder) |
| Launch | Unchanged from today (200 ms to app code in the simulator): the Books tab builds nothing until opened |
| Open a book, first page | under 300 ms warm, under 1.2 s first time for a 1 MB EPUB |
| Page turn | one frame; no hitch over 20 ms with the frame probe |
| Memory | under 120 MB for a 5 MB book (pages are drawn, not held as images) |
| Robustness | 60-book corpus opens; broken ones degrade without crashing; a fuzz pass on the zip and XML readers |
| Tests | Rust unit and corpus tests; Swift layout tests on recorded styled documents |

## 9.0 Decisions made (2026-10-24)

1. **Rendering: our own Core Text drawing.** Selection, highlights, VoiceOver and dictionary lookup are built by us from `CTLine` geometry. Phase 0 is no longer a choice between engines: it shrinks to a Core Text proof (page layout speed, selection hit-testing, VoiceOver per paragraph) to size phase 2.
2. **Books tab always shown.** Five tabs; test the bar width on the smallest iPhone.
3. **Free, no limits.** No subscription, no import limit.
4. **Backups include the book files.** There is no backup system for books yet; it comes later, and when it does it includes the files (so the schema keeps the file hash and size). Until then the plan's backup section is a design note only.
5. **Catalogues:** OPDS and local files (default taken, not asked). Wi-Fi upload page stays a later idea.
6. **Order:** engine first (phase 1), then the Core Text proof alongside layout (phase 2).

## 9. Risks and open decisions

1. **Core Text or TextKit 2** (section 4.2): decided by the spike.
2. **Selection, VoiceOver, dictionary** are free in `UITextView` and costly to rebuild. This is the largest cost of "drawing it ourselves". If the spike says the cost is high, we use the system text view for display and keep everything else ours. Please confirm this is acceptable: your instruction was a renderer of our own, and a system text *view* is not a reading library.
3. **Fixed layout and vertical/right-to-left text** (comics, Japanese novels). Real, but a small share of the books people open. Later; the app says so when it meets one rather than showing it wrongly.
4. **Copyright and App Review.** Same stance as manga: no content, no endorsement. Standard Ebooks, Gutenberg and the Internet Archive are the suggested catalogues in docs. The licence terms of each feed were not verified here (see sources); check before naming them in the app or the listing.
5. **Sync across devices.** Not in the first version. A local-first design with a locator that can become a CFI keeps the door open. Decide: iCloud, or a file the person moves, or KOReader sync.
6. **Tab bar room.** Five tabs on the floating bar: test the width; the Books tab is optional (Settings) so manga-only users see four.
7. **Scope creep.** Dictionary, translation, TTS, annotations export are each their own phase and can slip without breaking the core.

Decisions for you: (e) price: free with no limits, or a one-time purchase (Reddit is hostile to subscriptions and to limits on local features), (a) Core Text own selection vs system text view (risk 2), (b) is the Books tab shown from the start or only after the first book, (c) files in backups: never, always, or asked, (d) is OPDS the only catalogue kind, or also Calibre content server and Komga (Aidoku already does Komga, Kavita, Suwayomi).

## 10. Sources

- Reddit threads (read in full via TinyFish fetch): r/ebooks 1he8hgh, 1qvjlfg, 1ourirb, 1nqmeqv, 1tx8vi8, 1k4i29a; r/apple 14cjw4s; r/macapps 1qstqtv; r/ereader 1mg799q, 1qdgef1; r/readwise 1oueem7; r/readest 1tb3kaf; plus search results for dark mode, TTS, footnotes, privacy, KOReader sync (all under reddit.com, thread ids above)
- W3C EPUB 3.3 (container, package document, spine, navigation): <https://www.w3.org/TR/epub-33/Overview.html>; CFI: <https://w3c.github.io/epub-specs/epub33/epubcfi/>
- Native TextKit 2 EPUB viewer (pagination, CSS cascade, text map, limits): <https://github.com/vocaro/EPUBLib/pull/4>
- Core Text EPUB engine in an iOS reader: <https://github.com/kmworks/kmreader/pull/1279>
- TextKit 2 pagination limits and reports: Apple forum "Performance Issues with UITextView and TextKit 2" <https://developer.apple.com/forums/thread/729491>; Literature & Latte forum <https://forum.literatureandlatte.com/t/textkit-2-is-it-reliable/144184>; Michael Tsai on TextKit 2 <https://mjtsai.com/blog/2025/08/15/textkit-2-the-promised-land/>
- Reader comparisons (vendor-written; weak evidence): <https://justread.app/en/posts/best-epub-reader-apps-iphone-ipad-2026>, <https://getbookshelves.app/guides/best-epub-reader-iphone-ipad/>, <https://www.bookconv.com/blog/best-epub-reader-iphone-ipad>
- Readest (features, reviews): <https://apps.apple.com/us/app/readest-ebook-reader/id6738622779>
- Apple Books sync and import complaints: <https://discussions.apple.com/thread/8137580>, <https://discussions.apple.com/thread/254517709>, <https://talk.tidbits.com/t/how-apple-s-books-app-has-changed-in-ios-16/20125>
- EPUB compatibility pain (CSS, footnotes, fixed layout): <https://www.pgdp.net/wiki/CSS_features_not_in_EPUB>, <https://lists.w3.org/Archives/Public/public-digipub-ig/2014Jan/0005.html>
- OPDS catalogues: <https://opds-spec.org/2010/08/29/opds-catalogs-v1/>; Standard Ebooks feed <https://standardebooks.org/feeds/opds>; Gutenberg <https://m.gutenberg.org/ebooks.opds/>; Internet Archive <https://bookserver.archive.org/catalog/>

## 11. Status (2026-10-24)

Built and run in the simulator on six real books (Pride and Prejudice, Moby Dick, Frankenstein, War and Peace, Hamlet, Alice) and a
generated 8 MB single-file book: engine (EPUB, DRM refusal, contents nav and ncx, CSS subset, pictures, links, anchors, large files
read in pieces), schema 7 and facade, Books tab, shelf, import of several files at once, reader (Core Text pages, hyphenation,
themes, type settings, contents, scrubber, link trail), resume where you stopped. Decisions made: own Core Text drawing, Books tab
always shown, free, backups will include files (not built). Skipped for now by the owner: VoiceOver and dictionary lookup.
Not built: highlights and notes, search, footnote popovers, custom fonts and embedded fonts, OPDS catalogue, per-book settings,
text to speech, vertical and right-to-left layout, fixed layout.
