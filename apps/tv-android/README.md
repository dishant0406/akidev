# Akidev for Android TV

Akidev is the streaming app of this repository, made for Android TV. It shows films, series and anime, finds a copy through your TorBox account, and plays it on the TV. On the TV the app is called **Akidev**, and its package is `live.akidev.tv`.

It is not on the Play Store. Each version is a GitHub release, and the app updates itself from there.

## What you get

- **Home, Anime, Series, Films.** Rows of posters under one hero that follows the poster you are on. A "Continue watching" row brings you back to what you started.
- **Search.** Suggestions appear while you type. The TV keyboard opens only when you press OK on the field.
- **Title page.** Story, episodes as wide cards, and a Play button that resumes where you stopped.
- **Player.** Pick the source, the audio and the subtitles from one panel. The next episode is one press away.
- **Settings.** Enter your TorBox key (an eye button shows or hides it) and install app updates.

The key stays on the TV, encrypted, and goes only to TorBox. The player reads TorBox's link directly, so the app writes no video to the disk.

The look follows [`design/DESIGN_PLAN.md`](../../design/DESIGN_PLAN.md) and [`design/TV_PLAN.md`](../../design/TV_PLAN.md). The decisions behind the code are in [`docs/design.md`](../../docs/design.md), and the step-by-step flows are in [`docs/flow.md`](../../docs/flow.md).

## How it fits together

The screens are Kotlin with Compose for TV, and the player is Media3. Everything that talks to the network, such as catalogues, search, source lookup and the TorBox calls, is in a Rust engine inside the app.

| Folder | What it holds |
| --- | --- |
| `app/` | The screens, the player, the updater and the baseline profile |
| `engine/` | The Android library that wraps the compiled engine (generated, not committed) |
| `../../crates/tv-ffi/` | The engine's entry points that Kotlin calls |
| `../../crates/core/` | The code shared with the desktop app: providers, sources and caching |
| `fastlane/` | The release lane |
| `scripts/` | `build-engine.sh` and `bench.sh` |

**Sources.** Which sites to search is not in the app. A signed list is fetched from `https://sources.akidev.live` at each start. A new list applies at the next start, and Settings shows an amber dot when one is waiting. The copy built into the app is only for when the TV is offline.

## Set up

You need:

- The Android SDK and NDK (r28 or newer).
- JDK 17.
- `cargo-ndk`.
- The Rust targets for 32-bit and 64-bit ARM:

```bash
rustup target add armv7-linux-androideabi aarch64-linux-android
```

Many TVs run 32-bit code only (the KTC Lumio Vision this was built on does), so the engine is built for both CPU types.

## Build and test

Build the engine first. Run this again after you change anything in `crates/`.

```bash
apps/tv-android/scripts/build-engine.sh
```

Then, in `apps/tv-android`:

```bash
./gradlew :app:assembleDebug        # debug APK
./gradlew :app:testDebugUnitTest    # unit tests
./gradlew :app:lintRelease          # lint
./gradlew :app:assembleRelease      # shrunk APK, signed with the key in ~/.akidev-keys
./gradlew :app:bundleRelease        # the bundle for Google Play (not used today)
```

The Rust tests:

```bash
cargo test -p akidev-tv-ffi -p akidev-core
```

If your network inspects TLS, run `source ~/.gradle/aki-env.sh` first.

## Run on a TV

1. On the TV, turn on developer options and network debugging.
2. Connect and install:

```bash
adb connect <tv ip>:5555
adb install -r app/build/outputs/apk/release/app-release.apk
```

3. Compile the app for speed. Without this, a copy installed with adb starts about 0.5 seconds later and scrolls with about three times as many janky frames. Play does this on its own, adb does not.

```bash
adb shell cmd package compile -m speed-profile -f live.akidev.tv
```

To skip typing the key with the remote, use a debug build. It reads `files/torbox.key` once at start:

```bash
adb shell "run-as live.akidev.tv mkdir -p files"
printf '%s' "$KEY" | adb shell "run-as live.akidev.tv sh -c 'cat > files/torbox.key'"
```

To follow the app, run `adb logcat -s AkiTV`. It prints a timing line for each step and never prints the key or a link.

## Measure speed

Install a release build and save a TorBox key first. A debug build is far slower and says little.

```bash
TV=<tv ip>:5555 apps/tv-android/scripts/bench.sh 3
```

Add `start`, `scroll`, `screens` or `memory` to run one part. The script prints:

- The cold start, and when the first rows and pictures reach the screen.
- How smooth scrolling is.
- The time for a title page and a search.
- The time to the first video frame.
- Memory and garbage collections.

## Release a new version

A release makes a GitHub release named `tv-android-vX.Y.Z`. It holds the APK, its SHA-256 and notes made from the commits that touched the TV app.

1. Install Fastlane once, and set a token of the account that owns the repository (`dishant0406`, with contents write). Do not use the work account.

```bash
brew install fastlane
export GITHUB_API_TOKEN=<token>
```

2. Commit and push your changes to `main`. The lane stops if the tree is not clean.
3. Try it first. A dry run builds and tests, prints the notes, and publishes nothing:

```bash
cd apps/tv-android
fastlane release dry_run:true
```

4. Publish:

```bash
fastlane release
```

The first release uses the version in `app/build.gradle.kts`. Later ones add 1 to the patch number. To choose, add `bump:minor`, `bump:major` or `version:1.4.0`. Add `draft:true` to review the release before anyone sees it, and run `fastlane notes` to print the next version and the notes only.

The lane also commits the new version and pushes it to `main`.

## How a TV updates

1. The app looks at the GitHub releases when it starts and when you come back to it, at most every 30 minutes. It remembers the answer.
2. If a newer release exists, Settings shows an amber dot, then "Version X is ready" and a "Download and install" button.
3. The first time, Android asks you to allow installs from this app. The button opens that screen, and you press it again afterwards.
4. The app streams the APK straight into Android's installer and checks its SHA-256, so no APK is kept on the TV.
5. Android asks "Do you want to update this app?". The **Cancel** button has the focus, so press Left, then OK.
6. Android closes the app to update it. Open the app again from the TV home screen. Android 14 does not let an app reopen itself.

Android installs the file only if the same key signed it as the app on the TV, so no other file can replace the app. Keep `~/.akidev-keys` safe: with a new key, every TV would have to uninstall the app first, which also deletes its stored key.
