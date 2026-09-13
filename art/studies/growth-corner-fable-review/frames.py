#!/usr/bin/env python3
"""Independent measurements and contact strips for the growth-corner study's paired
frame sequence (art/studies/growth-corner-fable-review). Reads the committed study's
`selected-left-60fps/NNNN.png` pairs (left half original, right half candidate; each a
256×128 net) without modifying them, and writes NEW artifacts to an output directory:

  strips   8× nearest crops of the Top corner, the Left crown and the Back seam edge at
           chosen frames: original row, candidate row, |diff|×4 row;
  series   per frame: frame-to-frame max/mean change for original and candidate (the
           pop), original-vs-candidate max/mean and pixel count (where they differ), and
           the Top-face lit-pixel count of each (does the candidate light the top earlier
           or elsewhere: a ghost crown would show here);
  nets     full paired nets at a few frames, 3× nearest.

  frames.py CAPTURE_DIR OUT_DIR [--frames 0,1,60,119,120,121,179,180,181,219,220,221,240]
"""
import argparse
import json
import os
import sys

from PIL import Image, ImageDraw

# Net layout of a 256×128 net: face origins.
FACE_ORIGIN = {"Top": (64, 0), "Left": (0, 64), "Front": (64, 64), "Right": (128, 64), "Back": (192, 64)}
# Regions around the selected Left0 column's crown: Top corner (Top x 0..12, y 0..16),
# Left crown (Left x 0..12, y 0..16), Back seam edge (Back x 52..64, y 0..16).
REGIONS = [
    ("Top corner", (64, 0, 76, 16)),
    ("Left crown", (0, 64, 12, 80)),
    ("Back seam", (244, 64, 256, 80)),
]


def halves(path):
    im = Image.open(path).convert("RGB")
    return im.crop((0, 0, 256, 128)), im.crop((256, 0, 512, 128))


def diff_stats(a, b):
    pa, pb = a.load(), b.load()
    w, h = a.size
    worst, total, count = 0, 0, 0
    where = None
    for y in range(h):
        for x in range(w):
            d = max(abs(pa[x, y][c] - pb[x, y][c]) for c in range(3))
            if d:
                count += 1
                total += d
            if d > worst:
                worst, where = d, (x, y)
    return worst, (total / (w * h)), count, where


def lit_top(im):
    px = im.load()
    n = 0
    for y in range(64):
        for x in range(64):
            if px[64 + x, y] != (0, 0, 0):
                n += 1
    return n


def diff_image(a, b, gain=4):
    out = Image.new("RGB", a.size)
    pa, pb, po = a.load(), b.load(), out.load()
    for y in range(a.size[1]):
        for x in range(a.size[0]):
            po[x, y] = tuple(min(255, abs(pa[x, y][c] - pb[x, y][c]) * gain) for c in range(3))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("out")
    ap.add_argument("--frames", default="0,1,2,30,60,90,119,120,121,150,179,180,181,200,219,220,221,230,240")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    frames = sorted(int(n[:4]) for n in os.listdir(a.capture) if n.endswith(".png"))
    series = []
    prev = None
    for k in frames:
        o, c = halves(os.path.join(a.capture, f"{k:04}.png"))
        row = {"frame": k, "height": round(7 + k / 120, 4)}
        w, m, n, where = diff_stats(o, c)
        row["orig_vs_cand"] = {"max": w, "mean": round(m, 3), "pixels": n, "worst_at": where}
        row["lit_top"] = {"orig": lit_top(o), "cand": lit_top(c)}
        if prev is not None:
            wo, mo, no, _ = diff_stats(prev[0], o)
            wc, mc, nc, _ = diff_stats(prev[1], c)
            row["step"] = {"orig": {"max": wo, "mean": round(mo, 3), "pixels": no}, "cand": {"max": wc, "mean": round(mc, 3), "pixels": nc}}
        series.append(row)
        prev = (o, c)
    with open(os.path.join(a.out, "series.json"), "w") as f:
        json.dump(series, f, indent=1)
    # Summary lines.
    steps = [r for r in series if "step" in r]
    top_o = sorted(steps, key=lambda r: -r["step"]["orig"]["max"])[:5]
    top_c = sorted(steps, key=lambda r: -r["step"]["cand"]["max"])[:5]
    print("largest frame-to-frame steps, original:", [(r["frame"], r["height"], r["step"]["orig"]["max"], r["step"]["orig"]["pixels"]) for r in top_o])
    print("largest frame-to-frame steps, candidate:", [(r["frame"], r["height"], r["step"]["cand"]["max"], r["step"]["cand"]["pixels"]) for r in top_c])
    over = {t: (sum(1 for r in steps if r["step"]["orig"]["max"] >= t), sum(1 for r in steps if r["step"]["cand"]["max"] >= t)) for t in (16, 32, 64, 96)}
    print("frames whose step reaches >= t (original, candidate):", over)
    diffs = [r for r in series if r["orig_vs_cand"]["pixels"] > 0]
    print("frames where original and candidate differ:", len(diffs), "of", len(series), "; first", diffs[0]["frame"] if diffs else None, "last", diffs[-1]["frame"] if diffs else None)
    print("largest original-vs-candidate:", [(r["frame"], r["orig_vs_cand"]["max"], r["orig_vs_cand"]["pixels"], r["orig_vs_cand"]["worst_at"]) for r in sorted(series, key=lambda r: -r["orig_vs_cand"]["max"])[:5]])
    first_lit_o = next((r["frame"] for r in series if r["lit_top"]["orig"] > 0), None)
    first_lit_c = next((r["frame"] for r in series if r["lit_top"]["cand"] > 0), None)
    print("first frame with any lit Top pixel: original", first_lit_o, "candidate", first_lit_c)
    print("lit Top pixels at frames 0/60/120/180/240:", [(r["frame"], r["lit_top"]["orig"], r["lit_top"]["cand"]) for r in series if r["frame"] in (0, 60, 120, 180, 240)])
    # Contact strips.
    picks = [int(x) for x in a.frames.split(",")]
    scale = 8
    cw = sum(box[2] - box[0] for _, box in REGIONS) + 2 * (len(REGIONS) - 1)
    ch = 16
    colw = cw * scale + 6
    sheet = Image.new("RGB", (colw * len(picks) + 80, (ch * scale + 4) * 3 + 60), (22, 22, 26))
    dr = ImageDraw.Draw(sheet)
    for r, label in enumerate(["original", "candidate", "diff x4"]):
        dr.text((4, 10 + r * (ch * scale + 4) + ch * scale // 2), label, fill=(200, 200, 200))
    for i, k in enumerate(picks):
        o, c = halves(os.path.join(a.capture, f"{k:04}.png"))
        d = diff_image(o, c)
        for r, im in enumerate([o, c, d]):
            x0 = 80 + i * colw
            xx = 0
            for _, box in REGIONS:
                crop = im.crop(box).resize(((box[2] - box[0]) * scale, ch * scale), Image.NEAREST)
                sheet.paste(crop, (x0 + xx, 10 + r * (ch * scale + 4)))
                xx += (box[2] - box[0] + 2) * scale
        st = next(rr for rr in series if rr["frame"] == k)
        dr.text((80 + i * colw, 10 + 3 * (ch * scale + 4)), f"f{k} h{st['height']} diff {st['orig_vs_cand']['max']}/{st['orig_vs_cand']['pixels']}px", fill=(220, 220, 160))
        if "step" in st:
            dr.text((80 + i * colw, 22 + 3 * (ch * scale + 4)), f"step o{st['step']['orig']['max']} c{st['step']['cand']['max']}", fill=(160, 200, 220))
    dr.text((80, 2), "regions per column: Top corner (0..12,0..16) | Left crown (0..12,0..16) | Back seam (52..64,0..16); 8x nearest", fill=(160, 160, 160))
    sheet.save(os.path.join(a.out, "corner-strips-8x.png"))
    # Full nets at a few frames, 3×.
    for k in (0, 120, 180, 220, 240):
        im = Image.open(os.path.join(a.capture, f"{k:04}.png")).convert("RGB")
        im.resize((im.size[0] * 3, im.size[1] * 3), Image.NEAREST).save(os.path.join(a.out, f"net-pair-{k:04}-3x.png"))
    print("wrote", os.path.join(a.out, "corner-strips-8x.png"), "and net pairs")


if __name__ == "__main__":
    sys.exit(main())
