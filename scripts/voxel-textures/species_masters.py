#!/usr/bin/env python3
"""Write the INTERIM per-species face-texture masters for the GPU voxel renderer.

Placeholders for the signed-off plants (design/handoffs/species-texture-sets-2026-09-24.md),
drawn procedurally from each species dossier's parts and hexes
(design/art-direction/species-dossiers-2026-09-21.md: D6 springturf, D8 stonecushion, D9
bloomcrown, D12 lanternberry, D13 siphonreed) until the Codex art pass replaces them.
They follow the dossiers and decide no new look.

    python3 scripts/voxel-textures/species_masters.py [--out assets/voxel-textures/masters]

then derive the levels:

    cargo run --release -p cubarium --example voxel_texture_levels

Layout (the contract is the module doc of crates/cubarium/src/voxel/textures.rs):
`<out>/species/<species>/<role>-<face>-<variant>.png`, 48 x 48, **direct colour**: the
texel is the cell's colour. Leaf alpha is a cutout (0 is a hole the next voxel shows
through); bark is opaque. A top master is the voxel's top seen from straight above, row 0
at the back edge.

Which roles each species gets follows its baked model's tags (assets/voxel-models/125):
bloomcrown has a stem (bark), a low rosette (foliage index 0 -> leaf1) and its vanes and core (foliage 1
-> leaf2); lanternberry stems (bark) and leaflets (leaf); springturf, stonecushion and
siphonreed are foliage only (leaf). Accents (bloom, fruit) keep their state colours and
get no texture.

Scale: a face is drawn at 4-13 px on screen, 1/12 to 1/4 of the master, so every shape
here is at least 8 master px across and holes are large and few (fine speckle vanished at
4-6 px).
"""

from __future__ import annotations

import argparse
import pathlib
import random

from PIL import Image, ImageDraw

N = 48
VARIANTS = 4

# Dossier hexes.
P0 = "#1E2798"  # producer shadow step
P1 = "#2B6AD0"  # producer body
P2 = "#42C5F8"  # producer lit / newest
VIOLET = "#3A1A7A"
PLUM = "#2A0E4A"
LILAC = "#B99BE6"
DETRITUS = "#510B6D"
CLEAR = (0, 0, 0, 0)


def new(fill=CLEAR) -> Image.Image:
    return Image.new("RGBA", (N, N), fill)


def rect(d: ImageDraw.ImageDraw, x0, y0, x1, y1, c) -> None:
    """Inclusive pixel rectangle, clipped."""
    x0, x1 = max(0, x0), min(N - 1, x1)
    y0, y1 = max(0, y0), min(N - 1, y1)
    if x0 <= x1 and y0 <= y1:
        d.rectangle((x0, y0, x1, y1), fill=c)


def ramp(y: int, top: int, bottom: int) -> str:
    """P2 at the top third of [top, bottom], P1 in the middle, P0 at the base."""
    f = (y - top) / max(1, bottom - top)
    return P2 if f < 0.3 else (P1 if f < 0.7 else P0)


# --- bloomcrown (D9) -------------------------------------------------------------------


def bloomcrown_bark_side(rng):
    # The straight stem: mid violet with a plum shaded left edge and one cyan growth node.
    im = new(VIOLET)
    d = ImageDraw.Draw(im)
    rect(d, 0, 0, 9, N - 1, PLUM)
    ny = rng.choice((12, 20, 28))
    rect(d, 18, ny, 33, ny + 7, P2)
    return im


def bloomcrown_bark_top(rng):
    im = new(VIOLET)
    d = ImageDraw.Draw(im)
    rect(d, 0, 0, 9, N - 1, PLUM)
    rect(d, 0, 0, N - 1, 7, PLUM)
    return im


def bloomcrown_leaf2_side(rng):
    # The vanes: narrow upright blades with open space between, P0 at the base ramping to
    # P2 at the upper edge. Three blades a face, 8-10 px wide, gaps of 6-8 px.
    im = new()
    d = ImageDraw.Draw(im)
    x = rng.randrange(0, 5)
    while x < N:
        w = rng.choice((8, 10))
        top = rng.randrange(0, 9)
        for y in range(top, N):
            rect(d, x, y, x + w - 1, y, ramp(y, top, N - 1))
        rect(d, x, top, x + w - 1, top + 3, P2)  # the lit upper edge
        x += w + rng.choice((6, 8))
    return im


def bloomcrown_leaf2_top(rng):
    # From above: the vanes' lit upper edges, short blades with holes between.
    im = new()
    d = ImageDraw.Draw(im)
    for gy in range(0, N, 16):
        for gx in range(0, N, 16):
            x, y = gx + rng.randrange(0, 7), gy + rng.randrange(0, 7)
            if rng.random() < 0.5:
                rect(d, x, y, x + 9, y + 7, P2)
            else:
                rect(d, x, y, x + 7, y + 9, P1)
    return im


def bloomcrown_leaf1_side(rng):
    # The model's foliage index 0 is the low rosette under the stem (the core is in the
    # vanes' layer, index 1): short blades of the darker steps with holes between.
    im = new()
    d = ImageDraw.Draw(im)
    x = rng.randrange(0, 5)
    while x < N:
        top = rng.randrange(8, 20)
        for y in range(top, N):
            rect(d, x, y, x + 9, y, P1 if y < top + 10 else P0)
        x += 10 + rng.choice((6, 8))
    return im


def bloomcrown_leaf1_top(rng):
    im = new()
    d = ImageDraw.Draw(im)
    for gy in range(0, N, 16):
        for gx in range(0, N, 16):
            if rng.random() < 0.75:
                x, y = gx + rng.randrange(0, 5), gy + rng.randrange(0, 5)
                rect(d, x, y, x + 9, y + 9, P1)
                rect(d, x + 2, y + 6, x + 7, y + 9, P0)
    return im


# --- lanternberry (D12) ----------------------------------------------------------------


def lanternberry_bark_side(rng):
    # Bare violet stems: the whole cell is stem, shaded plum on the left.
    im = new(VIOLET)
    d = ImageDraw.Draw(im)
    rect(d, 0, 0, 7, N - 1, PLUM)
    return im


def lanternberry_bark_top(rng):
    im = new(VIOLET)
    d = ImageDraw.Draw(im)
    rect(d, 0, 0, 7, N - 1, PLUM)
    return im


def lanternberry_leaf_side(rng):
    # Leaflet dashes on bare stem: one violet stem through the cell, P1 dashes (2:1) spaced
    # along it with bare stem between, everything else open.
    im = new()
    d = ImageDraw.Draw(im)
    x = rng.randrange(18, 26)
    lean = rng.choice((-1, 1))
    for y in range(N):
        sx = x + lean * (y // 12)
        rect(d, sx, y, sx + 7, y, VIOLET)
    for k, y in enumerate(range(rng.randrange(0, 4), N - 8, 12)):
        sx = x + lean * (y // 12)
        side = 1 if (k + (lean > 0)) % 2 else -1
        if side > 0:
            rect(d, sx + 8, y, sx + 27, y + 9, P1)
        else:
            rect(d, sx - 20, y, sx - 1, y + 9, P1)
    return im


def lanternberry_leaf_top(rng):
    # From above: the stem's end and the dashes around it.
    im = new()
    d = ImageDraw.Draw(im)
    c = N // 2 + rng.randrange(-4, 5)
    rect(d, c - 4, c - 4, c + 3, c + 3, VIOLET)
    for dx, dy in ((-20, -12), (8, -18), (10, 8), (-18, 10)):
        x, y = c + dx + rng.randrange(-2, 3), c + dy + rng.randrange(-2, 3)
        rect(d, x, y, x + 15, y + 7, P1)
    return im


# --- springturf (D6) -------------------------------------------------------------------


def springturf_leaf_side(rng):
    # Pleated tuft blades: straps 8 px wide leaning outward from the centre, P0 at the
    # base to P2 at the rounded tips, a scalloped top with open space between the tips;
    # a shorter front rank in the darker step over their bases.
    im = new()
    d = ImageDraw.Draw(im)
    x = rng.randrange(-2, 3)
    while x < N:
        top = rng.randrange(0, 10)
        lean = -1 if x + 4 < N // 2 else 1
        for y in range(top, N):
            sx = x + lean * ((N - y) // 16)
            rect(d, sx, y, sx + 7, y, ramp(y, top, N - 1))
        tx = x + lean * ((N - top) // 16)
        rect(d, tx + 1, max(0, top - 2), tx + 6, top, P2)  # the cupped tip
        x += 8 + rng.choice((4, 6))
    x = rng.randrange(2, 8)
    while x < N:
        top = rng.randrange(28, 34)
        rect(d, x, top, x + 7, N - 1, P0)
        rect(d, x + 1, top - 2, x + 6, top - 1, P0)
        x += 8 + rng.choice((6, 8))
    return im


def springturf_leaf_top(rng):
    # From above: the fan of blade tips around the buried crown, holes between.
    im = new()
    d = ImageDraw.Draw(im)
    for gy in range(0, N, 16):
        for gx in range(0, N, 16):
            if rng.random() < 0.8:
                x, y = gx + rng.randrange(0, 5), gy + rng.randrange(0, 5)
                rect(d, x, y, x + 9, y + 9, P1)
                rect(d, x + 2, y, x + 7, y + 4, P2)
    return im


# --- stonecushion (D8) -----------------------------------------------------------------


def knots(rng, pitch: int = 16, size: int = 12, centre: int = 5) -> Image.Image:
    # A mosaic of tight rosette knots: lilac knots with violet centres, packed so the
    # surface is all knots; the seams between them are violet too.
    im = new(VIOLET)
    d = ImageDraw.Draw(im)
    off = rng.randrange(0, pitch // 2)
    for row, gy in enumerate(range(-pitch, N + pitch, pitch - 2)):
        shift = off + (pitch // 2 if row % 2 else 0)
        for gx in range(-pitch + shift, N + pitch, pitch):
            x, y = gx + rng.randrange(-1, 2), gy + rng.randrange(-1, 2)
            d.ellipse((x, y, x + size, y + size), fill=LILAC)
            m = (size - centre) // 2
            rect(d, x + m, y + m, x + m + centre - 1, y + m + centre - 1, VIOLET)
    return im


def stonecushion_leaf_side(rng):
    return knots(rng)


def stonecushion_leaf_top(rng):
    return knots(rng)


# --- siphonreed (D13) ------------------------------------------------------------------


def siphonreed_leaf_side(rng):
    # Hollow stem strips standing apart: two stems a face, 14 px wide, P1 walls around a P0
    # hollow, each with one cyan ring (the aeration organ); open between the stems.
    im = new()
    d = ImageDraw.Draw(im)
    for x in (rng.randrange(0, 6), rng.randrange(24, 30)):
        rect(d, x, 0, x + 13, N - 1, P1)
        rect(d, x + 5, 0, x + 8, N - 1, P0)
        ry = rng.randrange(6, 32)
        rect(d, x - 1, ry, x + 14, ry + 7, P2)
    return im


def siphonreed_leaf_top(rng):
    # From above: the stems' hollow ends, cyan rings round a dark bore.
    im = new()
    d = ImageDraw.Draw(im)
    for cx, cy in ((12, 14), (34, 32)):
        x, y = cx + rng.randrange(-3, 4), cy + rng.randrange(-3, 4)
        d.ellipse((x - 7, y - 7, x + 7, y + 7), fill=P2)
        rect(d, x - 3, y - 3, x + 2, y + 2, P0)
    return im


SPECIES = {
    "bloomcrown": {
        "bark-side": bloomcrown_bark_side,
        "bark-top": bloomcrown_bark_top,
        "leaf1-side": bloomcrown_leaf1_side,
        "leaf1-top": bloomcrown_leaf1_top,
        "leaf2-side": bloomcrown_leaf2_side,
        "leaf2-top": bloomcrown_leaf2_top,
    },
    "lanternberry": {
        "bark-side": lanternberry_bark_side,
        "bark-top": lanternberry_bark_top,
        "leaf-side": lanternberry_leaf_side,
        "leaf-top": lanternberry_leaf_top,
    },
    "springturf": {
        "leaf-side": springturf_leaf_side,
        "leaf-top": springturf_leaf_top,
    },
    "stonecushion": {
        "leaf-side": stonecushion_leaf_side,
        "leaf-top": stonecushion_leaf_top,
    },
    "siphonreed": {
        "leaf-side": siphonreed_leaf_side,
        "leaf-top": siphonreed_leaf_top,
    },
}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", default="assets/voxel-textures/masters")
    args = ap.parse_args()
    n = 0
    for species, faces in SPECIES.items():
        out = pathlib.Path(args.out) / "species" / species
        out.mkdir(parents=True, exist_ok=True)
        for stem, make in faces.items():
            for v in range(VARIANTS):
                # A seed per species, face and variant, so re-running reproduces the files.
                rng = random.Random(f"{species}-{stem}-{v}")
                make(rng).save(out / f"{stem}-{v}.png")
                n += 1
    print(f"{n} species masters -> {pathlib.Path(args.out) / 'species'}")


if __name__ == "__main__":
    main()
