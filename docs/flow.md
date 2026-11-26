# Flow

How data moves, which file does each step, and what the user sees when it goes wrong.

## The sources document

1. `sources.json` (in the akidev-sources repo) is built into every app and server (`core/src/sources.rs`, `EMBEDDED`). `core/build.rs` downloads it to `content/sources/sources.json` when that file is missing.
2. At start, `load_saved` reads the copy saved by an earlier run. `keep_fresh` fetches a newer signed copy
   every 6 hours. A copy is used only if it is signed, has a higher `version`, and passes `validate`.
3. The desktop app then calls `plugin::load` (`app/src/main.rs`), which puts the switched-on plugins on top.
4. `sources::current()` is the document plus the plugins. Searches read that.

Edge cases:
- A downloaded document that fails any check is ignored, and the app keeps what it has. Nothing is shown.
- A refreshed document replaces the document but keeps the plugins (`adopt` puts them together again).
- A live document that predates `screens` or `image_hosts`: the built-in values are used.

## Screens (tabs, sorts, genres, home rows)

Desktop: `sources::screens()` is read by `app/src/home.rs` (rows, banner), `browse.rs` (heading, sorts, genres,
search tabs) and `app.rs` (top bar). Website: `layout.tsx` calls `getScreens()` (`lib/api-server.ts`), which asks `GET /v1/screens`
(`server/src/api.rs`, `screens`) at most once every five minutes and keeps the answer in the site server's memory
(and the last answer if a refresh fails). The layout hands it to `providers/screens.tsx`. `topbar.tsx`, `browse.tsx` and
`home-view.tsx` read it with `useScreens()`. `lib/screens.ts` has the small helpers.

Edge cases:
- The server's `/v1/screens` call fails and there is no earlier answer: the layout passes `null`. The provider asks again from the browser, so
  the top bar fills in a moment later. A catalogue page with no screens shows "Couldn't reach the catalogue".
- `/films` works, `/movie` is a 404 (the slug is not in `tabs`).
- `/[kind]` when the server cannot read screens: the page throws, and `error.tsx` shows its retry card. A 404
  would be kept by the edge for a while.
- The layout is `force-dynamic`: the API is not running during the site build, so nothing is built ahead.

## Pictures on the website

`/v1/img` (`server/src/img.rs`) fetches a poster only over https from a host in `sources::image_hosts()` (the
providers' `image_hosts`, built-in and live) or in `IMAGE_HOSTS`. Redirects must stay on those hosts. Anything
else is refused, and the site shows a broken image.

## Adding a plugin (desktop)

1. Settings, Plugins (`app/src/settings.rs`). The person pastes a link and presses Add (`add_plugin`).
2. `plugin::install_from_link` checks the link is a public https address, fetches it once with no redirects and
   a 256 KB cap, then calls `plugin::install`.
3. `install` parses and checks the plugin (`Plugin::check`: format, id, name, each definition, every address),
   writes `plugins/<id>.json`, and calls `load`.
4. `load` reads every file, puts the switched-on, valid ones to use through `sources::use_plugins`, and lists
   them. `merge` refuses a source whose id is taken.
5. Settings shows the list. Toggle renames the file to `.json.off` or back. Remove deletes it. Reload re-reads
   the folder, for a file dropped in by hand.

A plugin that brings a kind (`documentary`, shape `series`):
- `merge` registers the kind (`Kind::register`) and adds a tab, so the top bar, the browse page and the search
  tabs show it. The browse page lists from the plugin's catalogues, because a plugin kind has no built-in chain.
- Its keys follow the shape: `documentary-prov~id-s1e2`. A key with the wrong suffix for the shape is refused.
- Play goes through the same release search as a series. Only indexers whose `kinds` include it are asked.
- Removed: the tab goes. Saved titles still read, as series, and show no title page until the plugin is back.

A catalogue plugin, once switched on:
- Its `home` rows are in `sources::screens()`, so the desktop home page draws them (`app/src/home.rs`,
  `row.rs` call `providers::list_row` with the row's provider).
- Search adds its titles after the built-in ones (`providers::search`).
- A title opens by its own key: `providers::detail` sees the `~`, asks that provider alone, and rewrites the
  ids (title, related titles, episodes) so they carry the provider.
- Switched off or removed: the rows, the search results and the keys go away. A key saved in the library or
  history then shows no title until the plugin is back.

Edge cases and what the person sees:
- A plugin that names a kind it does not bring, or sets `Host`, `Content-Length` or another header that decides
  where a request goes: refused, with the reason, and no kind is kept.
- Not https, an IP address, `localhost`, a `.local` name, a port, or a login in the link: "… is not …", nothing
  is downloaded.
- The host does not answer or answers an error: the error text, in red under the field.
- The file is not JSON, or the wrong format, or breaks a rule: the reason, in red. Nothing is kept.
- The id clashes with a built-in or another plugin: "an indexer called … is already there". Nothing is kept.
  If it was an update of a working plugin, the old file is put back.
- A damaged file already in the folder: listed with its reason in red, and the other plugins still work.
- Switched off: the sources vanish from searches at once. Removed: the file is gone.
- The server and the website never load plugins.

## Publishing the sources document

1. Edit `sources.json` (start from the live document, not the repo copy: the daily tracker job publishes from
   the live one) and raise `version`.
2. `cargo test -p akidev-core` and `akidev-sources check` validate it.
3. `publish-sources.sh` runs `check-old-apps.sh` (old app, new document), signs, and uploads.
4. The script derives the schema 2 document (the same one with `"schema": 2`, or `sources-v2.json` in the akidev-sources repo), signs
   both, and uploads `/v2` first and the root second. The daily tracker job refreshes the trackers and the version of
   both, and keeps what the live schema 2 document holds that the other does not.
5. An app asks `/v2` first (`sources::refresh_from`). If that has nothing, is not signed by our key, or has a format
   it does not read, it asks the root. A document is used only if it is signed, newer and valid.
6. Publish before shipping a server or app that needs a new field (`empty_when`, `headers`). The exact steps,
   and what degrades if the order is wrong, are in `docs/plans/SKELETON_PLAN.md` ("How to publish this work").

## Playing a title on the server

1. `Player::play` joins the search for that title (one search for everyone who asks for the same episode).
2. `Player::resolve` waits for one of 4 slots, plans the releases (`Streamer::plan`), keeps only the viewer's pick
   if there is one, and orders the releases by what costs the server least.
3. `Player::try_sources` asks the sources the settings name, in order. Each is given its share of the time left
   (a third, then a half, then all, for three). A source the viewer's pick cannot use (the Archive) is skipped.
4. A source that gives a file ends the search. `Busy` ends it with "busy". Otherwise the last reason is told, or,
   when no release was found at all, the reason for that.
5. Edge cases: a name in `STREAM_SOURCES` that is not a source answers "Unknown stream source"; a file that failed
   is remembered for a while and not tried first next time; a file that played clears that note.

## Playing through a stream service (2026-11-22)

1. The viewer opens Settings, adds a plugin that brings a service (a link or a file), and types the service's key under
   Stream services, then Save key. The key goes to `Config.service_keys` and the settings file (readable only by its owner).
   Files: `app/src/settings.rs` (`services_section`, `save_key`), `core/src/config.rs`.
2. The viewer presses play. `app/src/watch.rs` makes a plan and calls `Streamer::resolve_plan` (`core/src/stream.rs`).
3. `resolve_plan` first calls `from_services_in`. For the best four releases, for each enabled service (`sources::current().services`):
   - a service that needs a key and has none is skipped;
   - `services::resolve_with` calls the module for `files` until it answers (`retry` waits, at most 5 s a time, 40 calls, inside
     20 s for all services);
   - `releases::pick_for_identity` picks the episode's file from the list;
   - the module is called for `link`, and the link is checked: https, public host, on one of the plugin's `media_hosts`.
4. The first link wins: `Resolution::Ready` with that url. `watch.rs` hands it to mpv. There are no torrent stats, so none show.
5. When no service has it, `resolve_plan` goes on to the torrent engine as before.

Edge cases and what the viewer sees:
- No key, wrong key, release not cached, service down, or a link on an unnamed host: the service gives nothing, the torrent
  engine runs, and the viewer sees no difference except a slower start. The reason is logged at debug level (`[service]`).
- The module crashes, loops or exceeds a limit (`core/src/code.rs`): the same.
- A service that keeps asking to retry: stops at the 20 s budget or after 40 calls.
- A plugin removed or switched off: its service leaves `sources::current().services` at once, and its key stays in the file.
- A module file missing or changed: the plugin shows its problem in Settings and is not used (`plugin::load`).

## Signing in to a manga source (2026-11-22)

The engine part, and the iPhone and Android screens that sit on it.

1. The app asks the engine about a source (`Source::info`). `has_sign_in` says the definition has an `auth` section, and `signed_in` says a session is held.
2. The app draws one field per `auth.fields` entry (hidden when `secret` is true) and calls `Source::sign_in` with what was typed, by field id. An empty field stops it before anything is sent: "Email is needed to sign in".
3. `sign_in` sends the `login` request to `base_url` only, with the fields as holes. It reads `login.vars` from the answer. Those values are the session, and the `user` value is what the app can show as "signed in as".
4. The engine hands every change of the session to the app's `SessionKeeper` (`save`), and asks it for the saved session when a source loads (`load`). The iPhone app keeps it in the Keychain (`SessionKeychain.swift`), the Android app in the Keystore (`SessionKeystore.kt`). The password is not kept.
5. Every request to `base_url` now carries `auth.headers` with the session filled in. A mirror gets nothing.
6. If a request gets 401 and the definition has `refresh`, `refresh_unless_done` asks the refresh request once (two refused requests at the same time refresh once), then the first request is asked again.
7. `sign_out` asks the `logout` request if there is one, then forgets the session either way.

**Edge cases and what the person sees.**
- Wrong account (the site answers 400, 401, 403, 404 or 422): "`<source>` did not accept that sign-in: is the account right?". Nothing is signed in. The password is not in the message.
- The site answers 200 but none of the session values are in the answer: "answered, but its answer holds no session".
- The refresh fails or is refused: the session is cleared, and the error says "the session ended, sign in again".
- A source with `auth` still works signed out. The headers are left off, so the free titles load.
- The sign-in cannot reach `base_url`: "could not sign in", with the network reason. It does not try a mirror.
- Command line: `akidev-manga-cli run` signs in first when `AKI_AUTH_<FIELD ID>` are set in the environment, so nothing is typed on the command line.
- Fixtures: the calls `sign_in` (with made-up `fields`) and `sign_out` test a definition with recorded answers.

**On the phone.** Sources tab, the source, then the Account row. "Sign in" opens a sheet titled "Sign in to <source>" with one labelled field per entry in `auth.fields` and the line "Sent to <host> only. Akidev keeps a session, not your password." The Sign in button stays off until every field has text. A refused sign-in shows the message under the fields and clears it as soon as a field changes. On success the sheet closes and the row shows the name the site gave and a Sign out button. Sign out tells the site (if the definition says how) and removes the session from the Keychain or Keystore. Removing the source removes its session too.

**Checked on the iPhone simulator** against `content/manga/demo-server.py` (its `paid` source): a wrong password, a right one, the name shown, a relaunch that gets the session back from the Keychain, and a premium chapter that opens after the server refuses the old token (`DEMO_EXPIRE=1`), with the refreshed session saved back.

**A locked chapter.** The chapter list shows a lock in the place of the read mark on a chapter the source flagged (`chapters.chapter.locked`), while the person is signed out. If the source gives `chapters.locked_label`, that word replaces the small line above the title. Tapping it opens the sign-in sheet (the default, `locked_action: "sign_in"`, and only when the source has a sign-in), and when sign-in works the sheet closes and the chapter opens at its place. With `locked_action: "none"` the lock is only shown and the tap opens the chapter. A date can lock a chapter only while it is ahead (`in_future`). Signed in, it opens like any chapter. If it still fails (the account does not cover it), the reader shows the site's own error. A source with no sign-in still shows the lock when it flags a chapter, and opens it as before. The library keeps the flag with the chapter (`chapter.locked`), so the mark is there offline.

**Recovery.** Sign out and sign in again.

## What a screen reader and large text do now (2026-11-22)

- A screen reader (VoiceOver on iPhone, TalkBack on Android) reads each control by its name. The name starts with the text you see. Icon-only buttons name the action ("Add bookmark"). Pictures that only decorate are skipped.
- Tabs, chips and the tab bar say which one is selected. The tab bar also says how many are unread ("Updates, 3 unread").
- Messages that appear without you touching anything are announced: sign-in errors, "Downloaded", share progress, "Added to library". This lives in `akiAnnounce` (iPhone, `App/UI`) and in live regions (Android, `design/`).
- Large text: buttons, tabs, chips and rows grow in height and wrap. They do not cut text. Long titles on a list still end in an ellipsis, and the full title stays available to the screen reader.
- Reduced motion (iPhone) and "Remove animations" (Android) turn slides and scales into plain fades or no motion.
- Errors: if a call fails, the screen shows a specific title from `describe(error, failed)` and the message from the engine under it. If the phone is offline the title is "Could not reach the site". The share flow retries only for the offline error type, not for a piece of text.

## Removing a series from history (2026-11-23)

1. You long press a card in "Continue reading" (`HomeView.swift` `continueMenu`, `HomeScreen.kt` with `DiscoverModel.continueMenu`) or a row in History (`LibraryView.swift`, `LibraryScreen.kt` with `LibraryModel.historyMenu`).
2. "Remove from history" calls `forgetHistory(source, series)`. The engine deletes that series' progress rows.
3. The app refreshes, then loads the history again. The card or row goes away. Other series stay.
4. "Mark all as read" (shelf only) marks every cached chapter as read. The series counts as finished, so it leaves the shelf but stays in History.

Edge cases: if the call fails, nothing changes and no message shows (the error is ignored, as in the other library menus). If the chapter list was never cached, "Mark all as read" marks nothing. The chapters you read in that series become unread again after a removal.

## Opening Discover now (2026-11-23)

1. The screen asks `reload(force: false)` (`HomeView.swift` on iPhone, `DiscoverModel.reload` on Android).
2. For each source and each list (popular, new releases) the app reads `Engine.fresh_listing`. If a list was kept in the last ten minutes, it is shown and no request is sent. Otherwise the older kept list (`remembered_listing`), if there is one, is shown while a request goes out.
3. Requests run four at a time (`Feed.swift`, `FeedModel.kt`). Each answer replaces its shelf as it arrives and is kept by the engine for next time.
4. Pull to refresh and "Try again" call `reload(force: true)`: every list is asked for again.

Edge cases: with an empty cache every list is asked for (the first launch). A source that fails still shows its kept list, and the error goes to `report`, as before. The cache is cleared only by the engine's own keep step, so a list never gets older than ten minutes without a request, unless the phone is offline: then the older kept list stays on screen.

## The phone app page, `app.akidev.live` (2026-11-23)

1. A visitor opens `app.akidev.live` (the footer link "Phone app", or a link we share). The middleware (`src/middleware.ts`) shows the page `/app` there, with the address unchanged. `/app` on the site redirects to the host, and any other path on the host redirects to the same path on the site. The layout leaves out the top bar, the footer and every call to the API on that host, and sets `data-landing` on the page, which turns on the bright colours (`globals.css`). The page works when the API is down.
2. The server draws all the words at once (`src/app/app/page.tsx`): the opening, the gallery, what the app opens, the privacy lines, the questions and the last block. Nothing needs JavaScript to be read.
3. `FeatureStory` (`src/components/feature-story.tsx`) is a tall block of scroll that holds one full-screen stage. The stage is the whole first screen: the opening words and the store buttons at the top, and a phone rising under them. The page first draws a flat phone, so the first picture is ready at once.
4. When the browser is idle, and only if it can run WebGL and is not asked to save data, the page loads the 3D phone (`phone-scene.ts`, with the `three` package) and fades it in over the flat one. Its canvas fills the whole stage.
5. As the visitor scrolls, the page works out where the scroll is, from 0 to 6 steps. Each step holds its pose for half its scroll, then the phone moves to the next pose. A pose is where the phone is on the stage (across and up), how it is turned and tilted, and how big it is. The opening words and buttons fade out in the first part of the first step. In the middle of each move the screen on the phone and the words change. The stage also changes colour: white, grey, the Night theme for books, and the app's yellow for read aloud.
6. Each step has a large title and a short line of text. They sit on the side opposite to the phone, or above and below it on a narrow screen, so they never meet the phone. They show while the phone rests at its pose. They go as soon as it starts to move, and come back a moment after it has arrived. There are no dots or other controls: the scroll is the only control.
7. The phone moves on springs, so it never snaps. A visitor can drag the stage sideways to turn the phone, and it turns back to its pose when let go. The stage draws frames only while it is on screen.
7b. Between two steps the phone makes one smooth move to the next pose (`update()` in `feature-story.tsx`). It is large, so each screen nearly fills the window. The screen changes at the middle of the move. The page colour (`html` and `body`) moves from one step's colour to the next in the same move. After the last step the colour fades to white in the last hold, so the tour ends on plain white.
7a. The page snaps to the steps. One wheel or trackpad gesture goes to the next step, however hard the flick, and the code ignores the events a trackpad sends after the finger lifts (`onWheel` in `feature-story.tsx`). On a touch screen and with the keyboard, CSS scroll snapping does the same (`scroll-snap-type` on the page, with a snap point at the start of each step and at the end of the last one, all with `scroll-snap-stop: always`). Snapping is on only while the stage is pinned. Above and below it the page scrolls freely, so a visitor is never pulled back as they leave the tour or come back into it. Scrolling down from the last step leaves the tour, and scrolling up from below lands on the last step.
8. `ScreenGallery` is the same screens as a plain row. It shows only for a visitor who asks for less motion.
9. A click on "App Store" or "Google Play" (`store-button.tsx`) sends `download_clicked` and opens the store page in a new tab. The addresses are in `src/constants/site.ts`. The same buttons are in the last block of the page.

Edge cases:
- No WebGL, or the data saver is on: the flat phone stays. Nothing else changes.
- No JavaScript: the text and links work. The flat phone shows the first screen.
- Reduced motion: there is no stage. The opening words and buttons show as plain text, and the plain row of screens shows below. Nothing is pinned or turns.
- A touch screen: the phone turns with a sideways swipe. Vertical swipes still scroll the page (`touch-action: pan-y`). The phone sits above the words.
- A screen reader: every step is in the page as text, in order, so all six are read.
- Store address wrong or app removed: the store shows its own page. Nothing on our side can check it.

Remaking the pictures: serve the repo, then run `python content/manga/film/tools/web-assets.py URL OUT` and copy `screens/*.webp`, `app-loop.mp4` and `app-poster.webp` into `apps/web/public/phone/`.

## Searching, Done, and the Home icon (2026-11-24)

1. You type in the search field on Discover (`HomeView.swift` on iPhone, `HomeScreen.kt` on Android). You press the keyboard's Search key or the "Done" button in the field. Both call the same `onSubmit` (`AkiInput.swift`, `AkiInput.kt`).
2. `onSubmit` sets `submitted` and calls `reload()`. With "All sources" chosen, `GlobalSearchModel.search` starts one task per source (`GlobalSearchView.swift`, `GlobalSearchModel.kt`). Each source answers on its own and shows its row of covers when it arrives. A slow or broken source shows its own message.
3. A new search cancels the one still running. Answers from an old search are dropped (`self.term == term`).
4. With one source chosen, `BrowseModel.load` runs the search and shows one grid.
5. You tap the Home icon in the tab bar. If another tab is open, it opens Home as before.
6. If Home is already open, `AkiTabBar` calls `onReselect`. On iPhone, if `model.homeDirty` is true, `model.homeResets` goes up by one and the Home screen is made again (`AkiMangaApp.swift`). On Android, `DiscoverModel.goHome(router)` closes the pushed series, clears the text, search and filters, scrolls to the top and loads the dashboard.

Edge cases:
- Done on an empty field: it acts like pressing Search on an empty field. The dashboard lists show again.
- Done in a password or email field (sign-in forms): it only closes the keyboard.
- Home tapped on a clean dashboard: nothing happens, so nothing flickers.
- Home tapped while a series from the results is open: the series closes and the dashboard shows. This is the case that depends on `onDisappear` on iPhone. Check it on a phone after a change to the Home screen.
- People who read books: the Home tab is the Books home, which has no search. A tap on its icon does nothing.
- If a search crashes the app on a phone (the old "pure virtual function called" report): see the 2026-11-24 entry in `docs/design.md`. A Release build on a real phone is the only way to see it. The simulator does not.

## Asian drama release search (2026-11-25)

1. `Streamer::plan` reads the series detail from Cinemeta.
2. If `aliases.identity` names a provider for this kind and the detail has one title, `identity_aliases` asks that provider (TVmaze today) for the same IMDb id or TV database id. It adds at most two names. It also records whether the title is ambiguous. It copies country and language data when the primary catalog has none.
3. `series_search_terms` makes the episode forms for each known name.
4. The indexer engine asks the normal sources first. If none finds enough, it asks the two Nyaa live-action sources. Each Nyaa source asks once per title.
5. `rank_series_releases_by` applies the country filter only for an ambiguous title. The release name must have a word or tag of the language that `languages` links to that country (`languages::markers_for`). A country no language claims is not filtered. A year-bearing pack must match the series year.
6. `rank_releases_by` puts named episodes first. It then puts single-file fallbacks, season packs, and year-bearing packs for file inspection.
7. The torrent engine or debrid service reads the file list. `pick_video_file_for_release` must find the requested episode before playback starts.

Edge cases:

- TVmaze fails or answers slowly. Akidev keeps the Cinemeta title and continues the release search.
- TVmaze returns a different IMDb id and TV database id. Akidev does not use its alternate names.
- The title is ambiguous and no release has the correct country token. Akidev returns no source.
- A year-bearing pack has the wrong year. Akidev drops it before file inspection.
- A Nyaa request fails or returns no rows. The other sources continue.
- A year-bearing pack has no file for the requested episode. Playback rejects it.
- A release names a different episode or season. The existing hard filters reject it.

## Where the words come from (2026-11-25)

- The language words in release names, the country filter and the subtitle language names all read `languages` from the sources document in use (`core/src/languages.rs`). A document without it uses the built-in copy.
- When a new document is adopted, the three release patterns that hold language words are built again on the next use (`language_patterns` in `releases.rs`).
- When no release is found for a film or a series, `aliases.catalogue` says which kind's catalogue is asked for other names. A kind that is not listed asks nobody.
- The fallback search: `GET /v1/fallback/search` finds the kind with `Kind::parse`. A kind with no `prowlarr.categories` entry answers 400 "There is no fallback search for that kind." The category range comes from `sources::prowlarr()`, and results outside it are dropped.
- The deploy: `scripts/prowlarr-sync.js` reads `prowlarr.sites` from `https://sources.akidev.live/sources.json`. If the document cannot be read or has no `prowlarr` entry yet, it logs that and changes nothing. Publish the document before the deploy.

