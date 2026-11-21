# Flow

How data moves, which file does each step, and what the user sees when it goes wrong.

## The sources document

1. `sources.json` (in the akidev-sources repo) is built into every app and server (`core/src/sources.rs`, `EMBEDDED`). `core/build.rs` downloads it to `desktop/sources/sources.json` when that file is missing.
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
   and what degrades if the order is wrong, are in `pipeline/SKELETON_PLAN.md` ("How to publish this work").

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
