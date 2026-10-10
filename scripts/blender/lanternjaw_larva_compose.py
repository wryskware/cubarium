"""Compose the lanternjaw larva contact sheet from `lanternjaw_larva_sheet.py`'s tiles
(system Python, for PIL).

    python3 scripts/blender/lanternjaw_larva_compose.py DIR

Writes DIR/sheet.png: one row per candidate (tick, grub, nymph, leech), columns: on an
adult frondgrazer three-quarter | the same from the side (true scale, one camera scale) |
the larva alone, voxels, enlarged | its Blender model | the smallest adult lanternjaw bin
at the on-host scale.
"""

import json
import os
import sys

from PIL import Image, ImageDraw

D = sys.argv[1]
T = os.path.join(D, "tiles")
META = json.load(open(os.path.join(D, "meta.json")))
KINDS = ["tick", "grub", "nymph", "leech"]
BG, INK, HEAD, DIM = (24, 20, 34), (230, 221, 248), (255, 216, 96), (160, 150, 185)
ROW_H, PAD, LABEL_W, TOP = 300, 10, 210, 64
LOOK = {
    "tick": "flat plated clamp, fangs dug in, one tail nub + lure dot",
    "grub": "4 segments arched like an inchworm, pale head, cyan tail tip",
    "nymph": "miniature adult: big head + fangs, folded arms, legs round the flanks, upright stub + bud",
    "leech": "1-wide body along the spine, pale sucker, lifted end with two lure buds",
}


def tile(name):
    im = Image.open(os.path.join(T, name)).convert("RGBA")
    bg = Image.new("RGBA", im.size, (244, 242, 248, 255))
    im = Image.alpha_composite(bg, im).convert("RGB")
    return im.resize((round(im.width * ROW_H / im.height), ROW_H), Image.LANCZOS)


def wrap(text, n):
    words, lines, cur = text.split(), [], ""
    for w in words:
        if len(cur) + len(w) + 1 > n:
            lines.append(cur)
            cur = w
        else:
            cur = f"{cur} {w}".strip()
    return lines + [cur]


cols = [("{k}-host-q.png", "on adult frondgrazer, three-quarter (true scale)"),
        ("{k}-host-side.png", "on adult frondgrazer, side (true scale)"),
        ("{k}-vox.png", "larva alone, voxels 0.125 m (enlarged)"),
        ("{k}-model.png", "larva alone, Blender model"),
        ("adult-bin0-q.png", "smallest adult bin (drop-off), true scale")]
rows = [[tile(c.format(k=k)) for c, _ in cols] for k in KINDS]
widths = [max(r[i].width for r in rows) for i in range(len(cols))]
sheet = Image.new("RGB", (LABEL_W + sum(widths) + PAD * (len(cols) + 1),
                          TOP + len(KINDS) * (ROW_H + PAD) + PAD), BG)
d = ImageDraw.Draw(sheet)
a = META["adult_bin0"]
d.text((PAD, 10), "Lanternjaw larva candidates (rider stage), 2026-10-09: for Wrysk to pick", fill=HEAD)
d.text((PAD, 28), f"host: adult frondgrazer, {META['host']['cells']} cells, {META['host']['span']} voxels;  "
                  f"smallest adult lanternjaw bin: {a['length_m']} m, {a['cells']} cells, {a['span']} voxels;  "
                  "voxel 0.125 m", fill=DIM)
x = LABEL_W + PAD
for (_, cap), w in zip(cols, widths):
    d.text((x, 46), cap, fill=INK)
    x += w + PAD
for r, (k, ims) in enumerate(zip(KINDS, rows)):
    y = TOP + r * (ROW_H + PAD)
    m = META[k]
    d.text((PAD, y + 4), f"{r + 1}. {k}", fill=HEAD)
    lines = [f"{m['cells']} cells, {m['span']} voxels",
             f"(L x W x H, incl. {m['bite_cells']} bite cells)",
             f"built length {m['built_length_m']} m",
             f"voxel length {m['voxel_length_m']} m", ""] + wrap(LOOK[k], 30)
    for n, line in enumerate(lines):
        d.text((PAD, y + 24 + 16 * n), line, fill=INK if n < 4 else DIM)
    x = LABEL_W + PAD
    for im, w in zip(ims, widths):
        sheet.paste(im, (x, y))
        x += w + PAD
sheet.save(os.path.join(D, "sheet.png"))
print(os.path.join(D, "sheet.png"))
