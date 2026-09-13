#!/usr/bin/env python3
"""Rain-response study sheets (art/studies/rain-response): from one paired capture
(`old/` and `new/` native 256×128 nets plus `timeline.json`), a comparison sheet and the
measurements the report quotes. Rendering only; it authors nothing.

  sheet.py CAPTURE_DIR --face F --cx CX --cy CY --ticks 300,330,360 [--radius 12] [--out PNG]

Rows: old native crop, new native crop, |new − old| ×8 (white = 1/8 of full scale), then
the same three at 4× nearest. One column per listed tick (frame 0 of the tick). Below,
the measured motion: per frame, how many pixels of the crop differ between old and new
and the largest channel difference; and the same between consecutive new frames.
"""
import argparse
import json
import os
import sys

from PIL import Image, ImageDraw

FACE_ORIGIN = {4: (64, 0), 3: (0, 64), 0: (64, 64), 1: (128, 64), 2: (192, 64)}
FRAMES_PER_TICK = 3


def crop(png, face, cx, cy, radius):
    im = Image.open(png).convert("RGB")
    ox, oy = FACE_ORIGIN[face]
    u, v = cx * 4 + 2, cy * 4 + 2
    box = (ox + max(0, u - radius), oy + max(0, v - radius), ox + min(64, u + radius), oy + min(64, v + radius))
    return im.crop(box)


def diff_image(a, b, gain=8):
    out = Image.new("RGB", a.size)
    pa, pb, po = a.load(), b.load(), out.load()
    changed, worst = 0, 0
    for y in range(a.size[1]):
        for x in range(a.size[0]):
            d = [abs(pa[x, y][c] - pb[x, y][c]) for c in range(3)]
            m = max(d)
            if m:
                changed += 1
                worst = max(worst, m)
            po[x, y] = tuple(min(255, v * gain) for v in d)
    return out, changed, worst


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("--face", type=int, required=True)
    ap.add_argument("--cx", type=int, required=True)
    ap.add_argument("--cy", type=int, required=True)
    ap.add_argument("--ticks", required=True, help="comma-separated elapsed ticks")
    ap.add_argument("--radius", type=int, default=12)
    ap.add_argument("--out")
    ap.add_argument("--motion", help="elapsed range a-b to measure frame-to-frame motion over")
    a = ap.parse_args()
    ticks = [int(t) for t in a.ticks.split(",")]
    timeline = {e["elapsed"]: e for e in json.load(open(os.path.join(a.capture, "timeline.json")))}
    scale = 4
    cols = []
    for t in ticks:
        n = t * FRAMES_PER_TICK
        old = crop(os.path.join(a.capture, "old", f"frame_{n:05}.png"), a.face, a.cx, a.cy, a.radius)
        new = crop(os.path.join(a.capture, "new", f"frame_{n:05}.png"), a.face, a.cx, a.cy, a.radius)
        d, changed, worst = diff_image(old, new)
        e = timeline.get(t, {})
        here = [c for c in e.get("cells", []) if c["face"] == a.face and c["cx"] == a.cx and c["cy"] == a.cy]
        lvl = here[0]["level"] if here else 0.0
        rate = here[0]["rate"] if here else 0.0
        cols.append((t, old, new, d, changed, worst, lvl, rate, e.get("max_rate", 0.0)))
    w = cols[0][1].size[0]
    h = cols[0][1].size[1]
    pad = 4
    colw = w * scale + pad
    sheet = Image.new("RGB", (colw * len(cols) + 100, h * 3 + h * scale * 3 + 90), (24, 24, 28))
    dr = ImageDraw.Draw(sheet)
    labels = ["old 1x", "new 1x", "diff x8", "old 4x", "new 4x", "diff 4x"]
    y = 12
    for r, label in enumerate(labels):
        s = 1 if r < 3 else scale
        dr.text((4, y), label, fill=(200, 200, 200))
        for i, col in enumerate(cols):
            im = col[1 + (r % 3)]
            if s > 1:
                im = im.resize((w * s, h * s), Image.NEAREST)
            sheet.paste(im, (100 + i * colw, y))
        y += h * s + 2
    for i, (t, _, _, _, changed, worst, lvl, rate, mx) in enumerate(cols):
        dr.text((100 + i * colw, y), f"t{t} lvl {lvl:.2f}", fill=(220, 220, 160))
        dr.text((100 + i * colw, y + 12), f"rate {rate:.3f}/{mx:.3f}", fill=(160, 200, 220))
        dr.text((100 + i * colw, y + 24), f"{changed}px max {worst}", fill=(220, 160, 160))
    out = a.out or os.path.join(a.capture, f"sheet-f{a.face}-{a.cx}-{a.cy}.png")
    sheet.save(out)
    print("sheet", out)
    for t, _, _, _, changed, worst, lvl, rate, mx in cols:
        print(f"  tick {t}: level {lvl:.3f} rate {rate:.3f} (max {mx:.3f}) old-vs-new {changed} px changed, worst channel {worst}/255")
    if a.motion:
        lo, hi = (int(x) for x in a.motion.split("-"))
        prev = None
        worst_new, worst_old, sum_new, n = 0, 0, 0, 0
        for t in range(lo, hi + 1):
            for k in range(FRAMES_PER_TICK):
                f = t * FRAMES_PER_TICK + k
                new = crop(os.path.join(a.capture, "new", f"frame_{f:05}.png"), a.face, a.cx, a.cy, a.radius)
                old = crop(os.path.join(a.capture, "old", f"frame_{f:05}.png"), a.face, a.cx, a.cy, a.radius)
                if prev is not None:
                    _, _, wn = diff_image(prev[0], new)
                    _, _, wo = diff_image(prev[1], old)
                    worst_new, worst_old = max(worst_new, wn), max(worst_old, wo)
                    sum_new += wn
                    n += 1
                prev = (new, old)
        print(f"  frame-to-frame over {lo}-{hi} ({n} steps at {FRAMES_PER_TICK}/tick): worst channel step new {worst_new}/255, old {worst_old}/255, mean new {sum_new / max(n, 1):.1f}")


if __name__ == "__main__":
    sys.exit(main())
