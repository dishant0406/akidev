# Plugins that bring code (WebAssembly)

Status (2026-11-22, branch `wasm-plugins`): the engine, its limits and rules, plugin files (module, hash, install,
remove, download) and the first slice, an indexer whose requests are made by code, are built and tested, with a real
module (`desktop/plugins/code-indexer`). Not built: code for catalogues and mappers, stream services, and a field
in Settings for a plugin's login or key. Read this first if you pick the work up.

## What this is for

A JSON plugin can only say which requests to make and which fields to read. It cannot sign a request, log in or
scrape HTML. A code plugin brings a `.wasm` file for the steps JSON cannot say. The engine is `wasmi`, a pure Rust
interpreter (no JIT, so it also works on iOS), behind the cargo feature `code-plugins`. Only the desktop app turns
the feature on. The server and the phone engines do not.

## The one idea

The guest replaces only the **transport step**: it makes the requests and answers with JSON text. The plugin's
declarative `response` (or `card`, or mapper) then reads that JSON exactly as it reads any other answer. So every
check that already exists (hash checks, episode rules, ranking) stays untouched, and a guest can never hand the
app something the JSON path would not accept.

## The ABI (small on purpose)

JSON in, JSON out, over the guest's memory.

Guest exports: `memory`, `alloc(len: i32) -> i32`, and one function per job, `fn(ptr: i32, len: i32) -> i64`. It
returns `(ptr << 32) | len` of a JSON result it wrote into its own memory.

Host imports, module `akidev`:

- `http(ptr, len) -> i64`: one request. In: `{method, url, headers, body}`. Out: `{status, body}`. The host calls
  the guest's `alloc` to place the answer.
- `log(ptr, len)`: a line for the log.

Signing and HTML parsing are compiled into the guest, so the host has no crypto and no parser functions.

## Limits (each has a test)

fuel (an endless loop stops), memory (a grow bomb stops), a wall clock, a cap on requests per call, a cap on a
response, a cap on the input and output size, and a cap on the `.wasm` file.

## Network rules (the same as a JSON plugin, plus one)

Every guest request goes through `check_public_url`, the shared client (no redirect to a private host) and the
forbidden-header list. Plus: the plugin declares the hosts it may call, and a request to any other host is refused.
The hosts are shown when it is installed.

## Files

The `.wasm` sits next to the plugin JSON (`plugins/<id>.wasm`). The JSON names it with its sha256. The file is
downloaded from the same host as the JSON, and it is refused if the hash does not match. `install`,
`set_enabled`, `remove` and `load` carry the file with the JSON.

## What a plugin can bring as code

1. An indexer whose transport is code. (First slice.)
2. A catalogue, and a mapper, the same way.
3. A stream service, which is **new work**: the desktop app has no way to turn a release into a playable URL
   except its own torrent engine (debrid and the Internet Archive are code in `desktop/server`, for the website).
   So this needs a new playback path in the app, built last. The server's three sources stay in Rust.

## Not in the signed document

`sources.json` never refers to code. Only a plugin does, and a plugin is unsigned and shown as "unverified".

## Secrets

A login or an API key needs a field per plugin in Settings. Not in the first slice.
