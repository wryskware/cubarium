#!/usr/bin/env python3
"""Paste a sprite candidate into a real voxel capture at 8 and 4 px per voxel.

A candidate only counts once it has been seen at the size it will actually be drawn.
This sizes it from its logical width in voxels (one voxel = 0.125 m), drops the black
generation background to alpha, and pastes it at two or three plausible ground positions
on the current capture of the live world.

    compose_in_scene.py CANDIDATE.png --scale-voxels 6 --out DIR [--scene generated]

Captures live in `art/gen/runs/_scene/<scene>-<px>/final.png`, made with
    cubarium voxel --scene <scene> --seed 1 --sink png --seconds 2 \
        --config <px_per_voxel = N> --out art/gen/runs/_scene/<scene>-<N>/
If the 8 px capture is missing the 4 px one is upscaled x2 nearest instead.

Writes `<name>@8px.png` and `<name>@4px.png` and prints both paths.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

import numpy as np
from PIL import Image

SCENE_ROOT = pathlib.Path("art/gen/runs/_scene")


def to_alpha(img: Image.Image, cut: int = 24) -> Image.Image:
    """Near-black generation background -> transparent, then trim to the bounding box."""
    a = np.asarray(img.convert("RGBA")).copy()
    lum = a[:, :, :3].max(axis=2)
    a[:, :, 3] = np.where(lum <= cut, 0, 255).astype(np.uint8)
    out = Image.fromarray(a, "RGBA")
    box = out.getbbox()
    return out.crop(box) if box else out


def surface_row(scene: np.ndarray, x: int, tol: float = 10.0) -> int:
    """First row from the top at column x that is not the sky gradient."""
    h, w = scene.shape[:2]
    rgb = scene[:, :, :3].astype(np.float64)
    # sky rows are near-uniform along x; use them as the per-row reference colour
    flat = rgb.std(axis=1).max(axis=1) < 6.0
    ref = np.zeros((h, 3))
    last = None
    for y in range(h):
        if flat[y]:
            last = rgb[y].mean(axis=0)
        ref[y] = last if last is not None else rgb[y].mean(axis=0)
    for y in range(h):
        if not flat[y] and np.abs(rgb[y, x] - ref[y]).max() > tol:
            return y
    return h - 1


def compose(cand: Image.Image, scene_path: pathlib.Path, px: int, voxels: float) -> Image.Image:
    scene = Image.open(scene_path).convert("RGBA")
    arr = np.asarray(scene)
    target_w = max(1, int(round(voxels * px)))
    scale = target_w / cand.width
    sprite = cand.resize(
        (target_w, max(1, int(round(cand.height * scale)))), Image.Resampling.NEAREST
    )
    out = scene.copy()
    w = scene.width
    for frac in (0.22, 0.52, 0.80):
        x = int(w * frac)
        x = min(max(x, sprite.width // 2), w - sprite.width // 2 - 1)
        y = surface_row(arr, min(max(x, 0), w - 1))
        out.alpha_composite(sprite, (x - sprite.width // 2, max(0, y - sprite.height)))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("candidate")
    ap.add_argument("--scale-voxels", type=float, required=True, dest="voxels")
    ap.add_argument("--out", required=True)
    ap.add_argument("--scene", default="generated", choices=["generated", "authored"])
    ap.add_argument("--root", default=str(SCENE_ROOT))
    a = ap.parse_args()

    src = pathlib.Path(a.candidate)
    cand = to_alpha(Image.open(src))
    outdir = pathlib.Path(a.out)
    outdir.mkdir(parents=True, exist_ok=True)
    root = pathlib.Path(a.root)

    made = []
    for px in (8, 4):
        p = root / f"{a.scene}-{px}" / "final.png"
        if p.exists():
            img = compose(cand, p, px, a.voxels)
        else:
            base = root / f"{a.scene}-4" / "final.png"
            if not base.exists():
                print(f"no capture for scene {a.scene} under {root}", file=sys.stderr)
                return 2
            img = compose(cand, base, 4, a.voxels)
            if px == 8:
                img = img.resize((img.width * 2, img.height * 2), Image.Resampling.NEAREST)
        dst = outdir / f"{src.stem}@{px}px.png"
        img.convert("RGB").save(dst)
        made.append(str(dst))
    print(" ".join(made))
    return 0


if __name__ == "__main__":
    sys.exit(main())
