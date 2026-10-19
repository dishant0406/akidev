# Faster torrent search and start, desktop first

Status: research and measurements, no code changed for this plan. The server shares the same
`akidev-core`, so everything in "Search" helps it too.

## Where the time goes today

Measured on this machine, real network, cold process (`Streamer::plan`, which is titles, indexers and
ranking, before any torrent is opened):

| Title | Search time | Terms | Releases found |
|---|---|---|---|
| Anime, AniList 16498 ep 1 | **19.4s** | 12 | 24 + 24 |
| Film, Interstellar | **7.3s** | 2 | 7 + 9 |
| Series, TVMaze 169 S1E5 | 3.1s | 5 | 3 |
| Anime, AniList 21 ep 1000 | 2.5s | 2 | 20 + 9 |
| Series, TVMaze 82 S1E1 | 1.6s | 5 | 3 + 1 |

Then opening a torrent. On a healthy public swarm (Big Buck Bunny, 8 runs) the file list plus the first
3 MiB took **5.6 to 12s**, before the player has a URL. So a cold anime play is roughly 25 to 30s of
waiting before the first frame, and the search is the larger part.

One request per indexer, from `curl`: SubsPlease 0.5s, Nyaa 0.6s, Torrents-CSV 0.7s, EZTV 0.9s, AnimeTosho
1.2s, ApiBay 1.3s, **TokyoTosho 4.0s**, **YTS mirror `movies-api.accel.li` no answer in 15s**. Torrentio,
Knaben and Nyaa answered 503 to `curl` (probably a bot filter on curl's user agent; the app sends its own,
so check with the app's client before relying on this).

## Why the search is slow (from the code)

`search_all` (`core/src/indexers/mod.rs:278`):

1. It asks every indexer for every search term. The anime above is 12 terms (3 title variants times 4
   episode forms) times about 6 indexers, so around 70 requests.
2. They go in batches of 24 and each batch waits for its slowest request before the next starts
   (`pairs.chunks(MAX_CONCURRENT_REQUESTS)`). Three batches, each held up by a 4s TokyoTosho or a 5s
   timeout, is 15 to 19s.
3. Nothing is used until all of it is done. The ranking, and the choice of what to open, wait for the
   last request even when the first 300ms already returned a perfectly good 1080p release.
4. A dead mirror costs a full 5s (`indexer_timeout_ms`) on first contact, and each host of an indexer is
   tried one after another, so an indexer with 3 mirrors can cost 15s. The cooldown stops repeats within
   a run, but every launch starts with it empty.
5. The film case is almost entirely the dead YTS mirror: 5s waiting for it, then the second host.
6. The search is repeated for every play. The cache is in memory only, so closing the app loses it.

## How Stremio is fast (what I could confirm, and what I could not)

Confirmed:

- Stremio does no searching when you press play. Search is an addon's job, and an addon answers a stream
  request with a ready list: for a torrent each entry carries `infoHash`, an optional `fileIdx` (the file
  inside the torrent; if omitted the largest is used), `announce` (tracker URLs) and `fileMustInclude`.
  So the app never parses release names to pick a file, never fans out over 12 spellings, and knows the
  trackers up front.
- Torrentio's addon answers by looking up an IMDb id, in one request, from a store of already-scraped
  torrents. Third-party projects that copy it describe it as an addon that queries a PostgreSQL database
  filled by a separate scraper service. I could not read Torrentio's own code or README, so this is from
  its clones, not from the source.
- Stremio's own streaming server runs a torrent engine ("enginefs" over torrent-stream). Its example
  config searches for peers with a floor and a ceiling (`min: 40`, `max: 150`) with DHT as a source, and
  closes an idle engine after 2 minutes. I could not find what its startup does beyond that, or whether
  it does anything special to speed up metadata.
- Without a debrid service, Torrentio users also report slow starts on small swarms, so Stremio's
  speed is mostly the instant lookup, not a faster torrent engine.

Not confirmed: Stremio's real peer limits, how it fetches metadata, any trick beyond the above. Do not
build on guesses about these.

The lesson: Stremio turned "search the web at click time" into "look up one id in an index". We
cannot index the internet, but we can make the lookup one or two requests, use what answers first, and
not wait for the rest.

## Plan, in order

### 1. Measure each stage, always (small, do first)

Log, per play: titles fetched, each indexer's time and row count, rank, first usable result, torrent
file list, first bytes. The torrent side already logs after the last change; the search side does not.
Without this the rest is guessed. Done when one play prints one line per stage with milliseconds.

### 2. Stop waiting for the slowest request (largest win, medium)

- Replace the batches with a stream: run all requests at once under a semaphore, and collect results as
  they arrive.
- Start opening torrents as soon as there is a good candidate. Rule: when the first indexer returns
  releases that pass the ranker, wait a short window (about 700ms) for others, then rank what exists and
  start the race. Keep collecting in the background and add later arrivals to the "other releases"
  list the viewer can pick from.
- A hard ceiling of about 4s for the search as a whole, not 5s per request.

Expected: search time falls from the slowest indexer to roughly the median one, so about 1 to 2s
instead of 3 to 19s. That is the biggest single change.

### 3. Send far fewer requests (medium)

- Not every indexer needs every spelling. Torrentio and EZTV take an id and need one request. Knaben
  needs every word to match and is helped by the season term. Give each indexer definition a list of the
  forms it is worth asking (`terms: ["episode", "season"]`), in `sources.json`, instead of the full
  cross product.
- Order terms by yield: the code's own comment shows "S01E05" forms find the most. Ask the best two forms
  first, and the rest only if the first round found nothing playable. Today the 12 are asked at once.
- Cap title variants at 2 for the first round.

Expected: about 70 requests become about 15 for the first round.

### 4. Make dead hosts cheap (small)

- Hedge: when the first mirror has not answered in about 1s, ask the second at the same time and take
  whichever answers; do not wait for the 5s failure.
- Per-request timeout 2.5s for indexers that normally answer in under a second; keep 5s only as the
  overall ceiling.
- Remember each host's health on disk (cooldown and median latency) so a mirror that was dead last time
  is not the first thing asked this time. Remove or demote YTS `movies-api.accel.li` now: it did not
  answer in 15s.

### 5. Remember and prefetch (medium)

- Write the release list for a title and episode to disk with a TTL (about 20 minutes for a seeders
  list that goes stale, longer for the title and ids).
- When the desktop app shows a title page or the next episode, search it in the background so the play
  button starts from a warm list. Binge watching then pays the search once.
- Prefetch the next episode's top torrent to metadata plus the first pieces when the current episode is
  past about 70%.

### 6. Use an index where one exists (medium to large, needs a decision)

- Make the id-based sources the first round and the text-search indexers the second: Torrentio, EZTV
  (by IMDb id), and for anime a Stremio-style addon that takes a Kitsu or AniList id. Each answers in one
  request and brings `infoHash`, `fileIdx` and trackers, which lets us skip name parsing and pick the
  file directly.
- Anime needs an id map (AniList to Kitsu or IMDb). A public mapping dataset exists; verify its licence
  and freshness before adopting it.
- Public addon instances can be blocked or slow, so this stays a speed-up in front of the current
  indexers, never a replacement.
- Bigger and later: keep our own small index (`info_hash`, title, episode, size, seeders, updated)
  filled by a background scrape of the indexers we already use. It is the real equivalent of what
  Stremio relies on, but it is a service to run, so it only makes sense if steps 2 to 5 are not enough.

### 7. Torrent start, after the search (5 to 12s on a healthy swarm)

- Persist the DHT routing table in the config directory (`persistence: None` today): the first play after
  every launch has to find DHT nodes from scratch.
- Metadata is the slow part of opening. Use the trackers an addon supplies, and a longer tracker list, so
  peers are found in the first second. Try HTTP caches of `.torrent` files by hash in parallel with the
  swarm: if one has it, the file list arrives at once. I have not tested that this works for our hashes;
  try it before promising anything.
- Shorten `STAGGER` (6s today) to about 3s so the second and third candidates start sooner, since the
  losers are dropped anyway.
- Desktop only: a listen port with UPnP, which an earlier note suggested; it matters most for small
  swarms and does little for large ones.

## Order and size

| Step | Effort | Expected effect |
|---|---|---|
| 1 Measure | 0.5 day | none alone, needed to prove the rest |
| 2 Stream results, ceiling | 1 to 2 days | search 3 to 19s down to about 1 to 2s |
| 3 Fewer requests | 1 day | less load, fewer slow tails |
| 4 Cheap dead hosts | 0.5 day | removes the 5 to 15s tails |
| 5 Disk cache and prefetch | 1 to 2 days | repeat plays and next episode near instant |
| 6 Id-based sources first | 2 to 3 days | one-request lookup like Stremio |
| 7 Torrent start | 1 to 2 days | up to several seconds of the 5 to 12 |

Do 1, then 2 and 4 together: they fix what the measurements show without a new source or service.

## Caveats

- One machine, one network, one run per title; these are cold-process times and the real figure varies
  with the indexers' mood. The shape (a long tail from a few slow indexers, and batching that waits for
  it) is the finding, not the exact seconds.
- Sources for Stremio's design are its public repositories' README pages and third-party docs; I could
  not read the Torrentio source.

## Sources

- Stremio addon protocol, stream objects (`infoHash`, `fileIdx`, `announce`): <https://www.mintlify.com/Stremio/stremio-core/addons/resources>
- Stremio enginefs (peer search, timeouts): <https://github.com/Stremio/enginefs>
- Torrentio scraper: <https://github.com/TheBeastLT/torrentio-scraper>
- A Torrentio clone that queries a scraped database: <https://github.com/Treamz/torrentio-addon>
- Stremio peer discovery issue (stats fields): <https://github.com/Stremio/stremio-bugs/issues/2657>
- libtorrent streaming: <https://libtorrent.org/streaming.html>
