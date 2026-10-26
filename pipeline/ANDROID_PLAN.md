# Akidev on Android: the same app, native Kotlin and Compose, with the same liquid glass

Written 2026-10-26. A plan with research, not a build. Nothing in the repo has changed for it.

## 1. What is asked

An Android app that looks and feels like the iOS app (`manga-app/`): the same screens, the same Aki components, the same
glass on the floating tab bar, buttons, pills and reader bars, **native** (Kotlin, Jetpack Compose, no web view, no cross-platform
UI toolkit) and as fast and small as we can make it. Same product, so the same rules: the app ships no content, runs the Rust
engine, reads manga and EPUB books, reads books aloud on the device.

Say plainly what "the same" can mean. On iOS 26, Liquid Glass is **made by the system**: `glassEffect` is Apple's own refraction,
and every app gets the same material. On Android nothing like it is in the platform, so the glass is **drawn by us** (a shader
over a copy of what is behind), through a library. It can look very close; it will not be pixel-identical to Apple's, and it costs
the GPU something Apple's does not cost the app. The plan is built around that: a spike first (section 5, phase 0), a thin layer of our own
over whichever library wins, and three tiers so cheap phones are not made slow for a look.

## 2. What the research found

Sources: TinyFish search and fetch over GitHub, the libraries' own docs, Android developer docs and Reddit, 2026-10-26.

### 2.1 Glass libraries for Compose

| Library | What it is | Needs | Notes |
| --- | --- | --- | --- |
| **Backdrop** (Kyant0/AndroidLiquidGlass, Apache-2.0, 4.3k stars, 286 commits, Compose Multiplatform) | Draws a copy of the background into a foreground shape with a chain of effects: colour filter (opacity, brightness, contrast, saturation, `vibrancy()`, exposure), then `blur(radius)`, then **`lens(refractionHeight, refractionAmount, depthEffect, chromaticAberration)`**. No high-level components; you build them (docs have a glass bottom bar, an interactive bottom bar that responds to touch, a bottom sheet, a slider, progressive blur, smoother rounded corners). | Blur: API 31. Lens and gamma: **API 33**, and the shape must be corner-based. `rememberLayerBackdrop` + `Modifier.layerBackdrop` on the content; `drawBackdrop(...)` on the glass. | The closest to Apple's look (lens + depth + chromatic aberration). Glass on glass needs the `exportedBackdrop` parameter; doing it the wrong way crashes with SIGSEGV on the RenderThread (its FAQ says so). The author calls it "relatively high performance"; no numbers published. |
| **Haze 2** (chrisbanes/haze, Compose Multiplatform) | `hazeSource` marks the content, `hazeBlur` and **`hazeGlass`** draw it: refraction, depth blur, tint, lighting, pointer/focus/press responses, `GlassStyle.regular` (diffuse, adapts to size) and `.clear` (shallow blur). `HazePerformanceMode` (Default = Balanced, Performance, Quality, Adaptive). Reduced-transparency, increased-contrast and visible-border accessibility settings built in. Material 3 modules. | Glass is still **experimental** in 2.0; stabilisation aimed at 2.1. Performance guide says measure with Macrobenchmark on a release build on the slowest device. | The most carefully maintained and the best documented for performance. Its Glass look is softer than Backdrop's lens; confirm by looking. Check its licence at adoption (not read in this pass). |
| **Cloudy / Mirage** (skydoves) | Blur and liquid glass modifiers, `Sky` holds the captured background; CPU fallback for older phones. | Lens refraction, progressive blur and the moving light need API 33; below that: tint + edge light + shape, no refraction. | Younger, grew from a blur library into a general effect library; useful as a source of ideas for the fallback tier. |
| KMPLiquidGlass, gaze-glassy | Forks and downstreams of Backdrop for Compose Multiplatform. | | No reason to use over Backdrop itself. |
| Own AGSL shader | `RuntimeShader` (API 33) over a `RenderEffect`; people have published single-file liquid glass shaders (a r/androiddev thread; one comment warns that capturing the bitmap "could become very expensive if you do not use the api correctly"). | API 33 | Most control, most work and most risk. Only if the libraries cannot meet the look or the speed. |

Facts that shape the design (from Android's developer docs and the libraries): `RenderEffect` blur is API 31; `RuntimeShader`
(AGSL) is API 33; below 33 there is no way to refract, only to blur (31+) or to fake it with a tint. Compose has no
built-in backdrop blur (the Compose issue for it is open); all libraries above do it by drawing the content once into a graphics
layer and sampling that. Cost scales with **how much area is covered by glass, how many glass surfaces there are, and whether what is
behind them moves** (a still source can be cached and reused; a scrolling one cannot).

### 2.2 How many phones can do what

Android version share, from two sources that disagree a little (measurement differs): Capgo's chart (December 2025): API 29+ 90.7%,
API 31+ 69.2%, **API 33+ 57.8%**, API 35+ 26.7%. StatCounter, September 2026: Android 16 37.65%, Android 15 15.93%, Android 13
11.58%. So by now well over half of phones can run the full lens effect, and a large minority cannot. That is why the glass has tiers.

### 2.3 The Rust engine on Android

- The engine is already a **uniffi** crate (`desktop/manga-ffi`, crate type includes `cdylib`). UniFFI generates **Kotlin** bindings
  the same way it generates Swift; Android builds go through **cargo-ndk**. No rewrite: the same Rust runs on both.
- UniFFI's Kotlin bindings call native code through **JNA**, not JNI (confirmed in its docs and in an r/rust thread): it works, adds a
  small library and some per-call cost, and wants threads that call back into Kotlin to be attached once. Our calls are coarse (a
  list, a chapter, a page), but a few paths are chatty (per-sentence reading aloud). Measure; **BoltFFI** (a newer bindings
  generator for Swift and Kotlin that claims large speed-ups) is a fallback if JNA shows up in a profile. Not adopted on a claim.
- **16 KB page size**: Google Play requires native libraries to be 16 KB aligned for apps targeting Android 15+ (the developer page
  was updated 2026-10-16). NDK **r28+** does it by default; with r27 or lower it needs linker flags; AGP 8.5.1+ and uncompressed
  libraries. Rust through cargo-ndk with a current NDK is fine, but every native dependency has to pass `check_elf_alignment.sh`.
- Pieces that need checking on Android, none checked yet: `rusqlite` (we use the `bundled` feature everywhere except iOS, which is
  what Android needs, +size); `rustls` with the `ring` provider (builds for Android); `ptts` (Phonon, with the `xn` GEMM crates:
  aarch64 NEON should work, but **the speed on a phone is unmeasured even on iOS**); `getrandom`, `neo_frizbee` (SIMD paths).
- Sources (the manga site scrapers) run in the engine, so they come along unchanged.

### 2.4 Platform pieces that have an Android answer

| iOS today | Android |
| --- | --- |
| SwiftUI + `@Observable` | Compose + `ViewModel` + `StateFlow` (one holder per screen, like the iOS models) |
| Core Text book renderer, UIKit page views | `StaticLayout` (or Compose `TextMeasurer`) for line breaking and hyphenation, drawn to a `Canvas`; our paginator keeps the same page model. Android's text stack hyphenates and shapes complex scripts, but **vertical Japanese text has no built-in layout**: the iOS reader supports vertical chapters; Android needs our own column layout (see risks) |
| `UIScrollView` zoom pages for manga | Compose `LazyColumn`/`Pager` with `transformable`, or a `RecyclerView` for the long webtoon scroll if Compose does not hold 60 fps; large images through Coil with downsampling |
| `AVAudioEngine` + time-pitch, `LockScreen` (dlopen) | `AudioTrack` (PCM from Phonon, `PlaybackParams` for speed) inside a **Media3 `MediaSessionService`** (foreground service type `mediaPlayback`), which also gives the lock screen, Bluetooth buttons and Android Auto/Assistant for free |
| `BackgroundTasks`, `UserNotifications` | WorkManager, `NotificationCompat` |
| Files / `UIDocumentPicker` | Storage Access Framework (`OpenDocument`) |
| TestFlight | Play Console internal testing track |
| iPad layouts (centred columns) | Window size classes; foldables and tablets from day one (the same "nothing stretches" rule) |

Android gives two requirements the iOS app does not have: **edge-to-edge** is forced on recent target versions (the glass bars
must draw under the system bars with correct insets), and **predictive back** needs the back gesture to animate (the iOS
swipe-back is the model).

## 3. Decisions

1. **Native Kotlin + Jetpack Compose, one Android module set, not Compose Multiplatform.** The iOS app stays SwiftUI. The only shared
   code is the Rust engine and the written spec (`manga-app/USER_FLOW.md`, `DESIGN.md`). Sharing UI code would give the lowest common look, not the best of each.
2. **A thin `AkiGlass` layer of our own over the glass library.** Screens never import Backdrop or Haze; they call `Modifier.akiGlass(...)`
   or `AkiGlassButton`, as iOS screens only use Aki components and `scripts/check-ui.sh` enforces it. The library can be swapped
   (or replaced by our shader) after the spike without touching a screen.
3. **Three glass tiers**, picked once at start and changeable by the person:
   - **A, API 33+ and a fast enough GPU:** the full effect: vibrancy, blur, lens refraction with depth, edge light, press squish.
   - **B, API 31–32, or A on a slow device:** blur + tint + edge light + shape (no refraction).
   - **C, API 29–30, or Reduce transparency:** a solid frosted surface with tint, border and shadow (no sampling at all).
   Reduce transparency (and Increase contrast) from Android's accessibility settings forces C, as Haze's accessibility hooks suggest.
4. **minSdk 29** (Android 10; about 90% of phones in the Capgo numbers), target the latest. Open question 1.
5. **arm64-v8a only in release** (plus x86_64 for the emulator in debug), shipped as an app bundle so each phone downloads only its
   own library. Release builds use R8 full mode and a Baseline Profile.
6. **Same design tokens, new file.** `Tokens.swift` values (colours, radii, the one yellow `#FFD21F`, greys `#F4F4F5`) become
   `AkiTokens.kt`; screens never write a colour (a lint/detekt rule or a script that mirrors `check-ui.sh`).
7. **Same icons** (the Solar set already fetched by `scripts/fetch-icons.py`): the script gains an Android output (vector drawables or
   `ImageVector`s), shown only through `AkiIcon`.
8. **Where it lives:** `manga-android/` beside `manga-app/`. Gradle modules: `:app`, `:design` (tokens, glass, Aki components),
   `:engine` (generated bindings + the `.so`), `:reader` (manga), `:books` (EPUB renderer), `:listen` (audio service). Modules keep build
   times down and make the design module testable alone.
9. **Font:** iOS uses the system rounded face. Android has none; a rounded geometric sans must be bundled or downloaded. Pick one
   (SIL OFL, e.g. Nunito or Quicksand), measure its size cost (one variable font file) and keep it to one file. Open question 2.

## 4. Component parity

The iOS UI is 44 files in `App/UI` (about 3 500 lines) plus screens (about 4 000) plus Books (about 1 900) and Reading (about 740).
The Android design module mirrors the UI folder one to one so a change in one app finds its twin: `AkiGlass`, `AkiButton`,
`AkiIconButton`, `AkiCard`, `AkiRow`, `AkiTabBar` (the floating capsule with a **sliding yellow glass indicator**, the Backdrop docs'
interactive bottom bar is the template), `AkiTabs` (the pills), `AkiMenu`/`AkiMenuPill`, `AkiSwitch`, `AkiSlider`, `AkiSelect`,
`AkiInput`, `AkiTextArea`, `AkiHeader`, `AkiSection`, `AkiShelf`, `AkiBadge`, `AkiDialog`, `AkiEmptyState`, `AkiSpinner`,
`AkiRefresh`, `AkiOnboarding`, `AkiBackdrop` (the soft yellow glow, which is what the glass refracts), `AkiCovers`, `AkiImage`,
`AkiZoomPage`, and the book pieces (`BookPage`, `BookLayout`, `BookStyle`, `BookMarks`, `BookComfort`, `BookFonts`).
Rules carried over from `DESIGN.md`: one yellow, glass only for the controls and navigation layer, glass never samples glass (use
the library's exported backdrop for sheets over bars), tint means something.

## 5. Phases

### Phase 0: spike and gates (nothing else starts before the answers)

Four things to prove, each with a go/no-go. All on **real phones**: one low-end (3–4 GB, mid-2020s budget), one mid, one current flagship, one on API 31–32.

1. **Glass cost.** A page with the Aki home background, a scrolling list of covers, a floating glass tab bar, two glass buttons and a glass
   header, built three ways: Backdrop, Haze 2 Glass, and Cloudy. Macrobenchmark `FrameTimingMetric` while scrolling and while a
   sheet animates, release build, same device state; compare frame overrun and the look, side by side with an iPhone screenshot.
   **Gate:** the winner keeps frame overrun near zero on the mid phone in tier A, and the low-end phone in tier B or C. If none does,
   the fallback is an own AGSL shader with a lower-resolution capture, or tier B everywhere.
2. **Engine builds and runs.** `cargo ndk` builds `akimanga` for `aarch64-linux-android` with NDK r28+, 16 KB aligned (run
   `check_elf_alignment.sh`), Kotlin bindings generated, and a screen calls `sources()` and a search. Record the `.so` size by
   profile (the iOS app has an `ios` profile tuned for size: reuse it) and the JNA cost of a chatty call (1 000 small calls).
3. **Phonon on a phone.** The ignored real-model test (`AKI_VOICE_MODEL`) running on the Android phone: seconds of audio per second
   of compute on the low-end and the mid phone, and the memory the model takes. Gate: faster than real time on the mid phone
   (the same bar as iOS, where it is also unmeasured on a real device).
4. **Book page.** One chapter laid out with `StaticLayout` with justification and hyphenation, drawn on a `Canvas`, paged: time to
   lay out a long chapter, page-turn latency, and a look next to the iOS page for the same text.

Output: a numbers table appended to this file, and the decision on library, tiers' thresholds and the reading-aloud plan on Android.

### Phase 1: foundation

Project and modules, version catalog, CI that builds the engine and the app, `AkiTokens`, theme (light and dark; the reader is
always black), icon pipeline, `AkiGlass` with the three tiers and the tier picker, the first Aki components with screenshot tests
(Paparazzi or Roborazzi, to be chosen) against reference images, `scripts/check-ui` equivalent, edge-to-edge insets helper, predictive
back. App size and cold-start numbers recorded from the first build, so every later change is measured against them.

### Phase 2: shell and manga

Floating tab bar and the five tabs per mode (the manga-first and books-first orders from `AppTab`), onboarding with the glass choice
cards, Home, Library, Updates, Sources (add repositories, install sources), Settings, search, series page (hero image, sheet rising over it,
chapter cards), downloads, the manga reader (paged left-to-right and right-to-left, webtoon scroll, zoom, double-tap, page
preload), the library and updates caches and speed work equal to the iOS pass (no heavy work at launch).

### Phase 3: books

Add from Files (SAF) and catalogues (OPDS), library with sort, filter, search, categories and custom covers, the EPUB renderer
(engine blocks → `StaticLayout` → paginated pages, themes, fonts, margins, footnotes, pictures, links, search, highlights and notes,
the `BookPage` overlays for selection and highlights), scroll mode. The renderer is the biggest piece of new design on Android.

### Phase 4: reading aloud

Voices repo gate in Settings (the same "check when Settings opens, not at launch" rule), model and voice install into
`<id>.part`, `ListenService` (Media3 session, foreground `mediaPlayback`, audio focus, becoming-noisy handling for unplugged
headphones, interruption pause), the listen bar, tap-to-read, lock-screen and Bluetooth controls, idle stop after three minutes
paused.

### Phase 5: tablets, polish, release

Window size classes (tablet and foldable layouts with the iPad rules: centred columns, covers capped, nothing stretched), TalkBack
labels, font scaling, Baseline Profile and startup profile, Macrobenchmark suites for cold start, scroll, reader and glass, size
audit, Play Console internal testing, then production. App signing, the privacy form and the data-safety form are the owner's
(they need the owner's account).

## 6. Performance rules for the glass (from the libraries' docs and the spike)

1. **One captured source per screen**, the page content; glass surfaces read it. A glass surface is drawn **over** the content, not inside it.
2. **Few surfaces, small area.** The tab bar, the header, a handful of round buttons. Nothing glass in list rows. A sheet over the tab bar uses the exported-backdrop path.
3. **Keep what moves out of the source** when the glass does not need to show it (Haze's guidance): a still source is cached; a scrolling one is re-sampled every frame.
4. **Lower the capture resolution and blur radius** before dropping the effect (Haze's `Performance` mode does this).
5. **Pause when still.** No per-frame work when nothing under the glass changes and the person is not touching it.
6. **The reader has no glass over the page while turning pages**: the bars are glass over a black or paper page that is not moving behind them; they fade out while reading.
7. Release builds only for any number. Debug Compose is many times slower.

## 7. Risks

| Risk | Why | Answer |
| --- | --- | --- |
| Glass is slow on cheap GPUs | Sampling and refraction cost grows with covered area; Android phones vary far more than iPhones | Phase 0 gate, tiers, a person-visible setting, Haze/Backdrop performance modes |
| Libraries are young | Backdrop 286 commits, alpha-era API; Haze Glass "experimental" until 2.1; Cloudy new | Own `AkiGlass` seam; pin versions; keep the shader option |
| Books on Android are a new renderer | No Core Text; different hyphenation and kerning; vertical text unsupported | Spike 4; horizontal text first; vertical Japanese chapters later (own layout), shown as "not supported yet" rather than wrongly |
| JNA call cost and the extra library | UniFFI Kotlin uses JNA | Coarse calls; measure in spike 2; BoltFFI as a fallback |
| 16 KB alignment | A Play requirement; dependencies built with old NDKs | NDK r28+, run the alignment script in CI |
| Phonon speed and memory on phones | Unmeasured on iOS too | Spike 3; same on-demand download, never in the install |
| The look drifts from iOS | Two codebases | Same token names, same component names, screenshot tests, a parity checklist in `USER_FLOW.md` |
| Size | JNA, bundled SQLite, a font, Compose itself | Measure from phase 1 on; AAB ABI splits; R8 full; budget set after the first numbers |
| Distribution for a source-loading reader | Store policies differ from Apple's | Owner's call at release; the app ships no content, as on iOS |

## 8. Verified when

Each phase is its own change: engine tests on the Android target where they can run, the app run on the phones from phase 0 (and an
emulator), screenshots next to iOS for every screen, Macrobenchmark numbers in the commit message, the Aki rules script green, a
fresh-eyes review of the diff before commit, no debug code left. Nothing is pushed or published until the owner says so.

## 9. Questions for the owner

1. **minSdk:** 29 (more phones, three tiers), or 31 (no tier C, simpler, loses roughly a fifth of phones in the Dec 2025 numbers)?
2. **Font:** a bundled rounded face (OFL) such as Nunito, or the system font and accept a different feel?
3. **Phones to test on:** which Android devices do you have? The glass gate needs at least one cheap one.
4. **Order:** manga first (as the plan) or books first (the app's current default for new people)?
5. **Package id:** `live.akidev.manga` (same as iOS)?

## 10. Sources

- Backdrop: <https://github.com/Kyant0/AndroidLiquidGlass>, docs <https://kyant.gitbook.io/backdrop> (effects, backdrops, bottom bar and bottom sheet tutorials, FAQ)
- Haze 2: <https://github.com/chrisbanes/haze>, <https://chrisbanes.me/posts/haze-2.0.0/>, performance guide <https://chrisbanes.github.io/haze/2.0.1/performance/>
- Cloudy / Mirage: <https://github.com/skydoves/Cloudy>, <https://proandroiddev.com/mirage-cloudy-grows-from-a-blur-library-into-a-graphics-effect-library-3b8eaf9cc7ca>
- AGSL and RenderEffect: <https://developer.android.com/develop/ui/views/graphics/agsl/using-agsl>, <https://developer.android.com/reference/android/graphics/RenderEffect>
- Reddit: r/androiddev "Wrote a neat Liquid Glass Shader for Jetpack Compose" (1pyq5s6), "Liquid Glass Animation with Jetpack Compose" (1nlawb3)
- 16 KB pages: <https://developer.android.com/guide/practices/page-sizes>
- UniFFI Kotlin and JNA: <https://mozilla.github.io/uniffi-rs/latest/kotlin/gradle.html>, <https://sal.dev/android/intro-rust-android-uniffi/>
- Version shares: <https://capgo.app/android-distribution-chart/> (December 2025), <https://gs.statcounter.com/android-version-market-share> (September 2026)
- Media3 background playback: <https://developer.android.com/media/media3/session/background-playback>

## 11. What was decided and built (2026-10-26)

The owner answered section 9: **minSdk 29**, **a bundled rounded face (Nunito)** plus the book fonts (Lora, Atkinson), **manga first, then books**
(both are built), **package `live.akidev.manga`**, **Kotlin-only book renderer**, **Haze** for glass, simple readable code with micro components.
Where the build differs from the plan above:

- **Three Gradle modules, not six:** `:app`, `:design`, `:engine`. Reader, books and listening are packages in `:app`; splitting them added
  build files and no clarity.
- **Two glass looks, not three tiers:** Haze glass on Android 12+, and a solid surface on Android 10-11 or with "Reduce glass". Lens refraction was not built.
- **arm64 only**, debug and release (the emulator on this Mac is arm64 too); no x86_64.
- **Tests:** JVM tests for the pure logic, an instrumented test for typesetting, lint at zero warnings. No Macrobenchmark yet.
- **Not built:** vertical Japanese text; measuring on the owner's cheap phone (glass cost, reading-aloud speed) is still to do.
- **Compared with iOS side by side (2026-10-26):** the screens, menus, dialogs, sheets and controls were put next to the iOS screens in light and dark; the page is the artifact "Akidev iOS and Android". Menus and dialogs now draw in the app's own window so their glass shows the screen; sheets float with a half-height start; soft shadows replace outlines; glass over pictures shows the picture.
- **Not done:** vertical Japanese text (the engine flags it; Android lays it out horizontally), a Macrobenchmark module (`scripts/startup.sh` times a cold start instead), and moving the small database writes (book position, highlights) off the main thread.


## 12. Speed and size, measured on the owner's phone (2026-10-27)

Samsung Galaxy A05 (Android 14, 720x1600), release build compiled the way Play does, demo source. Before is the build of 2026-10-26.

| What | Before | After |
| --- | --- | --- |
| Cold start, nothing compiled (graphics cache warm) | 576 ms | 576 ms |
| Cold start with the baseline profile | 446 ms (hand-written profile) | about 410 ms (recorded profile) |
| Open a series, share of frames late | 100%, median frame 117 ms | 27%, 32 ms |
| Scroll a series | 71% late, 53 ms | 28%, 34 ms |
| Open the reader | 63% late, 89 ms | 13%, 34 ms |
| Scroll the reader strip | 100% late, 93 ms | 12%, 21 ms |
| Back out of the reader and the series | 65% late, 105 ms | 14%, 34 ms |
| Download from Play (arm64, xhdpi) | 7,597,545 bytes | 7,510,132 bytes |
| Release APK | 17.16 MB | 16.35 MB |

- Cause of the jank: the graphics chip, not the code. With "Reduce glass" the same scenarios were 3-5% late, so the fix keeps glass at rest and drops it while things move (`AkiGlassBudget`, `GlassGovernor`). Haze's performance modes and style options (no chromatic aberration, no refraction) were tried and gave little: the cost is per glass control, not per effect.
- Tried and left out because it did not measure better: opening the engine on a thread of its own at start (the first screen waits for it anyway), a decode thread pool and decoding pictures straight from their files (reader scroll with a cold picture cache: 13.0% before, 12.0% after, which is noise).
- Size: the engine library is 12.7 MB of the 17 MB; 2.9 MB of it is unwind tables the panics need, so it stays. Packed relocations and merging identical code took 0.8 MB off it (`scripts/build-engine.sh`). R8 already puts every class in one package; fonts were left whole because books use their Cyrillic and Vietnamese letters.
- Not measured: a fast phone (the governor never switches there by design), reading aloud after the engine link flags (no voice installed on the phone at the time; the library loads and fetches a repository on an API 29 emulator and the phone).
