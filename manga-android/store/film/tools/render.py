# Renders the film to frames, in parallel worker processes, then encodes with ffmpeg.
# Usage: python render.py URL OUT.mp4 --fps 30 --scale 0.5 --sub 1 --workers 6
# --sub N: N captures per frame spread over the frame's time, averaged (motion blur, like ffmpeg tmix).
import argparse, asyncio, io, os, subprocess, sys, tempfile, multiprocessing as mp
import numpy as np
from PIL import Image

def work(args):
    url, fps, scale, sub, frames, outdir = args
    from playwright.async_api import async_playwright
    async def run():
        async with async_playwright() as p:
            b = await p.chromium.launch()
            pg = await b.new_page(viewport={'width': 1440, 'height': 1440}, device_scale_factor=scale)
            await pg.goto(url); await pg.evaluate('window.__ready')
            for f in frames:
                acc = None
                for k in range(sub):
                    t = (f + (k + 0.5) / sub - 0.5 * (sub > 1)) / fps if sub > 1 else f / fps
                    t = max(0.0, t)
                    await pg.evaluate(f'window.seek({t})')
                    img = np.asarray(Image.open(io.BytesIO(await pg.locator('#frame').screenshot(type='png'))).convert('RGB'), dtype=np.float32)
                    lin = (img / 255.0) ** 2.2
                    acc = lin if acc is None else acc + lin
                out = ((acc / sub) ** (1 / 2.2) * 255 + 0.5).clip(0, 255).astype(np.uint8)
                Image.fromarray(out).save(f'{outdir}/f{f:05d}.png')
            await b.close()
    asyncio.run(run())

if __name__ == '__main__':
    ap = argparse.ArgumentParser()
    ap.add_argument('url'); ap.add_argument('out')
    ap.add_argument('--fps', type=int, default=30); ap.add_argument('--scale', type=float, default=0.5)
    ap.add_argument('--sub', type=int, default=1); ap.add_argument('--workers', type=int, default=6)
    ap.add_argument('--seconds', type=float, default=32.0); ap.add_argument('--frames-dir')
    a = ap.parse_args()
    n = int(round(a.seconds * a.fps))
    outdir = a.frames_dir or tempfile.mkdtemp()
    os.makedirs(outdir, exist_ok=True)
    todo = [f for f in range(n) if not os.path.exists(f'{outdir}/f{f:05d}.png')]
    chunks = [todo[i::a.workers] for i in range(a.workers)]
    with mp.Pool(a.workers) as pool:
        pool.map(work, [(a.url, a.fps, a.scale, a.sub, c, outdir) for c in chunks if c])
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-framerate', str(a.fps), '-i', f'{outdir}/f%05d.png',
                    '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '16', '-preset', 'medium', a.out], check=True)
    print('wrote', a.out, n, 'frames')
