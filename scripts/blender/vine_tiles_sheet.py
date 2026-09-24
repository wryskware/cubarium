"""Cut out and check the latticevine tile masters, then draw their contact sheet.

    python3 scripts/blender/vine_tiles_sheet.py MASTERS_DIR SHEET.png [OLD_MASTERS_DIR]

Run by `latticevine.py --tiles` after rendering. It forces every master to straight
cutout alpha (0 or 255; colour zeroed where transparent) and writes one sheet:

  * a 10 x 8 test wall from a random covered shape: masks from the shape's own adjacency
    (bit 0 up, 1 right, 2 down, 3 left), each shared edge's exit position hashed from the
    edge's position (as the presenter does), random densities, a few rooted foot cells,
    one hanging root under the top, random accents. On top: the same wall with the old
    per-mask tiles (OLD_MASTERS_DIR, names vine-<density>-<mask>.png) beside the new one,
    at 48 px (2x); then the new wall at 12 px (4x and native). The 12 px wall uses a
    stand-in 4:1 reduction (mean colour of the opaque pixels in each 4 x 4 block, opaque
    where at least half the block is), not the texture tool's;
  * the whole set at 1x, one block per kind and density, 81 tiles each, plus the accents.

The backdrop is a flat rock tone (#4F4670, the lineup's rock) so the cutout edges and the
dark runners read; the presenter draws the real rock.
"""

import os
import random
import sys
import zlib

from PIL import Image, ImageDraw

MASTERS, SHEET = sys.argv[1], sys.argv[2]
OLD = sys.argv[3] if len(sys.argv) > 3 else None
ROCK = (0x4F, 0x46, 0x70, 255)
BG = (0x0C, 0x08, 0x18, 255)
INK = (0xE6, 0xDD, 0xF8, 255)
DENS = ("full", "thin", "bare")
KINDS = ("plain", "climb", "hang")


def exit_combos(mask):
    bits = [e for e in range(4) if mask >> e & 1]
    return [sum(1 << b for k, b in enumerate(bits) if n >> k & 1) for n in range(1 << len(bits))]


def name(kind, dens, mask, exits):
    base = {"plain": "vine-", "climb": "vine-root-", "hang": "vine-root-"}[kind]
    return f"{base}{dens}-{mask:x}-{exits:x}{'-hang' if kind == 'hang' else ''}.png"


def old_name(kind, dens, mask):
    return {"plain": f"vine-{dens}-{mask:x}.png", "climb": f"vine-root-{dens}-{mask:x}.png",
            "hang": f"vine-root-{dens}-{mask:x}-hang.png"}[kind]


def cutout(path):
    im = Image.open(path).convert("RGBA")
    px = im.load()
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = px[x, y]
            px[x, y] = (r, g, b, 255) if a >= 128 else (0, 0, 0, 0)
    im.save(path)
    return im


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


def edge_bit(key):
    """The presenter's rule stand-in: a hash of the edge's world position -> exit position."""
    return zlib.crc32(repr(key).encode()) & 1


def wall_cells(W=10, H=8, seed=11):
    rng = random.Random(seed)
    cov = [[False] * W for _ in range(H)]
    roots = [(H - 1, 1), (H - 1, 5), (H - 1, 8)]
    for r, c in roots:
        cov[r][c] = True
    hang = (0, 3)
    cov[0][3] = True
    while sum(map(sum, cov)) < 52:
        r, c = rng.choice([(r, c) for r in range(H) for c in range(W) if cov[r][c]])
        dr, dc = rng.choice(((-1, 0), (-1, 0), (0, 1), (0, -1), (1, 0), (0, 1)))
        if 0 <= r + dr < H and 0 <= c + dc < W:
            cov[r + dr][c + dc] = True
    cells = []
    for r in range(H):
        for c in range(W):
            if not cov[r][c]:
                continue
            m = e = 0
            # horizontal edge between rows r-1 and r at column c is ("h", r, c); vertical
            # edge between columns c-1 and c at row r is ("v", r, c)
            nb = [(r > 0 and cov[r - 1][c], ("h", r, c)), (c < W - 1 and cov[r][c + 1], ("v", r, c + 1)),
                  (r < H - 1 and cov[r + 1][c], ("h", r + 1, c)), (c > 0 and cov[r][c - 1], ("v", r, c))]
            for bit, (on, key) in enumerate(nb):
                if on:
                    m |= 1 << bit
                    e |= edge_bit(key) << bit
            dens = "bare" if r == H - 1 and (r, c) not in roots else rng.choices(DENS, (7, 3, 1))[0]
            kind = "climb" if (r, c) in roots else ("hang" if (r, c) == hang else "plain")
            acc = rng.choices((None, "bud", "flower", "fruit"), (9, 1, 2, 2))[0]
            cells.append((r, c, kind, dens, m, e, acc))
    return W, H, cells


def assemble(W, H, cells, tile_of, accents, px=48):
    im = Image.new("RGBA", (W * px, H * px), ROCK)
    for r, c, kind, dens, m, e, acc in cells:
        im.alpha_composite(tile_of(kind, dens, m, e), (c * px, r * px))
        if acc:
            im.alpha_composite(accents[acc], (c * px, r * px))
    return im


def main():
    tiles, accents = {}, {}
    for k in KINDS:
        for d in DENS:
            for m in range(16):
                for e in exit_combos(m):
                    f = name(k, d, m, e)
                    tiles[f] = cutout(os.path.join(MASTERS, f))
    for a in ("bud", "flower", "fruit"):
        accents[a] = cutout(os.path.join(MASTERS, f"vine-accent-{a}.png"))
    bad = [f for f, im in list(tiles.items()) + list(accents.items()) if im.size != (48, 48)]
    if bad:
        sys.exit(f"wrong size: {bad}")
    extra = sorted(set(f for f in os.listdir(MASTERS) if f.endswith(".png")) - set(tiles)
                   - {f"vine-accent-{a}.png" for a in accents})
    if extra:
        sys.exit(f"unexpected files: {extra[:5]}")
    for k in KINDS:
        for d in DENS:
            fr = [sum(1 for p in tiles[name(k, d, 15, e)].getdata() if p[3]) / 2304 for e in range(16)]
            print(f"coverage {k:5s} {d}: mask f mean {sum(fr) / 16 * 100:.0f} %")

    W, H, cells = wall_cells()
    new = assemble(W, H, cells, lambda k, d, m, e: tiles[name(k, d, m, e)], accents)
    small_acc = {a: reduce4(im) for a, im in accents.items()}
    small = assemble(W, H, cells, lambda k, d, m, e: reduce4(tiles[name(k, d, m, e)]), small_acc, px=12)
    old = None
    if OLD:
        cache = {}

        def old_tile(k, d, m, e):
            f = old_name(k, d, m)
            if f not in cache:
                cache[f] = Image.open(os.path.join(OLD, f)).convert("RGBA")
            return cache[f]
        old_acc = {a: Image.open(os.path.join(OLD, f"vine-accent-{a}.png")).convert("RGBA") for a in accents}
        old = assemble(W, H, cells, old_tile, old_acc)

    S, G, PER = 48, 2, 27
    blocks = [(k, d) for k in KINDS for d in DENS]
    left = 130
    wall_w = new.width * 2
    top_w = (wall_w * 2 + 40) if old else wall_w
    set_w = left + PER * (S + G)
    Wd = max(top_w + 20, set_w + 20, small.width * 4 + small.width + 60)
    set_h = len(blocks) * (3 * (S + G) + 14) + S + 40
    Hd = 30 + new.height * 2 + 30 + small.height * 4 + 40 + set_h
    sheet = Image.new("RGBA", (Wd, Hd), BG)
    dr = ImageDraw.Draw(sheet)
    y = 8
    dr.text((10, y), "10 x 8 test wall, same shape, masks from adjacency, exits hashed per edge. "
                     + ("left: old tiles (one per mask), right: new tiles (exits, S-curves, per-file seeds). " if old else "")
                     + "48 px shown 2x on #4F4670 rock", fill=INK)
    y += 18
    x = 10
    if old:
        sheet.alpha_composite(old.resize((wall_w, old.height * 2), Image.NEAREST), (x, y))
        x += wall_w + 40
    sheet.alpha_composite(new.resize((wall_w, new.height * 2), Image.NEAREST), (x, y))
    y += new.height * 2 + 12
    dr.text((10, y), "new wall at 12 px: 4x | native", fill=INK)
    y += 16
    sheet.alpha_composite(small.resize((small.width * 4, small.height * 4), Image.NEAREST), (10, y))
    sheet.alpha_composite(small, (10 + small.width * 4 + 40, y))
    y += small.height * 4 + 30
    dr.text((10, y), "the whole set at 1x: each block is one kind and density, 81 tiles (every mask 0-f with every "
                     "exit combination, in order); name vine[-root]-<density>-<mask>-<exits>[-hang].png", fill=INK)
    y += 18
    for k, d in blocks:
        dr.text((10, y + S // 2), f"{'plain' if k == 'plain' else 'root ' + k} {d}", fill=INK)
        i = 0
        for m in range(16):
            for e in exit_combos(m):
                cell = Image.new("RGBA", (S, S), ROCK)
                cell.alpha_composite(tiles[name(k, d, m, e)])
                sheet.alpha_composite(cell, (left + (i % PER) * (S + G), y + (i // PER) * (S + G)))
                i += 1
        y += 3 * (S + G) + 14
    dr.text((10, y + S // 2), "accents", fill=INK)
    for i, a in enumerate(("bud", "flower", "fruit")):
        cell = Image.new("RGBA", (S, S), ROCK)
        cell.alpha_composite(accents[a])
        sheet.alpha_composite(cell, (left + i * (S + G), y))
    os.makedirs(os.path.dirname(SHEET), exist_ok=True)
    sheet.convert("RGB").save(SHEET)
    print(f"sheet: {SHEET} ({len(tiles)} tiles, {len(accents)} accents)")


main()
