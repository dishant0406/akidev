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
