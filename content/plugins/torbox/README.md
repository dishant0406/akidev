# TorBox, as a stream service plugin

TorBox already holds many releases and serves the finished file from its own servers, so playback starts at once and
nothing joins a torrent swarm. This plugin lets the desktop app use it. It is also the example for anyone who wants to
add a different service.

## Build

```bash
cargo build --release --target wasm32-unknown-unknown
```

The module is `target/wasm32-unknown-unknown/release/torbox.wasm`. Put its SHA-256 in the plugin.

## The plugin file

```json
{
  "plugin": 1, "id": "torbox", "name": "TorBox", "version": 1,
  "module": { "file": "torbox.wasm", "sha256": "<64 hex digits of the module>" },
  "services": [{
    "id": "torbox", "name": "TorBox", "export": "resolve",
    "base": "https://api.torbox.app/v1/api/torrents",
    "hosts": ["api.torbox.app"],
    "media_hosts": ["tb-cdn.pw", "tb-cdn.io"],
    "key_label": "TorBox API key"
  }]
}
```

Host the JSON and the `.wasm` side by side on one https host. The person who installs it types their TorBox API key in
Settings, under Stream services. The key is kept in their settings file (readable only by them) and given only to this
module.

## What the module does

The app calls `resolve` with `{base, key, want, release, media, file, state}`.

1. `want: "files"`: if the account holds the release, answer its files. If not, add it, but only when TorBox already
   has it cached (`add_only_if_cached`), then answer `retry` until the files are listed. Not cached: `unavailable`, and
   the app uses the torrent engine.
2. The **app** picks which file is the episode, with the same code the server's debrid source uses. A module never
   decides that.
3. `want: "link"` with the picked `file.id`: answer `{status: "ready", url}`. The link must be https on one of
   `media_hosts`, and the app hands it to the player as it is.

At any step the module may answer `{status: "retry", after_ms, state}` and the app calls again after the pause, with
`state` given back. A module has no clock, so it never waits itself.

## Test

`cargo test -p akidev-core --features code-plugins services::` runs this module against a fake TorBox. Rebuild and copy
the module to `core/tests/fixtures/torbox.wasm` after a change.
