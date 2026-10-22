# Porting Paperback sources to akidev-manga: catalog and plan

Status: research done, nothing ported yet. Written 2026-10-22.
Inputs: a full clone-and-read of the live Paperback extension ecosystem, the akidev-manga engine
(`desktop/manga/`), and live probing of every candidate site from this machine.

## 1. The answer

**The port is cheap where a site runs a known CMS, and there is one CMS family that is worth more than
everything else: Madara.** `manga-sources/themes/madara.json` already exists and is already proven. A
Madara site is not a port, it is an 8-line file:

```json
{
  "schema": 1, "id": "manhuaplus", "name": "ManhuaPlus", "version": 1,
  "lang": ["en"], "nsfw": false, "extends": "madara",
  "base_url": "https://manhuaplus.com"
}
```

That single fact decides the plan: **do the Madara family first**, in bulk, because each site costs
minutes and no new engine work. Then add a second theme for the next-biggest CMS family (Guya, FMReader,
MangaBox, Genkan), then treat the bespoke sites one at a time.

**Where the engine is the real limit, not the ecosystem:** 25 of the 0.9-era extensions need something
`desktop/manga` cannot express today — `crypto` (AES/base64 image blobs), `tiles` (tile descrambling), or
`challenge` (Cloudflare/Turnstile). Those are the sites the `MANGA_PLAN.md` already parked as "not here
yet" (`aes`, `xor` and tile descrambling steps; no web view). Porting them is engine work, not JSON work,
and should be scheduled as such — not smuggled in as "just more sources".

## 2. What the ecosystem actually contains

Every live extension repository for Paperback 0.8 **and** 0.9, cloned and read (not just the one old
README that search engines keep returning):

| Repository | Notes |
| --- | --- |
| `inkdex/extensions` | The largest 0.9 repo; the official-ish community one |
| `karrot0/KakarotExtension` | 0.9; large, mostly challenge-gated sites |
| `LucifersCircle/pirate.vodka-extensions` | 0.9 |
| `PoppingMangoSources/general-extensions-mangago` | 0.9, manga/manhwa/manhua/novels |
| `nyzzik/extensions`, `kittycatgit/kitty-extensions-0.9`, `Catta1997/Sinon-Paperback-Extensions` | 0.9 |
| `TheVodraz/paperback-extensions`, `michaelasper/paperback-corruptbytes`, `itskodama/paperback-extensions` | 0.9 |
| `TheNetsky/extensions-generic` (+ fork), `TheNetsky/netskys-extensions` | 0.8; the Madara/FMReader/Guya/MangaBox/Genkan/NepNep/Bentai families |
| `xonlyfadi/xonlyfadi-extensions`, `kolbyml/kolbymls-extensions` | 0.8 |
| `Paperback-iOS/extensions` | Official, but **trackers only** (AniList, MAL, MangaUpdates) — no content sources |

The directory that ties them together is `paperbackextensionrepo.xyz`, generated from
`PaperbackExtensionRepo/Paperback-Extension-Repo-Compilation-List`. Its `repos.json` lists 24 repos.

**Scale:** 215 unique source ids across those repos. By content rating: 48 MATURE, 37 ADULT, 30 SAFE,
29 EVERYONE, 71 unrated (the 0.9-era repos put rating in the app manifest, not the versioning file).
**85 are MATURE/ADULT — that is the manhwa-heavy half, and it is what this port is about.**

Two traps worth recording so nobody re-learns them:

- `therobbiedavis/paperback-extension-repo` is the top search hit and is **0.6-era; it will not install**
  on 0.8 or 0.9. The ecosystem's own directory says to skip it.
- 0.9 sources do not run on 0.8, but 0.8 sources do run on 0.9. Newer is not automatically better.

## 3. What is portable, and what is not

`desktop/manga` reads a definition: requests + `css`/`attr`/`regex`/`path`/`template`/`date`/`number`
steps, plus `vars`, `prepare`, `repeat`, `settings`, `filters`, and `themes` for CMS families. A
Paperback extension is TypeScript. So the question per source is: **is the extension's logic expressible
as a fixed request and a read of the answer?**

| Verdict | Count | Meaning |
| --- | --- | --- |
| Portable as JSON, theme-backed | most of the 0.8 CMS families | New file extends an existing theme; no engine work |
| Portable as JSON, bespoke selectors | a minority | A normal hand-written definition + fixtures |
| Needs engine work | 25 of the 0.9 extensions | `crypto` (AES / base64 image blobs), `tiles` (descrambling), `challenge` (Cloudflare/Turnstile) |

The 25 needing engine work, by feature:

- **crypto:** `ReadComicsOnline`, `ZipComic`, `kittycatgit/generic`, `MadaraDex`, `MangaGo`, and the
  `shared`/`generic` bases several 0.9 repos build on.
- **tiles (descrambling):** `Hijala`, `Hitomi`, `NHentai`, `QiManga`, `RinkoComics`, `nyzzik/AsuraScans`.
- **challenge (Cloudflare/Turnstile):** most of `karrot0/KakarotExtension`, `FansubGeneric`,
  `HentaiHand`, `OmegaScans`, `RokuHentai`, `mangastream`, and nearly all of `corruptbytes` and
  `pirate.vodka`.

Note the overlap: several sites need two or three at once. These are the same three capabilities
`MANGA_PLAN.md` already listed under "Not here yet", so the work is already scoped in the right place.

## 4. Live-site reality check (this is the part that changes the plan)

**A corporate TLS-inspecting proxy sits in front of this machine** (`issuer=CN=Groww-Decryption-ECDSA`
on every host). It does not just MITM: it returns a **categorisation block page** — `HTTP 503` with
`<base href="/login/">` — for adult/manhwa sites. So a `503` from here means *the proxy refused*, not
*the site is dead*. Nothing in this plan is allowed to call a site dead on a 503 alone; that has to be
re-tested from the server or CI.

That said, the probe is still useful, because "unblocked here" implies "reachable there". Probing every
Madara/Guya/FMReader/MangaBox/Genkan/NepNep site from the 0.8 catalog:

**Reachable, unblocked, and the selectors actually match:**

| Site | Why it is a good first port |
| --- | --- |
| `manhuaplus.com` | Verified end to end: `/manga/page/1/?m_orderby=views` has 16 `page-item-detail` rows, detail page has `post-title`/`summary_image`/`genres-content`/`author-content`/`post-status`, chapters POST to `/ajax/chapters/` returns 3,364 `wp-manga-chapter` rows, chapter pages carry the images |
| `3asq.org` | Arabic, and a live example of the mirror trap below |

**Unblocked but not Madara / dead / too thin:** `azoranov.com` (778 KB, no `wp-manga`), `hiperdex.com`,
`platinumscans.com`, `h.mangabat.com` (all custom or heavily JS-driven), `disastercans.com` and
`neoxscans.net` (a 500-byte stub), `macbold.com` (**a parked HugeDomains page**), `mangatx.com` (130
bytes), `epikmanga.com` / `cmreader.info` (redirect), `mahoushoujobu.com` and `guya.cubari.moe` (404 on
the listing path), `hachirumi.com` (live, but **Guya** CMS — `embed-responsive-7by10`, `/read/manga/…`,
not Madara).

**Blocked by the proxy, so unknown from here:** `toonily.com`, `webtoon.xyz`, `coffeemanga.io`,
`setsuscans.com`, `immortalupdates.com`, `madaradex.org`, `manhuaus.com`, `1stkissmanga.me`,
`lilymanga.com`, `manhwa18.com/.net`, `ksgroupscans.com`, `klmag.net`, `danke.moe`, `manga4life.com`,
`mangasee123.com`, `manganato.com`, `mangakakalot.com`, and the rest of the `corruptbytes` /
`pirate.vodka` sets. **These are the highest-value manhwa sites and they are all one `curl` from CI.**

**Two traps found by probing, worth writing into the definition format's habits:**

1. **Mirror domains.** `3asq.org`'s cards link to `https://3asq.online/manga/one-piece/` — the HTML is
   served from `.org` but every id/cover/chapter link points at `.online`. The madara theme reads
   `href` and then `regex`es `/manga/([^/]+)/?$`, so it survives this; a definition that hard-codes the
   base into an id regex would not. Use `mirrors` for the sibling domains.
2. **Lazy-load + CDN.** Covers are `data-src` with `wp-content/themes/madara/images/dflazy.jpg` in
   `src`; pages are served from `cdn.<site>` with no `page-break` class on `manhuaplus.com` (the images
   are plain `<img src>` with `alt="1"`, `alt="2"`, …). The existing `"data-src|data-lazy-src|src"`
   chain handles the covers; the pages step needs its own selector per site, and `alt` numbering is a
   usable ordering signal.

## 5. The plan

### Phase 0 — make the port mechanical (small, do first)

1. Add a `manga-sources/scripts/port-madara.sh` (or a `themes` helper) that takes `id`, `name`,
   `base_url`, `lang`, `nsfw` and writes `sources/<id>.json`, then runs `record` + `check`. The point is
   that site #30 costs the same as site #1.
2. Add a **liveness probe** to CI that curls each `base_url` and reports `reachable / blocked / dead`,
   reading the proxy block page as *blocked*, never as *dead*. Without this, every future triage is
   guesswork from a laptop behind a proxy.
3. Decide the mirror convention (`mirrors` in the definition) and use it from the first site.

### Phase 1 — the confirmed Madara sites (done, 2026-10-22)

`manhuaplus` and `3asq` shipped as `sources/<id>.json` extending `madara`, with fixtures recorded by
`akidev-manga-cli record` and checked by `cargo test -p akidev-manga` (55 tests green). `manhuaplus` is
the template: it proves the whole chain (list → detail → chapters → pages) against real markup.

Two things had to be learned the hard way, and both are now recorded in `manga-sources/README.md`:

1. **A redirect eats a `POST` body.** `3asq.org` answers `301 → 3asq.online`, and the chapter list is a
   `POST /ajax/chapters/`. The redirect is followed but the body is dropped, so the site returns the
   *detail* page and the definition reads zero chapters — a silent, plausible-looking failure. The host
   that answers without redirecting goes in `base_url`; the other goes in `mirrors`. Expect this on every
   mirror pair in Phase 2.
2. **Not every chapter list counts rows.** Madara's usual `?t=N` numbering is 1-based, but `{offset}` is
   `page * size` and there is no arithmetic step, so `?t={offset}` starts at page 0. `chapters.repeat`
   now also fills `{page}` (raised by one each time), and `total` became optional: asking stops when a
   page brings no chapter not already seen. That covers a site that never says how many chapters it has.

Also worth knowing: `manhuaplus` puts chapter images in `div.reading-content img` (no `page-break`, which
the theme assumes), so its `pages.images` is overridden. Check that selector per site rather than assuming.

### Phase 2 — the rest of the Madara family (needs the proxy out of the way)

Every blocked Madara site in §4 gets the same 8-line file. This is the bulk of the value and the least
work: `toonily`, `coffeemanga`, `setsuscans`, `immortalupdates`, `manhuaus`, `madaradex`, `1stkissmanga`,
`lilymanga`, `mangatx`, `manhwa18`, `ksgroupscans`, `klmag`, `nitroscans`, `novelmic`, `pianmanga`,
`teensmanhua`, `xunscans`, `reset-scans`, `skscans`, `wdscans`, `platinumscans`, `neoxscans`,
`coloredmanga`, `isekaiscan`, `leviatanscans`, `mangabob`, `mangacultivator`, `mangalek`, `manhuapro`.
Run them from CI, record fixtures there, commit only what passes.

### Phase 3 — a second CMS theme

`Guya` (`hachirumi.com` is live and unblocked; `guya.cubari.moe`, `danke.moe`, `mahoushoujobu.com`) is
the next family. Then `FMReader` (`epikmanga`, `ksgroupscans`, `klmag`, `manhwa18.com`, `manhwa18.net`),
then `MangaBox` (`mangabat`, `manganato`, `mangakakalot`) and `Genkan`/`NepNep` (`mangasee`, `manga4life`).
Each theme is one hand-written file plus a live site to validate against, and then its family is free.

### Phase 4 — engine work (AES and challenge done, 2026-10-22)

**What the 25 "needing engine work" actually are, checked one by one against the code rather than by keyword.** The
earlier triage over-counted: a search for "tile" mostly found *UI tile counts* (Hitomi, NHentai, RinkoComics,
QiManga, Hijala all use "tile" for a grid cell), and a search for "crypto" found OAuth and md5-of-a-query helpers,
not image decryption.

| Need | Sources | State |
| --- | --- | --- |
| AES-CBC decryption | `MadaraDex` (chapter protector), `MangaGo` (fixed key), `kittycatgit/generic` (the base several 0.9 repos build on) | **done** (`aes_cbc` step) |
| Tile descrambling | `nyzzik/AsuraScans` only | **not built, and should not be** (below) |
| A check (Cloudflare/Turnstile) | ReadComicsOnline, ZipComic, QiManga, HiperDex, OniSaga, VortexScans, Thunderscans, MangaBaka, LNORI, EHentaiGeneral, LeagueOfComicGeeks, and the `shared` bases | **report and refuse** (already the engine's behaviour) |

**AES-CBC: shipped.** `desktop/manga/src/crypt.rs` and the `aes_cbc` step in `def.rs`/`eval.rs`. Two shapes, both
taken from the extensions: a passphrase stretched with the payload's salt through `MD5(previous ++ pass ++ salt)`
to 48 bytes (key 32, IV 16) — OpenSSL's `EVP_BytesToKey`, which is what the protector scripts do — and a key stated
in the definition (a hex 32- or 16-byte key with its IV). Verified three ways: NIST SP 800-38A AES-256-CBC vector,
round-trip tests against ciphertext made independently with `openssl enc` and Node's `crypto`, and
`examples/madara-protected.json` — a recorded `#chapter-protector-data` page that the engine turns into three image
addresses. That last one is the real proof: it fails unless the whole chain (selector → unescape → regex → key
derivation → decrypt → JSON) works.

**Challenge: already correct, one gap fixed.** A site that wants a check returns the typed `fetch::Challenge`, which
the facade turns into `MangaError::Challenge` and the app shows as "This site checks visitors first" — no bypass,
as `MANGA_PLAN.md` §15 decided. The gap: `looks_like_challenge` only believed a check on a 403/429/503, but
Cloudflare also serves its interstitial as a plain 200. It now believes the interstitial's own markup whatever the
status, and keeps the bare word "captcha" gated on the status (ordinary pages say the word). A test drives a local
server that answers 200 with an interstitial and asserts a typed `Challenge`, not "the page has no title: has the
site changed?".

**Tiles: measured, and deliberately not built.** The one source that has the algorithm, `nyzzik/AsuraScans`,
unscrambles a page when the API sends `tiles`, `tile_cols` and `tile_rows` with it:

```js
if (!config.url || !config.tiles || !config.tile_cols || !config.tile_rows) return config.url;
```

`api.asurascans.com` no longer sends them. Checked live on 2026-10-22 against two real series: every page of
`overgeared` ch. 344 and of `dungeon-architect-f6174291` came back as `{url, width, height}` and nothing else, so
`unscramble` returns the address untouched. Building tile descrambling would mean adding the `image` crate, a
definition field, a uniffi call and a path in the Swift reader that **no source would exercise and no fixture could
verify** — an unverifiable capability behind a confident-looking feature. The honest position: Asura is portable
with no engine work at all (it is a JSON API), and tiles gets built the day a source actually sends a permutation,
at which point it is a small job: decode, copy each `tile_cols × tile_rows` cell from `tiles[source]` to its
destination, re-encode.

**So the ceiling moved.** Of the 25: 3 are now portable JSON, ~20 need a browser the app deliberately does not have,
and 1 has nothing left to descramble. The engine work this port needed is finished; what remains is not engine work.


### Phase 5 — bespoke sites

Everything left: custom themes like `rinkocomics.com` (`.comic-card`, `/comic/<slug>/`, live and
unblocked, but no `wp-manga`), `asuracomic.net` (Astro, live, 765 KB), `flamecomics.com` (Next.js).
These get a hand-written definition each. `rinkocomics` is the best first one because it is live,
unblocked, and its markup is simple.

## 6. Legal and product line

`manga-sources/README.md` sets the rule: only sources that are legal to read — a public API that welcomes
third-party readers, or work under an open licence. Most of the catalog is scanlation aggregators, which
is a different category, and the repo already carries `nsfw` and the `ADULT`/`MATURE` ratings for a
reason. The port should keep that honesty:

- `nsfw: true` on adult sources, so the app can gate them.
- An attribution line per source naming the site, since the Paperback extension authors did the
  reconnaissance and their repos are the provenance.
- A decision needed from the owner (§7) on whether aggregators ship in the default repository at all, or
  in a separate one the person adds deliberately.

## 7. Decisions needed

1. **Aggregators in the default repo, or a second repo?** The current four sources are all clean-licence.
   Shipping 85 MATURE/ADULT aggregators in the same `index.json` changes what that repository means.
2. **Who runs the fixtures?** Fixtures recorded through this proxy will bake in proxy answers. They must
   be recorded from CI or the server.
3. **Is `challenge` a hard no?** If yes, ~25 sources are permanently out and the plan's ceiling is ~190.
4. **Do we write the second theme now, or wait for Phase 1 to land?** Guya is small and live; doing it
   early de-risks the "theme per CMS" approach before we depend on it.
