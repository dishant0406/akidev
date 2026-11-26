# Snowfl indexer plugin

Snowfl is a public meta-search service. Its client key changes at runtime. This module reads the key from the public homepage and JavaScript bundle. It then returns torrent rows to Akidev.

The module uses at most eight requests. It resolves missing magnets for the first five results only. It removes music, software, and NSFW rows.

## Build

```bash
cargo test
RUSTFLAGS="--remap-path-prefix=$HOME=/builder" cargo build --release --target wasm32-unknown-unknown
```

The module is `target/wasm32-unknown-unknown/release/snowfl.wasm`. Put its SHA-256 value in `plugins/snowfl.json` in the sources repository.

## Verify

Use the Akidev sources repository script:

```bash
.claude/skills/akidev-sources/scripts/try-module.sh \
  target/wasm32-unknown-unknown/release/snowfl.wasm search \
  '{"host":"https://snowfl.com","kind":"movie","path":"/","query":[["q","sintel"]],"json":null}' \
  snowfl.com
```

The plugin runs only in the desktop app. The server does not load code plugins.
