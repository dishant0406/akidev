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
./gradlew :app:assembleRelease            # R8 and baseline profile, about 17 MB
```

## Things worth knowing

- **Minimum Android 10 (API 29), phones with a 64-bit ARM processor.** Blur needs Android 12; before that, and with "Reduce glass" in
  Settings, every control is a solid surface with the same shape and border.
- **Glass rules (Haze):** a glass control cannot sit inside the layer it samples. The backdrop is layer -1, scrolling content layer 0.
  Corners of glass shapes are fixed dp, not percent.
- **Fonts:** Nunito (rounded, for the interface) and Lora and Atkinson Hyperlegible (books) are bundled; licence in `FONTS-OFL.txt`.
- **Books** are laid out with `StaticLayout`, one per block, then cut into pages by `ChapterLayout`. Positions are the engine's UTF-16
  offsets, so highlights, notes and "where I stopped" mean the same on iOS and Android. Vertical Japanese text is not supported.
- **Reading aloud** uses the engine's speaker (Phonon) and an `AudioTrack`; a foreground service keeps it going with the screen off.
- **Release builds** never allow plain http. Debug builds allow it for the demo server on this computer.
- Nothing is sent to a server of ours. Notifications are posted by the app itself when Android lets it check for updates.
