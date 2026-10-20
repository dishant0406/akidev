# Party start and source fallback: findings and plan

> Implemented 2026-10-20: steps 1, 2 and 3.1 to 3.4, with the host-driven fallback on. `max_candidates` stays 3 as the number opened at once, because every listed release is now tried in turn. See `desktop/party/PROTOCOL.md`, "Starting together" and "The same source".

Two problems reported while hosting a watch party:

1. The host starts a film and nothing happens on the joiner until the host's film is playable. Only then
   does the joiner start looking for a source.
2. With ten sources listed, the first one fails and the player says "not playable" instead of trying
   the others.

Everything below comes from reading the code (desktop app, core, server, web). Nothing was reproduced
live yet; step 0 of the plan does that. Desktop is assumed on both sides (host and joiner on laptops).

## 1. Why the joiner starts late

The chain today, for a desktop host:

1. Host opens a film: `WatchPage::new` calls `start` (`desktop/app/src/watch.rs:393`), which runs
   `Streamer::resolve` in the background: `plan` (title, indexer search) then `open_best` (race the
   swarms, up to 75 s).
2. While that runs `self.player` is `None`, and `tick` returns before `party_tick`
   (`watch.rs:598`). **Nothing is published.** The only early publish is for a *release pick*
   (`play_release`, `watch.rs:414`), not for a new film.
3. The first publish happens in `tell_party` once the player exists. Only now does the guest's
   `heard` see a new item and `route::navigate(Route::Watch)` (`party.rs:511`).
4. The guest's page starts `begin(release)`: its own `plan` (second search) and its own swarm open
   (up to 75 s more).
5. The host holds paused up to 45 s for the guest (`hold_tick`).

So the guest's whole cost is added after the host's: `T_host(search + open) + T_guest(search + open)`.

The web is the same shape: `usePartyPlayback.publish` returns until `readyState >= 1`
(`akidev-frontend/src/components/player-party.ts:63`), and the player only mounts after
`usePlay` has the stream. This is edge case L8 in `desktop/party/EDGE-CASES.md`, listed as open.

Two more costs inside that:
- A guest must have the host's release hash before it may open anything (the "same source" rule), and
  the hash is only known when the host's open has finished. That is the real serial dependency.
- The guest repeats the search the host already did.

## 2. Why "not playable" after one source

For a film resolved with no pick, the core does already race several releases. It stops too early,
and the paths around it do not retry:

| # | Cause | Where |
| - | ----- | ----- |
| a | **Only 3 of the listed releases are ever tried.** `plan` offers 24, but `open_best` takes `candidates(&plan.ranked, config.max_candidates)` and the default is `max_candidates: 3`. With 10 sources, 7 are never touched. The error is then "Found releases, but none could be played." | `core/src/config.rs:60`, `stream.rs:184`, `stream.rs:413` |
| b | **"Opened" means "the first bytes arrived", not "it plays".** Success is `hold_open` (start of the file within 15 s). If mpv then cannot decode it (audio codec, broken file), `tick` sets `Phase::Unavailable("The video could not be played…")` and never tries another release. The web does the same (`onError` shows "Your browser cannot play this file"). | `watch.rs:602`, `player.rs:531`, `components/player.tsx:542` |
| c | **A picked release is exactly one release.** `resolve_release` uses `plan.only`, by design ("what they asked for"), so a dead pick ends on the error screen and the next one must be picked by hand. | `stream.rs:107`, `stream.rs:508` |
| d | **A guest never falls back (decided 2026-10).** If the host's release is slow or dead on the guest's side, the guest shows "Can't play" and offers Try again or Leave. In a party with 10 sources, this is probably what you saw. Only the host can move the party to another source, and nothing prompts it to. | `watch.rs:1858`, memory note `watch-party` |
| e | The server (web) retries only `TORRENT_TRIES = 3` within one source type, and the deadline is shared (75 s + 60 s grace). | `server/src/play.rs:32` |

(I checked whether racing candidates evict each other under `max_torrents: 3`: `make_room` counts only
committed torrents, so no.)

## 3. The plan

### Step 0: reproduce (half a day)

Two desktop apps (or a desktop host and a web guest) against `localhost` with the testing recipe in
the `watch-party` memory. Log timestamps for: host opens film, host publishes, guest navigates, guest
`plan` done, guest open done. Confirm the serial chain above and get numbers. For problem 2, point the
indexer mock at ten releases where the first four are dead hashes and the fifth has no decodable audio.
Keep both as regression tests (below).

### Step 1: announce the film at once (fixes most of problem 1, small)

- Desktop: in `WatchPage::new`/`start`, when the role is host and this is the party item, publish
  `{item: Video{id, release: None}, playing: false, position: resume, choosing: "finding"}` before
  `begin`. Move the "player is `None`" guard in `tick` so the host's heartbeat keeps going while it
  resolves (it must also publish the real release the moment `resolved()` returns `Ready`, before the
  player is built).
- Web: publish from the watch page as soon as it mounts as host, and again with `release_id` as soon
  as `/v1/play` answers, not when the `<video>` has metadata.
- Guest with `release: None`: navigate at once, show "Finding the source with the host", run `plan`
  now (warms the indexer cache and the title), and wait for the host's release before opening a swarm.
  Hold the `status` as `loading` for the key.
- Desktop: keep the `Plan` on the page so the second step (`only(release)`) does not search again.
  `Streamer` gets `plan` and `resolve_in_plan(plan, hash)` as public steps (they already exist inside
  `resolve_release`).

Saves: the guest's search, and the guest's navigation delay. Does not yet save the swarm opens.

### Step 2: race the same candidates everywhere (the real parallel start, medium)

Add an optional `candidates: [infoHash]` to `Playback` (ordered, up to 5), written in
`desktop/party/src/protocol.rs` and `akidev-frontend/src/lib/party/protocol.ts`, and described in
`PROTOCOL.md`. Additive, so old apps ignore it (check there is no `deny_unknown_fields`: there is none).

- The host publishes it as soon as its own `plan` returns, long before its `open_best` ends.
- A guest opens `candidates[0]` straight away, in parallel with the host, and commits to whichever
  release the host finally announces (`release`). Usually the same, so the swarm is already warm; if
  not, one extra open.
- Web guests: the server only knows how to resolve a release through `/v1/play?release=`. Add a cheap
  `?warm=1` that does `plan` and starts the open without delivering, so the host's and the guests'
  requests share work (the `flights` key already deduplicates identical requests).

### Step 3: real fallback (fixes problem 2)

1. **More candidates, in two passes.** Raise the default `max_candidates` to 6 and, when the race
   ends with none, carry on through the rest of `plan.ranked` (up to `OFFERED`) until the deadline,
   instead of stopping at 3. Unit tests next to `only_the_best_few_are_tried`.
2. **Retry when playing fails, not only when opening fails.** If the player errors before the first
   frame (or within a few seconds of it), mark that release as failed for this page and call
   `Streamer::resolve_excluding(id, &failed)`, up to 3 times, saying "Trying another source…". Same
   on the web: `onError` before `canplay` re-requests `/v1/play?skip=hash,hash`. A host or solo viewer
   only; a guest follows rule 4.
3. **A picked release that fails offers "Try the next best" first**, then "Choose another source".
   It still never replaces a pick silently.
4. **Host-driven fallback for parties (keeps "one cut for everyone").** A guest that cannot open or
   play the host's release already reports `status {trouble, id#release}`. The host reacts: after a
   grace (about 15 s after the others are ready, or at once for a hard error), it takes the next entry
   of `candidates` that no one has reported trouble on, publishes it with `choosing: "finding"`, and
   everyone moves together. Guardrails: at most 3 switches per film, only before the film has really
   started, a toast on the host ("Maria couldn't load that source, trying another"), and the old
   behavior (Try again or Leave) when the candidates run out. This reverses part of the 2026-10
   decision "a guest never falls back": the guest still never picks a different cut itself; only the
   host does, for everybody.

### Step 4: tests

- `desktop/core`: fallback through ranked dead releases (`open_best` returns the 4th), `resolve_excluding`.
- `desktop/server/src/e2e.rs` already runs a real loopback party: add "host's first release is dead on
  the guest, party moves to the second" and "guest has the page open before the host's source is
  ready".
- Party simulator from EDGE-CASES "How we will know it works": assert time-to-play with the guest
  `T = max(host, guest)` instead of `host + guest`.
- Two-browser puppeteer run for the web side.

### Order and size

| Step | What | Size | Risk |
| ---- | ---- | ---- | ---- |
| 0 | Reproduce with timings | S | none |
| 3.1 | More candidates, second pass | S | low (only more attempts) |
| 1 | Announce at once, keep the plan | M | low, no protocol change |
| 3.2, 3.3 | Retry on play failure, "next best" | M | low |
| 2 | Shared candidates race | M | medium (protocol field, both apps) |
| 3.4 | Host-driven fallback | M | medium (policy change) |

Steps 3.1 and 1 can ship first, together; they need no protocol change and fix most of what was
reported.

## Questions for you

1. **Host-driven fallback (3.4)**: OK to let the host move everyone to the next source when a guest
   cannot get the first? It is the only way a party survives a bad source, but it loosens the earlier
   "guest never falls back" rule (the guest still never switches alone).
2. **Which devices were the host and the joiner** in the failure (desktop to desktop, or one on the
   website)? I assumed desktop on both; the web path has the same late-publish problem but a
   different retry story (the server does the opening).
3. **Candidate count**: 6 tried by default (more bandwidth and swarm joins on a bad title) is my
   suggestion. Say if you want a different number.
