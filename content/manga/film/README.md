# Akidev launch film

A 59.5 second square film (1440×1440, 60 fps) for the Android and iPhone app. It is one continuous take: every
scene grows out of the one before, and a cursor drives every change. The app is drawn in its dark theme, from its
own tokens, as a phone-width column the camera moves over. The beat map is [BEAT_MAP.md](BEAT_MAP.md).

## What is here

- `film.html` is the whole film. Everything on screen is a function of time: `seek(t)` sets every style, nothing
  animates on its own. It has to be served over http, not opened as a file (see below).
- `tools/` has the scripts that make the assets, the music and the video.
- `stills/` has the review stills from the first round (light theme, 32 s version).
- `inputs/` (gitignored) has the pictures and sounds. `out/` (gitignored) has the rendered videos.

## How the timing works

Basically there are two clocks. The scenes are written in "story beats", the 64 beats of the beat map. The film runs
on the music's clock: 129 BPM, 128 beats. One story beat is exactly two film beats (`WARP` in `film.html`), so every
tap, pop and push lands on a beat or an eighth note of the music. The drop is film beat 32 (0:14.9, the Install zoom),
the breakdown is film beats 96-112 (the Night page and read aloud), the final drop is film beat 112 (the iPhone).

## The music

The score is made in `tools/score.py`, on the film's own grid, so it can't drift from the picture. Drums and effects
are one-shots generated with ElevenLabs (`tools/kit.py`). Chords (Am F C G), bass, arpeggio and lead are played as
notes in the script. The app's own sounds (taps, pops, pushes, the iris shutter) are part of the rhythm, and the pops
play the notes of the chord under them. It reads the film's cue list (`SOUND` in `film.html`, exposed as
`window.__events()`), so moving a cue in the film moves its sound too. It is mastered to -14 LUFS.

We tried two other ways first. A Mixkit song (Digital Clouds) felt too light. A generated ElevenLabs track had the
right sections, but its notes sat about 114 ms off the film's grid on average, which is the same as random, so
nothing on screen ever hit with the music.

## How to rebuild

Serve the repository root (any static server; the film reads fonts, icons and inputs by relative path):

```bash
python3 -m http.server 8000 --directory /Users/dishants/projects/akidev
```

The scripts need Python with `playwright` (Chromium), `numpy`, `pillow`, `scipy` and `librosa`, plus `ffmpeg`.

```bash
python tools/stills.py http://127.0.0.1:8000/content/manga/film/film.html out/stills 14.9 44.7
```

```bash
python tools/render.py http://127.0.0.1:8000/content/manga/film/film.html out/video.mp4 --fps 60 --scale 1 --sub 4 --workers 12 --seconds 59.5349 --frames-dir out/frames
```

```bash
python tools/score.py http://127.0.0.1:8000/content/manga/film/film.html out/score.wav
```

```bash
ffmpeg -i out/video.mp4 -i out/score.wav -c:v copy -c:a aac -b:a 256k -shortest out/akidev-launch.mp4
```

`tools/web-assets.py` takes the clean screens and the recording for the website's `/app` page from this film (see `docs/flow.md`).

`--sub 4` is the motion blur: four captures spread over each frame, averaged in linear light. `tools/pops.py` scans
the frames for one-frame jumps (a frame that changes 3× more than both neighbours).

## Inputs and where they came from

All of these are originals or public domain. Nothing from a real manga, series or source appears in the film.

- Covers, the webtoon strip and the spread: generated with Seedream 5.0 Flash on OpenRouter (`tools/covers.py`), with
  the bubble lettering added in Nunito. The source photo: `tools/gen_one.py`.
- The book: Alice's Adventures in Wonderland, Project Gutenberg (public domain), from its official mirror.
- Drums, effects and UI sounds: generated with ElevenLabs (`tools/kit.py`, and the UI set in the same way). The small swish (on sheets and into each drop) is made in `tools/score.py`.
- Fonts: Nunito and Lora (SIL OFL), from the Android app. Icons: the Solar set by 480 Design (CC BY 4.0), from the
  iPhone app. The credits must carry the Solar line.

`inputs/music/` and `inputs/sfx/` still hold the Mixkit files from the first try; the film no longer uses them.

## Choices worth knowing

- The made-up names (the covers' titles, "Your amazing source", "Mira's phone", the "Weekend" category) are marked
  (film) in the beat map. Every other string is real app copy or the Play listing.
- The book starts on the Paper theme, not Auto: in a dark app, Auto would already be dark and the Night flood would
  show nothing.
- The close floods yellow, not black: black disappears on the dark canvas.
- Sheets float 8 pt in from the edges with every corner rounded, so nothing touches the edge of the screen.
- The app lives in an iPhone from beat 4 (it grows out of the pill) until the close flood covers it. The app's
  393 × 852 column is the safe area: the screen reaches 47 pt above it (status bar and island) and 20 pt below it
  (home indicator), so nothing of the app sits under them. Every app scene is in one layer clipped to the screen
  (`buildApp`, clip set in `setStage`).
