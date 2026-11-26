# Example: an indexer that brings code

A JSON plugin can only say which requests to make and which fields to read. This one signs its request and reads
an HTML page, which JSON cannot say, so it brings a WebAssembly module (`src/lib.rs`).

## Build

```bash
cargo build --release --target wasm32-unknown-unknown
```

The module is `target/wasm32-unknown-unknown/release/code_indexer.wasm`. Its SHA-256 goes in the plugin.

## The plugin file

```json
{
  "plugin": 1, "id": "signed-torrents", "name": "Signed torrents", "version": 1,
  "module": { "file": "code_indexer.wasm", "sha256": "<64 hex digits of the module>" },
  "indexers": [{
    "id": "signed-torrents", "name": "Signed torrents", "kinds": ["series"],
    "hosts": ["https://torrents.example.com"],
    "code": "search",
    "request": { "path": "/list", "query": { "q": "{term}" } },
    "response": { "format": "json", "rows": "items",
                  "fields": { "title": "title", "hash": "hash", "seeders": "seeders" } }
  }]
}
```

Host the JSON and the `.wasm` file side by side on the same https host. The app downloads the module from the
same place as the JSON, and refuses it if its hash is not the one the JSON names.

## What the module may do

- It exports `memory`, `alloc(len) -> ptr` and one function per job (`search`), which takes `(ptr, len)` of JSON
  and returns `(ptr << 32) | len` of JSON it wrote.
- It imports `akidev.http(ptr, len) -> packed`: one request, `{method, url, headers, body}` in and
  `{status, body}` (or `{error}`) out. Only hosts in the indexer's `hosts` are allowed, over public https only, at
  most 8 requests a call, answers up to 2 MB, no `Host` header and the other forbidden ones.
- It has no files, no clock and no other network. It stops after 300 million instructions, 64 MB of memory or
  20 seconds.
- Whatever it answers is read by the indexer's own `response`, so a module cannot get a release past the checks
  the JSON path has.

## Try the test

`cargo test -p akidev-core --features code-plugins an_indexer_whose_requests` runs this module against a signed
page served from the test. Rebuild and copy the module to `core/tests/fixtures/code-indexer.wasm` after a change.
