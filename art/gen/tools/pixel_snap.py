#!/usr/bin/env python3
"""Detect a generated image's fake pixel grid and nearest-downsample to the true grid.

Arm-A candidates come out of the models at an unknown, inconsistent logical scale: the
picture looks like pixel art but every "pixel" is some non-integer number of real pixels
wide, with soft seams. This finds that period and resamples to one sample per logical
pixel.

Method: build a 1-D edge signal per axis (mean absolute difference between neighbouring
columns / rows), then score every candidate period p in [--min-grid, --max-grid] by how
much of the edge energy lands on a lattice of spacing p (best phase). The winning period
is the grid; the logical size is round(W / p) x round(H / p). Sampling takes the modal
colour of each logical cell, which is robust to the soft seams.

    pixel_snap.py IN.png --out OUT.png [--grid N] [--min-grid 2] [--max-grid 32]

Prints: `<in> | detected grid <p> px | logical <w>x<h> | confidence <c>`.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

import numpy as np
from PIL import Image


def edge_signal(a: np.ndarray, axis: int) -> np.ndarray:
    """Mean |difference| between adjacent lines along `axis` (length N-1)."""
    d = np.abs(np.diff(a.astype(np.float64), axis=axis))
    other = tuple(i for i in range(a.ndim) if i != axis)
    return d.mean(axis=other)


def score_period(sig: np.ndarray, p: float) -> float:
    """Fraction of edge energy sitting on the best-phase lattice of spacing `p`."""
    n = len(sig)
    total = sig.sum()
    if total <= 0:
        return 0.0
    best = 0.0
    steps = 12
    for k in range(steps):
        phase = p * k / steps
        idx = np.round(np.arange(phase, n, p)).astype(int)
        idx = idx[(idx >= 0) & (idx < n)]
        if len(idx) < 2:
            continue
        # a lattice hit counts the boundary sample and its neighbour (soft seams)
        hit = set(idx.tolist())
        for i in idx:
            if i + 1 < n:
                hit.add(i + 1)
        got = sig[sorted(hit)].sum()
        # normalise by the share of samples used, so small p is not free
        expected = total * len(hit) / n
        best = max(best, got / expected if expected > 0 else 0.0)
    return best


def detect_grid(img: Image.Image, lo: float, hi: float) -> tuple[float, float]:
    a = np.asarray(img.convert("RGB"))
    sx = edge_signal(a, axis=1)  # variation across columns -> vertical seams
    sy = edge_signal(a, axis=0)
    cands = np.arange(lo, hi + 0.001, 0.25)
    scores = []
    for p in cands:
        scores.append(score_period(sx, p) + score_period(sy, p))
    scores_a = np.array(scores)
    # prefer the smallest period within 1% of the best, so we do not lock onto 2p
    best = scores_a.max()
    winner = float(cands[np.argmax(scores_a >= best * 0.99)])
    return winner, float(best / 2.0)


def modal_downsample(img: Image.Image, w: int, h: int) -> Image.Image:
    """One output pixel per logical cell, taking the cell's most common colour."""
    a = np.asarray(img.convert("RGBA"))
    H, W = a.shape[:2]
    ys = np.linspace(0, H, h + 1).round().astype(int)
    xs = np.linspace(0, W, w + 1).round().astype(int)
    out = np.zeros((h, w, 4), dtype=np.uint8)
    for j in range(h):
        for i in range(w):
            cell = a[ys[j] : max(ys[j + 1], ys[j] + 1), xs[i] : max(xs[i + 1], xs[i] + 1)]
            flat = cell.reshape(-1, 4)
            # modal colour: pack RGBA into one uint32 key
            keys = (
                flat[:, 0].astype(np.uint32) << 24
                | flat[:, 1].astype(np.uint32) << 16
                | flat[:, 2].astype(np.uint32) << 8
                | flat[:, 3].astype(np.uint32)
            )
            vals, counts = np.unique(keys, return_counts=True)
            k = int(vals[counts.argmax()])
            out[j, i] = (k >> 24 & 255, k >> 16 & 255, k >> 8 & 255, k & 255)
    return Image.fromarray(out, "RGBA")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("image")
    ap.add_argument("--out", required=True)
    ap.add_argument("--grid", type=float, default=None, help="force this grid period")
    ap.add_argument("--min-grid", type=float, default=2.0)
    ap.add_argument("--max-grid", type=float, default=32.0)
    ap.add_argument(
        "--logical", default=None, help="force the logical size, e.g. 96x96 (overrides --grid)"
    )
    a = ap.parse_args()

    src = pathlib.Path(a.image)
    img = Image.open(src)
    conf = float("nan")
    if a.logical:
        w, h = (int(v) for v in a.logical.lower().split("x"))
        grid = img.size[0] / w
    elif a.grid:
        grid = a.grid
        w, h = max(1, round(img.size[0] / grid)), max(1, round(img.size[1] / grid))
    else:
        grid, conf = detect_grid(img, a.min_grid, a.max_grid)
        w, h = max(1, round(img.size[0] / grid)), max(1, round(img.size[1] / grid))

    out = modal_downsample(img, w, h)
    pathlib.Path(a.out).parent.mkdir(parents=True, exist_ok=True)
    out.save(a.out)
    print(f"{src} | detected grid {grid:g} px | logical {w}x{h} | confidence {conf:.3f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
