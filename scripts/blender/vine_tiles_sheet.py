"""Cut out and check the latticevine tile masters, then draw their contact sheet.

    python3 scripts/blender/vine_tiles_sheet.py MASTERS_DIR SHEET.png

Run by `latticevine.py --tiles` after rendering. It forces every master to straight
cutout alpha (0 or 255; colour zeroed where transparent) and writes one sheet:

  * the whole set: plain, rooted (climb) and rooted (hang) tiles, each density x 16 masks,
    plus the three accents, at 2x;
  * a 6 x 6 test wall from a random covered shape: the masks come from the shape's own
    adjacency (bit 0 up, 1 right, 2 down, 3 left), densities are random, the foot row is
    rooted (climb), and a few cells carry an accent. It is assembled at 48 px and at 12 px.
    The 12 px wall uses a stand-in 4:1 reduction (the mean colour of the opaque pixels in
    each 4 x 4 block, opaque where at least half the block is), not the texture tool's.

The backdrop is a flat rock tone (#4F4670, the lineup's rock) so the cutout edges and the
dark runners read; the presenter draws the real rock.
"""

import os
import random
import sys

from PIL import Image, ImageDraw

MASTERS, SHEET = sys.argv[1], sys.argv[2]
ROCK = (0x4F, 0x46, 0x70, 255)
BG = (0x0C, 0x08, 0x18, 255)
INK = (0xE6, 0xDD, 0xF8, 255)
DENS = ("full", "thin", "bare")


def cutout(path):
    im = Image.open(path).convert("RGBA")
    px = im.load()
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = px[x, y]
            px[x, y] = (r, g, b, 255) if a >= 128 else (0, 0, 0, 0)
    im.save(path)
    return im


def name(kind, dens, mask):
    return {"plain": f"vine-{dens}-{mask:x}.png", "climb": f"vine-root-{dens}-{mask:x}.png",
            "hang": f"vine-root-{dens}-{mask:x}-hang.png"}[kind]


def reduce4(im):
    out = Image.new("RGBA", (im.width // 4, im.height // 4))
    src, dst = im.load(), out.load()
    for by in range(out.height):
        for bx in range(out.width):
            cols = [src[bx * 4 + i, by * 4 + j] for j in range(4) for i in range(4)]
            op = [c for c in cols if c[3] == 255]
            if len(op) >= 8:
                dst[bx, by] = tuple(sum(c[k] for c in op) // len(op) for k in range(3)) + (255,)
            else:
                dst[bx, by] = (0, 0, 0, 0)
    return out


def wall(tiles, accents, seed=5):
    rng = random.Random(seed)
    n = 6
    cov = [[False] * n for _ in range(n)]
    # grow a covered shape up from two roots on the foot row
    frontier = [(5, 1), (5, 4)]
    for r, c in frontier:
        cov[r][c] = True
    while sum(map(sum, cov)) < 26:
        r, c = rng.choice([(r, c) for r in range(n) for c in range(n) if cov[r][c]])
        dr, dc = rng.choice(((-1, 0), (-1, 0), (0, 1), (0, -1), (1, 0)))
        if 0 <= r + dr < n and 0 <= c + dc < n:
            cov[r + dr][c + dc] = True
    cells = []
    for r in range(n):
        for c in range(n):
            if not cov[r][c]:
                continue
            m = 0
            if r > 0 and cov[r - 1][c]:
                m |= 1
            if c < n - 1 and cov[r][c + 1]:
                m |= 2
            if r < n - 1 and cov[r + 1][c]:
                m |= 4
            if c > 0 and cov[r][c - 1]:
                m |= 8
            dens = rng.choices(DENS, (6, 3, 1))[0]
            kind = "climb" if (r, c) in frontier else "plain"
            acc = rng.choices((None, "bud", "flower", "fruit"), (8, 1, 2, 2))[0]
            cells.append((r, c, kind, dens, m, acc))
    big = Image.new("RGBA", (n * 48, n * 48), ROCK)
    for r, c, kind, dens, m, acc in cells:
        big.alpha_composite(tiles[name(kind, dens, m)], (c * 48, r * 48))
        if acc:
            big.alpha_composite(accents[acc], (c * 48, r * 48))
    small = Image.new("RGBA", (n * 12, n * 12), ROCK)
    for r, c, kind, dens, m, acc in cells:
        small.alpha_composite(reduce4(tiles[name(kind, dens, m)]), (c * 12, r * 12))
        if acc:
            small.alpha_composite(reduce4(accents[acc]), (c * 12, r * 12))
    return big, small


def main():
    tiles, accents = {}, {}
    for kind in ("plain", "climb", "hang"):
        for d in DENS:
            for m in range(16):
                f = name(kind, d, m)
                tiles[f] = cutout(os.path.join(MASTERS, f))
    for a in ("bud", "flower", "fruit"):
        accents[a] = cutout(os.path.join(MASTERS, f"vine-accent-{a}.png"))
    for k in ("plain", "climb", "hang"):
        for d in DENS:
            fr = [sum(1 for px in tiles[name(k, d, m)].getdata() if px[3]) / 2304 for m in range(16)]
            print(f"coverage {k:5s} {d}: mask f {fr[15] * 100:.0f} %, mean {sum(fr) / 16 * 100:.0f} %")
    bad = [f for f, im in list(tiles.items()) + list(accents.items()) if im.size != (48, 48)]
    if bad:
        sys.exit(f"wrong size: {bad}")

    S, G = 96, 6  # tile display size (2x), gap
    rows = [(k, d) for k in ("plain", "climb", "hang") for d in DENS]
    left, top = 150, 40
    grid_w = left + 16 * (S + G)
    grid_h = top + len(rows) * (S + G) + S + 60
    big, small = wall(tiles, accents)
    wall_w = big.width * 2 + 40 + small.width * 4 + 40 + small.width + 40
    W = max(grid_w, wall_w) + 20
    H = grid_h + big.height * 2 + 80
    sheet = Image.new("RGBA", (W, H), BG)
    dr = ImageDraw.Draw(sheet)
    dr.text((10, 10), "latticevine tile masters, 48 px, shown 2x on #4F4670 rock. mask bits: 1 up, 2 right, 4 down, 8 left",
            fill=INK)
    for m in range(16):
        dr.text((left + m * (S + G) + S // 2 - 4, top - 14), f"{m:x}", fill=INK)
    for ri, (k, d) in enumerate(rows):
        y = top + ri * (S + G)
        dr.text((10, y + S // 2 - 6), f"{'plain' if k == 'plain' else 'root ' + k} {d}", fill=INK)
        for m in range(16):
            x = left + m * (S + G)
            cell = Image.new("RGBA", (48, 48), ROCK)
            cell.alpha_composite(tiles[name(k, d, m)])
            sheet.alpha_composite(cell.resize((S, S), Image.NEAREST), (x, y))
    y = top + len(rows) * (S + G) + 10
    dr.text((10, y + S // 2 - 6), "accents", fill=INK)
    for i, a in enumerate(("bud", "flower", "fruit")):
        cell = Image.new("RGBA", (48, 48), ROCK)
        cell.alpha_composite(accents[a])
        sheet.alpha_composite(cell.resize((S, S), Image.NEAREST), (left + i * (S + G), y))
        dr.text((left + i * (S + G), y + S + 2), a, fill=INK)
    y = grid_h
    dr.text((10, y), "6 x 6 test wall (masks from the shape's adjacency; foot roots at columns 1 and 4; random "
                     "densities and accents): 48 px shown 2x | 12 px shown 4x | 12 px native", fill=INK)
    y += 20
    sheet.alpha_composite(big.resize((big.width * 2, big.height * 2), Image.NEAREST), (10, y))
    x = 10 + big.width * 2 + 40
    sheet.alpha_composite(small.resize((small.width * 4, small.height * 4), Image.NEAREST), (x, y))
    x += small.width * 4 + 40
    sheet.alpha_composite(small, (x, y))
    os.makedirs(os.path.dirname(SHEET), exist_ok=True)
    sheet.convert("RGB").save(SHEET)
    print(f"sheet: {SHEET} ({len(tiles)} tiles, {len(accents)} accents)")


main()
