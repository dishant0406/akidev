# Source reliability — implementation status and verification limits

This is the current implementation checklist, not a live reliability report. The historical
[Torrential source review](TORRENTIAL_RESEARCH.md) remains unchanged: its pinned evidence and
recommendations describe that review's scope/time, not the completion state of this work.

## Implemented in the current working tree

| Area | Scope and evidence |
| --- | --- |
| Honest health | `desktop/core/src/model.rs`, indexer extraction, `stream.rs`, server fallback and frontend contracts preserve nullable seeders. Missing/invalid observations are unknown; valid zero is observed zero. Positive/unknown/zero trial tiers do not measure live swarm health. |
| Ordered provenance | `desktop/core/src/indexers/mod.rs` merges hash sightings in planned source/query order, preserves the first `source`, and collects unique `sources`. Known seeder counts use maximum, not sum; tracker construction uses only configured public trackers, capped at 32. |
| Failure-aware cache | Core caches only successful parsed per-request observations: ten minutes non-empty, sixty seconds empty. Failures are not stored as healthy empty answers; version/endpoints/request context scope keys. Internal reports preserve failed/pending variants. Server fallback does not cache answers whose link lookups have not completed. Unavailable web play results are immediately stale, with automatic retry disabled. |
| Global admission | Core permits at most 96 detached request producers and 24 concurrent HTTP exchanges process-wide, counting hedged mirrors; each plan is capped at 96 requests. Cache hits and coalesced callers do not acquire new-producer admission. Unsent/local-congestion requests do not poison mirror health. This is a process cap, not distributed/per-tenant admission. |
| Identity and startup | Shared release/file selection carries catalogue aliases and known film year into torrent/debrid checks, rejects conflicting title/year/episode/season evidence and extras, and requires the whole bounded torrent head range. Server media preparation probes via the same reader used for playback and rejects known runtime mismatches. This does not certify semantic audiovisual identity. |
| Stream-source budgets | `desktop/server/src/play.rs` admits four resolves, shares identical concurrent work, cancels abandoned work after grace, and divides the remaining post-plan resolve budget among remaining sources. Candidate retries and preparation share each source deadline. A pinned hash never substitutes another hash; Archive is excluded and skip still applies. Catalogue/search planning and the outer watchdog are distinct budget stages. |
| Archive fail-closed head check | `desktop/server/src/sources/archive.rs` constrains catalogue-relative item/file identity and episode/season evidence, rejects extras/short/unknown duration, and receives the catalogue film year through the production year-aware entry point. It uses the administrator-pinned origin, rejects redirects, bounds queries/items/responses and locally runs `ffprobe` on at most 8 MiB of media head, with pipe-only protocols and bounded time/output. Acceptance requires a decoded video frame, bounded dimensions and known consistent duration. |

**Completed request validation:** Catalogue kind/key, known episode counts, known series coordinates and future valid UTC airdates are checked before searching and opening; incomplete catalogue metadata remains unknown rather than fabricated. Library remembered file choices are revalidated against immutable torrent identity.

**Progressive playback:** One bounded live search tail retains the original admitted requests. Desktop/core attempts consume newly arrived validated candidates under the original concurrency/trial/deadline limits. Server debrid selection consumes unseen late candidates; pinned requests wait for their exact hash and exclusions survive late updates. No duplicate search endpoint or orchestration service was introduced.

**Telemetry:** Native GPUI render submission and browser video-frame callbacks are distinguished from resolved URLs. Fixed-size session accounting tracks active play, actual advancement, pause/seek-aware stalls, recovery and bounded interval/buffer metrics. Only bounded numeric/classified summaries use existing analytics; no title/hash/URL is attached. Server health exposes aggregate attempt/ready/failure counters, not per-user playback history.

## Verification limitations

- Local verification completed: workspace formatting and strict Clippy; full Rust workspace tests (core 325 passed, server 142, desktop 49, library 28, plus other workspace suites); frontend tests/type/lint/production build. Existing opt-in network/platform tests remain ignored. Production npm audit reports zero advisories after Next.js 15.5.29 and PostCSS 8.5.33 patches.
- Baseline production API was reachable on version 0.1.25. Deployment/release identifiers and post-rollout playback measurements must be recorded below after actual execution; local passing tests do not establish a deployed result.
- Checked-in fixtures/test assertions are not observed current upstream availability. A parsed
  answer, matching listing, selected file, downloaded head, probe/decode, first frame and sustained
  playback are separate evidence stages.
- Archive's local `ffprobe` is an actual bounded head inspection, **not full-file integrity**, complete
  decoding or content fingerprinting. Late indexes, valid redirects and files that cannot prove
  duration from the head may fail closed. Metadata/probed runtime does not prove the correct edition,
  dub, subtitles, uploader rights or semantic episode content.
- API-datacenter and desktop-local reachability differ. Upstream counts may be stale; a cached debrid
  hash or two index sightings is not a current playback guarantee. Partial release-list responses
  can still have a short successful HTTP cache lifetime; internal partial reports are not a public
  complete diagnostics protocol.

## Acceptance and rollout using existing paths

1. Run existing core/server regression suites, format/clippy and affected frontend type/lint/tests/
   build with dependencies already available. Include unknown/zero seeders, duplicate ordering,
   provider failures versus empty answers, admission/coalescing/hedges, wrong title/year/season,
   sequel/year collisions, four-digit anime, extras, complete head reads, deadlines and pinned skips.
   Exercise Archive redirects, unknown duration, malformed/truncated/non-video heads, duration
   conflicts and actual generated media with `ffprobe`. Record skipped/unavailable-media-tool tests.
2. Verify strict catalogue requests and actual-file identity on authorized/public-domain/owned media;
   verify cancellation does not remove unrelated/user-owned work. Check load and resource bounds.
3. Deploy **website frontend plus API/server** at one tested revision through the existing production
   Deploy workflow. Rebuild/release **desktop and library artifacts** containing the shared-core fixes
   through existing release/build paths; a website deploy or signed definition update alone cannot
   update installed core code. No new secrets, dependencies or configuration are needed.
4. Only after authorized live checks, report separately candidate recall, wrong-file rate, startup
   latency, first frame, stalls and sustained play, native versus web, plus CPU/disk/bandwidth and
   cancellation behavior. A smoke resolve or first-byte response is not a broad success percentage.

## Future hypotheses — not completed scope

The research proposes measured capability-aware ranking, broader sustained-play recovery, metainfo locators,
Archive web-seed/layout enrichment, throughput backpressure and admin capability discovery. These
need separate design, security/rights review, controlled experiments and acceptance evidence; they
are not shipped merely because the present health/cache/file/head controls exist. Do not transplant
arbitrary URL fetches, upstream private credentials, host-network assumptions or downloaded binary
installers. Keep signed definitions, anime mapping, existing torrent engine, debrid and HLS delivery.
