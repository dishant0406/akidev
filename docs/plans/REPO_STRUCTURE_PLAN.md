# Repo structure plan

Status (2026-11-26): steps 1 to 7 are done on the branch `repo-structure`, not merged. Step 8 (server deploy files) is not done. The desktop packaging files went to `apps/desktop/packaging/`, not `apps/desktop/assets/`, because the app already has an `assets/` folder.

Goal: one clear home for each thing. A new developer must find any file by asking "which product is it for, and is it an app, a library, data, design or infra?" This is a plan. Nothing is moved yet.

## What the repo holds today

Two products live in one repo.

1. **Akidev, movie streaming.** The website (`akidev-frontend`), the desktop app (`desktop/app`) and the API server (`desktop/server`). The landing page for the phone app, `app.akidev.live`, is a route inside the website (`akidev-frontend/src/app/app`).
2. **Aki Manga, manga and books.** The iPhone app (`manga-app`) and the Android app (`manga-android`). Both use the same Rust engine (`desktop/manga`, `desktop/manga-ffi`, which uses `desktop/core`).

Why it is messy now:

- The folder `desktop/` is the Rust workspace for everything. It holds the desktop app, the server, the shared core, the phone engine, the data and the plugins. The name says "desktop app" but most of it is not.
- The phone apps have unequal names: `manga-app` is iOS only, and `manga-android` names its platform.
- Shared phone material has no home. Icons (`manga-app/scripts/fetch-icons.py` writes to both apps), the design language (`manga-app/DESIGN.md`), the store listing text, the demo server and the sources document live inside one app or in `desktop/`.
- Plans are in `pipeline/`. Design is in `design/`, `design-shots/`, `branding/` and `manga-app/DESIGN.md`. Release and server scripts share one `scripts/` folder.

## Target structure

```
akidev/
├── apps/                     things you build and ship
│   ├── web/                  streaming website and the app.akidev.live landing page (Next.js)
│   ├── desktop/              streaming desktop app (Rust)
│   ├── server/               API server (Rust, Docker)
│   ├── manga-ios/            iPhone app (Swift)
│   └── manga-android/        Android app (Kotlin)
├── crates/                   Rust libraries, shared by the apps above
│   ├── core/                 search, sources, torrents (used by server, desktop and phone engine)
│   ├── party/  library/      watch party, local library
│   ├── manga/                manga and books engine
│   └── manga-ffi/            the engine as a library for Swift and Kotlin
├── content/                  data and assets that apps read or share
│   ├── sources/              sources.json, publish and tracker scripts
│   ├── plugins/              anirena, code-indexer, snowfl, torbox
│   └── manga/                shared by both phone apps: icons master and fetch script, store listing text, demo server
├── design/                   DESIGN_PLAN.md, manga.md, branding/, shots/
├── infra/                    caddy/, docker-compose.yml, deploy, cloudflare, server setup scripts
├── release/                  release-ios.sh, release-android.sh, release-common.sh
├── fastlane/                 stays at the root: Fastlane looks for it there
├── docs/                     design.md, flow.md, plans/ (was pipeline/)
├── vendor/                   copies of other people's crates (was desktop/vendor)
├── Cargo.toml  Cargo.lock    one Rust workspace for crates/ and the Rust apps
├── package.json              npm workspace (apps/web)
└── README.md  CLAUDE.md  AGENTS.md  LICENSE  ATTRIBUTION.md
```

Rule for the layout: `apps/` holds what ships, `crates/` holds Rust code that more than one app uses, `content/` holds data and pictures that apps read, and everything else supports those.

## Old name to new name

| Old | New |
|---|---|
| `akidev-frontend/` | `apps/web/` |
| `desktop/app/` | `apps/desktop/` |
| `desktop/server/` | `apps/server/` |
| `manga-app/` | `apps/manga-ios/` |
| `manga-android/` | `apps/manga-android/` |
| `desktop/core`, `party`, `library`, `manga`, `manga-ffi` | `crates/<same name>/` |
| `desktop/vendor/` | `vendor/` |
| `desktop/plugins/` | `content/plugins/` |
| `desktop/sources/` and `desktop/scripts/publish-sources.sh`, `refresh-trackers.sh`, `check-old-apps.sh` | `content/sources/` |
| `desktop/scripts/` (bundle, dmg, installer) | `apps/desktop/scripts/` |
| `desktop/assets/`, `desktop/signing/` | `apps/desktop/assets/`, `apps/desktop/signing/` |
| `desktop/Cargo.toml`, `Cargo.lock` | `Cargo.toml`, `Cargo.lock` at the root |
| `manga-app/DESIGN.md`, `USER_FLOW.md` | `design/manga.md`, `apps/manga-ios/USER_FLOW.md` stays or moves to `docs/` |
| `manga-app/scripts/fetch-icons.py`, `demo-server.py` | `content/manga/` |
| `manga-android/store/` (listing, screenshots, film) | `content/manga/store/` (a folder per store inside) |
| `design/`, `design-shots/`, `branding/` | `design/` (with `shots/` and `branding/` inside) |
| `caddy/`, `docker-compose.yml`, `scripts/deploy.sh`, `cloudflare*.sh`, `setup-server.sh`, `prowlarr-sync.js` | `infra/` |
| `scripts/release-*.sh` | `release/` |
| `pipeline/` | `docs/plans/` |

## What does not change

- Crate names (`akidev-core`, `akidev-manga` and the rest), package names, bundle id `live.akidev.manga`, and every public URL. Only folders move.
- Signing keys and store settings.

## Order of work

Each step is one commit. Use `git mv` so file history stays. Run the check at the end of each step before the next one.

1. **Docs and design.** Move `pipeline/` to `docs/plans/`. Gather `design/`, `design-shots/`, `branding/` and `manga-app/DESIGN.md`. Check: links in `CLAUDE.md`, `README.md` and the memory notes still point to real files.
2. **Release scripts.** Move `scripts/release-*.sh` to `release/`. Check: `bash -n` passes, and `fastlane lanes` still loads. Done 2026-11-26. The server deploy files are NOT moved here: see step 8.
3. **Content.** Move `sources`, `plugins` and the shared phone material. Fix `publish-sources.sh`, the plugin loader paths and `fetch-icons.py`. Check: the core tests that load plugins and `sources.json` pass.
4. **Rust workspace.** Move the crates and the two Rust apps. Put `Cargo.toml` at the root with explicit members. Fix `path =` lines, the `[patch]` and `exclude` lines, the Dockerfile of the server and the desktop bundle scripts. Check: `cargo test` for the workspace, and the server Docker build.
5. **Phone apps.** Move `manga-app` and `manga-android`. Fix `build-ffi.sh` and `build-engine.sh` (they point at the engine), `project.yml`, the Gradle files and `fastlane/Fastfile` (`ROOT` paths, `testflight.sh`). Check: `xcodegen` plus an iOS archive with `BUILD_ONLY=1`, `./gradlew :app:bundleRelease`, and `fastlane lanes`.
6. **Website.** Move `akidev-frontend` to `apps/web`. Fix `package.json` workspaces, the Dockerfile and the `docker-compose.yml` context. Check: `npm run build` and `npm test`.
7. **Cleanup.** Update `README.md` with a map of the folders. Add a short README to each top folder. Update the memory notes and `CLAUDE.md` paths.

8. **Infra, last and with care.** Move `caddy/`, `docker-compose.yml`, `.env.deploy.example`, `scripts/deploy.sh`, `cloudflare*.sh`, `setup-server.sh` and `prowlarr-sync.js` to `infra/`. This one changes how the live server deploys: the workflow runs `bash scripts/deploy.sh` over SSH, and `deploy.sh` and the compose file use paths relative to the repo root on the server (`/opt/konflix`). Check before merging: `docker compose config` gives the same output before and after (save it first), and read the deploy workflow line by line. Do it only with the owner at hand, because the first deploy after the move cannot be tested here.

Do the steps on a branch. Merge only when all checks pass. Touch no other code in these commits, so any failure comes from a path.

## Risks

- About 130 files mention a path that will move (`desktop/` 56, `manga-app/` 24, `akidev-frontend` 19, `design/` 16, `pipeline/` 14, `manga-android/` 14). Search for each old name after every step.
- The six GitHub workflows use path filters and build contexts. A missed path means a silent job that never starts. Read each workflow after step 2, 4 and 6.
- Xcode and Gradle keep absolute-looking relative paths. The iOS project is built from `project.yml`, so edit that file, not the generated `.xcodeproj`.
- The release scripts and Fastlane lanes I wrote use `ROOT` and fixed paths. They need edits in step 5.

## Decisions (2026-11-26)

1. The phone app folders are `apps/manga-ios` and `apps/manga-android`.
2. The `app.akidev.live` landing page stays inside `apps/web`. A separate `apps/landing` only pays off if it gets its own host or build.
3. `fastlane/` stays at the root, because Fastlane needs it there.
