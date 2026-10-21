# iPhone and Android apps: plan

Status: plan only, nothing built. Written 2026-10-19 from web research, a store-policy review and an audit of
our crates (sources at the end). Decisions needed are in section 9.

## 1. The answer

**Possible, yes. With the same GPUI code, not yet.**

- GPUI has no shippable phone support today. An iOS backend (`gpui_ios`) was merged into Zed on
  2026-10-14, marked experimental, and is not in a published `gpui-pre` release (our `gpui-kit` pins
  `gpui-pre 0.3.9`). Android is on the maintainers' "someday" list (zed#43207 closed as not planned,
  moved to gpui-projects#12).
- The **engine** is another matter: most of our Rust (providers, indexers, release ranking, torrent
  streaming, party rules, the encrypted container, the downloader) can run on a phone behind a thin
  interface. About 85% of the desktop app crate is GPUI screen code, and none of that carries to a different
  UI layer.
- The harder problem is **distribution**: Apple and Google both ban apps that facilitate unauthorized file
  sharing or downloads. The full app (torrent sources, music from YouTube) cannot be a store app; a store
  build could only be a player for the user's own files and server.

So the real choice is: which UI layer, and which channel (own APK and sideloaded iOS app, the website as a
phone app, or a reduced store app).

## 2. Where GPUI on phones stands (verified 2026-10-19)

- **iOS, upstream:** zed PR #63068 "gpui_ios" (opened 2026-08-22, merged 2026-10-14). Metal, UIKit via objc2,
  touch and scroll momentum, safe-area and keyboard insets, a UITextInput bridge, a simulator example. Not done:
  edit menus, the Return and hardware keys as GPUI key events, networking/TLS notes; accessibility not
  mentioned. The maintainers asked for bug reports and small PRs. No evidence of a physical-device run in the PR.
- **Android:** nothing official. Community: `itsbalamurali/gpui-mobile` (iOS and Android through wgpu, v0.1.0,
  no releases, ran on one phone; an open bug where Vulkan and GL both claim the surface so nothing renders on an
  emulator; its full platform layers depend on unpublished upstream crates) and `Dylanmurzello/zed-android-port`
  (the Zed editor on Android tablets, alpha, GPL-3.0). Someone on the iOS PR says they are building both
  backends in a fork.
- **No shipped phone app** built on GPUI was found anywhere.
- **Our UI code:** gpui-component and our screens assume a mouse (hover, right-click, dense layouts). Even with
  a backend, every screen needs a touch redesign, and VoiceOver and TalkBack support is planned nowhere.

Effort to do it ourselves: iOS, start from `gpui_ios` and fight an upstream that moves daily; Android, write the
backend (roughly 6 to 10 engineer-weeks to a usable alpha, my estimate), then the long tail of GPU and keyboard
quirks. Revisit in 2027.

## 3. UI-layer options (all reuse the Rust crates through uniffi or commands)

| Option | Reuses | Notes | Rough effort |
| --- | --- | --- | --- |
| **A. Tauri 2 mobile shell around the existing Next.js UI**, a native video player as a plugin | All Rust crates and the web UI | Stable since 2.0. Webview video cannot play every MKV/HEVC/ASS file, so a native player plugin is needed. | 6 to 10 weeks |
| B. Native SwiftUI and Kotlin/Compose over uniffi | Rust crates only | Best fit, two UI codebases. | 14 to 22 weeks |
| C. Flutter with flutter_rust_bridge | Rust crates only | Mature; a third UI technology. | 10 to 16 weeks |
| D. Adopt `gpui_ios` and write the Android backend | Most view code after a touch pass | Experimental, `gpui-pre` churn, no accessibility. | 16 to 30+ weeks |
| E. Wait for GPUI mobile | Everything | Android is "someday". | Indefinite |
| Dioxus, Slint, egui, React Native, Compose Multiplatform | Rust crates only | Each a rewrite of the screens; none beats A or B for us. | not pursued |

## 4. What carries over from our code (audit)

| Piece | Verdict |
| --- | --- |
| `akidev-core` providers, indexers, ranking, subtitles, search, sources, update, media ids | Portable. Switch `reqwest` and `ytmapi-rs` from native-tls to **rustls** on phones (Android has no system OpenSSL for Rust). Check what pulls in `aws-lc-sys` (a C/CMake build). |
| `serve.rs` (loopback stream server with token and Host check) | Works as is: AVPlayer and ExoPlayer can read `http://127.0.0.1`. Needs a cleartext-loopback allowance on both systems. |
| `akidev-core` torrent engine (`librqbit` 9.0.1, our patch) | Pure Rust, should build; **unverified on either target**. Foreground only: iOS suspends networking about 30 s after backgrounding; Android needs a foreground service (and Play limits their types). |
| `akidev-core::music` | The browse and search side is portable. **yt-dlp as a subprocess cannot run on a phone.** The server already proxies audio (`/v1/music/stream/{videoId}`), so mobile music can use it, or music is dropped from the phone. |
| `akidev-party` | `protocol` and `sync` portable. webrtc-rs should cross-compile but is heavy; mobile can use the platform WebRTC and reuse only the rules (the website has its own port in TypeScript). |
| `akidev-library` | `container` and `store` portable (rusqlite builds with the NDK or Xcode). `keys.rs` takes the file fallback on iOS and Android today; it needs the iOS Keychain and the Android Keystore (a small native bridge). The downloader works in the foreground only. |
| `akidev-server` | Stays a server (ffmpeg, ffprobe, debrid). A phone calls it. |
| App crate | About 2k of its 16.5k lines (`progress`, `captions`, `choices`, `history`, `route`, `runtime`) is free of GPUI and reusable; the rest is GPUI. `saved.rs` (the `akc://` mpv hook), `audio.rs`, `updater.rs`, `keepawake`, `notify-rust` are desktop only. |
| Website | Already does browsing, watching, music and parties on a phone, with a Media Session for lock-screen audio. Missing: installability (no manifest or service worker), offline library, reliable background audio, local torrent playback. |

The core API (`Streamer::plan/resolve/open_best`, `providers::*`, `Library::*`, `Host/Guest`, `Music::*`) is
async, returns owned serde types and mentions no GPUI type: a good fit for uniffi or JSON over a thin facade.
The app glue (`LazyStreamer`, `LazyLibrary`, analytics) is rewritten per platform.

Things to replace on a phone: the video player (mpv or AVPlayer/ExoPlayer), the `akc://` offline playback
(a decrypting loopback server using `Sealed::read_at`, or ExoPlayer `DataSource`/`AVAssetResourceLoader`),
music resolution, the keychain, notifications, keep-awake (wake locks), deep links (URL scheme, universal
links, intent filters), file paths (sandbox directories passed in; core and library already take a `dir`).

## 5. Video on phones

- **iOS:** MPVKit (libmpv and FFmpeg as xcframeworks) renders through MoltenVK; its LGPL build is the one to
  use for the App Store (LGPL relinking and static linking is a legal grey area: use dynamic frameworks and
  take advice); it needs iOS 17 and the project says it is not maintained often. AVPlayer is simpler and gives
  Picture-in-Picture and AirPlay but plays no MKV and little HEVC 10-bit, and renders no ASS.
- **Android:** mpv-android is an app, not an embeddable library (third-party AARs vary). Media3/ExoPlayer plays
  MKV and HEVC, takes a custom `DataSource` (for saved copies), and gives Picture-in-Picture, MediaSession and
  Cast; ASS is partial.
- **Likely answer:** ExoPlayer on Android; mpv on iOS if ASS and MKV matter, AVPlayer plus the server's HLS
  conversion if they do not. Picture-in-Picture, AirPlay and Cast are built by hand around mpv.

## 6. Distribution (this decides the shape of the app)

| Channel | Verdict |
| --- | --- |
| **The website as a phone app (PWA)** | Lowest risk, works today. Installable on both; iOS gives no background fetch, so no background downloads; test background audio on a real device (open WebKit bugs); torrents must go through our server. |
| **Android APK outside Play** (GitHub, our updater) | Good for the full app. Google's developer verification starts enforcement in four countries on 2026-09-30 and globally in 2027: register the package name and signing key before then, with an identity separate from any store account. |
| **iOS sideloaded** (AltStore, SideStore, Sideloadly) | Possible; free Apple IDs sign for 7 days and three apps. In the EU, Japan and Brazil, AltStore PAL exists but Apple revoked iTorrent's entitlement without warning. Stremio ships its full iPhone app this way. |
| **Store listing** | Only a reduced build: a player and library for the user's own files and server, with no built-in indexers, no "torrent" wording, no YouTube downloads. Even clean builds get pulled (Stremio Lite was removed from the App Store in January 2026). Apple guideline 5.2.3 names downloading from third-party sources; 2.5.19 forbids hidden features. |

## 7. Recommended path

Phase 0, **spikes (1 to 2 weeks).** Answer the unknowns before choosing anything:
1. Install the targets and compile `akidev-core` with librqbit for `aarch64-apple-ios`, `aarch64-apple-ios-sim`
   and `aarch64-linux-android`, with rustls; find what pulls `aws-lc-sys`; check `rusqlite` and `fs2`.
2. A torrent-stream spike on a real phone (foreground), including the loopback server.
3. A uniffi crate with one call (`providers::search`) called from Swift and Kotlin.
4. Video: mpv on iOS and ExoPlayer on Android on an MKV/HEVC 10-bit/ASS sample, then a custom byte source (the
   saved-file case).
5. Decide whether the existing web UI is usable enough on a phone to ship inside a shell.

Phase 1, **the website as a good phone app (2 to 3 weeks)**, independent of everything else: a web app
manifest and service worker, an install prompt, touch polish of the player and the party panel, lock-screen
audio, and a plain note about what a phone browser cannot do. This is shippable on its own and de-risks the
rest.

Phase 2, **the mobile core (3 weeks):** a new crate (`akidev-mobile`) exposing the engine through uniffi,
with a trait for the things the host provides (directories, keychain, notifications, network state), an
`AsyncRuntime`, and no GPUI. Desktop keeps using the crates directly.

Phase 3, **an Android APK (6 to 8 weeks)**, UI chosen after phase 0: the Tauri shell with the web UI and an
ExoPlayer plugin (fastest), or native Compose. Music goes through the server. Torrents work in the
foreground; a foreground service for downloads. Updater and signed APK on GitHub.

Phase 4, **iOS for sideloading (6 to 8 weeks):** the same core and UI choice, mpv or AVPlayer, TestFlight for
testers if useful, an IPA for AltStore. EU alternative marketplace as an optional extra.

Phase 5, **a store-listed reduced app: probably never**, or only after a legal read. Plan for the account risk
before attempting it.

Track `gpui_ios` and the community Android forks; re-evaluate in 2027. If GPUI mobile matures, option D
becomes a rewrite of screens only, since the core would already be behind the phone interface.

## 8. Risks

1. librqbit on phones is unverified, and background limits make torrenting a foreground feature.
2. Music has no subprocess on a phone: server audio only.
3. Store policy and account termination risk (Apple has revoked entitlements; Google has struck torrent
   clients); keep identities separate.
4. Android developer verification will block unregistered APKs on ordinary phones from 2027.
5. Video: webview limits, AVPlayer limits, LGPL packaging of mpv for iOS.
6. Two to three weeks of upkeep per OS per year (Xcode and NDK updates), signing, and CI on macOS runners.
7. No accessibility on GPUI mobile; a webview or native UI has it.

## 9. Decisions needed

1. **UI layer** after the spikes: the web UI in a Tauri shell (fastest), native Swift and Kotlin (best fit), or
   a bet on GPUI mobile (slowest).
2. **Music on phones:** through our server (recommended), or leave music to the desktop and the website.
3. **Torrents on phones:** local foreground streaming (recommended), server-side streaming only, or both.
4. **Channel:** the website app first, then an APK, then iPhone sideloading (recommended); no store listing.
5. **Identity and legal:** register for Android verification with an identity separate from any store account,
   and get legal review before any store attempt.

## Sources

Zed PR #63068 (gpui_ios), zed#43207 and gpui-projects#12 (Android), mikayla-maki/gpui-projects `mobile.md`,
itsbalamurali/gpui-mobile and issue 66, Dylanmurzello/zed-android-port, gpui-kit docs, MPVKit, mpv-android,
rqbit, Tauri 2 and Dioxus release notes; Apple App Store Review Guidelines, Google Play IP policy, Android
developer verification page, TorrentFreak, Tecnoblog and ELFhosted on Stremio and Nuvio, firt.dev on iOS PWAs.
Code audit of `desktop/{core,party,library,server,app}` and `akidev-frontend`.
