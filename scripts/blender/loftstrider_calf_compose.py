"""Compose the loftstrider calf sheets from `loftstrider_calf_sheet.py`'s tiles (system
Python, for PIL).

    python3 scripts/blender/loftstrider_calf_compose.py DIR

Writes into DIR:
  loftstrider-calf-true-size.png    each calf next to the adult, one camera (true size)
  loftstrider-calf-same-height.png  the same pairs, each framed on its own height
  loftstrider-calf-blend-<calf>.png the calf blended to the adult across the five bins
The first row of the two pair sheets is today: the adult, the one bin every loftstrider
draws, next to the adult scaled by 0.464 (how the founders' bins are made).
"""

import json
import os
import sys

from PIL import Image, ImageDraw

D = sys.argv[1]
T = os.path.join(D, "tiles")
META = json.load(open(os.path.join(D, "meta.json")))
BG, INK, HEAD, DIM = (24, 20, 34), (230, 221, 248), (255, 216, 96), (160, 150, 185)
COLS = [("vox", "side", "voxels 0.125 m, side"), ("vox", "q", "voxels 0.125 m, three-quarter"),
        ("bl", "side", "Blender, side"), ("bl", "q", "Blender, three-quarter")]
# the candidates in the order rendered (an eased blend's alias `<calf>e` is not a row)
CALVES = [k for k in META if k.startswith("calf-") and "-bin" not in k and not k.endswith("e")]
BLENDS = [k for k in META if k.startswith("calf-") and f"{k}-bin1" in META]
LABEL_W, PAD = 300, 10


def tile(name, kind, view, frame):
    return Image.open(os.path.join(T, f"{name}_{kind}_{view}_{frame}.png")).convert("RGB")


def counts(m):
    s = m["sectors"]
    return f"eyes {s['eyes']}  tusks {s['tusks']}  fronds {s['fronds']}  feet {s['feet']}"


def describe(name):
    m = META[name]
    f = m["factors"]
    return [f"{m['height_m']:.2f} m tall, {m['cells']} cells, {m['span']} vox",
            f"of the adult: body {f['body']:.2f}  legs {f['leg']:.2f} (thick {f['leg_r']:.2f})",
            f"neck {f['neck']:.2f} (thick {f['neck_r']:.2f})  head {f['head']:.2f}",
            f"fronds {f['frond']:.2f}  tusks {f['tusk']:.2f} of head  eyes {f['eye']:.2f}",
            f"azimuths kept at 0.125 m: {counts(m)}"]


def pair_sheet(frame, title, out):
    rows = [("scaled", "TODAY", ["left: the adult, the one bin every loftstrider draws",
                                 "(specimen bake: newborns draw it at full size)",
                                 "right: the adult x 0.464, how the founders'",
                                 "bins are made (not baked for the loftstrider)"] + describe("scaled")[:1]
             + [describe("scaled")[-1]])]
    names = {"calf-a": "A  mild", "calf-b": "B  middle", "calf-c": "C  extreme",
             "calf-l46": "B body, legs 0.46", "calf-l38": "B body, legs 0.38",
             "calf-l30": "B body, legs 0.30"}
    rows += [(c, names.get(c, c), describe(c)) for c in CALVES]
    tw, th = tile("adult", "vox", "side", frame).size
    pw = 2 * tw
    W = LABEL_W + len(COLS) * (pw + PAD) + PAD
    H = 60 + len(rows) * (th + 30)
    sheet = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(sheet)
    d.text((PAD, 12), title, fill=HEAD)
    d.text((PAD, 30), "each pair: adult (left) | candidate (right); candidates only, Wrysk picks", fill=DIM)
    for ci, (_, _, cap) in enumerate(COLS):
        d.text((LABEL_W + ci * (pw + PAD), 44), cap, fill=INK)
    for ri, (name, label, lines) in enumerate(rows):
        y = 60 + ri * (th + 30)
        d.text((PAD, y + 10), label, fill=HEAD)
        for li, line in enumerate(lines):
            d.text((PAD, y + 34 + li * 16), line, fill=INK)
        for ci, (kind, view, _) in enumerate(COLS):
            x = LABEL_W + ci * (pw + PAD)
            sheet.paste(tile("adult", kind, view, frame), (x, y))
            sheet.paste(tile(name, kind, view, frame), (x + tw, y))
    sheet.save(out)
    print(out)


def blend_sheet(calf, out):
    bins = [calf, f"{calf}-bin1", f"{calf}-bin2", f"{calf}-bin3", "adult"]
    rows = [("vox", "side", "voxels, side"), ("vox", "q", "voxels, three-quarter"), ("bl", "q", "Blender, three-quarter")]
    tw, th = tile("adult", "vox", "side", "true").size
    W = 160 + len(bins) * (tw + PAD)
    H = 70 + len(rows) * (th + PAD) + 116
    sheet = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(sheet)
    s0 = META["scaled"]["factors"]["body"]
    d.text((PAD, 12), f"loftstrider {calf}: blend to the adult across the five size bins (true size, one camera)", fill=HEAD)
    eased = [k for k in META[calf]["factors"] if META[f"{calf}-bin1"]["factors"][k] != round(
        META[calf]["factors"][k] + (1 - META[calf]["factors"][k]) * 0.25, 3)]
    how = (f"moves linearly from the calf's to 1 at t = n/4" if not eased else
           f"moves linearly from the calf's to 1 at t = n/4, except {', '.join(eased)} (eased, see bins)")
    d.text((PAD, 30), f"bin n: length factor s = {s0:.3f} + (1 - {s0:.3f}) n/4 (today's founder rule); every part factor "
                      + how, fill=DIM)
    for bi, name in enumerate(bins):
        x = 160 + bi * (tw + PAD)
        s = s0 + (1 - s0) * bi / 4
        d.text((x, 50), f"bin {bi}  s {s:.2f}  t {bi / 4:.2f}", fill=INK)
        for ri, (kind, view, cap) in enumerate(rows):
            y = 70 + ri * (th + PAD)
            if bi == 0:
                d.text((PAD, y + th // 2), cap, fill=INK)
            sheet.paste(tile(name, kind, view, "true"), (x, y))
        m = META[name]
        y = 70 + len(rows) * (th + PAD)
        for li, line in enumerate([f"{m['height_m']:.2f} m, {m['cells']} cells", m["span"] + " vox",
                                   f"legs {m['factors']['leg']:.2f}  body {m['factors']['body']:.2f}",
                                   counts(m)[:counts(m).index("fronds")], counts(m)[counts(m).index("fronds"):]]):
            d.text((x, y + li * 16), line, fill=INK)
    sheet.save(out)
    print(out)


def spread_sheet(out):
    """Round 3: the adult once, then l38 at each spread, every tile framed on its own height."""
    rows = [k for k in META if k == "adult" or k.startswith("calf-l38")]
    tw, th = tile("adult", "vox", "side", "fit").size
    W = LABEL_W + len(COLS) * (tw + PAD) + PAD
    H = 60 + len(rows) * (th + 30)
    sheet = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(sheet)
    d.text((PAD, 12), "loftstrider calf l38: leg spread comparison, SCALED TO THE SAME HEIGHT", fill=HEAD)
    d.text((PAD, 30), "splay = knee/ankle reach beyond the hip, toe = toe length (both x l38's); candidates only", fill=DIM)
    for ci, (_, _, cap) in enumerate(COLS):
        d.text((LABEL_W + ci * (tw + PAD), 44), cap, fill=INK)
    for ri, name in enumerate(rows):
        y = 60 + ri * (th + 30)
        f = META[name]["factors"]
        label = "ADULT (reference)" if name == "adult" else (
            f"l38  splay {f.get('splay', 1):.2f}  toe {f.get('toe', 1):.2f}")
        d.text((PAD, y + 10), label, fill=HEAD)
        m = META[name]
        for li, line in enumerate([f"{m['height_m']:.2f} m, {m['cells']} cells, {m['span']} vox",
                                   f"azimuths kept: {counts(m)}", f"foot pieces: {m['pieces']['feet']}"]):
            d.text((PAD, y + 34 + li * 16), line, fill=INK)
        for ci, (kind, view, _) in enumerate(COLS):
            sheet.paste(tile(name, kind, view, "fit"), (LABEL_W + ci * (tw + PAD), y))
    sheet.save(out)
    print(out)


if "calf-l38-s60" in META:
    spread_sheet(os.path.join(D, "loftstrider-calf-l38-spread.png"))
    sys.exit(0)

pair_sheet("true", "loftstrider calf candidates next to the adult, TRUE RELATIVE SIZE (one camera, ground aligned)",
           os.path.join(D, "loftstrider-calf-true-size.png"))
pair_sheet("fit", "loftstrider calf candidates next to the adult, SCALED TO THE SAME HEIGHT",
           os.path.join(D, "loftstrider-calf-same-height.png"))
for c in BLENDS:
    blend_sheet(c, os.path.join(D, f"loftstrider-calf-blend-{c}.png"))
