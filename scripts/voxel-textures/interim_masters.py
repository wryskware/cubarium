#!/usr/bin/env python3
"""Write the INTERIM face-texture masters for the GPU voxel renderer.

These are placeholders (design/handoffs/voxel-textures-lod-2026-09-23.md): quiet
patterns within a step or two of today's material colours, so terrain stops reading as
flat solid voxels until generated textures replace them. They decide no look.

    python3 scripts/voxel-textures/interim_masters.py [--out assets/voxel-textures/masters]

then derive the levels:

    cargo run --release -p cubarium --example voxel_texture_levels

Every master is 48 x 48. A top master is the voxel's top seen from straight above, row 0
at the back edge; the renderer squeezes it to the foreshortened top face per level.

Terrain masters are colour: the texel is the face's colour. Plant masters (bark, leaf,
drape) are grey: the renderer multiplies the stand's own style colour by r / 128, so
128 is the style colour itself and the colour pass (colours.rs) still decides hue. Their
alpha is a cutout: 0 is a hole the next voxel shows through.
"""

from __future__ import annotations

import argparse
import pathlib
import random

from PIL import Image

N = 48
VARIANTS = 4

# Today's colours, crates/cubarium/src/voxel/present.rs.
BEDROCK = 0x1A1038
ROCK = 0x2C2070
SOIL = 0x59206B
# The art direction's working family (art/gen/tools/palette_quantise.py): the producer
# ramp for the turf, detritus for its litter.
PRODUCER_LO = 0x1E2798
PRODUCER_LIT = 0x2B4AC8
DETRITUS = 0x510B6D


def rgb(h: int) -> tuple[int, int, int]:
    return ((h >> 16) & 255, (h >> 8) & 255, h & 255)


def step(h: int, k: int, per: float = 0.14) -> tuple[int, int, int]:
    """`h` moved `k` palette steps: each step is 14 % brighter or darker."""
    f = (1.0 + per) ** k
    return tuple(max(0, min(255, round(c * f))) for c in rgb(h))


def blotches(rng: random.Random, cell: int) -> list[list[float]]:
    """Value noise on a `cell`-px lattice, bilinear, in 0..1: the large quiet shapes."""
    n = N // cell + 2
    lattice = [[rng.random() for _ in range(n)] for _ in range(n)]
    out = []
    for y in range(N):
        row = []
        for x in range(N):
            gx, gy = x / cell, y / cell
            x0, y0 = int(gx), int(gy)
            fx, fy = gx - x0, gy - y0
            a = lattice[y0][x0] * (1 - fx) + lattice[y0][x0 + 1] * fx
            b = lattice[y0 + 1][x0] * (1 - fx) + lattice[y0 + 1][x0 + 1] * fx
            row.append(a * (1 - fy) + b * fy)
        out.append(row)
    return out


def canvas(c) -> list[list[tuple]]:
    return [[c for _ in range(N)] for _ in range(N)]


def save(px, path: pathlib.Path, grey: bool = False) -> None:
    img = Image.new("RGBA", (N, N))
    for y in range(N):
        for x in range(N):
            p = px[y][x]
            img.putpixel((x, y), p if len(p) == 4 else (*p, 255))
    img.save(path)


def speckle(px, rng, colours, density: float, clump: int = 2) -> None:
    for y in range(0, N, clump):
        for x in range(0, N, clump):
            if rng.random() < density:
                c = rng.choice(colours)
                for dy in range(clump):
                    for dx in range(clump):
                        px[y + dy][x + dx] = c


def crack(px, rng, colour, length: int, steep: bool) -> None:
    x, y = rng.randrange(4, N - 4), rng.randrange(4, N - 4)
    for _ in range(length):
        if 0 <= x < N and 0 <= y < N:
            px[y][x] = colour
        if steep:
            y += 1
            x += rng.choice((-1, 0, 0, 1))
        else:
            x += 1
            y += rng.choice((-1, 0, 0, 1))


# --- terrain ---------------------------------------------------------------------------

# Strata rows shared by every rock side variant, so a wall's layers run on across voxels.
ROCK_STRATA = [(5, 2), (17, 1), (29, 3), (41, 1)]


def rock_side(rng):
    px = canvas(step(ROCK, 0))
    b = blotches(rng, 16)
    for y in range(N):
        for x in range(N):
            if b[y][x] > 0.72:
                px[y][x] = step(ROCK, 1)
            elif b[y][x] < 0.25:
                px[y][x] = step(ROCK, -1)
    for y0, h in ROCK_STRATA:
        for y in range(y0, y0 + h):
            for x in range(N):
                px[y][x] = step(ROCK, -1)
        for x in range(N):  # the lit lip under each layer
            if rng.random() < 0.8:
                px[y0 + h][x] = step(ROCK, 1)
    for _ in range(2):
        crack(px, rng, step(ROCK, -2), rng.randrange(6, 14), steep=True)
    return px


def rock_top(rng):
    px = canvas(step(ROCK, 0))
    b = blotches(rng, 12)
    for y in range(N):
        for x in range(N):
            if b[y][x] > 0.7:
                px[y][x] = step(ROCK, 1)
            elif b[y][x] < 0.28:
                px[y][x] = step(ROCK, -1)
    crack(px, rng, step(ROCK, -2), rng.randrange(8, 16), steep=False)
    speckle(px, rng, [step(ROCK, 2)], 0.02, 2)
    return px


def bedrock_side(rng):
    px = canvas(step(BEDROCK, 0))
    b = blotches(rng, 12)
    for y in range(N):
        for x in range(N):
            if b[y][x] > 0.66:
                px[y][x] = step(BEDROCK, 1)
            elif b[y][x] < 0.3:
                px[y][x] = step(BEDROCK, -1)
    for _ in range(3):
        crack(px, rng, step(BEDROCK, -2), rng.randrange(5, 12), steep=rng.random() < 0.5)
    speckle(px, rng, [step(BEDROCK, 2)], 0.015, 2)
    return px


def bedrock_top(rng):
    px = canvas(step(BEDROCK, 0))
    b = blotches(rng, 16)
    for y in range(N):
        for x in range(N):
            if b[y][x] > 0.68:
                px[y][x] = step(BEDROCK, 1)
            elif b[y][x] < 0.3:
                px[y][x] = step(BEDROCK, -1)
    speckle(px, rng, [step(BEDROCK, 2)], 0.015, 2)
    return px


def soil_grain(rng):
    px = canvas(step(SOIL, 0))
    b = blotches(rng, 24)
    for y in range(N):
        for x in range(N):
            if b[y][x] < 0.3:
                px[y][x] = step(SOIL, -1)
    speckle(px, rng, [step(SOIL, -1)], 0.18, 2)
    speckle(px, rng, [step(SOIL, 1)], 0.08, 2)
    speckle(px, rng, [step(SOIL, -2)], 0.04, 2)
    # A pebble or two of rock.
    for _ in range(rng.randrange(1, 3)):
        x, y = rng.randrange(0, N - 4), rng.randrange(0, N - 3)
        for dy in range(3):
            for dx in range(4):
                if (dx, dy) not in ((0, 0), (3, 0), (0, 2), (3, 2)):
                    px[y + dy][x + dx] = step(ROCK, 1 if dy == 0 else 0)
    return px


def soil_side(rng):
    return soil_grain(rng)


def soil_top(rng):
    px = soil_grain(rng)
    # Litter and the odd tuft of producer on open ground: sparse, dark.
    speckle(px, rng, [rgb(DETRITUS)], 0.04, 2)
    speckle(px, rng, [rgb(PRODUCER_LO)], 0.03, 2)
    return px


def turf_side(rng):
    """Soil whose top is open to the sky: a grass-and-litter fringe over its top rows."""
    px = soil_grain(rng)
    grass = [rgb(PRODUCER_LO), rgb(PRODUCER_LO), rgb(PRODUCER_LIT)]
    for x in range(N):
        # 8..16 rows deep, varying by column, with an occasional longer strand.
        depth = 8 + int(8 * rng.random() ** 1.5)
        if rng.random() < 0.12:
            depth += rng.randrange(4, 10)
        for y in range(min(depth, N)):
            if y < 3:
                px[y][x] = rgb(PRODUCER_LIT) if rng.random() < 0.5 else rgb(PRODUCER_LO)
            elif rng.random() < 0.12:
                px[y][x] = rgb(DETRITUS)
            else:
                px[y][x] = rng.choice(grass)
        # The fringe's lower lip, one row darker, where it hangs over the soil.
        if depth < N:
            px[depth][x] = step(SOIL, -2)
    return px


TERRAIN = {
    "bedrock-side": bedrock_side,
    "bedrock-top": bedrock_top,
    "rock-side": rock_side,
    "rock-top": rock_top,
    "soil-side": soil_side,
    "soil-top": soil_top,
    "turf-side": turf_side,
}

MAKERS = dict(TERRAIN)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", default="assets/voxel-textures/masters")
    args = ap.parse_args()
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    for stem, make in MAKERS.items():
        for v in range(VARIANTS):
            # A seed per face and variant, so re-running reproduces the files.
            rng = random.Random(f"{stem}-{v}")
            save(make(rng), out / f"{stem}-{v}.png")
    print(f"{len(MAKERS) * VARIANTS} masters -> {out}")


if __name__ == "__main__":
    main()
