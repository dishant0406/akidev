# Plugins that bring code (WebAssembly)

Status (2026-11-22, branch `wasm-plugins`): the engine, its limits and rules, plugin files (module, hash, install,
remove, download) and the first slice, an indexer whose requests are made by code, are built and tested, with a real
module (`desktop/plugins/code-indexer`). Code for catalogues and mappers is built too. Stream services are built on branch `stream-services` (section at the end):
the service entry, the app's resolve step, a key field in Settings and TorBox as a real plugin. Read this first if you
pick the work up.

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

## Stream services (started 2026-11-22, branch `stream-services`)

A plugin's `services` entry turns a release into a playable https link, the way a debrid service does. The module only
wraps the service's API. The app keeps the judgement.

```json
"services": [{ "id": "torbox", "name": "TorBox", "export": "resolve",
               "base": "https://api.torbox.app/v1/api/torrents", "hosts": ["api.torbox.app"], "media_hosts": ["torbox.app"], "key_label": "TorBox API key" }]
```

`base` is the address of the API, given to the module. `hosts` are the hosts the module may call (as `akidev.http`), and `media_hosts` the hosts the link it returns may be
on. The key is typed in Settings, kept in the settings file (`service_keys`) and given only to the module.

**The calls.** The app calls the export with `{want, release, media, key, state}`. `want` is `files` and then `link`:

1. `want: "files"`: the module answers `{status:"files", files:[{id, name, size}], state}` (the release is cached and
   these are its files), or `{status:"unavailable", reason}` (not cached; the torrent engine is tried).
2. The **app** picks the episode's file from that list, with `releases::pick_for_identity`, the same code the
   server's debrid source uses. A module never decides which file is the episode.
3. `want: "link"`, `file: {id}`: the module answers `{status:"ready", url}`.

At any step a module may answer `{status:"retry", after_ms, state}`. The app waits and calls again with the `state` it
was given back, until the deadline. A module cannot sleep (it has no clock), so the app owns the waiting.

**The link** must be https, on a public host, on one of `media_hosts`, and is handed to mpv as it is (no headers: a
link that carries its own token, as TorBox's does, is what is supported first). It is never stored.

**Order.** Enabled services are tried first, in the order the plugins are listed, for the best few releases of the
plan, within a third of the resolve time. When none answers, the torrent engine runs as before.

**Not in this step:** downloads to the library and Watch party use the same link later; a Keychain for the key; a link
that needs request headers.
