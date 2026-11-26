# Generates the drum and FX one-shots for the score with ElevenLabs. Usage: python kit.py OUT_DIR
import json, os, sys, urllib.request
from concurrent.futures import ThreadPoolExecutor
OUT = sys.argv[1]; KEY = os.environ['ELEVENLABS_API_KEY']; B = 60 / 129
KIT = {
    'kick': ('punchy modern EDM kick drum one-shot, tight transient, deep sub thump, dry, no reverb', 0.6),
    'clap': ('crisp layered EDM clap snare one-shot, bright, tight, short room', 0.6),
    'hat': ('loud crisp closed hi-hat hit, drum machine, bright tick, one-shot', 0.5),
    'ohat': ('open hi-hat one-shot, bright sizzle, short decay', 0.6),
    'crash': ('bright crash cymbal one-shot, long shimmering decay', 2.5),
    'impact': ('huge cinematic sub drop impact, deep boom with a tail, launch trailer hit', 3.0),
    'riser': ('white noise riser sweep building tension, rising pitch, ends abruptly at the peak', round(8 * B, 2)),
    'revcrash': ('reverse crash cymbal swell rising into a hit, ends abruptly', round(4 * B, 2)),
}
def gen(item):
    k, (text, dur) = item
    body = json.dumps({'text': text, 'duration_seconds': max(0.5, dur), 'prompt_influence': 0.7}).encode()
    req = urllib.request.Request('https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_128', body, {'xi-api-key': KEY, 'Content-Type': 'application/json'})
    try:
        open(f'{OUT}/{k}.mp3', 'wb').write(urllib.request.urlopen(req, timeout=300).read()); return 'ok ' + k
    except urllib.error.HTTPError as e:
        return f'FAIL {k}: {e.code} {e.read()[:200]!r}'
with ThreadPoolExecutor(1) as ex:
    for r in ex.map(gen, [i for i in KIT.items() if not os.path.exists(f'{OUT}/{i[0]}.mp3')]): print(r)
