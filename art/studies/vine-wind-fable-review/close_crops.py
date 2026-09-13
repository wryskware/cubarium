#!/usr/bin/env python3
"""Second review pass: tight 12x crops of the vine column and its crown, per-row
displacement at the interior peak, and where the few new-only pixels are.

Quiet reference is frames 1080..1199: the recording starts at 30 s, the packet
rises 5 s, holds 8 s, falls 5 s and is exactly zero from 48 s (frame 1080).
"""
import json
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
SRC = ROOT / "captures/vine-wind-retile-2026-09-13"
OUT = ROOT / "captures/fable-vine-wind-review-2026-09-13"
SCALE = 12
LUMA = np.array([0.2126, 0.7152, 0.0722])


def load(case, mode, label, i):
    return np.asarray(
        Image.open(SRC / f"{case}-{mode}-{label}" / f"frame_{i:04d}.png").convert("RGB")
    ).astype(np.float64)


def crop_sheet(case, mode, frames, box, tag):
    y0, y1, x0, x1 = box
    rows = []
    for i in frames:
        a = load(case, mode, "old", i)[y0:y1, x0:x1]
        b = load(case, mode, "new", i)[y0:y1, x0:x1]
        d = np.clip(np.abs(a - b) * 4, 0, 255)
        sep = np.full((a.shape[0], 1, 3), 90.0)
        rows.append(np.concatenate([a, sep, b, sep, d], axis=1))
    hsep = np.full((1, rows[0].shape[1], 3), 90.0)
    img = rows[0]
    for r in rows[1:]:
        img = np.concatenate([img, hsep, r], axis=0)
    im = Image.fromarray(img.astype(np.uint8))
    im.resize((im.width * SCALE, im.height * SCALE), Image.NEAREST).save(
        OUT / f"close-{case}-{mode}-{tag}-x{SCALE}.png"
    )


def per_row_shift(case, mode, i, box):
    """Luma-weighted horizontal centroid per row, old and new, minus the quiet centroid."""
    y0, y1, x0, x1 = box
    xs = np.arange(x0, x1)
    quiet = np.mean([load(case, mode, "old", k)[y0:y1, x0:x1] @ LUMA for k in range(1080, 1200, 10)], axis=0)
    out = []
    for label in ["old", "new"]:
        f = load(case, mode, label, i)[y0:y1, x0:x1] @ LUMA
        row = []
        for r in range(f.shape[0]):
            wq, wf = quiet[r].sum(), f[r].sum()
            if wq <= 0 or wf <= 0:
                row.append(None)
                continue
            row.append(round(float((f[r] * xs).sum() / wf - (quiet[r] * xs).sum() / wq), 3))
        out.append(row)
    return {"rows_from_top": list(range(y0, y1)), "old": out[0], "new": out[1]}


def main():
    report = {}
    # interior column Front cx10: lit x 99..111, y 58..113 (crown on Top from y 58)
    ibox = (56, 116, 97, 114)
    crop_sheet("interior", "full", [338, 339, 340, 341, 342], ibox, "peak")
    crop_sheet("interior", "full", [796, 797, 798, 799, 800], ibox, "worst")
    crop_sheet("interior", "growth", [338, 339, 340, 341, 342], ibox, "peak")
    crop_sheet("interior", "growth", [796, 797, 798, 799, 800], ibox, "worst")
    # wide-apart frames at the same wind phase to show the sway envelope at native scale
    crop_sheet("interior", "full", [200, 340, 480, 620, 760], ibox, "sweep")
    report["interior-full-peak340-row-shift"] = per_row_shift("interior", "full", 340, ibox)
    vbox = (56, 116, 57, 75)
    crop_sheet("vertex", "full", [315, 316, 317, 318, 319], vbox, "peak")
    crop_sheet("vertex", "full", [896, 897, 898, 899, 900], vbox, "worst")
    crop_sheet("vertex", "growth", [1175, 1176, 1177, 1178, 1179], vbox, "jump1177")
    report["vertex-full-peak317-row-shift"] = per_row_shift("vertex", "full", 317, vbox)
    # Right cx15: stem near x 183..195, y 64..113; crown crosses onto Top near x 122, y 0
    crop_sheet("right", "full", [315, 316, 317, 318, 319], (60, 116, 180, 199), "peak-stem")
    crop_sheet("right", "full", [315, 316, 317, 318, 319], (0, 12, 119, 135), "peak-crown")
    crop_sheet("right", "full", [627, 628, 629, 630, 631], (60, 116, 180, 199), "worst-stem")
    crop_sheet("right", "full", [627, 628, 629, 630, 631], (0, 12, 119, 135), "worst-crown")

    # pixels ever lit only in the new variant: where and how bright
    for case, mode in [("interior", "full"), ("interior", "growth"), ("vertex", "full"), ("right", "full")]:
        mo = np.zeros((128, 256))
        mn = np.zeros((128, 256))
        for i in range(0, 1200):
            mo = np.maximum(mo, load(case, mode, "old", i) @ LUMA)
            mn = np.maximum(mn, load(case, mode, "new", i) @ LUMA)
        ys, xs = np.nonzero((mn > 0) & (mo == 0))
        report[f"{case}-{mode}-new-only-pixels"] = [
            {"x": int(x), "y": int(y), "max_luma_8bit": round(float(mn[y, x]), 2)} for y, x in zip(ys, xs)
        ]
        ys, xs = np.nonzero((mo > 0) & (mn == 0))
        report[f"{case}-{mode}-old-only-pixels"] = [{"x": int(x), "y": int(y)} for y, x in zip(ys, xs)]
        # exactness of the zero-wind tail
        same = sum(
            int(np.array_equal(load(case, mode, "old", i), load(case, mode, "new", i))) for i in range(1080, 1200)
        )
        report[f"{case}-{mode}-quiet-tail-identical-frames"] = same
    (OUT / "close-observations.json").write_text(json.dumps(report, indent=1))
    print(json.dumps(report, indent=1))


if __name__ == "__main__":
    main()
