# Share a manga or a book between phones, with no server

Status: plan, nothing built. Researched 2026-11-03. Applies to `manga-android/` and `manga-app/` (iOS). The shared Rust engine is in `desktop/manga` and `desktop/manga-ffi`.

## 1. What is asked

On Android and iOS, a person taps Share on a book or a manga series. A friend with Akidev, on the same Wi-Fi, sees them and taps Accept. The file goes straight from phone to phone. No backend. It must work in all four pairs (Android to Android, Android to iOS, iOS to Android, iOS to iOS). The app must stay light.

## 2. How LocalSend does it

Source: the [LocalSend protocol](https://github.com/localsend/protocol) (v2). It is plain REST over the local network. There is no server in the middle.

**Discovery.** Every device listens on port **53317** (UDP and TCP). On start, a device sends a small JSON message to the multicast address **224.0.0.167**:

`{alias, version "2.0", deviceModel, deviceType, fingerprint, port, protocol "http"|"https", download, announce: true}`

A device that hears an `announce: true` answers with `POST /api/localsend/v2/register` (its own info) to the sender's port. If multicast is blocked, a fallback sends the same `register` request to every address in the local subnet (for example 192.168.1.1 to .254) and reads the answer.

**Identity.** In HTTPS mode, each device makes a self-signed certificate. Its `fingerprint` is the SHA-256 of that certificate, so the other side can pin it. In HTTP mode the fingerprint is a random string.

**Send (sender to receiver):**
1. `POST /api/localsend/v2/prepare-upload` with the sender info and a list of files (`id`, `fileName`, `size`, `fileType`, `sha256`, optional `preview`). Only metadata. The receiver's person sees "Dishant wants to send you 1 book" and taps Accept or Decline. Answer: `sessionId` and one token per file. Errors: 403 declined, 401 wrong PIN, 409 busy, 429 too many requests.
2. `POST /api/localsend/v2/upload?sessionId=&fileId=&token=` with the file bytes. Receiver checks the SHA-256 (422 if wrong).
3. `POST /api/localsend/v2/cancel?sessionId=` stops it.

There is also a reverse "download" mode (a phone serves a URL, a browser fetches it). We do not need it in phase 1 (see 6).

Nothing here needs the internet. That answers "no backend": the receiver's phone is the server for the length of one transfer.

## 3. What our apps already have

- The Rust engine is shared. It is a uniffi crate (`desktop/manga-ffi`) used by Swift and Kotlin. It already has rustls (`ring`), reqwest and tokio.
- A book is a file in the books folder (`BookRow.file`, `bytes`, `cover`) plus a row in SQLite (`desktop/manga/src/store.rs`). It is a plain EPUB.
- A manga chapter from a source is read online, or from a downloaded folder of page images (`downloads.rs`: `root/<source>/<series>/<chapter>/`). We do not share those images.
- The engine already **imports a CBZ** (a zip of images): `desktop/manga/src/local.rs`, with size limits (3000 pages, 40 MB a page, 2 GB total). It unpacks the pages into `local/<series>/` and keeps no copy of the CBZ.
- A manga source is a small JSON file (`SourceDef`, `repo.rs`). The engine can install one from a repo (`install_from_repo`) or from pasted JSON (`install_source`), and knows where each source came from (`store.origin`). The apps already open `akimanga://add-repo?url=...`.
- Android already has a FileProvider that hands a backup file to the system share sheet.

So most of the pieces exist. What is missing is the network part and the screens.

## 4. What is shared

Three kinds of thing, all sent as files, so one protocol carries everything. **What is sent depends on what the person is reading, and the person does not choose:**

| What the person is reading | File sent | Typical size | What the receiver does |
|---|---|---|---|
| A book | the `.epub` as it is, `application/epub+zip` | 1 to 50 MB | `add_book`, like "Open in Akidev" |
| A manga from a source (downloaded or not) | one small `.akidev.json` (`application/vnd.akidev.share+json`), the "share card" (below). **Never the chapter images.** | a few KB | adds the source if missing, then adds the series, which opens and reads online |
| A comic the person imported from a CBZ file | one `.cbz`, `application/vnd.comicbook+zip` | 20 to 500 MB | the existing CBZ import (`local::import`) |

The rule: **a comic that came from a source is shared as the source, and a comic that came from a file is shared as the file.** There is no source for an imported comic, so the file is the only thing we can send. We never build a CBZ from downloaded chapters. Downloads stay on the phone that made them.

The share card for a source comic holds:
- the series: title, cover link, source id, series id;
- the source, in one of two forms, taken from `store.origin(source)`:
  - from a repo: the repo link (the receiver installs it with `install_from_repo`, the same path as `akimanga://add-repo`);
  - added by hand, with no repo: the whole source definition JSON (`SourceDef`, a few KB), which the receiver installs with `install_source`;
- optional, off by default: the reading position ("I am on chapter 40").

What the receiver does with a card, in order:
1. If the source is already installed, skip to step 3.
2. Show the question "Add the source *Name*?" with its name, its language and the NSFW flag, and wait for the answer. A new source is code that runs, so it is never added without a yes (this is the same question the add-repo flow already asks). If the person says no, nothing is added and the card is dropped.
3. Add the series to the library and open it. The chapter list loads from the source on the receiver's own phone.

If the source later fails on the receiver's phone (the site is blocked in their country, for example), the series shows the normal error screen. That is the same as any other series from that source.

One thing to build for the CBZ case: the engine unpacks an imported comic into `local/<series>/` and does not keep the original CBZ. To share it, the sender zips those page images back into one CBZ (store only, no recompress, since images are already compressed). It is a small function in `desktop/manga/src/local.rs`, the reverse of `import`.

Because the wire format is LocalSend's, a person with the real LocalSend app can also send us an EPUB or CBZ, and we can send to them. That is a free bonus, not a goal. (The share card is ours, so the LocalSend app only sees it as a small file.)

## 5. The hard part: finding each other

Facts found in research:

- **Android** can use multicast UDP, but only after taking a `WifiManager.MulticastLock` (permission `CHANGE_WIFI_MULTICAST_STATE`). Android's own discovery service, `NsdManager`, speaks mDNS (Bonjour).
- **iOS blocks multicast UDP** unless the app has the `com.apple.developer.networking.multicast` entitlement. Apple gives it by request (web form, answer by email, reports say about 5 days) ([forum](https://developer.apple.com/forums/thread/666240), [how to use it](https://developer.apple.com/forums/thread/663271)). Bonjour does not need it.
- iOS shows a "Local Network" permission prompt the first time (`NSLocalNetworkUsageDescription`, and `NSBonjourServices` for Bonjour) ([Apple](https://developer.apple.com/documentation/bundleresources/information-property-list/nslocalnetworkusagedescription.md)). Builds started from Xcode skip the prompt, so test it from TestFlight.
- A third-party LocalSend library for Rust says that on iOS it opens no multicast sockets and leaves discovery to Swift (`NWBrowser`, `NWListener`) ([rightkit-send](https://docs.rs/crate/rightkit-send/latest/source/README.md)). I did not find how the official LocalSend iOS app does it.

**Decision: use mDNS (Bonjour) as the main discovery on both phones.** Each app advertises the service `_akidev._tcp` on port 53317 and browses for it.

- iOS: `NWListener` (advertise) and `NWBrowser` (browse). No special entitlement.
- Android: `NsdManager` (register and discover).
- Both are standard mDNS, so Android and iOS see each other.

When a device is found, it gives its address and port to the Rust engine. From then on, everything is the LocalSend protocol (`register` first, to learn the alias and fingerprint).

Fallbacks, in this order, so a flaky network does not end the feature:
1. LocalSend multicast (`224.0.0.167:53317`), Android only, with the MulticastLock. This also finds the real LocalSend app. iOS can join later if the owner asks Apple for the entitlement (open question 2).
2. A subnet scan: `register` to every address of the /24, a few hundred small requests. Works when mDNS is blocked. Costs a little battery, so it runs only when the person taps "Can't find them?".
3. A code on screen. The receiver screen shows its address as a short code and a QR code. The sender types the code or scans the QR. This always works if the two phones can reach each other. The QR scan is phase 3 (it needs the camera).

Not solved on purpose: phones on different networks, a guest Wi-Fi with "client isolation", and some hotel or school Wi-Fi. Then nothing local works, with LocalSend too. We say so in the screen ("both phones need the same Wi-Fi").

## 6. Decisions

1. **The protocol lives once, in Rust.** A new module in `desktop/manga` (about `share/`): the server, the client, the SHA-256 check, the sessions, the multicast and scan. Swift and Kotlin only start it and show the screens. Why: both apps already link this engine, the code is written and tested once, and nothing new is added to the app size except a small HTTP piece.
2. **Native code only for discovery** (`NsdManager`, `NWBrowser`/`NWListener`) because the OS does not let Rust do it on iOS. Each is about 60 lines.
3. **HTTPS with a self-signed certificate, like LocalSend,** with the fingerprint pinned by the sender after `register`. This needs one new crate to make the certificate (`rcgen`, or the same job with `rustls` pieces we already have). **Dependencies need the owner's OK.** The simple option is plain HTTP, which LocalSend also allows. On a home Wi-Fi it is acceptable for books, but anyone on the Wi-Fi could read a transfer. Recommended: HTTPS.
4. **Always ask the receiver.** No auto-accept in phase 1. The prompt shows who, what and how big. This is also our main safety wall.
5. **The HTTP server is small and hand-fit,** not the whole of `axum`, if a measurement shows `axum` costs the app size. The engine's existing `axum` is optional and used by the desktop only. Measure with the same script as in `ANDROID_PLAN.md` section 12 before choosing.
6. **Do the cheap thing first.** Phase 0 is the system share sheet and "Open in Akidev" (file types for `.epub` and `.cbz`). That gives AirDrop (iOS), Nearby Share (Android) and any chat app for free, and it is also how the receiver opens files later. It is half a day, so do it before the network work.
7. **Foreground only.** iOS stops an app in the background, and a phone that goes to sleep stops the Wi-Fi. During a transfer we keep the screen on (the apps already have a keep-awake path) and say "Keep Akidev open". Android may add a foreground service later for very large series.
8. **The receive screen is a switch, not always on.** The listener runs only while the person is on the Share screen (or taps "Receive"). Nothing listens when the person is reading. Why: battery, privacy, and fewer ways to attack the phone.
9. **Cap everything the receiver gets.** Max file size (2 GB, the same as CBZ import), max file count per session (200), file names are cleaned (no `..`, no slashes), one session at a time (409 for the rest), 429 if a device asks too often. The sha256 must match. A book is checked by the existing EPUB reader before it is added. A CBZ goes through `local::import`, which already has limits. A pointer file is parsed as JSON with a size cap (64 KB), and the repo link goes through the existing add-repo question, so the person still confirms a new source.
10. **Look.** One Share sheet on both apps, following `design/DESIGN_PLAN.md` and `manga-app/DESIGN.md` (glass, yellow for state). States: "Looking for people nearby", empty (with the tips and the code fallback), sending with a progress ring, done, declined, failed with a retry. The screens are written natively in each app and named the same.

## 7. Phases

**Phase 0 (about 1 day).** The system share sheet from the book and the series menus, and "Open in Akidev" for `.epub`, `.cbz` and `.akidev.json`. Android: intent filters plus the FileProvider that already exists. iOS: `UIActivityViewController` and document types in `Info.plist`. Done when: an EPUB sent with AirDrop opens in the app, and one from Android Nearby Share opens on Android.

**Phase 1 (about 1 week). Engine.** The `share` module: register and info, prepare-upload, upload, cancel, receiver sessions, sender client, SHA-256, caps, tests that run two engines in one process over localhost. Plus the uniffi calls. Done when: a test sends a 100 MB file between two engines, a wrong hash is refused, a decline works, a second sender gets 409, and an interop test passes against the real LocalSend app on a laptop (send both ways).

**Phase 2 (about 1 week). Books on Android and iOS.** Discovery (`NsdManager`, `NWBrowser`/`NWListener`), the Share sheet, Receive screen, and the book end to end. Done when all four phone pairs transfer an EPUB, the reading position of the sender is not copied, and the permission prompts and denied cases show the right screen.

**Phase 3 (about 1 week). Manga.** The share card (build on the sender, read on the receiver, with the "Add the source?" question), and the CBZ builder for imported comics (zip `local/<series>/`, no recompress). The scan fallback and the code and QR screen. Done when: a source series shared from Android opens online on iOS and the other way round, the source is added only after a yes, a series from an already installed source skips the question, a hand-added source (no repo) arrives complete, and an imported CBZ comic reads offline on the other platform.

**Phase 4 (optional).** Auto-accept for "my other devices" (pair once), progress sync in the pointer, a foreground service on Android, the Apple multicast entitlement for iOS compatibility with the LocalSend app.

## 8. Risks

- **Android `NsdManager` is known to be flaky on some phones** (resolves one service at a time, stale entries). The scan fallback and the code screen are there for this. Test on the Galaxy A05.
- **iOS "Local Network" prompt.** If the person taps Don't Allow, nothing is found. The screen must say how to turn it on in Settings, and the code screen cannot help (the connection is also blocked), so the message must be plain.
- **Speed.** Wi-Fi 5 to 6 gives 20 to 100 MB/s, so a 100 MB chapter takes seconds. A 2 GB series takes minutes, and the phone must stay awake.
- **App size.** The HTTP piece and the certificate code add to the Rust library (the Android library is already shrunk, see `ANDROID_PLAN.md` section 12). Measure before and after, and drop `axum` or HTTPS if the cost is too high.
- **Copyright.** The share card moves no manga images, only a link to where the source finds them, so the app does not hand out copyrighted pages. Only a CBZ the person imported themselves is copied, and it is a direct person-to-person copy on a local network, like AirDrop. Keep the words neutral ("send to a friend").
- **A shared source is code that runs on the receiver's phone.** It is never added without the question, and the question shows the name and the NSFW flag. A hand-added source arrives as full JSON, so it goes through the same checks as a pasted source (`SourceDef::validate`).
- **Apple review.** Local network use needs a clear usage string. The multicast entitlement is not needed for this plan.

## 9. Questions for the owner

1. HTTPS (one new crate, pinned fingerprint, recommended) or plain HTTP (simplest, readable on the same Wi-Fi)?
2. Should we ask Apple for the multicast entitlement now, so iOS can also talk to the real LocalSend app? It is free but takes days. Not needed for Akidev-to-Akidev.
3. Is compatibility with the real LocalSend app wanted, or only Akidev to Akidev? (The plan keeps its wire format either way, so this only changes how much interop testing we do.)
4. Phase 0 first (system share sheet and Open in Akidev) as a quick release, then the network work?
5. Should the reading position travel in the share card? The plan has it off by default, as a switch on the Share sheet.
6. If the receiver says no to adding the source, do we still add the series as a bookmark-only entry (it would not open), or drop the card? The plan drops it.

## 10. Sources

- [LocalSend protocol v2](https://github.com/localsend/protocol): discovery, register, prepare-upload, upload, cancel, fingerprint, PIN.
- [Apple forum: multicast entitlement](https://developer.apple.com/forums/thread/666240) and [how to enable it](https://developer.apple.com/forums/thread/663271): needed for multicast and broadcast on iOS, not for Bonjour; request by web form.
- [Apple: NSLocalNetworkUsageDescription](https://developer.apple.com/documentation/bundleresources/information-property-list/nslocalnetworkusagedescription.md): the Local Network prompt covers direct connections as well as Bonjour.
- [rightkit-send](https://docs.rs/crate/rightkit-send/latest/source/README.md): a LocalSend v2 library that leaves iOS discovery to `NWBrowser` and `NWListener`.
- Not checked: Android `MulticastLock` and `NsdManager` behaviour is from my own knowledge, not read from a source this session. The exact certificate rules of the LocalSend app were not found (the page for them returned 404).

## 11. What was built (2026-11-03)

This is the design and flow record for what exists. The plan above says what was wanted; this says what happened.

**Decisions taken.** HTTPS with a self-signed certificate (new crates: `rcgen`, `tokio-rustls`, `hyper`, `hyper-util`, `http-body-util`, all on `ring`, so no second crypto library; `reqwest` gained its `stream` feature). The sender does not check the receiver's certificate: the encryption only hides the traffic from a passive listener. What keeps a stranger out is the person's yes and a secret token per file. A person on the same Wi-Fi who pretends to be the receiver could read a send. For a book, that was judged acceptable; a pairing code would fix it later. The reading position is not in the card (open question 5, answered "not now"). No QR code: the address is typed as `ip` or `ip:port`.

**Where it lives.**
- Engine: `desktop/manga/src/share/` (`mod.rs` types and certificate, `receive.rs` the server, `send.rs` the client, `find.rs` multicast and scan, `card.rs` the share card), `local::export` (a comic's pages back into a CBZ), tests in `desktop/manga/tests/share.rs`.
- App calls: `desktop/manga-ffi/src/share.rs` (`Share`, `ShareSend`, `ShareEvents`, and on `Engine`: `share`, `share_book`, `share_card`, `share_comic`, `share_read_card`, `share_add_card`, `share_clear_outgoing`), tests in `desktop/manga-ffi/tests/share.rs`.
- Android: `manga-android/.../share/` (`Nearby.kt` finds phones with Android's service discovery, `ShareModel.kt`, `ShareScreen.kt`), plus `AppModel.takeIn` and `openFile`.
- iOS: `manga-app/App/Share/` (`Nearby.swift` publishes and browses Bonjour, `ShareModel.swift`, `ShareView.swift`, `OpenFile.swift`).

**Flow, receiving.** The person opens the Share screen (Settings on Android, the books and library menus on iOS). The app starts the engine server on port 53317 (or a free port if it is taken), publishes `_akidev._tcp`, and on Android also listens for LocalSend's multicast hello under a Wi-Fi multicast lock that is released when the screen closes. A sender calls `prepare-upload`; the screen asks "X wants to send you ...". On Accept the files arrive into a folder of their own, each checked for size and SHA-256. When the last file is in, `finished` fires and the app imports each file by kind: book with `import_book`, comic with `import_file`, card with `share_read_card` and then `share_add_card`. A card whose source is not installed first asks "Add the source X?" with the name, languages, NSFW flag and repository, because a source is code. Then `clean` removes the received files.

**Flow, sending.** Share on a book sheet, a series menu or a shelf long-press. What is sent depends on what it is: a book sends its EPUB; a series from a source sends a card; a series from the device source (an imported CBZ) sends a CBZ made from its pages. The screen lists nearby phones (Bonjour or service discovery, multicast hello on Android), with "Can't find them?" for a scan of the whole /24 and a typed address. Tapping a phone runs the send: it waits for the other person's yes (up to 60 s), shows progress, and has Stop. After the send the app calls `share_clear_outgoing`.

**Edge cases and what the person sees.**
- No answer in 60 s or a No: the sender sees "said no"; the receiver sees nothing (a No is their own doing) or "Stopped" if nobody answered.
- The receiver is already receiving from someone else: 409, the sender sees "busy".
- A file with a wrong hash or wrong size: the receiver ends the whole send ("Stopped - a file did not arrive intact"); the sender sees "It did not go through".
- The sender goes away mid-send: the receiver lets go at once (a guard on the request), removes the half file, and tells the screen.
- A second sender while one is asking: 409. A send idle for 120 s is replaced.
- More than 200 files, or a file over 2 GB: refused with 400 before the person is asked.
- A name like `../../x.epub` is cut to `x.epub` and kept inside the send's own folder.
- A card is capped at 256 KB, must name its source's location, and its cover is dropped unless it is an http(s) address. A card that names a different source than the one it carries is refused.
- A card for a series from a source with no repository carries the whole source definition and goes through the same checks as a pasted source.
- Nothing listens when the Share screen is closed.

**Verified.**
- `cargo test -p akidev-manga -p akidev-manga-ffi`: all pass, including 9 engine integration tests and 2 app-call tests for the share (two engines over localhost, a card that makes the other engine install the source, a CBZ round trip, refusals, stop).
- Android (API 36 and 29 emulators): received a book from a Mac sender; sent a book to a Mac receiver by typed address; the card question and its error path; open-in from another app. Release build and lint pass.
- iOS Simulator (iPhone 17 Pro, iOS 26.4), 2026-11-03: received a book sent from the Mac with curl (the Accept question showed, the book landed on the shelf with its title and author); sent a book to a small HTTPS receiver on the Mac (it got the same bytes, sha-256 matched, the screen said "Sent").
- Android emulator to iOS Simulator: the Android phone found the iPhone by its typed address (`10.0.2.2:53317`), the iPhone asked "Android SDK built for arm64 wants to send you a book", and after Accept the book was on the iOS shelf.
- The empty shelf screens on both apps now have "Receive from a friend" (it was only in the sort menu, which a new person with no books would not open). Both compile; that one change was not looked at on a screen.

**Not verified, and why.**
- Discovery by itself (Android service discovery and iOS Bonjour finding each other, the LocalSend multicast path, Scan): the emulator and simulator networks are separate from each other, so every test above used a typed address. Bonjour and the Local Network prompt only behave properly on a real iPhone.
- A card sent between phones on the screens (the engine path is tested; the Android and iOS question screens for a new source were only seen on Android).
- Series and comic Share from the series screen on a device, large files, and the Sending, Stop and failed states on screen.
- Interop with the real LocalSend app; the real Galaxy A05; the linked iOS app size (the iPhone library grew 7.4 MB, +3.5%, as a static archive).
- A `.akidev.json` card cannot be an iOS document type (iOS sees only `json`), so a card arrives only through the Share screen on iOS. "Open in Akidev" from Files or AirDrop on iOS was not tried.

## 12. The sheet and the QR code (2026-11-03, after the owner tried it)

The owner found Android showing Share as a page and iOS as a sheet, an IP address on screen, and too many buttons. This replaces the screens described in section 11. The engine, the protocol and the flows below the screen are unchanged.

**The same sheet on both apps.** A bottom sheet, no page. Wording is identical on both.
- Send (from a book or series): "Send", the item's name, then the people nearby as name-only rows. A tap sends at once. While nobody is found: a spinner and "Looking nearby...". After 8 s: "Can't see them? Both phones need the same Wi-Fi." and the app quietly scans the Wi-Fi once (no button). One secondary button: "Scan their code".
- Receive: "Receive", a QR code, "Ask them to scan this, or open Share on their phone.", and "Visible as <name>". Without a Wi-Fi address: "Connect to Wi-Fi to receive."
- Progress replaces the body of the same sheet: waiting, sending (bar, percent, "Keep Akidev open", Cancel), done ("Sent to <name>", Done), declined/busy/failed (one sentence, Try again, Close). An incoming send keeps the native Accept/Decline question, then shows "Receiving from <name>" with progress.
- Removed: the IP and port text, the typed address field, the "Scan the Wi-Fi" button, the tip paragraphs and the section headers.

**The QR code.** It holds `akidev://share?h=<ip>&p=<port>`. The engine owns the format: `Share.code()` makes it and `Share.probeCode(code)` reads it (anything else is refused). The apps pass their own Wi-Fi address into `Engine.share(..., wifiAddress, ...)`, because the engine's guess from the routing table picked the wrong interface on the Mac (172.29.6.88 instead of 192.168.29.53). The QR is drawn with zxing on Android and CoreImage on iOS. The sender scans with the camera: CameraX plus zxing on Android (CAMERA permission asked only when "Scan their code" is tapped), AVFoundation on iOS (`NSCameraUsageDescription`). A code sends at once. Denied or no camera: "Allow the camera to scan a code" with a button to open settings.

**Dependencies added (approved by the owner).** Android: CameraX `camera-camera2`, `camera-lifecycle`, `camera-view` 1.6.2 and `com.google.zxing:core` 3.5.4; the release APK grew from 17,171,943 to 18,308,148 bytes (+1.1 MB), all native libraries still 16 KB aligned. iOS: none. `scripts/check-ui.sh` no longer bans `AVFoundation` (the scanner needs it; loading it measured 0 ms in the simulator because UIKit loads it already, not measured on a device).

**The Android sheet bug.** Pulling the reader settings sheet (or any sheet) to full height put its handle under the status bar, and touches there went to the system bar, so the content could not be scrolled. `AkiSheet` capped its height at 94% but only padded for the navigation bar. One line fixes every sheet: `.statusBarsPadding()` before `.navigationBarsPadding()` in `manga-android/design/.../AkiSheet.kt`.

**Verified.** Engine and app-call tests pass, clippy is clean. Android API 29: every state except the camera, the QR decoded from a screenshot, the full-height sheet scrolls and collapses. iOS Simulator: every state except the live camera; a 30 MB send went in and out; a valid code sent at once through a temporary hook (removed). Both apps build and lint, `check-ui.sh` passes.

**Not verified.** The live camera scan on both (the Android emulator's camera is disabled by policy; the iOS Simulator has no camera). Choosing the Wi-Fi address on a phone with Wi-Fi and mobile data (Android uses the active network, iOS uses `en0`). The "Receiving from" screen and the "Add the source?" question inside the new sheet on Android, series sharing on the new sheet, dark theme, iPad, API 36. A phone with a display cutout for the sheet fix. Bonjour and Android discovery between real devices.
