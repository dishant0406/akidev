# Watch party protocol

Devices that play the same thing together: films, series, anime and music. One device is the
**host**; the others are **guests**. The host can be the desktop app or a browser tab, and so can
a guest, in any mix.

## Who talks to whom

- A guest and the host connect with **WebRTC**, over one data channel labelled `party`. Every
  guest has its own connection to the host; guests do not talk to each other.
- `api.akidev.live` is only the **meeting point** (`desktop/server/API.md`, "Watch party"). The
  host opens a party and gets a six-letter code; a guest posts an offer for that code; the host
  takes it and posts an answer. After that the server is not involved, and it never sees what is
  played. Offers and answers are complete: the device waits until it has found its addresses (at
  most ten seconds) and sends them all at once.
- Nothing is streamed between devices. A message names what to play, and every device finds its
  own stream from that name.

## Messages

One JSON object per data-channel message, as text. `t` says which. The Rust list is
`party/src/protocol.rs`, the web one `akidev-frontend/src/lib/party/protocol.ts`.

| `t` | from | fields | meaning |
| --- | --- | --- | --- |
| `hello` | guest | `name`, `id` | First message: who is joining. `id` is optional: a random id for this run of the app or page, the same each time it joins, so a host that sees it again drops the old connection and the guest is not there twice. |
| `state` | host | `item`, `playing`, `position`, `at`, `choosing` | What is playing. `position` is seconds, read at `at` (the host's clock, milliseconds since 1970). Sent when it changes, and every 2 seconds, paused too, so a guest can tell a host that is still there from one that is gone. A guest who joins is sent the last one at once. |
| `members` | host | `members: [{ name, host, state }]` | Who is in the party, and how each guest is doing with what is playing: `state` is `ready`, `loading` or `trouble`. |
| `play`, `pause` | guest | | Asks the host to play or pause. |
| `seek` | guest | `position` | Asks the host to seek. |
| `status` | guest | `state`, `id` | How this device is doing with the item whose key is `id` (the media id, and for a video `#` and its release): `ready` (can play it now), `loading` (still finding a source or buffering) or `trouble` (cannot find or play it). |
| `ping` | guest | `id`, `sent` | Measures the way and the clock difference. |
| `pong` | host | `id`, `sent`, `at` | The answer: `sent` as it came, `at` the host's clock. |
| `bye` | either | | Leaving. From the host it ends the party. |

`item` is `null` (nothing), `{ "kind": "video", "id": "series-tt0903747-s1e5", "release": "<info hash>", "candidates": ["<info hash>", ...] }` (a media id, as
in `desktop/server/API.md`, the release the host plays, and, until it has one, the releases it is trying, best first; `release` and `candidates` are optional,
and the website ignores `candidates`), or `{ "kind": "music", "id": "<YouTube video id>", "title", "artist",
"cover" }` (the last three are only what a guest shows before its own copy has loaded).

Everyone can ask to play, pause or seek; only the host acts, and only the host chooses what plays. A guest that presses pause sends `pause`; the host pauses
and sends `state`, and every device, the asker included, follows it. That gives one order of
events, so two people pressing things at once cannot leave devices disagreeing.

## Starting together

A host says what it is opening the moment it opens it: a `state` with the video's `id`, no `release`,
`playing: false` and `choosing: "finding"`, repeated every 2 seconds while it searches. A guest goes to
that film at once and searches in parallel. When the host's search has found releases it adds
`candidates` (up to five, best first). A guest that has the list starts joining the swarm of the first
while the host is still opening its own, and when the host names the `release` it plays, the guest
switches to it if it is another one. A guest never opens a release the host has neither named nor
listed. The host holds the start for the guests as before (at most 45 seconds), but their searching and
joining no longer begins after the host's.

## The same source

A video item carries the release the host is playing (its info hash). A guest asks for exactly that
release (`release` in `POST /v1/play`, or `Plan::only` in the desktop app), so everyone watches the
same source, always. A guest never picks another release itself: if it cannot get the host's, its
`status` is `trouble` and the host sees "Can't play". **The host then moves everyone on**: when a guest
is in `trouble` for the release it plays, early in the film (the first minute), and the host did not pick
that release by hand, the host drops it, finds the next best (the same list, without the ones that
failed) and publishes it, at most three times for one film. Guests that were in trouble follow the new
release like any other. When the host picks another release by hand, the item changes (same `id`, new
`release`); guests do not change page, they switch release and seek to where the host is. The guest's
`status` names the item by key, `id` plus `#` plus `release`, so a guest that was ready for the old
release is loading the new one until it says so, and the host holds the start as it does for a new
film. A device from before releases were shared says the plain id, which counts as ready for any
release of that film. Only the host chooses the source, so a guest's window has no source button.

## Waiting while the host chooses

While the host has a panel open (sources, episodes or subtitles), its picture is paused and `state`
carries `choosing: "source" | "episode" | "subtitles"` with `playing: false`. A host that has just
picked another release says `choosing: "finding"` until its own picture has loaded. A device ignores a
word it does not know, so more can be added. A guest pauses and
shows "Choosing a source" (or an episode, or subtitles) over the picture. When the host closes the
panel it plays again if it was playing, and `choosing` is gone from the next `state`. An old device that
does not know the field still pauses, because `playing` is false.

## Keeping in step

A guest estimates the host's clock from `ping`/`pong`: the offset from the quickest round trip
seen, as `at + round_trip / 2 - now`. It acts on a `state` only once it has a reading, so a `state`
that comes first is kept until the first `pong`. From a `state` it works out where the host is now:
`position + (host_now - at)` when playing (moved on by at most 10 seconds, so a clock that is far
out cannot send a guest far away), `position` when paused. Then, against its own player:

| gap (own - host) | when playing | when paused |
| --- | --- | --- |
| under 0.15 s | nothing | nothing |
| 0.15 s to 0.8 s | run at 0.95 (ahead) or 1.05 (behind) | seek |
| over 0.8 s | seek | seek |

Both the Rust (`party/src/sync.rs`) and the web (`lib/party/sync.ts`) versions have tests for the
same cases.

A host that leaves what it was playing (closes the film, or the music stops being the party's)
publishes a `state` with `item: null` and `playing: false`; guests pause. A guest who joins late is
sent the last `state` moved on to now.

## Guests that are still loading

Finding a film can take a guest a while (a torrent has to be found and joined), and the host's
device may be quicker. A guest says how it is doing with an item in `status`, naming the item by
`id`. A guest that has said nothing about the item the host is playing now, or only about an
earlier one, is counted as loading it, so a new film shows every guest as loading until each
says `ready`. The host's list of members shows who is.

- **At the start of a film** a host with company waits: it stays paused until no guest is loading,
  at least one second (so the guests have time to say they are) and at most 45 seconds (so a guest
  that cannot load does not hold everyone). Then it plays, and the guests follow.
- **In the middle of a film** nothing waits. A guest that finishes loading, or stalls, climbs back to
  where the host is on its own (seeking, at most once every two seconds), and the host is shown a
  note that the guest is buffering, so it can pause for them if it likes.
- **Music** does not wait: a track loads in seconds, and a guest that is late seeks to the host.

## When things go wrong

- A guest that cannot find a stream for the item sends `status` with `state: "trouble"`; the host's
  list of members marks it. Everyone else carries on, and the host does not wait for it.
- A connection that fails or closes removes the guest, and the host frees its place on the server.
- A guest that loses the host without a `bye` (a network change, the host's page reloaded) keeps
  trying to join again with the same code and `id` for two minutes, every three seconds, saying so
  on screen; it tells the host again how it is doing once back in. A `404` ends that, as the party
  is gone.
- A guest that has heard nothing from the host for eight seconds says so on screen; a host that has
  heard nothing from a guest for fifteen drops it.
- A host that is a web page keeps its code and token for the tab, so a reload opens the same party
  again (`PUT /v1/party/{code}`) and its guests find their way back. A tab closed for good lets the
  server forget the party after two minutes. The desktop app's party ends when the app quits (it
  says goodbye first).
- A guest may send about twenty messages a second (a burst of forty): the excess is dropped, and a
  guest that goes on is dropped.
- A browser may refuse to play without a tap: the guest is asked for one.
- A device from before any of this keeps working: new fields are only added, and a word it does not
  know in `choosing` is ignored. Which versions may run at all is the published sources' to say (see
  `desktop/sources/README.md`, "Forcing an update").
- Devices on networks that block direct connections need the TURN relay (Cloudflare's, set up
  with `CF_TURN_KEY_ID` and `CF_TURN_TOKEN` on the server). Without it they cannot join; the server
  says so in its log when it starts. A desktop host behind such a network is told to host from the
  website, whose connection is the one the relay serves.
