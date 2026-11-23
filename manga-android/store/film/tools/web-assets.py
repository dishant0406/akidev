# The pictures for the website's /app page, taken from the film: clean screens of the app (no cursor) and a screen
# recording, all with a fixed camera on the phone's screen. The art in them is the film's made-up art, not real series.
# Usage: python web-assets.py URL OUT_DIR   ->  OUT_DIR/screens/<name>.webp, OUT_DIR/app-loop.mp4, OUT_DIR/app-poster.webp
import io, multiprocessing as mp, os, subprocess, sys
from PIL import Image
from playwright.sync_api import sync_playwright

BEAT = 60 / 129 * 2                      # seconds in one story beat
SCREENS = {                              # name: story beat where the screen has settled
    'discover': 19.15, 'series': 23.7, 'reader': 27.0, 'library': 33.95, 'share': 39.7,
    'book': 44.95, 'themes': 48.95, 'night': 51.95, 'aloud': 55.6,
}
IMMERSIVE = {'book', 'themes', 'night'}   # the reading bars are hidden, as they are while you read
LOOP = (18.6, 30.95)                     # story beats of the recording: Discover, a series, the reader, back to Discover
FPS = 30
# The 393 x 852 column plus the status bar above it (47) and the home indicator below it (20), filling the 1440 frame.
SCREEN_PX = dict(x=412, y=0, width=616, height=1440)

def page(p, url, scale):
    b = p.chromium.launch()
    pg = b.new_page(viewport={'width': 1440, 'height': 1440}, device_scale_factor=scale)
    pg.goto(url); pg.evaluate('window.__ready')
    pg.add_style_tag(content='[data-r="cursor"],[data-r="ring"],[data-r="hold"]{display:none!important}')
    pg.add_style_tag(content='body.immersive [data-glass][data-r^="bk"]{opacity:0!important}')
    pg.evaluate('CAM.length = 0; CAM.push([0, CX, 412.5, 919]);')   # one camera key: the whole screen, always
    return b, pg

def shot(pg, beat):
    pg.evaluate(f'window.seek({beat * BEAT})')
    return Image.open(io.BytesIO(pg.screenshot(clip=SCREEN_PX, type='png'))).convert('RGB')

def stills(args):
    url, out = args
    with sync_playwright() as p:
        b, pg = page(p, url, 1.5)
        for name, beat in SCREENS.items():
            pg.evaluate(f"document.body.classList.toggle('immersive', {str(name in IMMERSIVE).lower()})")
            shot(pg, beat).save(f'{out}/screens/{name}.webp', quality=84, method=6)
        b.close()

def frames(args):
    url, out, numbers = args
    with sync_playwright() as p:
        b, pg = page(p, url, 1)
        for n in numbers:
            shot(pg, LOOP[0] + n / FPS / BEAT).resize((540, 1264), Image.LANCZOS).save(f'{out}/frames/f{n:04d}.png')
        b.close()

if __name__ == '__main__':
    url, out = sys.argv[1], sys.argv[2]
    os.makedirs(f'{out}/screens', exist_ok=True); os.makedirs(f'{out}/frames', exist_ok=True)
    count = int((LOOP[1] - LOOP[0]) * BEAT * FPS)
    jobs = [list(range(i, count, 11)) for i in range(11)]
    with mp.Pool(12) as pool:
        first = pool.apply_async(stills, [(url, out)])
        pool.map(frames, [(url, out, j) for j in jobs])
        first.get()
    # Loop it: the last half second dissolves into the first, so the video ends where it begins.
    fade = 0.6; length = count / FPS
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-framerate', str(FPS), '-i', f'{out}/frames/f%04d.png', '-filter_complex',
        f'[0]split=3[a][b][c];[a]trim={fade}:{length - fade},setpts=PTS-STARTPTS[body];'
        f'[b]trim={length - fade}:{length},setpts=PTS-STARTPTS[tail];[c]trim=0:{fade},setpts=PTS-STARTPTS[head];'
        f'[tail][head]xfade=transition=fade:duration={fade - 0.04}:offset=0[mix];[body][mix]concat=n=2:v=1[v]',
        '-map', '[v]', '-c:v', 'libx264', '-crf', '26', '-preset', 'slow', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', '-an',
        f'{out}/app-loop.mp4'], check=True)
    Image.open(f'{out}/frames/f{int(fade * FPS):04d}.png').save(f'{out}/app-poster.webp', quality=84, method=6)
    print('wrote', out)
