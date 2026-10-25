# Akidev

The iPhone app for reading manga and manhwa: SwiftUI with Liquid Glass chrome (iOS 26), over the Rust
engine in `desktop/manga` through the uniffi facade in `desktop/manga-ffi`. The plan and the reasoning are in
[pipeline/MANGA_PLAN.md](../pipeline/MANGA_PLAN.md); the look is described in [DESIGN.md](DESIGN.md) (bright, rounded, yellow, glass; not the dark look of the website). Every screen and flow, with what is built and what is next, is in [USER_FLOW.md](USER_FLOW.md).

```
App/                    the SwiftUI app (Home, Library, Updates, Series, Reader, Sources)
Packages/AkiManga/      the Swift package: the generated bindings and the xcframework (both generated)
scripts/build-ffi.sh    builds the Rust engine for device and simulator and fills the package
scripts/demo-server.py  a fake Madara site for driving the app with no real site
project.yml             the Xcode project (xcodegen); AkiManga.xcodeproj is generated, not committed
```

## Build and run

```bash
brew install xcodegen          # once
./manga-app/scripts/build-ffi.sh
cd manga-app && xcodegen generate
xcodebuild -project AkiManga.xcodeproj -scheme AkiMangaApp -destination 'platform=iOS Simulator,name=iPhone 17' -derivedDataPath build build
```

Run `build-ffi.sh` again after any change in `desktop/manga` or `desktop/manga-ffi` (it regenerates the Swift
bindings from the Rust code, so they cannot drift). The scheme is `AkiMangaApp`: `AkiManga` is the package.

## Driving it without a real site

```bash
python3 manga-app/scripts/demo-server.py 8770
xcrun simctl launch booted live.akidev.manga -demoRepo http://127.0.0.1:8770/_demo/repo/ -source local
```

The server is a fake Madara site and a repository (`/_demo/repo/`: an index naming the madara theme and a source
called `local`, with hashes). `-demoRepo` (debug builds only) adds that repository at start and installs `local`
(or the id in `-demoInstall`); `-source local` opens Browse on it. `/_demo/sources/` serves the demo repository
folder, `desktop/manga/testdata/`. The pages are generated pictures; the
HTML is the hand-written fixtures in `desktop/manga/testdata/fixtures/madara-example`.

Other debug-only launch arguments for scripted runs: `-autoSeries "local|solo-leveling|Title"` opens straight on
a series, `-autoContinue YES` then opens the reader where it left off, and `-mode.<source>.<series> strip|paged|rtl`
picks the reading mode. `DEMO_LOG=1 python3 scripts/demo-server.py` prints every request.

## Books

The Books tab reads EPUB files from the device. The engine (`desktop/manga/src/book`) opens the zip, reads the package, contents
and chapters, and turns a chapter's XHTML and CSS into blocks of styled text with positions; the app (`App/UI/BookLayout.swift`,
`BookPage.swift`) sets those in type with Core Text, paginates on a background queue and draws the pages itself: no web view and no
reading library. A format is a `BookFile` in the engine, so another format is one more implementation. Debug-only launch arguments:
`-importBooks /folder` adds every EPUB in it, `-openBook 2` opens the third book on the shelf. Tests: `cargo test -p akidev-manga --test book`;
with `AKI_BOOK_CORPUS=/folder cargo test -p akidev-manga --test book corpus -- --ignored --nocapture` every EPUB in a folder is read and timed.

## Reading aloud

A book can be read aloud by a voice model that runs on the phone (Phonon 31M today). Models come from a **voices repository**, a
folder with the same signed-index layout as the source repositories (`desktop/manga/src/repo.rs`, `voices/<id>.json`); the one
the app asks is [dishant0406/akidev-voices](https://github.com/dishant0406/akidev-voices), which is the only address built in
(models are not content). Settings asks it when Settings is opened, never at launch, and shows Reading aloud only if it answered.
A model is downloaded, checked (it must say a test sentence) and kept in `voices/<id>/`; after that reading needs no network.
The engine side is `desktop/manga/src/voice.rs` (definitions, download, speaking with the `ptts` crate) and
`src/book/speech.rs` (a chapter's sentences with their places); the app side is `App/Reading/` (`BookListener` makes the speech a
few sentences ahead and queues it on an `AVAudioEngine`; the page follows the sentence). Adding a model needs no app release as long
as it is a kind the app runs (`family`, only `ptts` now): see `pipeline/TTS_PLAN.md`, section 11. Debug builds take
`-voicesRepo http://127.0.0.1:8771/` to read another repository; `AKI_VOICE_MODEL=/folder cargo test -p akidev-manga --release --test voice -- --ignored`
runs the test that needs a real model.

## Manga first or books first

A fresh install asks "What do you read?" (`App/OnboardingView.swift`): Manga and comics, or Books. Both are always in the app and have the
same features; the answer only decides which comes first: the order of the tabs (`AppTab.order(for:)`, Sources always last), what the
first tab is (Discover for manga; for books `Books/BooksHomeView.swift`: continue reading, not started, a shelf per category, catalogues),
what the tabs are (for books: Home, Library, Browse, Manga, Sources, with the manga screens in one `MangaHubView`; only the five tabs of the mode are
declared, because the system folds a sixth into "More"), and which section leads Settings and the Sources screen.
It is changed in Settings, Reader, "I mostly read". A phone that already has sources, repositories or books when this update arrives is
not asked and keeps manga first. Books share the categories series use (`book_category`, schema 10), and a book can have a cover of its own
(`book.custom_cover`, kept beside the book's own).

## Speed: what was measured (2026-10-25)

Measured on the simulator (a Mac's processor, so the numbers are for comparing, not for a phone), Release builds, median of 5 to 9 cold launches,
with a log line for each step. Launch is about 230 to 270 ms to the app's first line and 300 to 340 ms to the first screen; a blank SwiftUI app
built the same way reaches its first screen at 280 ms, so the app's own part of launch is small. What was found and changed:

| What | Before | After |
| --- | --- | --- |
| Launch cost of the lock screen's framework (MediaPlayer was linked, loaded by every launch) | +20 to 26 ms at launch | 0: it is loaded when a book is first read aloud |
| First page of a 3.7 MB, 30-chapter book (time from opening to the first page) | median 88 ms (the next two chapters were set in type at the same time) | 63 ms (they wait for the first page) |
| `library()` and `updates()` asked again with nothing written (a 400-series library; the app asks 3 times in a launch and on every tab) | 6.9 ms and 6.2 ms each time | 0.03 ms (answer kept until a write) |
| Everything else timed (opening the engine 1 ms, a series' chapters 0.04 ms, a tab's first frame, scrolling a 300-series library at 60 frames a second with no frame over 16.7 ms, 53 ms of queries in a launch) | already fast | unchanged |

`desktop/manga-ffi/tests/perf_bench.rs` times the engine's reads (`cargo test -p akidev-manga-ffi --profile ios --test perf_bench -- --nocapture --ignored`).
The rest of a launch is the system: the 5 ms engine start, the first SwiftUI layout, and the first request for the Discover lists.

## What the app ships with

Nothing. Akidev is a renderer: every source is a plugin, installed from a repository the person adds by address
(Sources tab). It has no built-in source, no default repository, and offers and endorses none. The repository that
holds the plugins we write is the separate repository [dishant0406/manga-sources](https://github.com/dishant0406/manga-sources).

One source can also be pasted in on its own (Sources tab, Add a source): it is checked, its name and languages are
shown, and themes it extends are merged in so it works without the repository that has them.
