# Watch party: edge cases and the plan to fix them

Findings from three independent read-throughs of the code (web app, desktop app, transport and
server), a look at how Syncplay, Jellyfin SyncPlay, Teleparty, Hulu Watch Party and Disney GroupWatch
fail, and live checks against akidev.live.

How sure each one is: **ran** = reproduced against the live site with two real browsers (or `curl`);
**code** = confirmed by reading the code; **maybe** = depends on a browser, a network or timing and
was not reproduced. Severity: **blocker** (breaks the party for everyone, or cannot be recovered
from), **bad** (clearly wrong, users will hit it), **meh** (annoying), **polish**.
Platforms: W web, D desktop, S server or transport, B both apps.

## Where this stands

Decisions: parties are private (the code is shared with friends, there is no public list); a guest
always plays the host's stream and never falls back to another; a host that reloads resumes the same
party; a desktop host on a network that blocks direct connections is told to host from the website
(no TCP/TLS TURN work in the desktop stack); everything is additive so old apps keep working, and the
published sources can say which versions may run (`min_version`, see `desktop/sources/README.md`).

**Done** (code and tests, both apps where it applies): L1 goodbye on quit and `pagehide`, L2 liveness
(heartbeat while paused, silence watchdogs), L3 guest rejoin, L4 web host resume, L5 re-register and
retry, L6 the guest's picture survives the party ending, R1 and R2 (status resent on connect, ghosts
expire), R5 (no fallback release), R7 (a buffering guest climbs back by itself with a seek cooldown,
the host is told "X is buffering"), S1, S2, S4, S5, S6 (web), S7, C2, C3 (web), C4, C6, C8 (tap to join, web), C9, N6
(no fallback), N7 (bookkeeping reset on a new role), N8 (cancel, no parallel join), N9 (names cleaned by the host),
T1 (no-store, cache rule), T3, T5, T6, T7, T8 (code leaves the address bar), T9, T10.

**Open, not part of this pass:** L7 and L8 beyond release picks (a host's in-page switch to the next
episode still briefly publishes nothing before the new item), R3 (a watchdog for a guest that never
gets `canplay`; guests now preload fully), R4, R6 (clearing a hold on a manual play), S3 (debounce a
flapping `readyState`), C1 and C7 (control scoping, host-only mode, kick), C5 (desktop keys while
loading), N1 to N5, N10, T2 (by decision), T4 (trickle ICE).

## What the other systems teach (these map onto our cases below)

- Jellyfin's top bug class is a room stuck **waiting for one client that never says ready** (a client
  that joined mid-film, a client that never answers, a stale buffering flag). Needs a timeout and a
  way to drop a stuck client. (our R1, R2, R3)
- Jellyfin's drift correction **seeks repeatedly, which restarts server-side video conversion** and
  leaves clients on a spinner; a client reporting a **stale position after a seek** counts as ready;
  a fixed tolerance plus a slow link makes a **correction loop**. (our S1, R7, S2)
- Syncplay prefers **slowing down over seeking back**, lets a lagging user opt out of dragging others,
  and can pause everyone when someone disconnects. (our S1, R7, L2)
- Teleparty and Hulu: sync breaks when people are on **different cuts** or when one person's player
  pauses for something the others do not have (ads). Drift **accumulates** across breaks. Disney
  GroupWatch never synced well and was shut down. (our R5)

## Checked live

| Check | Result |
| --- | --- |
| Host opens Sources (film pauses for everyone), the last guest leaves, host closes the panel | **The host's film stays paused.** (E1, ran) |
| Host ends the party while a guest is following | **The guest's video element is thrown away; the film restarts at 0:00, paused, on a different stream.** (L6, ran) |
| `GET /v1/party/{code}/ice` through Cloudflare | **`cf-cache-status: HIT`.** The cache rule covers it (T1, ran). `/offers` (has an Authorization header) is `BYPASS`. |
| A guest on the same film joins and the host's member list | Not conclusive (the solo guest was blocked by autoplay), left as R2 (code). |

## 1. Session lifecycle and resilience

| ID | Edge case | What happens | Sev | Plat | Sure | Fix |
| --- | --- | --- | --- | --- | --- | --- |
| L1 | Quit, close, crash or update-restart sends no goodbye. Desktop Cmd-Q and the auto-update restart never call `end`/`leave`; a web guest closing the tab sends no `bye`. | Ghost member for 30 s or more, party kept on the server for 120 s, every new film waits the full 45 s for the ghost. Guests of a quit host keep playing alone. | bad | D, W | code | Send `bye` (and end the party) in the quit hook with a 1 to 2 s cap; `pagehide` for guests too; host drops a guest silent for about 15 s. |
| L2 | No liveness. A paused host sends nothing; nobody times out a silent peer; the guest extrapolates the host's position forever. | A sleeping or frozen host leaves guests playing alone for 30 s or more until ICE fails, with a stale "Choosing" overlay stuck on. | bad | B | code | Host sends `state` every 2 s even paused; guest shows "Host disconnected" and pauses after about 8 s of silence; cap how far a position is projected. |
| L3 | No reconnect and no ICE restart. A wifi change, sleep or background kills the data channel. | Guest sees "The party ended" although the host is fine. No rejoin, code wiped. Same text for ended, refreshed and dropped. | bad | B | code | Keep the last code; retry join for about 30 s; ICE restart on `disconnected`; give `bye` a reason. |
| L4 | Web host refresh, tab close, discard, or iOS suspend ends the party. | A 2 hour movie night dies on one refresh; everybody needs a new code (and creates are limited to 20 per hour). | bad | W | code | Keep `{code, token}` in `sessionStorage`; the server accepts an idempotent re-register (see L5); `beforeunload` warning with guests. |
| L5 | Server restart or deploy wipes all parties; host sleep over 2 min. The host's offer loop gives up after 6 failures (about 12 s) and never restarts; the role stays `host` with a dead code. | Existing guests keep playing but nobody can join or rejoin; the only sign is one error line inside the panel. | bad | B, S | code | Retry with capped backoff until really gone; re-register the same code and token; toast and a visible "party closed" state. |
| L6 | When a party ends or the guest leaves, the guest's page re-derives its release (`wanted` becomes null) and the player is torn down. | **Film restarts at 0:00 on another stream.** Happens to every guest when the host closes the tab. | bad | W | **ran** | Latch the release being played; never un-set it on leave. |
| L7 | A host's Player publishes `item: null` whenever it unmounts, including in-page switches (next episode, release pick). | Guests flap: derived release flips to null, each guest fires a `/v1/play` for the default release, tears down its player, and the follower can pull a wandering guest back. | bad | W | code | Do not publish null for an in-page switch; publish the new item as intent (see L8). |
| L8 | Host opens a film or the next episode; guests are told only after the host has found its source (desktop) or only on Player mount (web). | Guests find their source one after another instead of in parallel. A torrent can outlast the 45 s hold. | bad | B | code | Announce `{item, playing:false}` the moment the host starts or presses Next. (Release picks already do.) |

## 2. Waiting, holds and readiness

| ID | Edge case | What happens | Sev | Plat | Sure | Fix |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | A guest on another page, or a ghost, is counted as "loading" for every new item key. | The host waits the full 45 s on every new film and release. | bad | B | code | An explicit `away` readiness; report status from the party provider, not from the page; expire silent guests. |
| R2 | Web `setStatus` records `said` even when not yet connected. The status effect fires during the join and is then never resent. | Host sees "Loading" for the guest forever (45 s hold) if the guest was already on that film. | bad | W | code | Only record `said` when it is sent; resend on connect and on role change. |
| R3 | A guest that never gets `canplay` (Safari or iOS under `preload="metadata"`) never says ready; a guest stuck with no peers is `loading` forever and never `trouble`. | 45 s hold every time; the guest sees a spinner forever. | bad | W | maybe | `preload="auto"` for guests; also send ready on `loadeddata`; a 60 s watchdog that says `trouble`. |
| R4 | "Ready" on desktop means the player is configured and not buffering, not that anything is buffered. | The hold releases and a torrent guest stutters straight away. | meh | D | maybe | Require a few seconds of cache before reporting ready. |
| R5 | A guest that fell back to another release (the host's is gone) still says "ready" for the host's key. | Different cut, framerate or intro: constant seeking, subtitles off, no indicator on either side. | bad | B | code | Put `duration` in `state`; guests compare, warn, and report the fallback in `status`. |
| R6 | A play or pause during the hold does not clear it; the hold release overrides a host pause or an open panel; on iOS the release `play()` is rejected with no prompt; a cached release switch skips the hold. | "Waiting for X" shown over a moving film; guests left behind; host stuck paused with no hint. | meh | B | code | Track the host's intent; clear the hold on any manual play or pause; "Tap to start for everyone" on rejection. |
| R7 | Mid-film, a buffering guest pauses nobody and chases the host with seeks. | The guest may never converge; every seek restarts buffering (and server conversion). | blocker | B | code | See S1, and an optional "wait for everyone" mode (a first-class `buffering` status). |

## 3. The sync engine

| ID | Edge case | What happens | Sev | Plat | Sure | Fix |
| --- | --- | --- | --- | --- | --- | --- |
| S1 | Follow re-seeks every 1 s while the guest is still seeking or loading. A slow seek reports the target time while the host keeps playing, so the next tick sees a gap over 0.8 s and seeks again. No `seeking` guard, no cooldown (web). Desktop re-seeks at most once per second. | **A guest whose seek takes over a second never lands.** Each seek restarts server conversion (HLS) or torrent ranges. | blocker | B | code | Skip while `seeking` or buffering; 3 to 5 s cooldown after a correction; aim at `positionNow` plus expected load time. |
| S2 | The clock offset keeps the lowest round trip forever and measures it on the wall clock. | After an NTP step or wake from sleep the offset is wrong for the rest of the party (up to the 10 s clamp: the guest seeks 10 s ahead on every heartbeat). | bad | B | code | Measure round trips with a monotonic clock; keep a 60 s window; reject absurd samples. |
| S3 | A host whose `readyState` flaps publishes `playing:false` and back. | Every guest pauses and re-seeks on each blip. | meh | B | code | Debounce `playing:false` caused only by buffering (about 500 ms to 1.5 s). |
| S4 | Playback rate (0.95 or 1.05) is left set on early exits (choosing, host stopped, item mismatch) and not reset when a new player starts; paused correction can repeat for keyframe-aligned landings. | Odd speed after the party, skipped corrections. | polish | B | code | Reset in those branches and in `begin`; wider paused tolerance (about 0.5 s). |
| S5 | Hidden tabs throttle timers to 1 s or slower. | Drift builds until the tab is visible. | meh | W | maybe | Resync on `visibilitychange` and `pageshow`. |
| S6 | A guest whose film ends before the host's (gap in 0.15 to 0.8 s) gets `play()` on an ended element. | Flash of the opening, then a seek back. Music has the same. | meh | W | code | Skip follow when `ended`; clamp the target to the duration. |
| S7 | The host's first publish uses default state (position 0, playing); a late joiner is projected without a cap. | Guests seek to 0 and then to the resume point (two conversions); joiners can be sent far past the real position. | meh | B | code | Publish the start position until metadata is loaded; cap projection to about 10 s. |

## 4. Controls

| ID | Edge case | What happens | Sev | Plat | Sure | Fix |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | A guest's pause, play, seek (and music controls) always ask the host, even when the guest shows a different film or music while the host plays a film (and the host only checks its own item). | A wandering guest pauses or seeks the host's film with another film's timeline, and cannot control their own film. | bad | B | code | Ask only when the page's item matches `party.item`; otherwise act locally; carry the item key in requests. |
| C2 | A guest presses an arrow key or the seek buttons while the duration is unknown (NaN or 0). | The seek clamps to 0 and **rewinds the film for everyone.** | blocker | B | code | Ignore seeks until the duration is known and the page is the party item. |
| C3 | Held arrow keys and scrub-bar drags send one seek per repeat or step, computed from a stale local position; the scrub thumb snaps back. | Dozens of seeks, each possibly restarting conversion; the host moves only 10 s. | bad | B | code | Ignore key repeats; local drag value and one seek on release; base on the pending target; throttle. |
| C4 | Desktop: a click on the picture toggles pause locally and bypasses `ask` (reverted by follow within 250 ms). Web: a double click sends pause then play. | Sound blip on desktop; flapping on web. | bad | B | code | Route through the same toggle; delay or cancel the single click on a double click. |
| C5 | Desktop keys act in every phase. | A guest still loading can pause or seek the host just by mashing space. | bad | D | code | Ignore keys unless the page is playing the party item. |
| C6 | Any guest can seek to the end. | The host's film ends and autoplays the next episode; the host's Continue Watching changes. | meh | B | code | Clamp guest seeks to `duration - 1`; or host-confirm. |
| C7 | There is no host-only mode, no kick, no rate limit, no ack or deny; requests are dropped silently (host on a title page, mismatched item, panel open). | Any guest (or a bug) can wreck the party; buttons "do nothing" with no reason. | bad | B, S | code | `controls: everyone or host`, `kick`, per-guest token bucket, `deny{reason}`. |
| C8 | Autoplay policy blocks the guest's `play()` (iOS needs a gesture for every new element; the player remounts per film, episode, release). OS-level pauses (PiP, headphone button) and media keys are undone by follow or bypass the party. | Permanently paused picture, no prompt. | blocker (iOS), bad | W | maybe | "Tap to join playback" overlay on `NotAllowedError`; route media keys through the toggle. |
| C9 | Panel pause semantics: `panelPaused` is stuck if the last guest leaves or the party ends while a panel is open; a guest play, pause or seek while the host has a panel open overrides it; the subtitles panel pauses everyone for a local choice. | **Host stays paused after closing the panel (E1).** Host plays behind a panel while guests wait. | bad | B | **ran** (E1) | Resume regardless of company; clear on role loss; ignore requests while choosing; do not pause for subtitles. |

## 5. Navigation, state and UX

| ID | Edge case | What happens | Sev | Plat | Sure | Fix |
| --- | --- | --- | --- | --- | --- | --- |
| N1 | A guest away from the film has no way back (web banner only on the watch page, desktop button only on the watch page); the host's "Choosing" overlay is drawn over a guest's own different film. | Lost guests; confusing overlay. | bad | B | code | A "Now playing, Join" row in the party panel and top bar; show the overlay only when the item matches. |
| N2 | The guest pushes a history entry per episode (the host replaces). | Back walks through old episodes, each resolving a stream. | meh | B | code | `replace` when already on a watch page. |
| N3 | A release or episode change unmounts the player, which exits fullscreen; the "Join them" banner and errors are not inside the fullscreen element. | Lose fullscreen on every switch; guests see nothing in fullscreen. | meh | W | code | Keep the player mounted during a switch; render banners inside the player. |
| N4 | Music and video collide: music publishes and can win while a film is held; a guest on a film page hears both when the host plays music; guest music controls ask the host for a film; joining a party overwrites the guest's music queue. | Items flip, overlapping audio. | bad | B | code | Pause music when the host's Player mounts; pause the film and show a banner when the party plays music; separate party queue. |
| N5 | A guest resolves with its own resume position (and a stale `start`), then seeks to the host; a page built before the party target arrives plays "best" and restarts on the host's release; `begin` overwrites the `resume_from` that follow just set. | Two conversions and a visible reload; HLS guests are pulled back to their old resume point. | bad | B | code | Start from the host's `positionNow`; defer resolving until the target is known; do not overwrite an explicit position. |
| N6 | A guest in Resolving, Unavailable or error state never follows a release change (desktop). Fallback is serial after the wanted release fails (web). | "Nothing playable" until Try again; fallback may outlast the 45 s hold. | bad | B | code | Run release-follow before the early return; start the default in parallel with a short timeout. |
| N7 | Desktop: "End party" then "Start party" on the same open page. `PartySync` survives, so every publish is dropped as a heartbeat. (Same in `MusicPlayer`.) | **Nothing is ever published until the host pauses or seeks; guests get nothing and guest requests are ignored.** | blocker | D | code | Reset `PartySync` and publish a non-heartbeat when the role becomes host. |
| N8 | Join UX: double click or auto-join plus button leaks a parallel connection and a ghost row; no cancel for about 115 s of "Connecting"; a pasted invite link is read as a garbage code; a failed join loses the typed code; "The party ended" is only visible inside the panel (and sticks); the host's own name can be blank. | Orphans, confusion, no feedback. | bad | B | code | Session or generation guard; abort signal and a Cancel button; parse `?party=`; keep the tab and code; toasts. |
| N9 | Names: no member ids, duplicates, bidi and zero-width characters pass; a half emoji from `slice(0,24)` makes `members` unparseable on desktop. | Desktop guests stop seeing member updates while that name is present. | meh | B | code | Truncate by code points; strip Unicode format characters; add `id` to `Member`; "(2)" for duplicates. |
| N10 | Focus is not retaken after a panel closes (desktop); status screens have no party button (web); the waiting pill is unbounded; the party popover fades with the controls. | Dead keys, no way to leave, odd UI. | polish | B | code/maybe | Retake focus; add the button; "Waiting for N people"; keep controls visible while the popover is open. |

## 6. Transport, server and security

| ID | Edge case | What happens | Sev | Plat | Sure | Fix |
| --- | --- | --- | --- | --- | --- | --- |
| T1 | The Cloudflare cache rule covers `/v1/party/*`; handlers set no `Cache-Control`. | **`/ice` is served from cache (HIT).** A cached 200 for a dead party; a cached 404 for a code for up to 3 minutes. | bad | S | **ran** | `Cache-Control: no-store` on every party response and exclude `/v1/party` from the rule. |
| T2 | The desktop WebRTC stack ignores TCP and TLS TURN and binds IPv4 only. Cloudflare's list has udp, tcp and tls. | Desktops on UDP-blocked networks (office, school, hotel) cannot host for web guests or join: about 45 to 60 s, then "did not connect". | bad | D | code | Bind IPv6 too; add a TCP-capable path; detect "only UDP relay" and tell the user. |
| T3 | TURN credentials are optional, fall back to STUN silently, are shared for 6 h across requests, are not cached on failure, and can expire mid-party. | Symmetric NAT or CGNAT users get a spinner and a generic error; cost abuse is possible. | bad | S | code | Boot warning and `relay: bool` in `/ice`; serve stale on error; 12 h TTL and a short cache; alert on usage. |
| T4 | No trickle ICE: candidates frozen after 10 s; a join costs up to 10 s on each side. | Relay-only users can fail because the relay candidate arrived late. | meh | B | code | Send the SDP at the first relay or srflx candidate; shorter cap. |
| T5 | The poll limiter (60 per 10 min per client) counts legitimate traffic. | Shared IPs (iCloud Private Relay, VPN, CGNAT, dorms) get random 429 on join. | bad | S | code | Count only unknown-code lookups. |
| T6 | Ghost slots: a failed `DELETE guests` leaks a place for the life of the party; 20 cap; the 21st sees "full". | Party becomes "full" for no reason. | bad | S | code | Host sends its active guest ids with each poll and the server reconciles. |
| T7 | Abuse: unbounded message floods (every `hello` and `status` is rebroadcast), slot squatting, 5000-party cap exhaustion by about 21 IPs, 6-letter code guessing (about 5.6e8 tries a day for a /48 holder), unauthenticated routes parse 2 MB bodies, crafted SDP makes the host probe addresses. | Griefing and resource exhaustion on public codes. | bad | S | code | Per-guest token bucket and bounded channels; hello deadline; per-client party cap; 8-char codes or host approval; 64 KB body limit; filter private candidates. |
| T8 | The invite URL (`?party=CODE`) stays in the address bar and PostHog records it. | The live code is in analytics and history; anyone with access can join. | bad | W | code | `history.replaceState`, scrub `party` in PostHog, `Referrer-Policy: same-origin`. |
| T9 | `Peer` is dropped without `close` on early failure (Rust); `end()` races with in-flight accepts (a zombie guest on a dead party); `bye` is lost because close runs at once (web guest, Rust host and guest). | Leaked sockets and TURN allocations; guests learn late. | meh | B | code | `Drop` closes; an `ended` flag; 200 to 500 ms before close. |
| T10 | No protocol version; unknown enum values fail the whole message in Rust; web and Rust parse differently (negative or fractional numbers). Hostile hosts can steer guests to arbitrary ids and cover URLs. | Old clients silently stop following; platforms disagree on edge values. | bad (future) | B | code | `v` in `hello` and `state`; map unknown variants to `Unknown`; shared JSON conformance tests; validate ids and covers. |

## Protocol gaps (what real use needs and the protocol cannot say)

1. **Readiness per item key is binary-ish.** Missing: `away`, `buffering`, "on a fallback release".
2. **`playing:false` conflates** paused, buffering, holding, switching and "left the film".
3. **Requests have no sequence, no item key and no answer** (ack or deny). Toggles race.
4. **No liveness** in either direction, no host heartbeat while paused.
5. **No session identity**, so no rejoin, no resume, no reason in `bye`.
6. **No permissions** (host only), no kick, no rate limit.
7. **No duration or timeline** in `state`, so a different cut is invisible.
8. **No version or capability negotiation.**
9. **Not needed yet:** reactions, chat, host transfer, next-episode hints.

## The plan, in the order I would do it

Each phase is shippable on its own.

**Phase 0, hotfixes (small, safe, this week).** T1 (no-store plus rule), C2 (seek with unknown duration),
C5 (desktop keys), C9/E1 (stuck panel pause), L6 (latch the release), N7 (restart on the same page),
N8 (join guards), T8 (strip the invite), L1 (goodbye on quit and `pagehide` for guests), C4 (picture
click), N1 (overlay only for the matching item), S6 (ended guard).

**Phase 1, sync correctness.** S1 (seek guard and cooldown), S3 (debounce), S2 (monotonic clock with
a window), L2 (heartbeat while paused and a host-silent state), L7/L8 (announce intent, no null on a
switch), R1/R2/R3 (readiness from the provider, resend on connect, watchdog, `preload`), N5/N6 (start
from the host's position, follow release changes from any state), R6 (hold intent), C1/C3 (control
scoping, key repeats, scrub).

**Phase 2, protocol v2 (backwards compatible).** `v` in `hello` and `state`; `seq`, `item` and
`ack`/`deny` on requests; `controls: everyone or host`, `kick`, rate limiting; `duration` and a
fallback flag in `status`; `away` readiness; `bye.reason`; a session id for rejoin; R5 and R7 (the
optional "wait for everyone" and different-cut warnings).

**Phase 3, transport.** Trickle ICE; desktop IPv6 and a TCP-capable path; relay visibility (T3);
reconnect and ICE restart; host resume token with idempotent re-register (L3, L4, L5); limiter fix
(T5); ghost-slot reconcile (T6); abuse limits (T7); close hygiene (T9).

**Phase 4, polish.** Rejoin affordances, toasts and reasons, fullscreen retention, history, music and
video collisions, names with ids, focus.

**How we will know it works (so these do not come back):**
- A deterministic **party simulator** (fake clocks and fake players, in the Rust crate and in TypeScript)
  that replays recorded scenarios under delay, jitter, buffering and seek storms, and asserts that all
  devices converge and nobody seeks more than once per correction.
- A scripted **two-browser suite** (like the probes used here) in CI for: late join, release switch,
  panel pause and resume, party end, host refresh, guest leave, host next episode, music and film.
- **Rust peer against web peer** interop on every protocol change, plus a shared JSON conformance file.
- A **chaos pass** before each release: throttle the network, kill -9 the host, sleep and wake.
