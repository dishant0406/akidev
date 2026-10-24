# Read aloud in Books: on-device voices (Gradium Phonon, system voices, more models you add)

Written 2026-10-24. A plan with research, not a build. Nothing in the app has changed for it.

## 1. What is asked

A **Listen** feature in the book reader that reads the book aloud, on the device, with:

1. A **Voices** screen listing the models the app can use: the three Phonon models from <https://gradium.ai/phonon>, the voices already in
   the phone, and room to **add more models**.
2. The person chooses a model (and a voice in it), downloads it if it needs downloading, and the book is read **on the device**: no
   server, nothing about the book leaves the phone.

Skipped by the owner earlier and still skipped: VoiceOver and dictionary lookup (this plan is about reading aloud, which is not VoiceOver).

## 2. What Phonon is (researched)

Gradium's first open-weight text-to-speech release, made to run on the device. Sources at the end of this file.

| | Phonon 31m | Phonon 40m | Phonon 40m multilingual |
| --- | --- | --- | --- |
| Languages | English | English, French, German, Spanish, Portuguese: **one model per language** | the same five in **one** model |
| File (8-bit) | 64.5 MB (`model.q8.gguf`) | 74.6 MB | 74.6 MB |
| Parameters | 31M | 40M | 40M |
| Weights licence | **Apache 2.0** (commercial use allowed) | **CC BY-NC 4.0** (no commercial use) | **CC BY-NC 4.0** |
| Hugging Face | `gradium/phonon-31m-en`, public | `gradium/phonon-40m-{en,fr,de,es,pt}`, **gated** | `gradium/phonon-40m-multilingual`, **gated** |
| Error rate (English, 500 hard sentences, from Gradium) | 1.70% | 2.22% | 2.11% |

For comparison, the same table says Kokoro 3.50%, Kitten TTS 7.40%, NeuTTS Nano 8.71%. These are the maker's own numbers; treat them as
directional. A listening test on the device is part of phase 0.

How it works: a flow-matching transformer makes audio latents in 80 ms frames, and Kyutai's Mimi codec turns them into 24 kHz mono
audio. It is built on Kyutai's Pocket TTS and says Pocket TTS checkpoints can be loaded if they bring their own config, tokenizer, weights
and voices. Streams frame by frame (the README reports under 20 ms to the first audio on Apple devices).

Voices: each is a small `.safetensors` file (one tensor, 1×512×125 float32). `gradium/phonon-voices` has **102 voices for Phonon 31m**
(90 catalogue voices across five languages and seven locales, 12 English character voices), licence Apache-2.0, with a `voices.jsonl`
that describes each. Making your own voice (cloning or designing) is Gradium's studio or API: not in scope.

Known limits (from the model card): text in another language than the chosen one is read with wrong pronunciation; numbers, dates and
currency are normalised by the `lang` setting, so it must match the text; long input is split into sentences, so tone does not carry across
a whole passage. The card says not to impersonate people or clone voices without consent, and to disclose synthetic speech where it
matters.

### 2.1 How it can run in our app

| Route | What it is | Models it works with | Cost and risk |
| --- | --- | --- | --- |
| **A. The Rust crate `ptts`** (MIT OR Apache-2.0) inside our own engine | Our engine is Rust already. `ptts` loads a model folder (`config.json`, `tokenizer.json`, weights, a voice file) and returns PCM, with a streaming call. CPU, with Arm acceleration (KleidiAI) | **All three, and any compatible checkpoint a person adds**: it reads the checkpoint at run time | **Measured:** the crate and its dependencies compile for `aarch64-apple-ios`; a probe linked with dead-stripping adds about **2.6 MB** to the binary. Speed on a real iPhone is **not measured yet** (phase 0) |
| **B. Gradium's Swift package `PhononTTS`** (Core ML on the Neural Engine) | An xcframework plus Core ML model bundles; "up to 22× faster than real time on an iPhone 16 Pro" (Gradium) | Core ML bundles must be **exported** from a checkpoint on a Mac with a Rust tool; Gradium releases a ready bundle only for 31m | Less battery and heat, but each model needs a prepared bundle: for the CC BY-NC models we would have to make and host bundles ourselves, which their licence does not clearly allow. Also another binary framework in the app |
| **C. The phone's own voices** (`AVSpeechSynthesizer`) | Apple's voices, including the "enhanced" and "premium" downloads and Personal Voice (iOS 17) | none: it is Apple's | Zero size. Tells us **which word** is being spoken (`willSpeakRange`), which Phonon cannot |

**Recommendation:** A as the engine for every Phonon model and for added models (one code path, works from a plain checkpoint folder);
C always available as the first voice and the fallback for languages Phonon does not read; B only later, only for 31m, only if A turns
out too slow or too hot on real phones.

## 3. Licences and what we may do (needs the owner's decision)

- **Phonon 31m and its 102 voices: Apache 2.0.** The app may download it for the person, show it in the list, and use it in a free or paid app.
- **Phonon 40m and 40m multilingual: CC BY-NC 4.0, gated on Hugging Face** ("does not allow commercial use"; Gradium says commercial use
  needs contacting sales). The files can only be had by a Hugging Face account that accepted the terms, with that account's token.
- Akidev is free and ships no content, but the App Store is distribution, and a donation or a later price changes "non-commercial".
  So the plan treats the 40m models as **bring your own**: the app never bundles or hosts them; the person accepts the terms at Hugging
  Face with their own account, pastes a read-only token (kept in the Keychain), and the app downloads from Hugging Face to the phone.
  The list shows them with "Non-commercial licence" and the exact words. **Decision for the owner:** list the 40m models by name in the
  app (the plan), or show them only after "Add a model" (more distance), and whether to write to Gradium to ask for permission to list and
  fetch them. Not legal advice; the terms above are quoted from the model pages.
- Hugging Face can block access to a gated model at any time and sends the person's username and email to the author when they accept:
  the screen says so in one line.
- Voices: Apache-2.0 per the card. To verify in phase 0: whether the 102 voices (made for 31m) work with the 40m models (the card is not explicit).

## 4. What readers want from reading aloud (research)

Reddit and store pages (TinyFish search, 2026-10-24; weak evidence, but consistent):

- **Natural voice, offline, no caps.** People moved away from ElevenReader, Speechify and similar when free tiers got time limits and
  paywalls; threads ask for "completely free, Kokoro-quality" reading of EPUB and PDF, and several developers answer with on-device
  apps (Kokoro at about 20× real time on the phone is a selling point).
- **Highlighting what is read** (the sentence, ideally the word) and **page following** it; Voice Dream and @Voice Aloud Reader are the
  names people miss.
- **Skip the junk:** footnote numbers, headers, page numbers, legal pages are not wanted read aloud ("most readers stumble over footnotes").
- **Lock-screen controls and background play**, **speed**, **sleep timer**, per-book voice, pronunciation fixes.
- Complaints about robotic system voices push people to model-based voices; complaints about size and heat push the other way: so a
  list with a free system voice first and models you can add is the honest answer.

## 5. Product design

### 5.1 The Listen bar (in the reader)

A round Listen button joins the bottom bar. Tap: reading starts from the top of the current page (or from the selected words: the
selection menu gets "Read from here"). A glass bar replaces the slider while listening: previous sentence, play/pause, next sentence, speed
(0.7 to 1.6), sleep timer (off, 15, 30, 60 minutes, end of chapter), the voice chip (opens Voices), close. The sentence being read is
highlighted on the page (the same drawing as a search find, in its own tint) and the page turns by itself; in scroll mode it scrolls.
Tapping the page while listening shows the bars; the audio keeps going. Front and back matter (we already know which chapters they are)
are **skipped by default**, a switch in the sheet; footnote marks and numbers are not read.

### 5.2 Voices (a screen, from the Listen bar's voice chip and from Settings)

```
Voices
  This phone
    ( ) Phone voices            built in, no download, follows the book's language      [Choose voice]
  Phonon                         on-device models from Gradium
    ( ) Phonon 31m              English · 64 MB · Apache 2.0                            [Download] / [Ready] / [Delete]
    ( ) Phonon 40m              5 languages, one model each · ~75 MB each · Non-commercial (needs a Hugging Face login)
    ( ) Phonon 40m multilingual 5 languages in one · 75 MB · Non-commercial (needs a Hugging Face login)
  Added by you
    ( ) <name>                  from huggingface.co/<id> · size · licence                [Delete]
  [ Add a voice model ]
```

- Each row: what it is, languages, size, licence, state (not downloaded, downloading with progress and cancel, ready, needs a login).
  Choosing a model that is not downloaded downloads it first (Wi-Fi only by default, like downloads in the manga app).
- **Voice** inside a model: a list of its voices (name, language and locale, gender, description from `voices.jsonl`), with a **Preview** that
  speaks one fixed sentence on the device. For 31m the 102 voices come from the Apache voices repo, fetched one at a time (small files).
- **Which model reads which book:** a default per language, and a per-book choice that wins. A book whose language the chosen model does
  not read is read by the phone voice, and the bar says so once.
- **Add a voice model:** type a Hugging Face id (`owner/name`, optionally `@revision`). The app asks Hugging Face for the file list and the
  licence, checks it looks like a Phonon or Pocket TTS checkpoint (config, tokenizer, weights, a voice), shows name, size and licence and
  asks to confirm, then downloads. A gated one asks for the person's token (a link to the page that makes one; stored in the Keychain,
  sent only to huggingface.co). The size is capped and nothing is run from a download: weights are data (safetensors, gguf).
- A footer line on the screen: "Voices run on this phone. Nothing you read leaves it. Downloads come from Hugging Face." and Gradium's
  guidance: synthetic voices, no impersonation.

### 5.3 States to design

First use (no model: offer the phone voice at once, and Phonon 31m as an upgrade), download failing or offline (resume), disk
nearly full, model corrupt (re-download), gated and token wrong or access revoked, model slow on this phone (say so and offer the
phone voice), phone call or headphones unplugged (pause), silent switch (reading aloud still plays, as a media app), low power mode
(offer the phone voice, which is cheaper).

## 6. Architecture

```
 Reader (Swift) ── Listen bar ── SpeechSession ── speech units ──┐
                                      │                          │
              ┌───────────────────────┴───────────┐     Rust engine: `voice` module
              │ VoiceEngine (Swift protocol)      │       units(chapter) -> sentences with positions
              │  - PhoneVoice (AVSpeechSynthesizer)│       models: registry, download, check
              │  - ModelVoice (calls the engine)  │       synthesize(model, voice, text, speed) -> PCM
              └───────────────────────┬───────────┘
                                      ▼
                      AVAudioEngine + player node → speaker, Bluetooth, lock screen
```

- **Speech units (Rust, `book::speech`).** From a chapter's blocks: sentences by Unicode sentence rules (the `unicode-segmentation` crate is the
  small standard choice; verify it is already in the dependency tree before adding), each with its start and end in the book's own
  positions (the same units highlights and search use), so highlighting needs no new mapping. Dropped: raised runs (footnote marks),
  soft hyphens, picture placeholders, text of links to notes. Headings read as their own unit with a pause after. Numbers and such are
  left to the model's `lang` normaliser.
- **Model voice (Rust, `voice` module, `ptts` crate).** `Voices::open(folder, lang)` loads once and keeps the model in memory while a book is
  being read, and drops it after a couple of minutes idle. `speak(text, voice, speed) -> PCM` returns one unit's audio (24 kHz mono floats).
  Speed uses the model's duration setting, not resampling, so pitch holds. Runs on a background queue; cancel is cooperative.
- **Scheduling.** Synthesis runs two to three units ahead of playback and is held in memory only (never written to disk); seeking or changing
  speed drops what was ahead. Time to the first sentence is the synthesis time of one sentence: measured in phase 0.
- **Playback (Swift).** `AVAudioEngine` with one player node, units scheduled as buffers; the playback position maps to the unit for
  highlighting. Background audio needs the `audio` background mode and an audio session of category playback; lock-screen and headphone
  controls need `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter`.
- **Phone voice (Swift).** `AVSpeechSynthesizer`, one utterance per unit; its word callback gives finer highlighting (the word as well as
  the sentence). Voice list from `AVSpeechSynthesisVoice.speechVoices()`, filtered to the book's language, enhanced and premium first.
- **Registry (store schema 10).** `voice_model(id, name, source, kind, languages, bytes, licence, gated, path, added_at)` and a per-language and per-book choice
  of model and voice. Files under `Application Support/AkiManga/voices/<id>/`, marked as excluded from backup (they can be downloaded
  again and are large). The Keychain holds the Hugging Face token.
- **Download (Rust).** Hugging Face HTTP API for the file list, licence and gate state; files fetched with the token as a Bearer header,
  resumable, checked against the size (and the LFS hash when given), written to a temporary name and renamed when whole; progress to the
  UI; Wi-Fi preference as in the manga downloader. The same trust rules as OPDS: https only, size caps, no redirects to other schemes.
- **Size budget.** The Phonon runtime adds about 2.6 MB to the binary (measured with a probe, dead-stripped). The app is at 11.4 MB; the
  plan's budget for this feature is 3.5 MB. Models are downloads, not part of the app.
- **Launch budget.** Linking `AVFAudio` and `MediaPlayer` for audio and lock-screen controls may cost launch time, as `PhotosUI` did
  (about 830 ms in the simulator). `scripts/check-ui.sh` bans `AVFoundation` and `AVKit` for that reason. Phase 0 measures `AVFAudio` and
  `MediaPlayer` alone, on the simulator and on a phone; if either costs launch time, the audio code goes behind the cheapest route
  that avoids it (a smaller framework, or loading the framework on first use) before any other work builds on it.
- **Privacy.** Reading aloud sends nothing anywhere. Network is used for downloading models and voice files only, from Hugging Face.
  The privacy page and App Store privacy answers are updated in the phase that ships downloads.

## 7. Phases

| # | Goal | Done when |
| --- | --- | --- |
| 0 | **Spike (3 days), on a real iPhone.** Build the `ptts` probe into the app's engine; load Phonon 31m; speak a paragraph | Time to first sentence, real-time factor, memory, battery after 10 minutes, and heat, written down for CPU. Listening notes on voices. Launch cost of `AVFAudio` and `MediaPlayer`. 31m voices tried on 40m. Decision on Core ML (route B) |
| 1 | **Engine (4 days).** `book::speech` units with tests on the six real books; `voice` module (open, speak, cancel); registry (schema 10); downloader with Hugging Face metadata, resume, token; tests with a local server | Units positions agree with search and highlights; a model downloads, resumes after a cut, and speaks in a test; corrupt and gated cases fail with plain messages |
| 2 | **Playback (4 days).** Audio engine, unit queue, highlighting on the page and in scroll mode, auto page turn, lock screen, background, interruptions, sleep timer, speed | Reads a whole chapter without a gap on the target phone; the highlight matches the voice within a sentence; survives a call and headphones unplugging |
| 3 | **Voices screen and Listen bar (4 days).** Screens in section 5; phone voice engine; per-language and per-book choice; skip front and back matter | The full flow in section 5 works from a fresh install, offline after downloading |
| 4 | **Add a model (3 days).** By Hugging Face id, checks, licence line, token flow; the 40m models | A compatible checkpoint added by id reads; a wrong one is refused with a reason |
| 5 | **Polish.** Word highlighting for the phone voice, pronunciation fixes, low power behaviour, accessibility, docs and review notes | Review by an independent reader; size and launch budgets held |

Each phase ends with tests, a recording on a real phone, an independent review of the diff, and a TestFlight build.

## 8. Risks and decisions

1. **CPU speed and heat on real phones (the main unknown).** If the real-time factor is under about 1.5 on older phones, reading will stutter:
   then the phone voice is the default for those, or route B is built for 31m. Phase 0 decides with numbers.
2. **The 40m licence** (section 3): list by name, hide behind Add, or ask Gradium first.
3. **Voices of 31m on 40m models:** unverified.
4. **Another binary dependency risk:** `ptts` is young (0.4.0); pin its version and keep the Rust code that calls it in one small module.
5. **Untrusted model files** (Add a model): weights are parsed by the runtime; a hostile file could crash it. Mitigation: only the Phonon/Pocket family,
   size caps, parse in a worker so a crash cannot take the reader's data (the reader is not in that thread), and say "only add models you trust".
6. **Mixed-language books:** one model per book at a time; a book that switches language is read badly by a single-language model. The
   multilingual model helps; per-paragraph language detection is later.
7. **Tone across sentences:** the model splits long text into sentences, so prosody resets; reading in paragraph-sized chunks, not sentences, is a tuning
   knob for phase 2 (with the cost of a longer wait for the first sound).
8. **Disclosure:** the Listen bar says "Synthetic voice" in the voice chip; no cloning is offered.

Decisions for the owner: (a) the 40m models: list by name, or only after Add a model; and whether to write to Gradium; (b) skip front and back matter
by default: yes (plan) or no; (c) Wi-Fi only for model downloads by default: yes (plan); (d) route B (Core ML) later only if the spike
says so: yes (plan); (e) per-book voice in phase 3 or later.

## 9. Sources

- Gradium, Phonon: <https://gradium.ai/phonon>; evaluation: <https://gradium.ai/blog/evaluating-phonon>; announcement thread: <https://www.reddit.com/r/LocalLLaMA/comments/1xaw2vt/phonon_is_now_opensource_31m_and_40m_tts_models/>
- Code: <https://github.com/gradium-ai/xn-ptts> (Rust workspace, `ptts`, FFI, Swift package under `ios/`, Core ML export under `ptts-coreml/`); crate: <https://docs.rs/crate/ptts/latest>
- Model cards: <https://huggingface.co/gradium/phonon-31m-en> (Apache 2.0), <https://huggingface.co/gradium/phonon-40m-multilingual> (CC BY-NC 4.0, gated), voices: <https://huggingface.co/gradium/phonon-voices> (Apache 2.0, 102 voices)
- Gated models: <https://huggingface.co/docs/hub/models-gated>
- Comparators: Kokoro (82M, Apache 2.0, Core ML build about 80 MB INT8) and Kitten TTS (15M to 80M, Apache 2.0) from <https://offlinetts.com/blog/browser-tts-showdown-kokoro-piper-kitten/> and <https://soniqo.audio/guides/kokoro>; Piper licence differs between sources (MIT or GPL-3.0): check before ever using it.
- iOS speech: the word callback of `AVSpeechSynthesizer` and Personal Voice: <https://useyourloaf.com/blog/synthesized-speech-from-text/>, <https://bendodson.com/weblog/2024/04/03/using-your-personal-voice-in-an-ios-app/>
- What readers say (Reddit): r/TextToSpeech "Still nothing like Voice Aloud Reader for iOS?", "A free assisted reading/readaloud app that runs Kokoro", r/ereader "Does anyone here regularly use text-to-speech", r/roastmystartup "on-device TTS so you can read…"
- Measured here: the `ptts` crate built for `aarch64-apple-ios` (release, size-optimised) is a 12.3 MB static library that links to a 2.6 MB binary after dead-stripping.

## 10. Adding models later without a release (not Phonon-specific)

Asked for: when a new small model comes out, the owner (or a person) can add it to the app **without shipping a new app version**, the way a manga
source is added from a repository. Researched on 2026-10-24; this section changes sections 5.2 and 6 as noted at its end.

### 10.1 What can and cannot be added without a release

A model is two things: **weights and files** (data) and **the code that knows how to run that kind of model** (the runtime). Only the first
can arrive after release. App Store rule 2.5.2 forbids an app to "download, install, or execute code which introduces or changes features or
functionality"; weights, voice files, tokenizers and a declarative description are data, and are what every on-device AI app (speech,
vision, language) downloads after install. So the design is the same one the manga sources already use (declarative JSON, checked, no
code), with one rule: **a new model in a runtime family the app already has needs no release; a new family of architecture needs one.**
No downloaded scripts, no downloaded plug-in code, no WebAssembly plug-ins (the last would be code under rule 2.5.2).

### 10.2 The small-TTS landscape (what the "families" would be)

| Family | Examples | Runs on | Licence of weights | Needs a G2P/phonemizer? | Fit |
| --- | --- | --- | --- | --- | --- |
| **ptts family** (flow-matching on Mimi codec) | Phonon 31m/40m (Gradium); Pocket TTS 100M and the new 18.2M, 6.9M, 1.9M models (Kyutai, released 2026-10-20, MIT code); community Pocket-TTS checkpoints | the Rust `ptts` crate (CPU), Core ML ports exist | Apache (31m), CC BY-NC (40m), Kyutai's own terms; ports credit CC BY 4.0 | No: a SentencePiece/HF tokenizer plus per-language text rules | **First family.** One runtime already covers Gradium's and Kyutai's lines, so the likeliest next releases fit |
| **Single-pass Core ML** | Kokoro-82M (Apache 2.0; about 80 MB INT8; 54 voices; the highest-rated small model in the comparisons), Core ML ports by FluidAudio, soniqo | Core ML (Neural Engine), **no binary added to the app** (the OS provides it) | Apache 2.0 | Kokoro needs one: its Apache G2P ("misaki") has Swift ports; the other route, espeak-ng, is **GPL-3.0** | **Second family, only if wanted.** Cheapest runtime (zero size), best battery; needs a G2P we may ship |
| **ONNX graphs** | Kitten TTS (15M to 80M, Apache 2.0), Piper/VITS voices (900+), Supertonic (66M to 99M, ONNX, OpenRAIL-M), Matcha, ZipVoice | ONNX Runtime: full build about 7.5 MB (smaller with a reduced build; the stock iOS framework is far bigger), or `sherpa-onnx` which wraps many of these | Mixed: Apache, MIT, OpenRAIL-M; Piper is MIT or GPL by source | Piper, Kitten and sherpa's builds use **espeak-ng (GPL-3.0)**; Supertonic does not say | **Not a first-wave family.** Big runtime, GPL phonemizer risk, per-model glue |
| **MLX** | `mlx-audio-swift` models (Kokoro, Qwen3-TTS and others) | Metal GPU via MLX Swift | per model | per model | Fast on new phones, larger models; later |

The GPL point matters: espeak-ng is GPL-3.0 and the Apache App Store terms and GPL do not combine; a model that **requires** it cannot
be run by the app unless a permissive phonemizer replaces it. So the rule for what a "recipe" may use: tokenizers (data), lexicons (data) and
G2P rules that are permissively licensed. This quietly selects the right families: ptts (no phonemizer) and Kokoro (misaki).

### 10.3 The design: voice repositories and model definitions

Reuse what exists. The manga side already has **repositories**: a folder on the web with a signed `index.json` that lists files with their version
and SHA-256, a trust screen on first add, a pinned key afterwards, and "min app version" per entry (`desktop/manga/src/repo.rs`). A voice
repository is the same thing with a new kind of entry:

```
index.json            { "schema": 1, "name": "...", "key": "<ed25519>", "voices": [ {id, name, version, path, sha256, family, languages, min_app, bytes, licence} ] }
voices/<id>.json      the model definition (below)
```

A **model definition** is data only:

```json
{
  "schema": 1, "id": "phonon-31m-en", "name": "Phonon 31m", "version": 3, "min_app": "1.4",
  "family": "ptts",                         // which runtime built into the app runs it
  "languages": ["en"], "sample_rate": 24000,
  "licence": { "name": "Apache-2.0", "url": "https://...", "commercial": true, "gated": false },
  "files": [ { "path": "model.q8.gguf", "url": "hf://gradium/phonon-31m-en@<commit>/model.q8.gguf", "sha256": "...", "bytes": 67633152 },
             { "path": "tokenizer.json", "url": "...", "sha256": "..." }, { "path": "config.json", "url": "...", "sha256": "..." } ],
  "voices": { "catalog": "hf://gradium/phonon-voices@<commit>/voices.jsonl", "file": "voices/{id}.safetensors", "default": "default-voice.safetensors" },
  "options": { "lang": "en", "quant": "q8" }
}
```

- **`family`** is a closed list the app knows (`ptts` first). An unknown family, or a `min_app` above this app's version, is shown in the list as
  "needs a newer version of Akidev" (the same wording the manga sources use), never failed silently.
- **`url`** may be any https address or `hf://owner/name@commit/path` (Hugging Face, pinned to a commit so files cannot change under the
  definition; gated ones use the person's token). Every file is checked by SHA-256 and size before use.
- **Three ways to add a model**, from least to most open:
  1. **A repository** (paste an address, or open an `akimanga://add-voices?url=` link): the list of models in it, install one by one.
     The owner (or Gradium, or anyone) can publish a new model by adding a file to the repository: **no app release**.
  2. **A Hugging Face id** (`owner/name@revision`): the app lists the files, and if they look like a known family (for `ptts`: config.json,
     tokenizer, weights, a voice) it **writes the definition itself** and shows it for confirmation. This is how a model nobody has put in a
     repository yet gets in.
  3. **A definition file** from Files (a `.json`), for a model the person tested.
- **A check before it is accepted:** after download the app loads the model and speaks one fixed sentence in the background. If it cannot, it is
  removed with the reason (wrong family, wrong tokenizer, file damaged). A model is never "ready" without that pass.
- **What the person sees** is the same list in 5.2: the repository's entries appear under "Available" with size, languages, licence and "Get";
  a model's licence line is shown from its definition, with the non-commercial and gated warnings built from `licence.commercial` and
  `licence.gated`, so a new restrictive model warns by itself.
- **Trust:** a repository is trusted by the person who adds it, the app says "A voice model is files that run on your phone's processor
  to make speech. Only add repositories you trust" (the wording the manga repositories already use), signed repositories cannot change
  hands unnoticed. Model files are data parsed by the runtime: size caps, parsing on a worker, and only the families the app knows. A
  hostile model can at worst crash that worker or sound wrong, not run code.
- **Owner's own repository.** The owner publishes `dishant0406/voice-models` (or adds `voices/` to the existing sources repository) with Phonon 31m, the 40m entries marked
  non-commercial and gated, and anything new later. Adding a model there is a commit and the app's "Check for updates" picks it up.

### 10.4 What a new *family* costs (so the answer to "what needs a release" is concrete)

| New thing | Release needed? | Work |
| --- | --- | --- |
| A new Phonon or Pocket TTS checkpoint, any size, any language, new voices | **No** | A definition file |
| A new voice for an installed model | **No** | A line in the catalogue |
| A **Core ML single-pass** model with the Kokoro-like recipe (token ids and a style vector in, waveform out) | **No, once the family exists** | The family is built once (about a week: the Core ML loader, the recipe, the G2P); each model after is a definition naming its inputs and outputs |
| A model with a **new architecture** (different loop, different codec) | **Yes** | A new family in the engine |
| An ONNX model | **Yes, once** (a runtime) | Adding an ONNX runtime is a decision about size (several MB) and the phonemizer licence, not about glue |

### 10.5 Changes to the rest of this plan

- **Section 5.2 (Voices screen):** the "Phonon" group becomes **Available** (from the repositories the person added, with the owner's repository added by
  default but removable, like nothing is built in for manga) and **Installed**; "Add a voice model" has the three ways above. Phonon is simply the first entries.
- **Section 6 (architecture):** `voice_model` gains `family`, `definition` (the JSON) and `repository`; the Rust `voice` module has one trait
  `Family { open(folder, definition) -> Speaker }` with `ptts` the only implementation; the registry reads definitions, not names.
  The downloader is shared with the manga repositories' fetch-and-verify code.
- **Phases (section 7):** phase 1 adds the definition schema, repository kind and the HF-id writer; phase 4 ("Add a model") becomes "repositories and HF ids";
  a phase 6 (optional) is the Core ML single-pass family.
- **New decisions for the owner:** (f) a separate voices repository, or `voices/` inside the existing sources repository; (g) whether
  the app ships with the owner's repository added (the manga side ships with none, by design, for review reasons; the same stance is the safe default here:
  the first Voices screen offers Phone voices and one button "Add the Akidev voice list"); (h) whether to support the Core ML single-pass family (Kokoro) in the first release or later.

### 10.6 Sources for this section

App Store Review Guidelines 2.5.1, 2.5.2, 4.2.3(ii), 4.7: <https://developer.apple.com/app-store/review/guidelines/>. espeak-ng licence (GPL-3.0): <https://github.com/espeak-ng/espeak-ng/blob/master/COPYING>;
the same issue for Kokoro: <https://github.com/hexgrad/kokoro/issues/247>. sherpa-onnx families (VITS/Piper, Matcha, Kokoro, Kitten, Pocket, Supertonic, ZipVoice): <https://k2-fsa.github.io/sherpa/onnx/tts/index.html>;
its iOS runtime size question: <https://github.com/k2-fsa/sherpa-onnx/issues/2643>. ONNX Runtime size: <https://onnxruntime.ai/docs/build/custom.html>, <https://github.com/microsoft/onnxruntime/discussions/6551>.
Supertonic (ONNX, OpenRAIL-M): <https://huggingface.co/Supertone/supertonic-3>. Pocket TTS: <https://github.com/kyutai-labs/pocket-tts>, <https://kyutai.org/blog/2026-10-20-pocket-tts-small/>, Core ML and ONNX bundles: <https://soniqo.audio/guides/pocket-tts>.
Kokoro Core ML: <https://huggingface.co/mattmireles/kokoro-coreml>, FluidAudio: <https://swiftpackageindex.com/FluidInference/FluidAudio>. MLX Audio: <https://github.com/Blaizzy/mlx-audio>.
The repository format reused: `desktop/manga/src/repo.rs`.

## 11. What was built (2026-10-25)

The owner chose: the models live in **their own repository** (`dishant0406/akidev-voices`, local copy `~/projects/akidev-voices`, Phonon 31M first),
and **Settings checks the repository when it is opened, not at launch, and shows Reading aloud and the model list only if it answered**.
This follows section 10 and replaces parts of sections 5 to 7:

- **Repository and definitions:** as in 10.3, with the index extended (`repo.rs`: `voices` next to `sources` and `themes`; the same
  `akidev-manga-cli index` writes it). A definition names its files (url, SHA-256, size), its `family` and its licence. Four files make
  Phonon 31M (62 MB): config, tokenizer, weights (q8), default voice.
- **Engine** (`desktop/manga/src/voice.rs`): downloads into `<id>.part`, checks size and hash as the bytes arrive, loads the model and
  makes it say one sentence, and only then gives the folder its name; a failure leaves nothing. https only (and this machine for tests);
  redirects only to https. Speaking is the `ptts` crate (0.4.0, pinned) on the CPU. The folder is the registry: no database table.
- **Sentences** (`desktop/manga/src/book/speech.rs`): UAX 29 sentence breaks, with abbreviations (Mr., Dr., St.) kept in one; footnote marks and
  soft hyphens not read; each sentence has its UTF-16 place in the chapter, so highlighting needs no mapping.
- **Playback** (`App/Reading/BookListener.swift`): speech is made three sentences ahead off the main thread and queued on an `AVAudioPlayerNode`; speed
  is `AVAudioUnitTimePitch` (pitch holds); a heading is followed by a pause; background audio, lock screen and headphone controls, pause on a call or
  unplugging. The voice is let go when reading stops or after three minutes paused.
- **Not built:** the phone voice (`AVSpeechSynthesizer`) is dropped: with the gate, reading aloud exists only where a model does. Other voices of a model
  (the 102 in `gradium/phonon-voices`), a sleep timer, adding a model by Hugging Face id, Phonon 40M (not listed: CC BY-NC and gated), a second family (Kokoro).

Measured on a Mac (M-series, CPU): load 45 ms, first sound 40 ms, peak memory 372 MB; 19.6 s of speech took 1.13 s in a plain release build (17 times faster
than real time). A phone has not been measured yet. Under the iPhone profile (`opt-level` "z", fat LTO) the same text speaks 6 times faster than real time, "s" 12 times
and 3 17 times, and a Release binary built the same way as before is 18.3 MB without speech, 24.5 MB with it at 3 and 26.0 MB at "s": so the speech crates
are built at 3 (`[profile.ios.package.*]` in `desktop/Cargo.toml`). The +6.2 MB (uncompressed, unstripped) is over the 3.5 MB this plan allowed; most of it is `ptts`, its
tokenizer library and the matrix kernels for every number type. Launch cost: `AVFAudio` is already loaded by SwiftUI (0.1 ms); `MediaPlayer`, for the lock screen,
adds about 22 ms in the simulator (PhotosUI measured 1093 ms the same way).
