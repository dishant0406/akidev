# Making the torrent source fast

Status: research only. Nothing here has been changed or measured on the live server yet.

## What the code does today

A play request goes `plan` (indexers, rank) -> sources in order `debrid, torrent, archive-org`
(`STREAM_SOURCES`) -> `Streamer::open_best` races up to 3 releases, the 2nd starting 6s after the 1st
(`STAGGER`) -> `Engine::open` adds the magnet with no file selected, waits for metadata (up to 45s),
chooses the file, waits up to 15s for the first 3 MiB (`HEAD_WAIT`) -> `media::probe` -> direct, HLS copy or
re-encode -> token. The whole resolve has a 75s deadline.

## Findings, most likely cause first

### 1. The server never accepts a peer (confirmed in code)

`Engine::start` (`desktop/core/src/torrent.rs:137`) builds `SessionOptions` with `..Default::default()`, and
the librqbit default is `listen: None` (`vendor/librqbit/src/session.rs:493`), which means no TCP listener
and no uTP. On top of that `docker-compose.yml` publishes only 80 and 443 on the host, and the api
container publishes nothing. So the engine is outbound-only: it can reach peers that are connectable, and
no peer can reach it. Two outbound-only peers cannot connect to each other at all, so every swarm shrinks to
its connectable members. For a small or old swarm (the usual case for a specific episode or a 1080p film)
that is the difference between 20 usable peers and 3. Large swarms hide it.

Fix: give the session a listener on a fixed port (TCP, and uTP once enabled), publish that port on the host
(`6881:6881/tcp` and `/udp` or another fixed one), open it in the Lightsail firewall, and set
`announce_port` to it so trackers and the DHT advertise the right port. The DHT already runs on UDP; its
port needs the same treatment so the node is reachable and counts as a good node.

### 2. Upload is capped at 256 KiB/s (confirmed in code)

`UPLOAD_LIMIT_BPS` defaults to `256 * 1024` (`desktop/server/src/config.rs:118`). BitTorrent unchokes the
peers that upload to you fastest, plus one random optimistic slot. A client that gives back almost nothing
is unchoked mostly by that optimistic slot, so each peer sends less. The cap was set to save bandwidth,
reasonably, but it costs download speed, and the evidence I found is anecdotes and the protocol design, not
a benchmark of rqbit. Note: with finding 1 unfixed nobody can connect to upload to anyway, so the two
interact; fix 1 first, then raise or remove this and measure.

Raising it costs transfer only for what is actually uploaded; the data cache is deleted on eviction, so
seeding stops with it. Lightsail's monthly transfer allowance is the real limit: check it before choosing
a number.

### 3. Peer discovery is thin

- Only 4 trackers in `sources.json` (`desktop/sources/sources.json:4`), all UDP. Adding a longer list of
  currently-live public trackers (the ngosang `trackers_best` list is the usual source) gives more peers in
  the first seconds, before the DHT has answered. Indexer magnets bring their own trackers too; check that
  the engine merges both.
- `persistence: None` for the DHT: after every restart or deploy the routing table is empty and has to be
  rebuilt, so the first plays after a deploy are the slowest. Persist the DHT table in `/data`, or warm
  the DHT at boot by looking up a popular hash.
- rqbit's peer connect timeout is short by default (its docs say 2s and suggest raising it for slow
  peers). On a distant or busy peer a 2s TCP connect often fails. Set `connect` options with a longer
  connect timeout (5-10s); it is cheap because connection attempts run in parallel.
- `ListenerMode` is `TcpOnly`. Many peers behind NAT are only reachable through uTP, and outbound uTP also
  needs it. rqbit marks uTP as not yet the default; test enabling `TcpAndUtp`.

### 4. Candidate choice uses stale numbers

`candidates()` sorts on the indexer's `seeders`, which is whatever the indexer last scraped, often hours or
days old and sometimes invented (Nyaa/Torrents-CSV/Torrentio all differ). The engine then opens the top 3,
6s apart, and a dead one costs up to 45s of metadata wait before it fails. A live UDP scrape (BEP 15, one
small packet per hash to the tracker) returns real seeder and leecher counts in about a second. Scrape the
top ~10 candidates in parallel, drop zero-seeder ones, rank by live seeders, then open. That makes "best
source" mean best right now.

Related: `needs_encode_by_name` and `heavy` already push the 10-bit/REMUX releases to the back, which is
right for this server's CPU; a live scrape would fit in the same sort key.

### 5. Startup latency inside the engine

- `HEAD_WAIT` is 15s and `fetch_start` waits for 3 MiB at the start plus 3 MiB at the end. Those are right
  for MP4/MKV, but the head wait runs after the metadata wait, so the worst case adds up to 60s before
  anything is probed. Starting the 3 MiB tail read at the same time as the head read (it is a
  background task now, started only after `commit`) saves one round trip when the index is at the end.
- librqbit's streaming lookahead is 32 MB (`torrent_state/streaming.rs:28`) and a stream does not
  use deadline-based piece assignment the way libtorrent does (`set_piece_deadline`, which puts urgent
  pieces on the fastest peers). rqbit just prioritizes pieces in the window. The local patch already fixed
  the idle-peer wake-up. A further upstream-able change would be to request the first few pieces from
  several peers at once (endgame-style duplication) during startup. This is the most invasive item and the
  last to try.

### 6. The cheapest big win is not a torrent change

Most time-to-play for a cold torrent is swarm connection, which no tuning removes. Debrid avoids it:
a cached release starts at once, and `releases()` already reports which hashes TorBox holds. Ideas in
order of effort:

1. Check TorBox cache status for the top ~24 hashes first (it is already one cheap call) and, when any is
   cached, play it before trying the swarm. This is what `debrid` first in `STREAM_SOURCES` does; verify
   production really has `TORBOX_API_KEY` set, or the "best source" is silently the slow one.
2. For uncached picks, ask TorBox to add the magnet in the background while the local swarm is tried, and
   switch to whichever is ready first. Same as the racing already done between releases, extended to
   sources.
3. Prefetch: when a viewer is on episode N, warm episode N+1's torrent (metadata + first pieces). One
   extra session slot, large gain for binge watching.

## Suggested order

1. Measure first. Log, per play: time to metadata, to first 3 MiB, peers connected (live and seen), and
   whether any inbound peer connected. `Engine::stats` already has speed and live peers. PostHog is wired in.
   Without it the rest is guessing.
2. Open a fixed listen port (TCP, then uTP), publish it, set `announce_port` (finding 1).
3. Lift the upload cap to a measured value (finding 2).
4. Longer tracker list, longer connect timeout, persisted DHT (finding 3).
5. Live scrape ranking (finding 4).
6. Debrid prefetch/race and next-episode warm-up (finding 6).
7. Piece-picking changes in the vendored librqbit (finding 5) only if 1-6 are not enough.

## What I could not verify

- Real numbers: I have no logs or measurements from the production box, so the size of each gain is unknown.
- Whether the Lightsail firewall already has a BitTorrent port open (compose publishes none; the firewall
  is outside the repository).
- The web results were mostly forum posts, libtorrent's own docs and academic papers from 2007-2012. They
  support the direction, not the amounts.

## Sources

- libtorrent streaming notes: <https://libtorrent.org/streaming.html>
- Hybrid piece selection for streaming (Columbia): <https://dna-pubs.cs.columbia.edu/uploads/fe85bde5-de2d-4d38-9bf3-569b129a807b.pdf>
- Adaptive window piece selection (Linköping): <https://www.ida.liu.se/~nikca89/papers/ton12.pdf>
- Upload cap and download speed, forum report: <https://forum.transmissionbt.com/viewtopic.php?p=64527>
- Choking and unchoking: <https://tribler.org/FairUnchoking>
- Port forwarding and connectable peers: <https://trash-guides.info/Downloaders/port-forwarding-troubleshooting/>
- rqbit: <https://github.com/ikatson/rqbit>
- Debrid cached versus uncached: <https://store.elfhosted.com/blog/2024/11/22/stremio-after-realdebrid/>
