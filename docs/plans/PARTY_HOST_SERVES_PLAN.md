# Party host serves its saved film: plan

Status: plan only, nothing built. Written 2026-10-18 from a read of the party code and web research
(sources at the end). The decisions needed are in section 11.

## 1. The idea

A party host who has a film or episode saved in the library serves it to the guests straight from their
laptop, so guests do not each pull it from the torrent swarm. Guests play it with seeking, in step, with no
dependence on the swarm having peers.

Scope of v0: **desktop host, desktop guests.** Web guests keep playing their own source (the same release,
from our server), as they do today, and are unaffected.

## 2. What it buys, and what it costs

Buys: a rare release plays as smoothly as a popular one; everyone reads the very same bytes (identical
timeline); nothing goes through our server.

Costs: the host's **upload** is the limit. A typical 1080p H.264 film is 6 to 10 Mbps per guest (HEVC 3 to
6, 720p 2 to 4). A host with 20 Mbps up feeds two or three guests. Fixed home uplinks are often 5 to 20 Mbps;
the global median is about 50 Mbps (Ookla, Jan 2025). So the design must cap guests, meter the upload, and
fall back to the swarm for anyone who cannot be fed.

## 3. How the party connection works today (facts from the code)

- One WebRTC data channel, label `party`, created by the **guest** (offerer); the host answers. Ordered,
  reliable, default settings. webrtc-rs 0.21; ICE servers from `/v1/party/{code}/ice` (STUN, and Cloudflare
  TURN as relay).
- Everything on it is JSON text (`Message`, tagged `t`). Binary is mangled and ignored. A message that does
  not parse is dropped, so unknown new `t` values and new fields are safe for older desktop apps.
- The host reads only the **first** channel per guest. A web host treats **every** new channel as a new guest
  and would corrupt its slot: a second channel must never be created towards a web peer.
- Nothing limits what the host queues (no send-buffer limit; `buffered_amount` is never read), and the host's
  `send_all` awaits guest by guest, so a stalled bulk send would delay `State` heartbeats.
- Inbound rate limit per guest: burst 40, 20 a second, flood 200. Host to guest is not limited.
- Liveness: guest calls the host silent after 8 s and lost after 32 s; host drops a silent guest after 15 s.
  A busy but healthy link must count as alive.
- On the host, `Sealed::read_at(offset, buf)` reads any byte range of the saved film (decrypting 64 KiB
  chunks, with a 2 MiB chunk cache), and is `Send + Sync`. The disk and the network will be the limits, not
  the decrypting.
- On the guest, `saved.rs` shows how mpv reads a custom source: `mpv_stream_cb_add_ro` with blocking `read`,
  `seek`, `size`. A second scheme registers the same way, per mpv handle (`player.rs`).
- Where a new source plugs in: `WatchPage::begin`, between `saved_copy` and the streamer. A guest already
  learns the host's release from `Item::Video{id, release}`.

## 4. Choices of transport

| Option | Verdict |
| --- | --- |
| **The existing WebRTC data channel, with extra channels** (webrtc-rs) | **v0.** Nothing new to ship; NAT traversal and TURN already work. Loopback benchmarks are 184 to 575 Mbps (webrtc.rs, July 2026); over the internet SCTP is bound by round-trip time (a Pion report: 850 Mbps at 2 ms, 45 Mbps at 100 ms), so plan 20 to 60 Mbps per channel and use 2 to 4 parallel channels. Unverified for 0.21 on a real network: first spike. |
| **iroh 1.3 / iroh-blobs** (QUIC, hole punching, relays, BLAKE3 verified ranges) | **v1 candidate.** Better loss recovery and verified ranges, but a new dependency, and iroh-blobs says it is not yet production quality. Revisit if SCTP disappoints. |
| BitTorrent from the host (guests use the host as `initial_peer`) | No. Needs an inbound port; our files are sealed (no plain file to seed); piece layout suits swarms, not seeks. |
| libp2p, raw quinn, tunnels (Cloudflare Tunnel, ngrok, Tailscale funnel) | No. Heavier or worse than iroh; tunnels carrying video are restricted by Cloudflare's terms and cost. |
| UPnP plus a direct HTTP range server | Optional later fast path for hosts that can open a port. CGNAT (common on mobile and some Indian ISPs) defeats it. |

## 5. The design (v0)

### 5.1 Who may serve whom

- Only a **desktop host** playing a **saved copy**, and only to **desktop guests** that say they can receive
  it. Everyone else is unchanged.
- **Opt-in.** The party panel gets a switch for a host with a saved copy playing: "Share this film from my
  computer", off until the host turns it on, with one line: it uses their upload, and only people in the party
  can get it.
- **Direct connections only by default.** If ICE chose a relay candidate for a guest, the host does not offer
  the file to that guest (it would cost relay bandwidth: Cloudflare TURN is $0.05/GB after 1,000 GB a month,
  and may drop traffic above 50 to 100 Mbps), and that guest plays the swarm source as today.

### 5.2 Capability handshake (additive, older apps untouched)

1. A guest's `Hello` gains an optional `caps: ["file"]`. Older hosts ignore the field.
2. When the host is playing a saved copy and sharing is on, it sends the guest a new JSON message on the
   `party` channel: `Serve { release, size, block, blocks }` (block size 1 MiB, number of blocks, the media's
   byte length). Older guests ignore the unknown message.
3. A guest that can receive replies `Want`. The host then creates the file channels (`file-0` to `file-N`,
   N = 2 by default, up to 4) towards **that guest only**, after confirming the guest is a desktop app (it sent
   `caps`). A guest that did not say `file` never sees a second channel, and a web host never gets one.
4. The hash table follows as the first binary message on `file-0`: one BLAKE3 hash per 1 MiB block
   (32 bytes each; about 100 KB for a 3 GB film). The guest keeps it in memory and verifies every block
   before giving bytes to mpv.

### 5.3 The file channels: a small binary protocol

Ordered, reliable channels. Messages of at most 16 KiB.

- Guest to host: `Get { id, offset, len }` and `Cancel { id }`, as binary messages on a file channel (so the
  host's inbound token bucket, which counts JSON on the party channel, is not tripped by a burst of requests).
- Host to guest: `Chunk { id, offset, last, payload }` in 16 KiB pieces. Requests are answered in order per
  channel; the guest spreads requests across channels.
- Pipelining: the guest keeps 8 to 16 MiB of requests in flight so the round trip is hidden.
- Back-pressure: the host sets a send-buffer limit (about 1 MiB), sleeps while `buffered_amount` is above it
  and wakes on `buffered_amount_low`. A fast disk never buffers the whole film in memory.
- Fairness: one task per guest (never inside the loop that sends `State`), with a bytes-based round robin and
  priority to a guest whose buffer is nearly empty. A small LRU of decrypted blocks so overlapping readers
  cost one decrypt.
- Liveness: bytes received on a file channel count as the host being alive.

### 5.4 The host's side

- A `FileSource` trait in the party crate (`len()`, `read_at()`), implemented in the app over `Sealed`, so the
  party crate does not depend on the library crate.
- The BLAKE3 table is computed once per item when sharing starts (reading the film: tens of seconds for a
  few GB), in the background with a "Preparing to share…" note, and cached. Better: computed at seal time and
  stored with the item, so it is free at party time. (v0: compute on first use and keep in the library DB.)
- Guest cap: at most 4 guests are served at once; each is offered only if the host's measured outgoing rate
  leaves room (the serving itself measures it: if a guest's reads stall, it is told "watching from the swarm").
  A live meter in the party panel: who is being served, at what rate, with a Stop button.

### 5.5 The guest's side

- A second mpv protocol, `peer://`, registered next to `akc://` in `player.rs`, with `cancel_fn` set so a
  stalled host cannot hang the window. `read` blocks on a bounded channel fed by the async side, `seek` and
  `size` come from `Serve`. mpv's seek-to-0 probe and its reads at the end of the file (MP4 `moov`, MKV cues)
  are served first.
- `begin()` tries, in order: a saved copy of the very release; **the host's file** (when `Serve` has arrived and
  the channel opens); the swarm for that release. If the channel fails mid-film, the guest keeps its position
  and continues from the swarm, with a note.
- mpv settings for a peer source: read-ahead of 20 seconds or more, a few hundred MB of demuxer cache, and
  play starts only after about 10 seconds are buffered. Readiness is `Loading` until then, so the host's
  existing hold waits for it.
- Sync is unchanged: the host's position is authoritative; a guest whose buffer runs dry reports `Loading`
  and the host's panel shows it ("X is buffering"), as it does today.
- No copy is kept: bytes are verified, handed to mpv, and gone with its demuxer cache. "Keep a copy" is a
  later, explicit opt-in (it is a bigger legal step).

## 6. Edge cases and failure handling

Host: sleeps or loses power (guests fall back to the swarm if the host returns; the party ends if it does
not, as today); upload collapses (guests buffer-pause, the host sees the meter, can stop sharing); the host
pauses sharing mid-film (guests switch to the swarm from their position); the host quits the Library item
(removal while sharing: the reader keeps the open file until the party ends, as for playback); a different
release is picked (sharing starts afresh for the new item); two parties at once (one file server per party).

Guest: joins late (starts at the host's position; the first reads are the start and end of the file); is on a
relay (refused, uses the swarm); has the release saved (plays its own copy, no request); has a bad block hash
(refetch up to 3 times, then fall back to the swarm); seeks far (outstanding requests cancelled, new ones sent);
leaves (its requests dropped, its slot freed); is a web guest (never offered; plays its own source).

Network: SCTP over a long round trip is slow (parallel channels); hole punching fails (relay, so refused);
the host's NAT closes (channel closes, guest falls back).

Older apps: ignore `caps`, `Serve` and `Want`; a web host is never sent a second channel; a web guest never
receives one.

## 7. Legal and cost

A host sending a copyrighted file to other people is redistribution, as torrent seeding is: it concentrates
the act on one identifiable person, the host. That is why sharing is **opt-in, per party, and off by default**,
with a plain line in the panel, a note in the privacy page, and no use of our servers or relays for the
bytes. TURN is not used for it (refused), Cloudflare Tunnel is avoided (its public-hostname tunnels fall under
the CDN video restriction). Get legal review before shipping.

## 8. Phases and effort (one person)

0. **Spikes (3 to 4 days).** (a) Measure webrtc-rs 0.21 throughput host to guest on two real machines and with
   `netem` latency, with 1, 2 and 4 channels; (b) the blocking `read` bridge into mpv with a fake slow peer
   and `cancel_fn`; (c) BLAKE3 over a 3 GB container; (d) creating a channel after the connection is up, and
   that an older desktop guest ignores it. Go/no-go on SCTP versus iroh.
1. **Protocol and transfer in the party crate (5 to 6 days).** Capability fields, `Serve`/`Want`, the file
   channels, the binary framing, back-pressure, verification, a loopback test through the real router (the
   e2e rig already exists), a test that an old guest and a web host are unharmed.
2. **Host serving (3 to 4 days).** `FileSource` over `Sealed`, hash table, per-guest tasks, fairness, cap, meter,
   relay refusal.
3. **Guest playing (4 to 5 days).** The `peer://` protocol, `begin()` order, fallback to the swarm, buffering
   and readiness, mpv tuning.
4. **Party panel and states (3 days).** The share switch, "Sharing with N" meter, per-guest status, "Watching
   from <host>" on the guest, errors in plain words.
5. **Hardening and docs (3 days).** Failure drills (sleep, kill, drop packets), the 2 to 5 GB film on two
   machines, privacy page, README, `PROTOCOL.md`.

About 3 to 4 weeks. The risk is concentrated in spike (a).

## 9. Testing

Loopback party with a seeder-style host and two guests: all guests read the same bytes; seeks; cancel; a
corrupted block; a guest that disconnects mid-read; the cap; the relay refusal. Real: two laptops on
different networks, a 2 GB film, host on a 20 Mbps uplink, one guest then three; pull the host's network
mid-film; host sleeps. Compatibility: the previous release as host and as guest, and the website as host and
as guest.

## 10. v1 and later

Trial iroh 1.x as the bulk transport (QUIC, better loss recovery, verified ranges); an optional direct HTTP
fast path for hosts that can open a port; guests relaying chunks to each other in a small mesh; a host that
keeps downloading in the background so its uplink is only busy at the start; host-side transcoding to
fragmented MP4 so web guests can join (heavy on the host's CPU: only if there is demand).

## 11. Decisions needed

1. **Opt-in per party, off by default** (recommended), or on whenever the host has the copy?
2. **Guest cap:** 4 at once (recommended), with the meter.
3. **Relay:** refuse the file for guests on a relay (recommended) or allow it (costs relay bandwidth).
4. **Do guests keep a copy?** No in v0 (recommended).
5. **Web guests:** unchanged in v0 (recommended).
6. **Legal review** before shipping: yes (recommended).

## Sources

Web research: webrtc.rs "From 13 Mbps to Beating Pion" (2026-07-18), Pion sctp issue 218, Cloudflare Realtime
TURN FAQ and pricing, Cloudflare "delivering videos" policy, iroh 1.3.0 and iroh-blobs 0.103.1 on docs.rs,
ProbeLab and arXiv 2604.12484 (libp2p hole punching), Tailscale NAT-traversal posts, Ookla global index, the
libmpv `stream_cb.h` header, Syncplay and Jellyfin SyncPlay docs. Code read: `desktop/party/src/{peer,host,
guest,signal,sync,protocol}.rs`, `desktop/library/src/container.rs`, `desktop/app/src/{saved,player,watch,
party}.rs`, `desktop/server/src/{party,api}.rs`, `akidev-frontend/src/lib/party/*`.
