# Akidev TV (Android TV)

The streaming app for Android TV. Kotlin and Compose for TV for the screens, Media3 for the player, and the Rust engine
(`crates/tv-ffi` over `crates/core`) inside the app. The viewer types their own TorBox key in Settings. The key is kept
encrypted on the TV and goes only to TorBox. The player reads TorBox's link directly: nothing is written to the disk.

Look and feel: `design/DESIGN_PLAN.md` and `design/TV_PLAN.md`. The icons are Solar (`python3 content/manga/fetch-icons.py --tv`). Decisions: `docs/design.md`. Step by step: `docs/flow.md`.

## Build

You need the Android SDK and NDK (r28 or newer), `cargo-ndk`, and `rustup target add armv7-linux-androideabi aarch64-linux-android`.
Many TVs are 32-bit only (the one this was built on is), so the engine is built for both CPU types.

```bash
apps/tv-android/scripts/build-engine.sh
```

Then, in `apps/tv-android` (see `~/.gradle/aki-env.sh` if your network inspects TLS):

```bash
./gradlew :app:assembleDebug        # debug APK
./gradlew :app:testDebugUnitTest    # unit tests
./gradlew :app:lintRelease          # lint
./gradlew :app:assembleRelease      # shrunk APK, signed with ~/.akidev-keys when it exists
./gradlew :app:bundleRelease        # the bundle for Google Play
```

The Rust tests: `cargo test -p akidev-tv-ffi -p akidev-core`.

## Measure speed

With a release build installed (a debug build is far slower and says little) and a TorBox key saved:

```bash
TV=<tv ip>:5555 apps/tv-android/scripts/bench.sh 3          # everything; or add: start | scroll | screens
```

It prints the cold start, when the home rows and the first pictures reach the screen, the smoothness of scrolling (the system's frame
times), a title page, a search, and the time to the first video frame. The app writes the same marks to `adb logcat -s AkiTV`
(`perf ...` lines). The findings so far are in `docs/design.md`.

## Try it on a TV

Turn on developer options and network debugging on the TV, then `adb connect <tv ip>:5555` and `adb install -r app/build/outputs/apk/debug/app-debug.apk`.
A debug build reads `files/torbox.key` once, so a test can set the key without typing it:

```bash
adb shell "run-as live.akidev.tv mkdir -p files"
printf '%s' "$KEY" | adb shell "run-as live.akidev.tv sh -c 'cat > files/torbox.key'"
```

`adb logcat -s AkiTV` shows a timing line for each step of finding a copy. It never shows the key or a link.
To type into the app from adb: `adb shell input keyevent KEYCODE_DPAD_CENTER`, `adb shell input text <words>`.

## Speed on a TV that you install with adb

The baseline profile (`app/src/main/baseline-prof.txt`) is compiled by the system. Play does it at install. For a copy installed with `adb install`, run the app once and then:

```bash
adb shell cmd package compile -m speed-profile -f live.akidev.tv
```

Without it the first minutes are slower (start 0.5 s later, about three times the janky frames while scrolling). `scripts/bench.sh [runs] [start|scroll|screens|memory]` measures the app.

## Releasing

```bash
brew install fastlane               # once
export GITHUB_API_TOKEN=...         # a token of the repository owner (dishant0406), contents write; not the work account
cd apps/tv-android
fastlane release dry_run:true       # builds, tests, prints the notes; changes and publishes nothing
fastlane release                    # next patch version (also: bump:minor, bump:major, version:1.4.0, draft:true)
```

The release is `tv-android-vX.Y.Z` on GitHub with the APK, its SHA-256 and the notes. Installed TVs find it by themselves (Settings shows "Download and install"). See `docs/design.md`, "Releasing the TV app".
