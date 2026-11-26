# The film's score, composed on the film's own beat grid (129 BPM, 128 beats). Drums and FX are ElevenLabs one-shots,
# chords, bass, arpeggio and lead are played as notes here, and every UI cue from the film (window.__events()) is part
# of the rhythm: each lands on a beat or an eighth note, and the pops play the current chord's notes.
# Usage: python score.py URL OUT.wav
import asyncio, json, subprocess, sys
import numpy as np, librosa
from scipy.signal import butter, sosfilt, fftconvolve
from playwright.async_api import async_playwright

SR = 48000
BPM = 129; B = 60 / BPM; BEATS = 128
N = int(round(BEATS * B * SR))
IN = '/Users/dishants/projects/akidev/content/manga/film/inputs'
rng = np.random.default_rng(7)
at = lambda beat: int(round(beat * B * SR))

def load(path):
    raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', path, '-f', 'f32le', '-ac', '2', '-ar', str(SR), '-'], capture_output=True, check=True).stdout
    a = np.frombuffer(raw, dtype=np.float32).reshape(-1, 2).copy()
    return a / (np.abs(a).max() + 1e-9)

def place(bus, sample, beat, gain=1.0, align_peak=False):
    start = at(beat) - (int(np.argmax(np.abs(sample).max(axis=1))) if align_peak else 0)
    lo, hi = max(0, start), min(len(bus), start + len(sample))
    if hi > lo: bus[lo:hi] += sample[lo - start:hi - start] * gain

def lp(x, hz):
    return sosfilt(butter(2, min(hz, SR / 2 - 100), 'low', fs=SR, output='sos'), x, axis=0)

def hp(x, hz):
    return sosfilt(butter(2, hz, 'high', fs=SR, output='sos'), x, axis=0)

freq = lambda m: 440 * 2 ** ((m - 69) / 12)

def saw(f, n, phase=0.0):
    t = np.arange(n) / SR
    return 2 * ((f * t + phase) % 1.0) - 1

def supersaw(m, n, voices=7, spread=0.18):
    out = np.zeros((n, 2))
    for v in range(voices):
        d = (v - (voices - 1) / 2) / ((voices - 1) / 2) * spread
        s = saw(freq(m + d), n, rng.random())
        pan = 0.5 + 0.45 * d / spread
        out[:, 0] += s * (1 - pan); out[:, 1] += s * pan
    return out / voices

def env(n, a=0.005, d=0.1, s=0.7, r=0.08):
    t = np.arange(n) / SR; e = np.ones(n) * s
    e[t < a] = t[t < a] / a
    m = (t >= a) & (t < a + d); e[m] = 1 - (1 - s) * (t[m] - a) / d
    tail = int(r * SR); e[-tail:] *= np.linspace(1, 0, tail)
    return e[:, None]

def swish(secs):
    # a small, soft swish: filtered air that swells in and fades out
    n = int(secs * SR); t = np.linspace(0, 1, n)
    air = sosfilt(butter(2, [800, 6000], 'band', fs=SR, output='sos'), rng.standard_normal((n, 2)), axis=0)
    air *= (np.sin(np.pi * t ** 0.6) ** 2)[:, None]
    return air / np.abs(air).max()

def reverb(x, secs=1.8, wet=0.25):
    n = int(secs * SR); t = np.arange(n) / SR
    ir = rng.standard_normal((n, 2)) * np.exp(-t / (secs / 5))[:, None]
    ir = lp(ir, 6000); ir /= np.abs(ir).sum(axis=0) ** 0.5 * 4
    y = np.stack([fftconvolve(x[:, c], ir[:, c])[:len(x)] for c in range(2)], axis=1)
    return x + wet * y

# ---------- harmony: Am F C G, one chord per bar (4 beats) ----------
CHORDS = [[57, 60, 64, 69], [53, 57, 60, 65], [55, 60, 64, 67], [55, 59, 62, 67]]   # Am F C G
ROOTS = [33, 29, 36, 31]                                                              # A1 F1 C2 G1
chord_at = lambda beat: int(beat // 4) % 4
# sections (film beats)
INTRO, BUILD, DROP1, MAIN, BREAK, DROP2, END = 0, 16, 32, 64, 96, 112, 122
full = lambda b: DROP1 <= b < BREAK or DROP2 <= b < END

# ---------- the film's cues ----------
async def cues(url):
    async with async_playwright() as p:
        br = await p.chromium.launch(); pg = await br.new_page()
        await pg.goto(url); await pg.evaluate('window.__ready')
        e = await pg.evaluate('window.__events()'); await br.close(); return e

url, out = sys.argv[1], sys.argv[2]
EV = asyncio.run(cues(url))

drums = np.zeros((N + SR * 4, 2)); music = np.zeros_like(drums); fx = np.zeros_like(drums); ui = np.zeros_like(drums)
K = {k: load(f'{IN}/kit/{k}.mp3') for k in ['kick', 'clap', 'hat', 'ohat', 'crash', 'impact', 'revcrash']}
K['clap'] = hp(K['clap'], 180)

# ---------- drums ----------
kicks = []
for b in range(BEATS):
    if full(b):
        place(drums, K['kick'], b, 1.0); kicks.append(b)
        if b % 2 == 1: place(drums, K['clap'], b, 0.55)
        place(drums, K['ohat'], b + 0.5, 0.16)
        place(drums, K['hat'], b, 0.10); place(drums, K['hat'], b + 0.25, 0.06); place(drums, K['hat'], b + 0.75, 0.06)
    elif BUILD <= b < DROP1 - 1:
        # the build: hats come in, then claps; a roll in the last bar, then one silent beat before the drop
        place(drums, K['hat'], b + 0.5, 0.12)
        if b >= 20 and b % 2 == 1: place(drums, K['clap'], b, 0.4)
        if b >= 24:
            step = 0.5 if b < 28 else 0.25
            for k in np.arange(0, 1, step): place(drums, K['clap'], b + k, 0.18 + 0.4 * (b - 24) / 8)
    elif BREAK <= b < DROP2 - 1 and b >= 108:
        for k in np.arange(0, 1, 0.25): place(drums, K['clap'], b + k, 0.15 + 0.35 * (b - 108) / 4)
    elif b < BUILD and b % 2 == 0:
        place(drums, K['kick'], b, 0.45)   # a soft pulse under the intro
        place(drums, K['hat'], b + 0.5, 0.08)
for b in (DROP1, 48, 64, 80, DROP2):
    place(fx, K['crash'], b, 0.45)
place(fx, K['impact'], DROP1, 0.9); place(fx, K['impact'], DROP2, 0.9); place(fx, K['impact'], END, 1.0)
# into each drop: a small swish that ends on the hit (in place of the long riser), then the reverse crash
for b in (DROP1, DROP2):
    place(fx, swish(0.5), b - 0.5 / B, 0.15)
place(fx, K['revcrash'], DROP1 - 4, 0.4); place(fx, K['revcrash'], DROP2 - 4, 0.4); place(fx, K['revcrash'], END - 4, 0.4)

# ---------- chords, bass, arpeggio, lead ----------
pad = np.zeros_like(music); stab = np.zeros_like(music); bass = np.zeros_like(music); arp = np.zeros_like(music); lead = np.zeros_like(music)
for bar in range(32):
    b0 = bar * 4; ch = CHORDS[bar % 4]; n = at(4) + int(0.3 * SR)
    if b0 >= END: break
    if not full(b0):
        p = sum(supersaw(m, n, 5, 0.12) for m in ch) / 4 * env(n, 0.25, 0.3, 0.8, 0.4)
        place(pad, p, b0, 0.9 if b0 < DROP1 else 0.6)
    else:
        p = sum(supersaw(m + 12 * (m < 60), n, 7, 0.2) for m in ch) / 4 * env(n, 0.01, 0.2, 0.85, 0.15)
        place(stab, p, b0, 0.55)
        t = np.arange(n) / SR; sub = np.sin(2 * np.pi * freq(ROOTS[bar % 4] + 12) * t) + 0.3 * np.sin(2 * np.pi * freq(ROOTS[bar % 4] + 24) * t)
        place(bass, np.tanh(1.6 * sub)[:, None].repeat(2, 1) * env(n, 0.005, 0.1, 0.9, 0.1), b0, 0.6)
    # the arpeggio: 16ths of the chord, from the build on (filtered in the build, open in the drops)
    if b0 < END:
        tones = ch + [ch[1] + 12, ch[2] + 12, ch[3] + 12, ch[2] + 12]
        for k in range(16):
            m = tones[k % len(tones)] + 12; nn = int(0.2 * SR)
            note = saw(freq(m), nn)[:, None].repeat(2, 1) * env(nn, 0.002, 0.08, 0.15, 0.05)
            place(arp, note, b0 + k * 0.25, 0.22 if full(b0) else 0.2)
# the lead: a short hook in the drops
HOOK = [(0, 76), (0.5, 76), (1, 74), (1.5, 72), (2.5, 74), (3, 76)]
for bar in range(32):
    b0 = bar * 4
    if full(b0) and bar % 2 == 0:
        shift = [0, -4, 3, -2][(bar // 2) % 4]
        for off, m in HOOK:
            nn = int(0.42 * B * SR * 2)
            note = supersaw(m + shift, nn, 3, 0.08) * env(nn, 0.004, 0.12, 0.5, 0.08)
            place(lead, note, b0 + off, 0.28)

# filters: the intro and build open up; the breakdown is soft
tt = np.arange(len(music)) / SR / B   # in beats
def sweep(x, lo_hz, hi_hz, b0, b1):
    y = x.copy()
    for k in range(int(b0), int(b1)):
        hz = lo_hz * (hi_hz / lo_hz) ** ((k - b0) / max(1, b1 - b0))
        s, e = at(k), at(k + 1)
        y[s:e] = lp(x[s:e], hz)
    return y
arp = sweep(arp, 500, 6000, 0, DROP1)
arp = np.where((tt >= BREAK) & (tt < DROP2), 1, 0)[:, None] * lp(arp, 1500) + np.where((tt < BREAK) | (tt >= DROP2), 1, 0)[:, None] * arp
pad = lp(pad, 2200)
lead = lp(lead, 5000)

# sidechain: the chords and bass pump with the kick
duck = np.ones(len(music))
for b in kicks:
    s = at(b); L = int(0.42 * B * SR * 2); t = np.arange(L) / SR
    duck[s:s + L] = np.minimum(duck[s:s + L], 1 - 0.7 * np.exp(-t / 0.09))
music = (stab + bass + pad) * duck[:, None] + arp * (0.6 + 0.4 * duck[:, None]) + lead * duck[:, None]
music = reverb(music, 1.6, 0.18)

# ---------- the app's sounds are part of the beat ----------
UI = {k: load(f'{IN}/eleven/{k}.mp3') for k in ['click', 'select', 'pop', 'badge', 'hold', 'push', 'flood', 'shutter', 'tick']}

PITCHED = {'pop', 'select', 'tick', 'badge'}
f0 = {}
for k in PITCHED:
    y = UI[k][:, 0]
    p, v, _ = librosa.pyin(y, fmin=150, fmax=2000, sr=SR, frame_length=2048)
    f0[k] = float(np.nanmedian(p[v])) if np.any(v) else None
count = 0
for e in EV:
    if e['name'] == 'sheet':
        # a small swish right on the sheet's move, not on the grid
        place(ui, swish(0.25), (e['t'] - 0.05) / B, 0.15)
        continue
    beat = round(e['t'] / B * 2) / 2 if e['name'] in ('click', 'hold', 'shutter', 'flood', 'push') else round(e['t'] / B * 4) / 4
    s = UI[e['name']]
    if e['name'] in PITCHED and f0[e['name']]:
        tone = CHORDS[chord_at(beat)][count % 4] + 24; count += 1
        steps = 12 * np.log2(freq(tone) / f0[e['name']])
        steps = ((steps + 6) % 12) - 6      # nearest octave
        s = np.stack([librosa.effects.pitch_shift(s[:, c], sr=SR, n_steps=steps) for c in range(2)], axis=1)
    place(ui, s, beat, 10 ** (e['db'] / 20) * 0.9, align_peak=True)

# ---------- mix and master ----------
mix = drums * 0.9 + music * 0.8 + fx * 0.7 + ui * 0.8
mix = mix[:N]
# the breakdown is quieter; the end rings out after the last hit
mix *= np.where((tt[:N] >= BREAK) & (tt[:N] < DROP2), 0.75, 1.0)[:, None]
fade = int(0.35 * SR); mix[-fade:] *= np.linspace(1, 0, fade)[:, None]
mix = np.tanh(mix * 1.2) / 1.2         # gentle glue
tmp = out + '.raw.wav'
subprocess.run(['ffmpeg', '-v', 'error', '-y', '-f', 'f32le', '-ac', '2', '-ar', str(SR), '-i', '-', tmp], input=mix.astype(np.float32).tobytes(), check=True)
m = subprocess.run(['ffmpeg', '-hide_banner', '-i', tmp, '-af', 'loudnorm=I=-14:TP=-1.5:LRA=11:print_format=json', '-f', 'null', '-'], capture_output=True, text=True).stderr
j = json.loads(m[m.rindex('{'):m.rindex('}') + 1])
f = (f"loudnorm=I=-14:TP=-1.5:LRA=11:measured_I={j['input_i']}:measured_TP={j['input_tp']}:measured_LRA={j['input_lra']}"
     f":measured_thresh={j['input_thresh']}:offset={j['target_offset']}:linear=true")
subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', tmp, '-af', f, '-ar', str(SR), out], check=True)
print('wrote', out, f'{N / SR:.3f}s', len(EV), 'cues;', 'pitched sounds:', {k: round(v) if v else None for k, v in f0.items()})
