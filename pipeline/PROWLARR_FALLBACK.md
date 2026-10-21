# The Prowlarr fallback search

The indexers in `sources.json` read JSON and RSS. Many torrent sites offer only web pages, so a title
that none of those indexers has could not be found. When the normal search finds nothing worth opening,
the apps ask one more indexer, `GET /v1/fallback/search` on the API, which searches through a
**Prowlarr** running next to it. Prowlarr knows hundreds of sites and keeps their definitions up to
date by itself, so none of them is written or translated here.

```
 web app ──► api ──┐ 1. the indexers in sources.json (1.5-3 s)
 desktop app ──────┤
                   └ 2. only if that found nothing worth opening:
                      GET api.akidev.live/v1/fallback/search?q=...   (cached, 2 searches at a time)
                              └─► Prowlarr (internal network only) ─► LimeTorrents, TorrentKitty, DMHY, Mikan
```

## What is where

| | |
|---|---|
| `desktop/server/src/fallback.rs` | Asks Prowlarr, returns `{title, hash, size, seeders, source}`, finds a hash for the results that have none, caches. Each search is a task of its own: it finishes and is kept if the asker has gone, and the same search asked twice at once runs once. |
| `desktop/server/src/api.rs` | The route, its allowance, and the `fallback` block of `/health`. |
| `desktop/core/src/indexers/mod.rs`, `definition.rs`, `config.rs` | `fallback: true` on an indexer: asked after the others, only if `enough` says they found nothing, with `fallback_timeout_ms` (15 s) to answer, and sent with the header `x-akidev-fallback`. |
| `desktop/sources/sources.json` | The `prowlarr` indexer, pointing at `https://api.akidev.live/v1/fallback/search`. |
| `docker-compose.yml` | The `prowlarr` and `byparr` services: pinned images, memory limits, no published ports, profile `fallback`. |
| `scripts/prowlarr-sync.js` | Makes Prowlarr's indexers match its `WANTED` list; run by the deploy. |
| `.github/workflows/deploy.yml` | Passes the secret, and checks the route after a deploy (a warning, never a failure). |

## Turning it on and off

On: set the repository secret `PROWLARR_API_KEY` to any random string (`openssl rand -hex 16`) and
deploy. The deploy starts Prowlarr, waits for it to be healthy, and runs the sync. `GET /health` then
reports `fallback: { configured: true, reachable: true }`.

Off: delete the secret and deploy; the container is removed and the route answers `404`. To stop the
apps asking at all, remove the `prowlarr` entry from `sources.json` and publish.

Prowlarr is not published and Caddy does not route to it. Nothing but the `api` container reaches it.

## Keeping it current

- **Site definitions:** Prowlarr's own task `IndexerDefinitionUpdate` fetches them from
  `Prowlarr/Indexers` daily, into the `prowlarr-config` volume.
- **Which sites are used:** edit `WANTED` in `scripts/prowlarr-sync.js` (a site's `definitionName`, as
  in `Prowlarr/Indexers/definitions/v11`) and deploy. Prowlarr tests a site before adding it, so one that is
  down or behind Cloudflare is reported in the deploy log and left out; the next deploy tries again.
- **Prowlarr itself:** the image tag in `docker-compose.yml` is pinned. Raise it when wanted; the deploy pulls it.

## Things to know

- **Hashes.** DMHY, Mikan and TorrentKitty put the hash in the result. LimeTorrents gives only a link,
  which Prowlarr answers with a redirect to a magnet after fetching the site's page: slow, and Prowlarr asks
  a site one thing at a time. So only the best 4 such results are followed, within the 13 seconds a search
  may take, and a result whose hash is not found in time is left out. Sites that give a `.torrent` file and no
  magnet (ACG.RIP) cannot be used.
- **Time.** A search takes 8-10 seconds against five sites, and the apps wait 15. It is only paid when the
  normal search found nothing. The same search again is served from memory for ten minutes.
- **Cloudflare.** From the server's address 1337x, ExtraTorrent, the Kickass mirrors, TorrentKitty, UIndex,
  TorrentDownload and TorrentProject all answer with "blocked by CloudFlare Protection". The sites marked
  `solver` in `scripts/prowlarr-sync.js` are added with a tag, and Prowlarr sends tagged sites through
  **Byparr** (the `byparr` service, a FlareSolverr-compatible solver that opens the page in a patched
  Firefox). Tried locally, ExtraTorrent went from blocked to added and then returned 50 results with hashes
  for a film in 2-8 seconds. Byparr cannot promise a pass: Cloudflare also judges the address, and a
  datacenter address is judged harder than a home one, so the deploy log says which sites got through.
- **Byparr's cost.** About 250 MB idle and about 1.1 GB while solving (a browser per page), 560 MB of disk for
  the browser (kept in the `byparr-cache` volume, downloaded once) and a 3.4 GB image. It is limited to
  1.2 GB. A solve takes 4-12 seconds; Prowlarr keeps the clearance cookie, so later searches are faster.
- **Memory.** Prowlarr about 210 MB idle, limited to 400, and Byparr as above, on a 4 GB box that already
  builds the site during deploys.
- **Old apps.** An app built before the flag existed treats the `prowlarr` entry as an ordinary indexer
  and asks it on every search. It does not send `x-akidev-fallback`, so the route answers `400` at once and
  nothing reaches Prowlarr. The indexer then rests for a minute in that app. They stop when they update.
- **Usage reporting.** Prowlarr sends usage data unless told not to; the compose file turns that off.
- **The allowance** is `PLAY_LIMIT_PER_10_MIN` × 3 searches per client address, counted only for searches
  that are not already kept (a kept answer costs nothing). The web app's own searches all come from the
  server's address, so they share one allowance; that is about the most Prowlarr can do anyway (two
  searches at a time, 8-10 seconds each). Past it the route answers `429`.
