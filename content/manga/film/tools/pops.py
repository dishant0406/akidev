# Flags frames whose change from the previous frame is 3x bigger than both neighbours' changes (a one-frame pop).
import sys, glob
import numpy as np
from PIL import Image
files = sorted(glob.glob(sys.argv[1] + '/f*.png')); fps = float(sys.argv[2])
prev = None; d = []
for f in files:
    im = np.asarray(Image.open(f).convert('L').resize((240, 240)), dtype=np.float32)
    d.append(0 if prev is None else float(np.abs(im - prev).mean())); prev = im
for i in range(2, len(d) - 1):
    if d[i] > 1.0 and d[i] > 3 * max(d[i - 1], d[i + 1], 0.15):
        print(f'frame {i} (t={i / fps:.3f}s, beat {i / fps * 2:.2f}): diff {d[i]:.2f} vs {d[i-1]:.2f}/{d[i+1]:.2f}')
