# Offline downloads (desktop app only): plan

Status: built on 2026-10-18 (see "As built" at the end); the sections before it are the plan as agreed.
First version 2026-10-18; rewritten the same day to be desktop-only.
The website gets no download feature of its own, only a way into the app.

## 1. What changed and why

The first plan had the website save files in browser storage, fed by a new server download path. Dropped:

- **Server cost.** Every web download would have been served from our one small box, and HLS re-encodes
  burn its CPU. Twenty people downloading at once would hurt everyone streaming.
- **Browser risk.** Quota is not the issue (browsers allow about 60% of free disk per site, not 2 GB), but
  Safari deletes site storage after 7 days of non-use, background downloads only exist in Chromium, and
  playing an encrypted multi-GB file from storage on iPhone was the unproven part of the plan.
- **The desktop app already does the hard part.** It finds a release and joins its swarm on the user's own
  machine, and libmpv plays any container, so no server conversion is needed. A download is the same
  engine keeping a file instead of discarding it. Our server pays nothing.

Result: **no server work at all** for downloads. The earlier decisions about a server bandwidth
allowance, server licences and a server "prepare" queue no longer apply.

## 2. What we are building

Users of the desktop app download episodes, films and music into a private library and play them offline
with no buffering.

- A saved copy plays only inside Akidev: it is stored in an app-private encrypted container, never as a
  plain `.mp4`. Honest level: protected from casual copying, not DRM. The UI says "private to Akidev",
  never "secure".
- A **Library** screen: Downloading (live progress, pause, resume, cancel), Saved, Collections, a storage
  meter.
- Collections group items the user's way (music collections work as playlists).
- A ping when a download finishes (in-app toast and an OS notification).
- A watch party can play from saved copies: smooth, no jitter.
- The website advertises it: a "Download in the app" button on episodes, films and titles that opens the
  app (deep link) or leads to the app download page.

Out of scope: any web storage, DRM, cross-device sync, accounts, sharing, bring-your-own-debrid (a later
option, see 12).

## 3. What exists today (the facts this plan builds on)

- `desktop/core/src/torrent.rs`: one librqbit `Session` (vendored, patched copy of 9.0.1, see
  `vendor/librqbit/PATCH.md`). It adds torrents with `only_files: Some(vec![])` (nothing selected: streaming
  fetches only the pieces asked for), `persistence: None`, `fastresume: false`, and **wipes its directory
  on start and on quit**. Caps: 8 GiB, 3 torrents. `serve.rs` serves them on `127.0.0.1` with a session
  token and a `Host` check.
- `desktop/core/src/stream.rs`: `plan(id)` finds and ranks releases; `Choice`s are what the Sources panel
  lists; `resolve_release(id, info_hash)` plays a chosen one. A release is identified by its info hash plus
  `file_index` (a pack addresses its files by index).
- `desktop/app/src/player.rs`: libmpv takes any URL or path. `watch.rs`: the watch page, resume position,
  party hooks (`party_release`, `follow_party`). `music_player.rs`: yt-dlp resolves audio locally.
- Paths (`main.rs`): `config_dir` (settings, progress, subtitle choice), `cache_dir` (images, subtitles,
  torrents, catalogue, sources, yt-dlp). There is **no data directory yet**; a library must not live in
  `cache_dir`.
- Navigation: `Route` enum + `AkidevApp::build`; CLI launch through `Route::from_args`. Settings page,
  design tokens and components under `design/*`.
- Party: `Item::Video{id, release}` / `Item::Music{..}` carry no URL; each device finds its own source.

## 4. Design

### 4.1 Getting the file

Sources behind one trait, so more can be added later:

1. **Torrent (v1).** The same ranking the streamer uses picks the release (or the user picks one in the
   Sources panel); then the download adds that torrent and selects **only the one file** (`only_files`,
   found with `list_only` first). The file is written under the library's staging folder, with the
   progress and speed from librqbit's stats.
2. **Music.** yt-dlp downloads the audio (m4a preferred) locally, with title, artist, album, cover and
   duration saved alongside.
3. Later: HTTP range with resume for archive.org and public-domain items; bring-your-own debrid key.

"Same quality as streaming" now means: it saves the very release streaming would play (or the one the
user chose), as the original file. Nothing is re-encoded.

**Engine decision (needs the spike):** keep the existing streaming `Session` as it is (ephemeral,
wiped), and add a **second `Session` for the library** with persistence and fastresume on, its own
directory under `data_dir`, and its own ports. This keeps the "wipe" policy safe and isolates download
bugs from playback. If the spike shows one session can do both cleanly, merge them later.

### 4.2 The container (`.akc`)

- Header: magic, version, plaintext size, a wrapped per-file key, a nonce salt, a small plaintext-free
  manifest (item id, release, file index, media type).
- Body: 64 KiB chunks, each XChaCha20-Poly1305 (or `ring` AES-GCM) with a unique nonce (salt + chunk
  index) and the chunk index and a final-chunk flag in the AAD, so chunks cannot be reordered or cut.
  Random access needs no index: chunk = offset / 64 KiB.
- Written as `.part` with an "incomplete" header, finished by an atomic rename after the last chunk.
- The **sealer** runs after the torrent finishes: read the plain file, write the `.akc`, verify, delete the
  plain file. Until then the plain file sits in the app-private staging folder (named by id, not title) —
  a known limit, stated in the risks.
- Keys: a master key in the OS keychain (`keyring`), falling back to a random per-install secret in an
  app-private 0600 file when no keychain exists; each file key is wrapped by the master key. Losing the
  keychain (new machine, reset) makes containers unreadable; the UI says so on first download.
- No server licence and no expiry: a device-local library has nothing to ask. (A later kill switch can
  reuse the `min_version` mechanism if ever needed.)

### 4.3 Playing

- libmpv2's `Protocol` (`mpv_stream_cb_add_ro`) registers `akc://`. `read` decrypts chunks (a small LRU of
  decrypted chunks), `seek` and `size` come from the header; callbacks never call libmpv (deadlock rule).
  No plaintext reaches the disk.
- Embedded subtitle tracks demux from the same stream. External subtitles (our downloaded VTT) are
  decrypted into memory and loaded separately.
- Fallback if `stream_cb` misbehaves on some demuxer: a loopback HTTP server on `127.0.0.1`, random port,
  token, `Host` check, Range support (the pattern `serve.rs` already uses). Never a decrypted temp file.
- Resume position, audio/subtitle choice and "watched" state are stored per item in the library DB and
  also written to the existing `progress.json` so Home's "continue watching" works.
- The watch page's `begin()` first asks the library: a saved copy of `(id, release)` plays at once, with
  no source search and a "Playing from downloads" cue.

### 4.4 Library store

- `rusqlite` (bundled), `data_dir/library.db`, WAL, `foreign_keys=ON`, `user_version` migrations,
  `integrity_check` at start (a corrupt DB is moved aside and rebuilt from container headers).
- Tables: `items` (id, kind, title, poster, season, episode, release, file_index, size, duration, state,
  added, last_played), `tracks` (audio/subtitle info), `collections`, `collection_items` (many to many),
  `progress`, `jobs` (state machine below).
- Files: `data_dir/library/<2-char>/<item-id>.akc`, staging in `data_dir/library/.staging/`.

### 4.5 Download manager

States: Queued, Fetching info, Downloading, Stalled, Paused, Waiting for space, Sealing, Ready, Failed.

- One active download at a time (2 for music), the rest queue. Per-series pause and cancel.
- A `keepawake` guard (sleep prevention) only while one is active.
- No peers or no bytes for N minutes becomes Stalled, with Retry and "Choose another release".
- Quit or update-restart pauses everything and resumes at launch through persistence and fastresume.
- A single-instance lock in the data directory: one writer for the DB and the session.
- Upload: a low upload cap while downloading, and **no seeding after completion** (pause the torrent as
  soon as the file is done and sealed, then drop it). The Windows firewall prompt on first listen is
  explained in the first-download dialog.
- Pin librqbit at 9.0.1 or later (9.0.0 had partial-seeding and Windows hashing bugs); check the vendored
  copy contains the Windows UDP DHT fix (issue 664).

## 5. Screens

Desktop `Route::Library` with a top-bar entry (a small amber dot while downloads run). Reuses the existing
tokens and components only (warm ink palette, state cards, buttons: primary, secondary, ghost); amber stays
for state (progress, active, focus).

- **Downloading:** poster, title, episode, a determinate progress bar, speed and time left, pause/resume,
  cancel. Queue positions. States with plain words ("Looking for peers", "Waiting for space", "Sealing").
- **Saved:** grid or list; a series expands to its saved episodes with sizes; Play, Remove, Add to
  collection. Empty state is a state card.
- **Collections:** cover mosaic, rename, add/remove, play all.
- **Entry points:** a download button on episode rows, the title page ("Download season" with a size
  estimate and a confirm), the film page, the watch page top bar, and music tracks and albums. Saved items
  show a check and "Play offline".
- **Settings:** library location (with free-space and a missing-drive state), storage cap, "Wi-Fi only"
  when detectable, upload limit.
- Apple-design rules: feedback on press, interruptible transitions that start from the current value,
  instant pause/cancel, reduced motion respected, springs critically damped except on drag release.

## 6. The website's part (no storage, no downloads)

- A "Download in the app" button on episode rows, films and titles, shown on desktop browsers.
- It opens `akidev://download?id=<media id>&release=<info hash>` (an id only, never a link or token). If
  the app does not open within a moment it goes to the `/download` page (the existing installer page),
  carrying `?for=<media id>` so the page says "Get the app to save this episode".
- Phones and tablets see "Get the desktop app to download" linking to the same page.
- The app side: register the `akidev` scheme (macOS `CFBundleURLTypes`, Windows `HKCU\Software\Classes`),
  a single-instance forwarder (local socket) so a running app receives the URL, and a `Route::from_args`
  case that opens the Library with a "Download this?" confirm (never starts silently).
- Marketing copy: "Save episodes and films to watch offline, private to the app." Honest about what it
  is, with no "unlimited" claims. Analytics: counts of clicks and app-open fallbacks only.

## 7. Watch parties with saved copies

- A guest's `party_release` is known: before resolving, check the library for `(id, release)`; if present,
  play it and report ready immediately, so the host does not hold the start.
- A host can start a party from Saved ("Start party with this"), or play a saved copy mid-party.
- Better sync than streaming: a saved copy is the same file as the host's torrent file (same info hash and
  file index), so timelines are identical; seeks are instant and nothing buffers.
- If a local copy's duration differs from the host's by more than a second, stream instead and say why.
- No protocol change (additive only), so older apps and the website keep working; the website guest streams
  as today. A later optional `status.source` hint can show "playing from downloads" in the member list.

## 8. Phases and rough effort (one person)

0. **Spikes (2 to 3 days).** (a) librqbit: add with `list_only`, select one file of a pack, keep it,
   restart and resume; a second Session beside the streaming one. (b) libmpv2 `Protocol` over an
   encrypted chunk reader: seeking, subtitles, a 10 GB file. (c) sealing speed and disk use. (d) deep link
   forwarding on macOS and Windows. Go/no-go and the engine decision.
1. **Library core (4 to 5 days):** SQLite, container, key manager, sealer, single-instance lock, tests.
2. **Download manager (5 to 6 days):** the librqbit source, states, resume, stall detection, space
   checks, keepawake, tests with a local swarm.
3. **Playback (3 to 4 days):** `akc://`, subtitles, resume and progress, party hook in `begin()`.
4. **Library UI (5 to 6 days):** route, screens, entry points, collections, settings, notifications.
5. **Music (2 to 3 days).**
6. **Deep link + web button (2 to 3 days).**
7. **Party polish and hardening (3 to 4 days):** edge-case sweep on macOS and Windows, a 2 to 5 GB film,
   copy, docs, rollout behind a setting.

About 4 to 5 weeks. The risk is concentrated in phase 0.

## 9. Edge cases

Storage and files
- Disk full mid-download: pause, "Waiting for space", check free space before queueing and before a
  season (file size × 1.1 reserved); re-check on resume.
- Library on another drive that is missing: items show "Drive not connected", nothing is deleted; moving
  the library is copy, verify, atomic rename, never a cross-volume rename.
- Killed or crashed mid-write: `.part` + incomplete header + atomic finish; per-chunk tags detect torn
  chunks; on start, any `.part` older than its job is resumed or cleaned.
- Plain staging file left behind after a crash: on start, re-seal or delete it; staging is named by id and
  excluded from search indexing where the OS allows.
- Windows: Defender may lock new files briefly (retry opens with backoff); long paths (`\\?\` or short
  id-based names); firewall prompt on first listen.
- macOS: the notarised, non-sandboxed app can write to the chosen folder; if sandboxed later, a
  security-scoped bookmark is needed.
- Two instances: second one forwards its URL or arguments and exits.
- Container version change: header version, lazy migration that re-wraps keys only, never re-encrypt data.
- Key loss (keychain reset, new machine, copied library): the container cannot be opened; show "Remove
  and download again", never a crash.

Source and content
- Torrent with 100 files, one selected: only the pieces for that file are fetched; boundary pieces may
  write bytes for neighbours (verify in the spike, clean extras).
- A fake or disguised file (size mismatch, not media): check extension and size against metadata, probe it
  before sealing, refuse non-media, never execute anything.
- Very slow or dead swarm: Stalled after N minutes with Retry and "Choose another release"; never loops
  silently.
- Release gone from indexers later: the saved copy keeps playing; "Update available" only if a different
  release exists, never swapped silently; a different release of an episode is a separate item.
- Same item requested twice: de-duplicate by `(id, release)`; the second request opens the existing job.
- Series of 1,000 episodes: confirm with a total size estimate, one at a time, skip episodes with no
  source and list them at the end, per-series cancel.
- Audio and subtitle tracks: embedded tracks come with the file; downloaded VTT subtitles are stored
  next to it (all languages offered at queue time, default the user's).
- Music: failed track does not fail the album; cover and metadata stored locally so the library renders
  offline; yt-dlp updates break extraction sometimes: retry with the bundled updater, then fail with a
  reason.

Playback
- Delete while playing: reference-count readers, defer removal until the player closes.
- Seek past a chunk boundary, very large files, high bitrate: decrypt-on-read cache sized to the demuxer
  window; measured in the spike.
- Resume position per item, reconciled with `progress.json` (last write wins).
- Party: copy differs from the host's duration, host plays a saved copy while a guest streams, guest has
  the copy and the host's release changes mid-party (follow the new release; stream if not saved).

Device and OS
- Sleep, lid close, hibernate: keepawake only while active; on resume continue from persistence.
- Network switch, metered or Wi-Fi-only setting (only where the OS exposes it; otherwise a manual toggle),
  battery saver: pause with a visible reason.
- Quit or update-restart mid-download: pause, resume at launch; no background service.
- Firewall or no inbound ports: slower but works; message explains.

Product
- Upload exposure: the user's IP is visible in the swarm while downloading; the first-download dialog
  says so; no seeding afterwards.
- Legal: saving copyrighted content can infringe where the source is unlicensed; the same responsibility
  as streaming a torrent today. Add a short factual note to terms and the privacy page (what is stored,
  that it is on the user's device).

## 10. Testing

- Rust unit: container read/write, seek across chunk boundaries, truncation and reorder detection,
  key wrap/unwrap, migrations, state machine, DB integrity recovery.
- Integration with a local test swarm (a tiny seeder in-process): select one file of a pack, pause and
  resume across a process restart, stall detection, disk-full handling, delete while playing.
- Playback: an `akc://` round trip through libmpv for MP4 and MKV, seeks, embedded subtitles, external
  subtitles; the loopback fallback.
- Party: two devices, one with a saved copy, sync within today's tolerances.
- Manual: macOS and Windows with a 2 to 5 GB film, laptop sleep mid-download, drive unplugged, two app
  instances, deep link from a browser.

## 11. Risks

1. Unproven pieces to settle in phase 0: stream and download beside each other, selecting one file of a
   pack cleanly, `stream_cb` through the Rust binding (no shipped example, no `cancel`, external subtitle
   behaviour unclear).
2. librqbit regressions (9.0.0 had partial-seeding and Windows hashing bugs; a Windows UDP DHT bug fixed
   in 2026): pin and test; our vendored patch must be rebased with any upgrade.
3. Plain staging file exists until sealed: acceptable for "casual copying" level, stated plainly.
4. Key loss makes a library unreadable.
5. Torrent availability: an old or rare episode may never finish; the UI must make that clear.
6. Disk, Windows and antivirus behaviour on big files.
7. Seeding and firewall prompts; user IP visible in the swarm.
8. Expectation: protected from casual copying, not DRM.

## 12. Later (not in v1)

Bring-your-own debrid key (the user's account, direct download on their machine, server pays nothing and
holds no account: Real-Debrid forbids shared accounts), HTTP range sources, quality or release
preferences ("prefer small encodes"), library export, accounts and cross-device sync, a web fallback only
if a clear need appears.

## 13. Decisions (defaults used unless you change them)

1. **Desktop only; the web only links to the app.** (Settled.)
2. **No server work, no licence expiry.** Dropped with the web path.
3. **Quality = the release streaming would play, or the one the user picks.** No re-encoding.
4. **Collections stay local to the device.** (Settled.)
5. **Seeding after completion: off.** Upload capped while downloading.
6. **Engine:** a second, persistent `Session` for the library (decided in the spike).
7. **Download unit:** the single selected file, not the whole pack (a season is queued episode by
   episode).
8. **v1 scope:** films and episodes with "download season", music right after, Library, collections and
   party playback in the same release.
9. **Default library location:** `data_dir/library`, user-changeable, with a storage cap setting.

## Sources

Web research: librqbit 9.0.1 docs and releases (`AddTorrentOptions`, `only_files`, persistence,
fastresume; issues 625, 635, 664), `libmpv2` 6.0 `protocol` module and the libmpv `stream_cb.h` header,
`chacha20poly1305`, `keyring` 4.2, `keepawake` 0.6, `notify-rust` 4.18, Tauri deep-link docs (pattern),
Real-Debrid terms, TorBox changelog. Earlier research on browser storage quotas and eviction (MDN, web.dev,
WebKit 14403) explains why the web path was dropped.
Code read: `desktop/core/src/{torrent,serve,stream}.rs`, `desktop/app/src/{watch,player,music_player,app,
route,backend,settings,main}.rs`, `desktop/party/src/protocol.rs`, `desktop/vendor/librqbit/PATCH.md`,
`akidev-frontend/src/{app/watch/[key]/[id]/page.tsx,components/player*.tsx,lib/progress.ts}`.


## As built

What exists, and where it differs from the plan above.

- `desktop/library` (`akidev-library`): `container` (as designed, 64 KiB chunks, XChaCha20-Poly1305,
  header-bound wrapped key), `keys` (keychain with a 20 second timeout, then a 0600 file), `store`
  (SQLite: items and collections, damaged list set aside), `download` (one worker, a persistent torrent
  session of its own with no listening port, pause/resume/remove, stall notes, space check at 2x the file
  plus 256 MiB, music over HTTP). 19 tests, including a seeder on this machine serving a three-episode
  pack.
- App: `saved.rs` (the `akc://` protocol, with a test that plays a sealed film to its end through libmpv),
  `library.rs` (lazy handle), `library_page.rs` (the screen), `downloads.rs` (queueing), hooks in the
  watch page, the music player, the episode list, the title page, track rows and albums, the top bar,
  completion toasts and system notifications, keep-awake while saving, the deep link.
- Parties need no extra code: a guest's or host's `begin()` finds the saved copy of the release first.
- `download_picker.rs`: the panel that opens from every Download button of a film or episode, listing all
  sources (the watch page's Sources list, reused) and the subtitle languages to keep; subtitles are stored
  in the library (`subtitles` table, files under `subtitles/`) and played from there offline.
- Website: `app-download.tsx` ("Download in the app"), `/download?for=` copy, privacy page.
- Verified end to end on 2026-10-18 with Sintel (Blender Foundation, Creative Commons): downloaded from a
  real swarm, sealed, the plain file deleted, `ffprobe` cannot read the saved file, and the app played it.

Differences and what is not done:

- No server licence or expiry (dropped with the web path), so there is no `status.source` hint either.
- The key lives in the keychain; an unsigned development build makes macOS ask for permission, and after
  20 seconds without an answer the key goes to the file instead.
- Not done: a library location and storage cap setting, a "Wi-Fi only" switch, rename for collections,
  "play all" for a collection, the release picker for "Download season" and music (the best source is taken), a second app instance forwarding links (a second launch opens its own
  window), HTTP and debrid sources.
- The disk check needs the file twice (plain and sealed) while sealing.
- A second app instance (Windows opens one when the website button is pressed while the app runs) is not
  forwarded to the first: it opens its own window, and starts its own streaming session, which empties the
  streaming folder. The same is true of any second launch today.
- The website button decides the app did not open when neither a blur nor a visibility change happens
  within 1.5 seconds; a browser that asks "Open Akidev?" first may show the download page too soon.

A fresh-eyes review found nine problems, all fixed with tests: a transient keychain failure creating a
new key (now a marker records that the keychain holds the key, and a failure then returns an error), one
episode's removal dropping a season pack's torrent that another episode was using, blocking work on the
window thread (the list opens in the background, the key is fetched at launch), pause and remove being
ignored while looking for peers or sealing, half-written saved files left behind, music downloads that
could not be stopped when the connection went quiet, saved tracks opening the video player, the list
being moved aside after a downgrade, and `/download` losing its cache.
