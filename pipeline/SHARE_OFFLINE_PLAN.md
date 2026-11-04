# Share with no common network: a temporary Wi-Fi for one send

Status: built 2026-11-03 (not committed, not tried on real phones). Written after the share feature (commit `faaea2c`). See "What was built" at the end.

## The ask

Today both phones must be on the same Wi-Fi. The owner wants: when the QR is scanned and there is no common network, a temporary network is made for the share, and it is stopped when the share ends.

## What the platforms allow (the facts everything depends on)

**Android can make a private Wi-Fi. iPhone cannot.** This one fact shapes the whole plan.

- Android: `WifiManager.startLocalOnlyHotspot()` (API 26+) makes a hotspot with no internet. The system picks the name (SSID) and password, the app cannot choose them. One request per app. It needs `NEARBY_WIFI_DEVICES` on Android 13+, and `ACCESS_FINE_LOCATION` on older ones. Closing the reservation stops it. Source: [Android docs](https://developer.android.com/develop/connectivity/wifi/localonlyhotspot).
- Android joining a Wi-Fi: `WifiNetworkSpecifier` with `ConnectivityManager.requestNetwork()`. The phone joins the named network for this app only, and the system asks the person first. On phones that cannot hold two Wi-Fi links, the phone leaves its own Wi-Fi while joined. The network must be handed to our code (`bindProcessToNetwork`), or our Rust sockets use the wrong route. Source: [Android docs](https://developer.android.com/reference/android/net/wifi/WifiNetworkSpecifier.Builder).
- iPhone joining a Wi-Fi: `NEHotspotConfiguration` with `joinOnce = true`. It needs the "Hotspot Configuration" capability. iOS shows its own "Join network?" prompt, the app cannot skip it. `removeConfiguration(forSSID:)` leaves it. Source: [Apple forum answer](https://developer.apple.com/forums/thread/111307). I could not load Apple's own page, so the capability name and the 8 to 63 character password rule come from memory and need checking in Xcode.
- iPhone making a Wi-Fi: no API at all. The person can turn on Personal Hotspot by hand in Settings, and then the phones share a network and today's flow already works.
- iPhone and Android with no Wi-Fi at all: iOS 26 added Wi-Fi Aware, the same standard Android has had since API 26. On paper it links the two. In practice, developers report pairing problems between iOS 26 and Android. See the [Apple forum thread](https://developer.apple.com/forums/thread/801280) and [heise](https://www.heise.de/en/news/Peer-to-peer-WLAN-by-order-of-the-EU-Apple-integrates-Wi-Fi-Aware-10446649.html). Not safe to build on yet.
- Wi-Fi Direct (`WifiP2pManager`): Android only. An iPhone cannot join it. Not useful for us.
- Google Quick Share and AirDrop now work together on some Android phones, but through their own stacks. We cannot use that for our files.

## What works with whom

| Sender / receiver | Temporary network | How |
| --- | --- | --- |
| Android to Android | Yes | The receiver makes the hotspot, the sender joins |
| Android to iPhone | Yes | The Android phone makes the hotspot, the iPhone joins |
| iPhone to Android | Yes | The Android phone makes the hotspot, the iPhone joins |
| iPhone to iPhone | No | Tell the person to turn on Personal Hotspot on one phone. Later: try Wi-Fi Aware |

So the Android phone is always the one that makes the network, whoever sends.

## The flow, in the owner's words

1. The receiver opens the Share sheet. It shows the QR as today.
2. If this phone has no Wi-Fi address (no network at all), or the person taps "No shared Wi-Fi?", an Android receiver starts a local-only hotspot. The QR now also holds the network name and password.
3. The sender scans. If the QR has a network name, the sender joins it first (the system asks the person once), then does the normal check and send.
4. The send runs as today.
5. When it ends (done, declined, cancelled, sheet closed, or 5 minutes pass), the receiver closes the hotspot and the sender leaves the network.

The QR text grows one part: `akidev://share?h=<ip>&p=<port>&n=<ssid>&k=<password>`. `n` and `k` appear only when a temporary network is used. An old app that ignores them still gets `h` and `p`.

## The two cases that need extra work

**A. The Android phone receives (the simple case).** The receiver is the hotspot owner and the server. The engine already runs a receiver. We only change two things: the address given to `Engine.share` is the hotspot address, and the QR carries `n` and `k`.

**B. The Android phone sends to an iPhone (the iPhone cannot make the network).** The Android phone makes the hotspot, and *it* shows the QR, because the iPhone has nothing to show. The iPhone scans it and joins. Now the Android phone does not know the iPhone's address. Fix: after joining, the iPhone calls the Android phone's `register` endpoint (LocalSend already has it). The Android phone learns the iPhone's address from the connection and then sends as usual. This needs a small listener on the sending side during a send. It is the one piece of new engine work.

## Safety

- The hotspot password is in the QR. Anyone who photographs the QR in time can join. They still cannot take a file: the receiver must say yes, and each file has its own token. The network lives only as long as one send.
- One hotspot at a time. A second send while one runs is refused, as today (409).
- Close the hotspot in every path: finished, declined, error, cancelled, sheet dismissed, app sent to the background, 5 minute limit. This is the main thing to test.

## Edge cases and what the person sees

- Phone cannot make a hotspot (Wi-Fi off, tethering on, airplane mode, a phone that does not support it): "This phone can't make a temporary network. Join the same Wi-Fi, or turn on Personal Hotspot on one phone."
- Sender denies the "join network" prompt: "You need to allow joining the temporary network." Back to the scan.
- Sender joins but the host is not reachable: after 10 seconds, show the error above and leave the network.
- Android phone cannot hold two Wi-Fi links: it leaves its own Wi-Fi for the share and comes back. Tell the person in the join prompt text.
- iPhone to iPhone with no network: show the Personal Hotspot hint. No other action.
- Location is off on Android 12 or older: the hotspot cannot start. Ask the person to turn it on.
- The other phone scans the old kind of QR while a temporary network is also up: the `n` and `k` parts win, because the old address would not work anyway.

## Permissions and store forms

- Android manifest: `NEARBY_WIFI_DEVICES` with `neverForLocation`, and `ACCESS_FINE_LOCATION` with `maxSdkVersion="32"`. Our minSdk is 29, so phones on Android 10 to 12 would see a location prompt just for the temporary network. Ask for it only when the person taps the button, never at start.
- Play Console: the Data safety form and the permissions declaration need updating for location on old Android versions.
- iOS: add the "Hotspot Configuration" capability to `project.yml`. It works for App Store apps. It only lets the app manage its own configurations.
- Both: update the "how it works" text so people know a temporary network is used.

## Build order

1. **Hint first (small, no risk).** When a phone has no Wi-Fi address, the Share sheet says "No shared Wi-Fi? Turn on Personal Hotspot on one phone, then join it from the other." This works on both platforms today and covers iPhone to iPhone.
2. **Android receives, any phone sends (case A).** Hotspot start and stop in `ShareModel`. QR gets `n` and `k`. Android scanner joins with `WifiNetworkSpecifier`. iOS scanner joins with `NEHotspotConfiguration`. Stop paths and the time limit.
3. **Android sends to iPhone (case B).** The listener on the sending side, the iPhone's `register` call after joining, the Android phone shows the QR.
4. **Spike, not a promise:** Wi-Fi Aware between two iPhones, then iPhone and Android. Only if step 1 turns out to be too clumsy for real use.

Done means: with Wi-Fi off on both phones, a book goes from phone to phone using only the QR, and the hotspot and the join are both gone afterwards (checked in the phone's Wi-Fi settings).

## What we cannot test here

Emulators and simulators have no hotspot and no real Wi-Fi radio. This needs two real phones, ideally: the Galaxy A05, a second Android phone, and an iPhone. Each row of the table above needs one real run. Phones differ a lot in hotspot support, so expect some surprises.

## Questions for the owner

1. Is "the Android phone always makes the network" fine? It means an iPhone user receiving from another iPhone only gets the Personal Hotspot hint.
2. Do we accept the location permission on Android 12 and older for this feature, or limit the feature to Android 13+ (no location prompt)?
3. Is the 5 minute limit right?

---

# Research round 2: the iOS blocker (2026-11-03)

The blocker, in one line: **an iPhone cannot make a Wi-Fi network, and it cannot join an Android phone's peer-to-peer link.** Apple's own engineer says no app API turns an iPhone into a hotspot, and no workaround exists ([Apple forum](https://developer.apple.com/forums/thread/801998), [Apple forum](https://developer.apple.com/forums/thread/749966)). Apple's advice is to reverse the roles: the other side makes the network and the iPhone joins it. That is what the plan above already does, so the blocker only bites in one place: **iPhone to iPhone with no shared Wi-Fi.**

## Every option I found for iPhone to iPhone

**1. Personal Hotspot by hand (works today, no code).** The person turns it on in Settings, the other phone joins it, and both are on one network. The app cannot switch it on or read its state. We only show a hint. Weak point: the person must leave our app.

**2. Apple peer-to-peer Wi-Fi through the Network framework (Apple phones only).** Set `includePeerToPeer` on the `NWParameters` for an `NWListener`, an `NWBrowser` (Bonjour) and the `NWConnection`. Apple's own guide says this is opt-in, and that it is the same mechanism Multipeer Connectivity used ([TN3213](https://developer.apple.com/documentation/technotes/tn3213-moving-from-multipeer-connectivity-to-network-framework.md)). Facts:
- No special entitlement is named. It needs our existing Local Network permission and Bonjour entry.
- It works only between Apple devices. Android cannot speak it.
- Apple says to stop the browse as soon as the person picks a service, and to switch it off as soon as the work is done, or other apps slow down.
- Forum reports are mixed: connections that hang, and one report needing a 3 second pause after discovery ([Apple forum](https://developer.apple.com/forums/thread/718461)). Multipeer Connectivity itself is flaky on iOS 26 ([Apple forum](https://developer.apple.com/forums/thread/803339)), and Apple says Xcode 27 deprecates it. This API sits on the same base, so test it.
- Simulators cannot test it. Two real iPhones are needed.

**3. Wi-Fi Aware, iPhone to iPhone (the standard way, iOS 26 and newer).**
- The framework and its entitlement `com.apple.developer.wifi-aware` are iOS 26+ and iPhone 12 or newer ([Apple entitlement page](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.wifi-aware.md)). Our deployment target is already iOS 26.0.
- It is a managed capability. One source says the team must ask Apple for it through a form and wait ([Espressif guide](https://developer.espressif.com/blog/2026/08/wifi-aware-esp-to-iphone/)). I could not find Apple's page that says this, so check it in the developer account first.
- Pairing is required, always with a six-digit PIN, through a system screen (DeviceDiscoveryUI) or AccessorySetupKit. Data then flows through the Network framework ([WWDC25 session 228](https://developer.apple.com/videos/play/wwdc2025/228)). Not testable in the simulator.
- Apple's guide says Wi-Fi Aware is the better choice than option 2 when you want peer-to-peer Wi-Fi.
- Cost: an extra system pairing step for the people, and a wait for the entitlement.

## iPhone with Android over a direct link (Wi-Fi Aware): not ready

- Apple and Android both claim the Wi-Fi Aware standard. In practice, as of the last reports (September 2025), pairing works only partly on one phone model (Galaxy S25), and the data path fails. Pixel 9 and Xiaomi 14 do not report the pairing feature at all. Apple's engineer says few phones follow all the rules and the vendors must fix it ([Apple forum thread 790195](https://developer.apple.com/forums/thread/790195)).
- Google's own Nearby Connections says its iOS support "is not currently compatible with any in-market devices" ([Google Nearby discussion](https://github.com/google/nearby/discussions/2447)).
- Android's side needs `isAwarePairingSupported()` to return true, and an API that many phones do not have. Apple has its own PIN pairing screen that Android must answer ([Android docs](https://developer.android.com/reference/android/net/wifi/aware/WifiAwareSession)).
- Android Wi-Fi Direct does not work with iPhone. Multipeer is Apple only. Quick Share with AirDrop is built into some phones and gives us no API ([MacRumors](https://www.macrumors.com/2026/02/17/android-iphone-airdrop-pixel-9-support/)).

So for iPhone with Android, the hotspot route in the plan above is still the only one that works. Do not build on Wi-Fi Aware for that pair.

## What would change the engine for options 2 and 3

Our engine (Rust) opens normal TCP sockets. On iPhone, the peer-to-peer links are only reachable through Apple's Network framework. So Swift must carry the bytes:
- The **receiving** iPhone runs an `NWListener` over the peer-to-peer link and copies each connection to the Rust server on `127.0.0.1`.
- The **sending** iPhone runs a small local listener on `127.0.0.1`. The Rust sender talks to that, and Swift copies the bytes over an `NWConnection` to the other phone.
- Our HTTPS stays on top, so the file and the checks stay end to end. This is a plain byte pipe, about 150 lines of Swift plus tests. It is guessed from how the APIs work, not yet tried.
- The QR then carries no IP. It carries the Bonjour service name (option 2), or nothing extra when Wi-Fi Aware pairing finds the phone (option 3).

## Updated recommendation

1. Do step 1 and 2 from the plan above first (hint plus Android as host). They solve every pair except iPhone to iPhone.
2. For iPhone to iPhone, **try option 2 as a short spike** (two real iPhones, one day): it has no entitlement wait and no PIN. If it connects reliably, keep it.
3. If option 2 is flaky, **apply for the Wi-Fi Aware entitlement** and build option 3, with the PIN screen.
4. Keep the Personal Hotspot hint as the last fallback in all cases.

## Updated questions for the owner

1. Is a short spike on two real iPhones acceptable? I cannot do it on simulators. Which iPhones do you have?
2. Do you want me to check the Wi-Fi Aware entitlement request in your Apple developer account before we plan more?
3. Are you fine with the PIN pairing step on iPhone to iPhone if we need option 3?

---

# Build contract (2026-11-03): what is being built

The owner chose: Android with iPhone (either way) over a temporary Android hotspot, and iPhone to iPhone over Apple's peer-to-peer link (no AirDrop sheet, nothing outside the app). This section is the contract both apps follow so they behave the same.

## Engine (done, tested)

- `desktop/manga/src/share/code.rs`: `Code { ip, port, network: Option<(ssid, password)>, peer }`, `text()` and `read()`. Query names: `h`, `p` (address), `n`, `k` (temporary Wi-Fi name and password), `b` (iPhone peer name). A network needs an address. An address needs both `h` and `p`. Percent-encoded.
- FFI (`desktop/manga-ffi/src/share.rs`), on `Share`:
  - `make_code(ip: Option<String>, ssid: Option<String>, password: Option<String>, peer: Option<String>) -> Option<String>`. It replaces `code()`. `ip` None means this phone's Wi-Fi address from `Engine.share(wifi_address)`. Returns None when there is nothing to say yet.
  - `read_code(code) -> ShareCode { ip, port, ssid, password, peer }`. Errors for text that is not ours.
  - `probe_code(code)` still finds the phone by address (`h`, `p`). It fails for a peer-only code.
- The server already listens on `0.0.0.0`, and `register` already tells the app about a phone through `seen`. A phone that joins a hotspot and calls `probe_code` therefore shows up on the host through `seen`. That is how the host learns the joiner's address.

## Who shows a code, who scans (same on both apps)

| Situation | Shows a code | Scans |
| --- | --- | --- |
| Same Wi-Fi | receiver (as today) | sender |
| Android receiver, no shared Wi-Fi | receiver: temporary Wi-Fi code | sender (Android or iPhone) |
| Android sender, any receiver, no shared Wi-Fi | sender: temporary Wi-Fi code | receiver |
| iPhone sender, iPhone receiver, no shared Wi-Fi | receiver: the normal code, with `b` | sender |

An iPhone never makes a temporary Wi-Fi. An iPhone receiver always shows one code: `h` and `p` when it has Wi-Fi, plus `b` when it can offer a peer link.

## Sheet

- Receive sheet: the QR, one line of help, and below it two quiet buttons: **Scan a code** (both apps) and **No shared Wi-Fi?** (Android only).
- Send sheet: the list of phones and **Scan their code** as now, plus **No shared Wi-Fi?** (Android only).
- **No shared Wi-Fi?** on Android: ask for the permission (nearby Wi-Fi devices on Android 13+, location on older), start the local-only hotspot, show the QR with `n`, `k`, `h`, `p` and the line "Ask them to scan this. It makes a private Wi-Fi just for this send." In send mode, when a phone is `seen` on that network, send to it at once.
- A scanned code is handled the same way on both apps:
  1. `read_code`. Not ours: "That is not an Akidev code".
  2. It has `n` and `k`: join that Wi-Fi first ("Joining their Wi-Fi…"; the system asks the person once). Failure: "Could not join their Wi-Fi."
  3. It has an address: `probe_code` (up to three tries, 1.5 s apart, because the host's address may take a moment). On success: in send mode, send at once. In receive mode, show "Connected. Waiting for <name> to send…".
  4. Probe fails, and the code has `b`, and this is an iPhone: use the peer link (iOS section). On Android a `b` it cannot use says: "That phone has no shared Wi-Fi with you. Tap “No shared Wi-Fi?” on this phone and ask them to scan your code."
  5. Probe fails, no `b`: "Could not reach that phone."
- The temporary Wi-Fi ends when the send is done, when a receive is finished, when the sheet closes, or after 5 minutes, whichever is first. A decline or a failure keeps it up so "Try again" works.

## Android

- New file `share/Hotspot.kt`: `startLocalOnlyHotspot` (ssid, password, the host's address on the hotspot interface) and `stop`; and a joiner that uses `WifiNetworkSpecifier` with `requestNetwork`, calls `bindProcessToNetwork(network)` when it is up (so the Rust sockets use it), and undoes it when done.
- Manifest: `NEARBY_WIFI_DEVICES` (`neverForLocation`), `ACCESS_FINE_LOCATION` with `maxSdkVersion="32"`, `CHANGE_WIFI_STATE`, `CHANGE_NETWORK_STATE`.
- Hotspots cannot be tested on the emulator. Be honest about that in the notes.

## iPhone

- Joining: `NEHotspotConfigurationManager.apply` with `joinOnce = true`; leave with `removeConfiguration(forSSID:)`. Needs the Hotspot Configuration entitlement (`com.apple.developer.networking.HotspotConfiguration`) in `project.yml`. The simulator cannot join a Wi-Fi.
- Peer link (iPhone to iPhone only), new file `App/Share/PeerLink.swift`, Network framework only:
  - Receiver (Receive sheet only, so Apple's peer-to-peer radio is on only when needed): an `NWListener` over TCP with `includePeerToPeer = true`, advertising Bonjour `_akidev-p2p._tcp` under a random name `akidev-xxxxxxxx` (this is the `b` in the code). Every incoming connection is copied both ways to `127.0.0.1:<engine port>`.
  - Sender: an `NWBrowser` for `_akidev-p2p._tcp` with `includePeerToPeer = true`, looking for the name `b`. When found, start a local listener on loopback; the sender gives `probe` the address `127.0.0.1:<local port>`. Every local connection is copied both ways to an `NWConnection` to the found endpoint (also `includePeerToPeer`). Stop the browser as soon as the name is found, and stop everything when the send ends (Apple says to switch peer-to-peer off promptly).
  - The copy loop waits for each send to complete before the next receive (no unbounded buffering). When one side ends or fails, cancel both.
  - TLS stays inside: the engine's HTTPS rides on top, so the peer link is a plain byte pipe.
- `Info.plist` (through `project.yml`): add `_akidev-p2p._tcp` to `NSBonjourServices`.
- AWDL does not work in the simulator. Two simulators on one Mac can still find each other through the Mac's network, which tests the copy loop, not the radio. Be honest about that.

## Docs

The two workers must not edit `pipeline/SHARE_OFFLINE_PLAN.md`, `docs/` or the memory files. They report what they built, what they tested and what they could not.

---

# What was built (2026-11-03)

Checks that passed: engine tests (108 engine, 3 share-through-the-app, 4 code), Android install, lint and unit tests (3 new), iPhone simulator build of the `AkiMangaApp` scheme, `scripts/check-ui.sh`.

**Engine.** `share/code.rs` and `Share.make_code`, `read_code`, `probe_code` (see the contract above).

**Android** (`manga-android/.../share/`): `Hotspot.kt` holds `Hotspot` (start and stop the local-only hotspot), `WifiJoin` (join with `WifiNetworkSpecifier`, bind the process to that network, undo), and a pure `hotspotAddress(...)` with a unit test. `ShareModel` has the `network` state, `makeNetwork()`, `endNetwork()` (done, receive finished, close, 5 minutes) and the scan flow. `ShareSheet` has "Scan a code" and "No shared Wi-Fi?" (which turns into "Use Wi-Fi instead" while the network is up). A decline or a failure keeps the network up. Manifest: nearby Wi-Fi devices (13+), location (up to 12), change Wi-Fi state, change network state.

**iPhone** (`manga-app/App/Share/`): `PeerLink.swift` (`PeerHost` for the receiver, `PeerGate` for the sender, a copy loop that waits for each send), `WifiJoin.swift` (`NEHotspotConfigurationManager`, join once), the Receive sheet's "Scan a code", a "Joining their Wi-Fi…" state. `project.yml` has the `_akidev-p2p._tcp` Bonjour entry and the new `App/AkiManga.entitlements` (Hotspot Configuration). The peer host runs only in the Receive sheet.

**A bug found on the way.** On Android the alert and the "X wants to send you" question were drawn outside the sheet, so they were hidden behind it. A phone could never accept a send. Both are now inside the sheet.

## Flow, step by step

1. Receiver opens Share, shows the code. Same Wi-Fi: sender scans it and sends, as before.
2. No shared Wi-Fi, Android receiver: the person taps "No shared Wi-Fi?", grants the permission, the phone makes a hotspot, the code now has `n`, `k`, `h`, `p`. The sender scans, joins (system prompt), `probe_code`, sends.
3. No shared Wi-Fi, Android sender: the sender taps "No shared Wi-Fi?" and shows the code. The receiver taps "Scan a code", joins, and `probe_code` registers it on the sender (the sender sees it through `seen`), which sends at once.
4. iPhone to iPhone: the receiver's code has `b`. The sender tries the address first. If that fails, `PeerGate` finds `b` over Bonjour, opens a loopback door, and `probe` goes through it.
5. The network ends after the send, after the receive, when the sheet closes, or after 5 minutes.

## Edge cases and what the person sees

- Permission denied: "Allow nearby devices / location to make a temporary Wi-Fi" (the text is not yet seen on a phone).
- Phone cannot make a hotspot: "This phone cannot make a temporary Wi-Fi…". Seen on the emulator. Nothing is left running.
- Join refused or failed: "Could not join their Wi-Fi."
- Phone not reachable after joining: "Could not reach that phone." The phone leaves the Wi-Fi.
- Android scans a code with only `b`: "That phone has no shared Wi-Fi with you. Tap “No shared Wi-Fi?” on this phone and ask them to scan your code."
- Not one of our codes: "That is not an Akidev code".

## Tried on a real phone (Moto G64, Android 15, over USB, 2026-11-04)

- The hotspot starts, with a name and a password, and the engine listens on it. `hotspotAddress` picked `ap0` (192.168.140.13), and the QR held the right address, port, name and password.
- Bug found and fixed: the Wi-Fi comes up before it has an address, so the first look found none and the app shut the hotspot down 0.4 s after it started. `Hotspot.start` now waits up to 5 s for the address.
- This phone drops its own Wi-Fi while it hosts (no Wi-Fi client and hotspot at once). Closing the sheet stops the hotspot and the Wi-Fi comes back. Wireless adb dies at that moment: use USB to test it.
- The Android 13+ permission prompt shows, and "Don't allow" shows "Allow Nearby devices for Akidev in Settings to make a temporary Wi-Fi."
- The real camera opens in "Scan a code". Bug found and fixed: the camera picture leaked outside the sheet (a separate surface ignores clipping); `Qr.kt` now uses the `COMPATIBLE` preview mode.
- Seen, not changed: the Receive sheet opens at about half the screen, so "No shared Wi-Fi?" starts half hidden below the fold (a flick up shows it). The alert card has a faint square box behind its rounded corners.

## Not verified

- Joining on both platforms (the Android join prompt, the iPhone join prompt), and a real send over the hotspot: this needs a second phone.
- WPA3 hotspots, and the Android 12 and older location path.
- Apple's peer-to-peer radio. Only the byte copy was tested, on two simulators through the Mac's network.
- Camera scanning of a real code (the camera opens, but no code was shown to it), and the Accept question inside the Android sheet.
- The next TestFlight archive must be checked: it should add the Hotspot Configuration capability to the App ID by itself.
- Play Console: the permission declaration and the Data safety form must be updated for location on Android 12 and older.
