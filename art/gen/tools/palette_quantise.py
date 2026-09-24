#!/usr/bin/env python3
"""Snap a sprite's colours to the Cubarium working palette family.

The art direction (§05) deliberately fixes a *relationship* between colours rather than a
palette table, so the table here is the **working family** named in
`design/art-direction/species-dossiers-2026-09-21.md` (ground rules) and
`design/appearance.md`. It is not canon and it is not a decision; it is what the dossiers
paint with. Revise it when Wrysk revises them.

Nearest-colour mapping is done in Oklab, not sRGB: the family is mostly dark saturated
violets, where sRGB distance confuses hue with lightness and turns plum into indigo.

Alpha is preserved untouched and fully transparent pixels keep their colour, so a
downsampled sprite's cut-out edge survives.

    palette_quantise.py IN.png --out OUT.png [--no-ramp] [--report]

Prints one line per palette entry actually used when --report is given.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

import numpy as np
from PIL import Image

# The working family. Names are the dossiers' names.
FAMILY: list[tuple[str, str]] = [
    ("floor", "#12093A"),  # the night floor; also the 1-px outline and contact line
    ("deep_plum", "#2A0E4A"),  # the one dark support
    ("mid_violet", "#3A1A7A"),
    ("detritus", "#510B6D"),
    ("pale_lilac", "#B99BE6"),  # exposed tissue / cutting edges, small accents
    ("producer_lo", "#1E2798"),  # cool end of the producer ramp
    ("producer_lit", "#2B4AC8"),  # the littershredder's lit top line
    ("electric_cyan", "#42C5F8"),  # ramp top; luminous tissue at full strength
    ("lip_half", "#2A86B8"),  # the fruiting lip at half intensity
    ("magenta", "#FF2AFC"),  # inherited body accent
    ("magenta_dim", "#7A2A78"),  # the starving / desaturated accent
    ("warm", "#FF9B50"),  # reserved: feeding flash only
]

# appearance.md defines the producer band as a continuous ramp by density, so sampling it
# gives the quantiser somewhere to put mid-band greens-of-blue instead of flattening them.
RAMP = ("#1E2798", "#42C5F8")
RAMP_STEPS = 5


def hex_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))


def srgb_to_oklab(rgb: np.ndarray) -> np.ndarray:
    """rgb: float array (..., 3) in 0..1 sRGB -> Oklab (..., 3)."""
    c = np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)
    r, g, b = c[..., 0], c[..., 1], c[..., 2]
    l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l_, m_, s_ = np.cbrt(l), np.cbrt(m), np.cbrt(s)
    return np.stack(
        [
            0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
            1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
            0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
        ],
        axis=-1,
    )


def build_palette(with_ramp: bool) -> list[tuple[str, tuple[int, int, int]]]:
    pal = [(n, hex_rgb(h)) for n, h in FAMILY]
    if with_ramp:
        a, b = np.array(hex_rgb(RAMP[0]), float), np.array(hex_rgb(RAMP[1]), float)
        for k in range(1, RAMP_STEPS - 1):  # endpoints are already in FAMILY
            t = k / (RAMP_STEPS - 1)
            pal.append((f"ramp_{k}", tuple(int(round(v)) for v in a + (b - a) * t)))
    return pal


def quantise(img: Image.Image, pal: list[tuple[str, tuple[int, int, int]]]) -> tuple[Image.Image, np.ndarray]:
    a = np.asarray(img.convert("RGBA")).copy()
    rgb = a[:, :, :3].astype(np.float64) / 255.0
    lab = srgb_to_oklab(rgb).reshape(-1, 3)
    pal_rgb = np.array([c for _, c in pal], dtype=np.float64) / 255.0
    pal_lab = srgb_to_oklab(pal_rgb)
    # squared Oklab distance to every entry; Oklab is already roughly uniform
    d = ((lab[:, None, :] - pal_lab[None, :, :]) ** 2).sum(axis=2)
    idx = d.argmin(axis=1)
    out = a.copy()
    out[:, :, :3] = np.array([c for _, c in pal], dtype=np.uint8)[idx].reshape(a.shape[0], a.shape[1], 3)
    # keep fully transparent pixels from being recoloured into a visible fringe
    clear = a[:, :, 3] < 10
    out[clear] = a[clear]
    counts = np.bincount(idx[~clear.reshape(-1)], minlength=len(pal))
    return Image.fromarray(out, "RGBA"), counts


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("image")
    ap.add_argument("--out", required=True)
    ap.add_argument("--no-ramp", action="store_true", help="named family entries only")
    ap.add_argument("--report", action="store_true", help="list the entries actually used")
    a = ap.parse_args()

    src = pathlib.Path(a.image)
    if not src.exists():
        print(f"no such image: {src}", file=sys.stderr)
        return 2
    pal = build_palette(not a.no_ramp)
    img, counts = quantise(Image.open(src), pal)
    pathlib.Path(a.out).parent.mkdir(parents=True, exist_ok=True)
    img.save(a.out)
    used = int((counts > 0).sum())
    print(f"{src} | {img.width}x{img.height} | {len(pal)} entries | {used} used | {a.out}")
    if a.report:
        total = counts.sum() or 1
        for (name, c), n in sorted(zip(pal, counts), key=lambda t: -t[1]):
            if n:
                print(f"  {name:14s} #{c[0]:02X}{c[1]:02X}{c[2]:02X}  {100 * n / total:5.1f}%  {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
