# Aki Manga

The iPhone app for reading manga and manhwa: SwiftUI with Liquid Glass chrome (iOS 26), over the Rust
engine in `desktop/manga` through the uniffi facade in `desktop/manga-ffi`. The plan and the reasoning are in
[pipeline/MANGA_PLAN.md](../pipeline/MANGA_PLAN.md); the look follows [design/DESIGN_PLAN.md](../design/DESIGN_PLAN.md).

```
App/                    the SwiftUI app (Library, Browse, Series, Reader, Sources)
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
xcrun simctl launch booted live.akidev.manga -demoServer http://127.0.0.1:8770 -source local
```

`-demoServer` (debug builds only) installs the `madara` theme and a source from that server at start, and
`-source local` opens Browse on it. The pages are generated pictures; the HTML is the hand-written fixtures in
`desktop/manga/sources/fixtures/madara-example`.

Other debug-only launch arguments for scripted runs: `-autoSeries "local|solo-leveling|Title"` opens straight on
a series, `-autoContinue YES` then opens the reader where it left off, and `-mode.<source>.<series> strip|paged|rtl`
picks the reading mode. `DEMO_LOG=1 python3 scripts/demo-server.py` prints every request.

## What the app ships with

MangaDex and nothing else. Other sources are added by the address of a definition (Sources tab, plus). The
app offers and endorses none.
