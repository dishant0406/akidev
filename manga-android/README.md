# Akidev for Android

The same app as `manga-app/` (iOS), written in Kotlin and Jetpack Compose. Manga and comics, EPUB books, reading aloud and
new-chapter alerts all run on the same Rust engine (`desktop/manga`, `desktop/manga-ffi`) through UniFFI bindings.
How it should look is written in `manga-app/DESIGN.md`; what it does is in `manga-app/USER_FLOW.md`.

## Layout

| Module | What it holds |
| --- | --- |
| `:design` | The Aki design language: tokens, glass (Haze), text, buttons, sheets, menus, tab bar, lists. Screens use only these. |
| `:engine` | The Rust engine as `libakimanga.so` (arm64) and its generated Kotlin. Both are built by a script and are not in git. |
| `:app` | Screens and their models, grouped by area: `manga`, `reader`, `books`, `listen`, `settings`, `sources`, `updates`, `app`. |

Each screen is a small composable plus a plain model class holding `mutableStateOf` fields (the same shape as an iOS `@Observable`).
Models that must keep their state while the person changes tab live in `AppModel`.

## Build and run

You need JDK 17, the Android SDK (platform 37, NDK r28), Rust with `cargo-ndk`, and an emulator or a phone with USB debugging.

```bash
./scripts/build-engine.sh   # builds the engine and its Kotlin bindings (first time, and after any Rust change)
./scripts/run.sh            # builds the debug app, installs it and opens it (add `clear` to start from nothing)
```

Checks:

```bash
./gradlew :app:testDebugUnitTest          # plain JVM tests: shelf order, book style, names
./gradlew :app:connectedDebugAndroidTest  # on a device: typesetting and pagination of a chapter
./gradlew lintDebug                       # lint, kept at zero warnings
./gradlew :app:assembleRelease            # R8 and baseline profile, about 16 MB
./scripts/size.sh                         # what Google Play would download for a phone, after :app:bundleRelease (about 7.5 MB)
./gradlew :app:generateReleaseBaselineProfile   # records the baseline profile again on a connected phone (needs the demo source installed, see below)
./gradlew :baselineprofile:connectedBenchmarkReleaseAndroidTest   # cold start with no profile, with it, and fully compiled
./scripts/startup.sh                      # cold start time on the phone or emulator (about 750 ms for a debug build on the emulator)
```

## Things worth knowing

- **Minimum Android 10 (API 29), phones with a 64-bit ARM processor.** Blur needs Android 12; before that, and with "Reduce glass" in
  Settings, every control is a solid surface with the same shape and border.
- **Menus and dialogs** draw in a layer at the root of the app (`AkiMenuLayer`, `AkiDialogLayer`) so their glass can blur the screen behind them. A sheet has a window of its own, so menus and dialogs opened from inside one use a popup or window of their own and a plain look.
- **Glass rules (Haze):** a glass control cannot sit inside the layer it samples. The backdrop is layer -1, scrolling content layer 0. A picture that glass sits on (a cover, a hero) is its own source at -0.5, so the glass shows the picture.
  Corners of glass shapes are fixed dp, not percent.
- **Fonts:** Nunito (rounded, for the interface) and Lora and Atkinson Hyperlegible (books) are bundled; licence in `FONTS-OFL.txt`.
- **Books** are laid out with `StaticLayout`, one per block, then cut into pages by `ChapterLayout`. Positions are the engine's UTF-16
  offsets, so highlights, notes and "where I stopped" mean the same on iOS and Android. Vertical Japanese text is not supported.
- **Reading aloud** uses the engine's speaker (Phonon) and an `AudioTrack`; a foreground service keeps it going with the screen off.
- **Speed.** Measured on a Galaxy A05 (cheap graphics chip), release build, 2026-10-27:
  - *Glass is what costs.* Each glass control on a moving page is blurred again every frame; six small pills over the reader's strip took 60 ms a frame. `AkiGlassBudget` (in `:design`) draws glass as a plain tinted fill while a list scrolls or a screen slides, and a moment after it stops the glass comes back. `GlassGovernor` turns this on by itself, only on a phone that is seen to miss frames while glass moves (half of 90 frames late), and remembers it; a phone that keeps up never changes. Scrolling containers tell the budget with `AkiGlassMotion(state.isScrollInProgress)`.
  - *The baseline profile is recorded, not written.* `baselineprofile/` walks the app (start, tabs, a series, the reader, settings) on a phone with the demo server's source and the plugin puts the result in `app/src/release/generated/baselineProfiles/`. Cold start with a warm graphics cache: 576 ms with nothing compiled, about 410 ms with the profile (fully compiled: 507 ms).
  - *The first start after an install or an update is slower* (about 1.4 s here): the phone's graphics driver compiles the shaders the first frame needs (42 of them, 1.2 s) and keeps them in `code_cache`. A benchmark that kills the app before the cache is written measures this every time, so `scripts/startup.sh` and the Macrobenchmark module (which drops the cache on purpose) show the first-start cost; leave the app running 8 seconds before timing a normal start.
  - Reads that grow with the collection (a series' chapters and downloads) and the reading position write run off the main thread.
  - The demo source needs `python3 manga-app/scripts/demo-server.py` and `adb reverse tcp:8770 tcp:8770`; release builds refuse plain http for pictures, so a release build for measuring needs the debug network config temporarily.
- **Release builds** never allow plain http. Debug builds allow it for the demo server on this computer.
- Nothing is sent to a server of ours. Notifications are posted by the app itself when Android lets it check for updates.
