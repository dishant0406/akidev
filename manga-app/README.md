# Aki Manga

The iPhone app for reading manga and manhwa: SwiftUI with Liquid Glass chrome (iOS 26), over the Rust
engine in `desktop/manga` through the uniffi facade in `desktop/manga-ffi`. The plan and the reasoning are in
[pipeline/MANGA_PLAN.md](../pipeline/MANGA_PLAN.md); the look is described in [DESIGN.md](DESIGN.md) (bright, rounded, yellow, glass; not the dark look of the website).

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
(or the id in `-demoInstall`); `-source local` opens Browse on it. `/_demo/sources/` serves the real repository
folder, `manga-sources/`, so `-demoRepo http://127.0.0.1:8770/_demo/sources/ -demoInstall mangadex` installs the
real MangaDex definition (the site itself is not reachable from the demo). The pages are generated pictures; the
HTML is the hand-written fixtures in `manga-sources/fixtures/madara-example`.

Other debug-only launch arguments for scripted runs: `-autoSeries "local|solo-leveling|Title"` opens straight on
a series, `-autoContinue YES` then opens the reader where it left off, and `-mode.<source>.<series> strip|paged|rtl`
picks the reading mode. `DEMO_LOG=1 python3 scripts/demo-server.py` prints every request.

## What the app ships with

Nothing. Aki Manga is a renderer: every source is a plugin, installed from a repository the person adds by address
(Sources tab). It has no built-in source, no default repository, and offers and endorses none. The repository that
holds the plugins we write is [manga-sources](../manga-sources/README.md).
