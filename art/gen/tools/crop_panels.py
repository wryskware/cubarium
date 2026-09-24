#!/usr/bin/env python3
"""Split a one-row sheet into per-view PNGs.

    crop_panels.py SHEET.png OUT_DIR [--n 5] [--even] [--thresh 18] [--pad 8]

Default: columns whose pixels all stay within --thresh of the background colour
(median of the image border) are gaps; foreground runs become panels, merged or split
to reach --n when given. --even: plain n-way grid. Writes <stem>-pNN.png, prints boxes.
"""
import argparse, pathlib
import numpy as np
from PIL import Image

def runs(mask):
    out, start = [], None
    for i, v in enumerate(list(mask) + [False]):
        if v and start is None: start = i
        if not v and start is not None: out.append([start, i]); start = None
    return out

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("sheet"); ap.add_argument("out")
    ap.add_argument("--n", type=int, default=0); ap.add_argument("--even", action="store_true")
    ap.add_argument("--thresh", type=float, default=18); ap.add_argument("--pad", type=int, default=8)
    a = ap.parse_args()
    im = Image.open(a.sheet).convert("RGB"); A = np.asarray(im).astype(float); H, W, _ = A.shape
    border = np.concatenate([A[0], A[-1], A[:, 0], A[:, -1]])
    bg = np.median(border, axis=0)
    fg = np.abs(A - bg).max(axis=2) > a.thresh
    if a.even:
        n = a.n or 5; cols = [[W * i // n, W * (i + 1) // n] for i in range(n)]
    else:
        colmask = fg.sum(axis=0) > max(2, H * 0.004)
        cols = [c for c in runs(colmask) if c[1] - c[0] > W * 0.01]
        while a.n and len(cols) > a.n:  # merge the pair with the smallest gap
            g = min(range(len(cols) - 1), key=lambda i: cols[i + 1][0] - cols[i][1])
            cols[g:g + 2] = [[cols[g][0], cols[g + 1][1]]]
        while a.n and len(cols) < a.n:  # split the widest run in half
            w = max(range(len(cols)), key=lambda i: cols[i][1] - cols[i][0])
            x0, x1 = cols[w]; m = (x0 + x1) // 2; cols[w:w + 1] = [[x0, m], [m, x1]]
    out = pathlib.Path(a.out); out.mkdir(parents=True, exist_ok=True)
    stem = pathlib.Path(a.sheet).stem
    for i, (x0, x1) in enumerate(cols, 1):
        rows = np.where(fg[:, x0:x1].any(axis=1))[0]
        y0, y1 = (rows[0], rows[-1] + 1) if len(rows) else (0, H)
        box = (max(0, x0 - a.pad), max(0, y0 - a.pad), min(W, x1 + a.pad), min(H, y1 + a.pad))
        im.crop(box).save(out / f"{stem}-p{i:02d}.png"); print(f"{stem}-p{i:02d}", box)

if __name__ == "__main__":
    main()
