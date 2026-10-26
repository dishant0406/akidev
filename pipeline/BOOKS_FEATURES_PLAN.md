# Books: five features readers ask for (time left, pronunciations, ruler and RSVP, stats, characters)

Written 2026-10-26. A plan, not a build. Nothing in the app has changed for it.

## 1. Where this comes from

A 2026-10-25 pass over Reddit (through the TinyFish search and fetch APIs; X gave almost nothing, its profile and reply pages are
behind a login). Threads read: r/kindle "what feature do you wish Kindle would add", r/Fantasy "ideal book tracking app", r/books "do you
read books with your friends", r/noveltranslations "how do you feel about TTS", r/readwise feature requests, r/koreader (an X-Ray
plugin thread), r/kobo, r/languagelearning. Reddit is self-selected, so what follows is "came up often", not a count.

| Ask | What people wrote (paraphrased) | Feature here |
| --- | --- | --- |
| How long is left | Time left in the chapter and in the book, on every e-reader forum. | 3.1 Time left |
| Listening that does not trip | Built-in TTS "bugs out every sentence", reads `-----` aloud, mispronounces names; the one app praised for a pronunciation list is a paid one. | 3.2 Pronunciations |
| Help for focus | Line guides and one-word-at-a-time reading for ADHD and dyslexia. | 3.3 Ruler and RSVP |
| Stats that feel good | StoryGraph is loved for stats and the progress bar; Kindle owners ask for hours per book and speed over time. | 3.4 Stats |
| Who is this again | A character lookup that only knows the book up to where you are (Kindle's X-Ray; a KOReader plugin people ask for). | 3.5 Characters |

Not in this plan, on purpose: a TikTok-style book scroll and AI-written summaries (the reaction to AI-made content was strongly
negative), reading with friends (needs accounts and a server; see section 6), cloud anything. Everything below runs on the phone and
nothing about a book leaves it.

## 2. Principles

1. **On the phone.** No server, no account, no network. Text is analysed in the Rust engine (`desktop/manga/src/book/`), never sent anywhere.
2. **Quiet.** The reader is mostly text. New things hide in the bars and sheets that already exist (the reader bar, the book details sheet,
   the listen bar). Nothing new is on screen until the person turns it on, except the one line of time left.
3. **Reuse.** Screens use only Aki components (`scripts/check-ui.sh`); look follows `manga-app/DESIGN.md`. The engine already cuts chapters
   into sentences with UTF-16 positions (`book/speech.rs`), which highlights, search and reading aloud share: every feature below uses
   those same positions, so nothing needs mapping.
4. **No launch cost.** No new work at launch and no new framework import at launch (no Swift Charts; bars are drawn with Aki shapes).
   Anything that walks a whole book runs once, in the background, and is kept.
5. **Additive.** New columns and tables only (schema 11 onward), each with a default so an old library opens unchanged. The App Store
   build in review is untouched; this ships to TestFlight first.

## 3. The features

### 3.1 Time left (and the sums that stats share)

**What the person sees.** The reader's footer shows "12 min left in chapter"; a tap switches it to "3 h 20 min left in book", another to the
page number. The same figure shows on the book's card in the library ("5 h left") and, when listening, on the lock screen as the book's
duration and position.

**How it works.**
- Words per chapter. The engine has `chapter_sizes` (bytes, used as weights for progress), which is not time. Add
  `BookReader.chapter_words()`: counts the words of each chapter with UAX-29 word bounds (the crate `speech.rs` already uses), skipping
  footnote marks like it does. Counted once, in the background, the first time the book is opened, and kept as `book.words` (total) and
  `book.chapter_words` (JSON array). Until it is ready, the footer shows nothing rather than a guess.
- Speed. Learned from the person, per person, not per book: an average of words per minute from page turns, counting only turns where the
  page was on screen between 2 s and 3 min and the move was one page forward (jumps, searches and scrubs are not reading). Starts at 238
  wpm (the usual adult average) and becomes the person's own after five samples; a moving average, so it follows the book (a dense
  textbook is slower than a thriller). While listening, the figure uses the voice's measured rate instead.
- Position. Time left in chapter = words after the current place in the chapter ÷ speed; the place's word offset comes from the page's
  character offset and the chapter's words (a cumulative count per sentence is not needed: words are counted from the layout's text up to
  the page start, one pass when a chapter is laid out).

**Schema 11.** `book.words INTEGER`, `book.chapter_words TEXT`, `book.finished_at INTEGER` (stats needs when a book was finished, and
`finish_book` / `set_book_progress` set it), table `reading_session` (see 3.4), and settings key `reading_speed` (wpm, float).

**Tests.** Rust: words of a chapter with footnote marks, hyphens, soft hyphens, CJK (counts characters ÷ 2 as a rough word, flagged in a
comment); speed average ignores outliers. Swift: footer formatting ("45 s", "12 min", "3 h 20 min").

**Risk.** Word counts of a 500 000-word book cost a full parse: measure; if it is over ~2 s on a phone, count only when idle and show "…"
until done.

### 3.2 Pronunciations (and symbols that are not read)

**What the person sees.** While listening, press and hold a word on the page → **Say it differently**: a small sheet with the word, a
box for how it should sound ("Hermione" → "her-my-oh-nee"), a **Hear it** button that speaks it now with the voice in use, and a switch
for **This book only / All books**. Settings → Reading aloud → **Pronunciations** lists them for editing and deleting.

**How it works.**
- Table `pronunciation (word TEXT NOT NULL, say TEXT NOT NULL, book TEXT NULL REFERENCES book(id) ON DELETE CASCADE, PRIMARY KEY (word, book))`
  (schema 12). `word` is stored lower-case, `book` NULL means all books.
- Applied in the engine, not in Swift: `speech.rs` gains `fn pronounce(text: &str, rules: &[(String, String)]) -> String`: whole words only
  (UAX-29 bounds), case-insensitive, the longest rule first, and the **position of the sentence is unchanged** (only `text`, what is
  spoken, changes), so highlighting is not disturbed. `BookReader.speech(chapter, book)` returns units with the rules applied; the
  listener re-fetches when a rule changes.
- Symbols. A new step in `units()`: a run of three or more of `- = * _ ~ # +` (or a block made only of them) is not read, and a block
  that is only `* * *` or a rule becomes a **pause** (like a heading's 0.6 s) instead of text. A block with nothing left to say already
  produces no unit (`a_block_with_nothing_to_say_has_no_unit`), so this is a filter before it.
- Not now: numbers, dates and currency (the Phonon model card lists them as weak). A small normaliser ("1,200" → "twelve hundred") is a
  separate step if listening tests show it matters.

**Tests.** `pronounce`: possessives ("Hermione's"), hyphenated words, case, overlapping rules, accented letters, positions unchanged;
symbol runs; scene breaks. Ffi test: a rule for a book does not apply to another.

### 3.3 Reading ruler and RSVP

Two separate tools for focus, both off by default, both in the reader's type menu (next to size, spacing, theme).

**Ruler (line focus).** A soft band over one line (or three) with the rest of the page dimmed. Drag it down the page; it follows the last
touch, and **Pacer** moves it to the next line at the person's speed (from 3.1) so the eye is led. How: the page already knows its
lines (Core Text layout in `ChapterLayout`); the band is a rect of one of them, the dim is one `CAShapeLayer` with an even-odd path above
`BookPage`'s text layer, redrawn on drag. A page turn puts the band on the first line. `BookStyle.ruler` = `.off / .line / .three`,
`pacer: Bool` (missing = off, per the "missing is the default" rule in `BookStyle`). With Reduce Motion the pacer steps instead of
gliding.

**RSVP (one word at a time).** From the reader menu: **Speed read**. A full-screen, calm page: one word at a fixed place, the letter
the eye should land on (the "optimal recognition point", about a third in) in the accent, everything else the reading theme's ink. Tap to pause (it
steps back three words so you are not lost), swipe up/down for speed (200–900 wpm in steps of 50, starting at 300, ramping up over the
first ten words), longer holds on commas, full stops and long words. It starts from the page being read and, when closed, the book
stays where RSVP got to (position saved like any reading, and counted in stats as reading).
- Engine: `BookReader.words(chapter, from, count) -> [BookWord]` (word text, UTF-16 start and length), UAX-29 word bounds, footnote marks
  skipped, the same positions as everything else; RSVP asks for the next 200 and keeps going into the next chapter.
- Amber is for state in `design/DESIGN_PLAN.md`; in the manga app's own look the accent is its yellow. The point is state ("this
  is where to look"), so it fits either.

**Tests.** `words`: hyphenated, apostrophes ("don't"), numbers with separators, an emoji, positions match `units`. Swift: the
timing function (pause lengths per word) is a pure function and gets a test.

**Risk.** RSVP is a feature people love or hate; it stays one menu item, never a default. Measure that the ruler does not slow page
turns (the overlay is one layer, redrawn on drag only).

### 3.4 Reading stats

**What the person sees.**
- On the Books home: a card "This week": minutes read, a seven-day strip (bars, the day's minutes, today marked), the streak.
- Tap → **Reading** page: today, week, month, year; books finished this year; words read; average speed by month; longest streak;
  daily goal (optional, minutes) and the streak counts days that reached 5 minutes or the goal.
- On a book's details sheet: time spent, started on, finished on, average speed, sessions.
- Year in books: books finished (cover grid) as a shareable image? Not now (see section 6).

**How it works.**
- Table `reading_session (id INTEGER PRIMARY KEY, book TEXT NOT NULL REFERENCES book(id) ON DELETE CASCADE, day INTEGER NOT NULL,
  seconds INTEGER NOT NULL, words INTEGER NOT NULL, listened INTEGER NOT NULL DEFAULT 0)`, with an index on `day`. One row per book per
  day per kind (read / listened) that is **added to**, not one row per visit, so the table stays small (a year is under 1 000 rows).
  `day` is the person's local day number.
- `BookModel` runs the clock only while the page is on screen, the app is active and the screen was touched in the last 2 minutes (the
  same clock that feeds the speed in 3.1). `BookListener` adds listening time while `.playing`. The model flushes every 30 s, on
  background and on close.
- Engine: `Store.add_reading(book, day, seconds, words, listened)` and queries `reading_days(from, to)`, `reading_totals(book?)`;
  the streak is computed in Swift from the day list (a pure function, tested).
- Bars are drawn with Aki shapes: no Swift Charts import, so launch and binary size do not change.

**Tests.** Store: adding to the same day merges; deleting a book removes its rows; totals. Swift: streak (gaps, today not yet read,
time-zone change → days are stored as local day numbers and not recomputed), formatting.

**Privacy.** Local only, shown to the person only, never uploaded. Removing a book removes its stats; a "Clear reading history" row
in Settings clears all. Open gap, already known: books' state (place, categories, highlights) is not in backups yet, so neither
would stats be; do it with that fix, not before.

### 3.5 Characters ("Who is this?")

**What the person sees.** In the reader menu, **Characters**: a list of the names in the book **up to where the person has read**, in the
order they first appear, each with how many times it was mentioned. Tap one → a card: the passage it first appeared in (tap to jump), the
last three mentions before here, the chapters it is in (a small strip), the names it most often appears with. Press and hold a name in the
text → the same card. Nothing after the furthest point read is used, so it cannot spoil. The person can add a note to a name ("the
baker's wife"), which is theirs and stays on the phone. Named **Characters** in the app, not X-Ray (a Kindle trademark).

**How it works (no machine learning, so no model to ship and nothing to download).**
- The engine reads each chapter once and collects **proper-noun candidates**: a capitalised word, or a run of capitalised words, that is not
  at the start of a sentence, plus capitalised words at a sentence start that **also** appear capitalised in the middle of a sentence
  somewhere. A word that appears in lower case elsewhere in the book ("Will" / "will", "March" / "march") is dropped unless it
  is overwhelmingly capitalised. Titles ("Mr.", "Dr.", "Captain") join the name after them; "of", "the", "de", "van" join a run when
  they sit between capitalised words ("Duke of Bridgewater").
- Aliases: a one-word name that is the first or last word of a longer name seen in the same chapter ("Harry" ↔ "Harry Potter") is
  merged with it. Names mentioned fewer than three times are not listed.
- Each name keeps its positions (chapter, UTF-16 offset), the same positions as highlights, so jumping and marking use what exists.
  The result is one blob per book, kept in `book_index (book TEXT PRIMARY KEY, version INTEGER NOT NULL, body BLOB NOT NULL)` (schema 13),
  rebuilt if `version` is older than the algorithm, built in the background the first time the person opens Characters (not at
  import, not at launch).
- Spoiler safety: the app keeps `furthest`, the furthest place read (progress is written as the current place, which goes backwards
  when the person jumps back; `furthest` only goes forwards, reset with "Reset book"). Queries take it as a bound.
- Only for languages written with capitals (Latin, Greek, Cyrillic); in Japanese, Chinese, Korean and so on the menu item is hidden.

**Tests.** Rust, on small fixture chapters: sentence-initial words are not names; "Will"/"will"; titles; "Duke of X"; aliases; spoiler
bound (a name only after `furthest` is absent, a name before has no count past it). A longer real test on an out-of-copyright novel
to look at the top 30 names by eye (this is the only honest check of precision).

**Risks and honesty.** A heuristic makes mistakes: it lists a place as a person, merges two characters with a shared surname, misses
a name written in lower case. The list needs a **Not a name** action (stored, hides it) and **Merge with…**. It does not describe
anyone: there is no generated text. A later experiment, not part of this plan: Apple's on-device model (Foundation Models, only on
devices that support it) for "Previously on…" recaps of the text read so far, behind an availability check and off where unavailable.

## 4. Order and size

| Phase | What | Schema | Rough size | Depends on |
| --- | --- | --- | --- | --- |
| A | Word counts, speed, time left, sessions, stats | 11 | Largest: engine counts, store, clock in `BookModel` / `BookListener`, home card and page | none |
| B | Pronunciations and symbols | 12 | Small: one engine function, one table, one sheet | none |
| C | Ruler, then RSVP | none | Medium: overlay layer, then `words()` and a screen | speed from A (for the pacer; the ruler works without it) |
| D | Characters | 13 | Medium: index, background build, list and card | none |

A first (it is the foundation and the most asked-for), B second (smallest, most visible to people who listen), then C, then D.
Each phase is its own change: tests, run on iPhone **and** iPad (centred columns, no stretching), README and `USER_FLOW.md`
updated, committed, and only pushed and published when the owner says so.

## 5. How each phase is checked

1. Engine tests (`cargo test` for `manga` and `manga-ffi`), including the new table and the migration from schema 10.
2. Built and run in the simulator on a real EPUB: the new screens, then `scripts/check-ui.sh`.
3. Measured, not assumed: launch time unchanged (nothing new runs at launch); word counting and the character index on the longest
   book available; page-turn latency with the ruler on. Numbers go in the commit message and this file.
4. Fresh-eyes review of the diff by a separate pass before it is committed.
5. No temporary logging or debug code left behind.

## 6. Left out, and why

- **Read with friends / buddy reads**: wanted in r/books (trade reactions at chapter 5, predictions, "you were right!"), but it needs
  accounts, a server and moderation. If it ever happens it should be spoiler-aware like 3.5, and shared through a link or file rather
  than a service. Parked.
- **Sharing a stats image or quote card**: needs a rendering pass and sharing UI; after A and the highlights work are in.
- **Word-level read-along**: the Phonon voice gives audio, not word timings; sentence-level marking stays.
- **Manga stats**: manga reading history already exists; counting chapters and time there is a separate question.
- **Language-learning cards (tap a word, make a card)**: wanted, and could grow out of 3.2 and 3.5, but it is its own feature.

## 7. Questions for the owner

1. Where should **Reading stats** live: a card on the Books home (as drawn above), or its own tab or a row in Settings?
2. RSVP: ship it with the first version of the ruler, or hold it until the ruler has been used?
3. Name for 3.5: **Characters** (as above), or **Who's who**?
4. Should stats also count listening time in the daily total (planned: yes, shown as a separate figure beside reading)?
