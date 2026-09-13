#!/usr/bin/env python3
"""Review-only temporal inspection of Astra's vine-wind retile captures.

Reads the authoritative native PNG frames under
captures/vine-wind-retile-2026-09-13/<case>-<mode>-<old|new>/frame_NNNN.png and
writes contact sheets plus a JSON of independent temporal observations to
captures/fable-vine-wind-review-2026-09-13/. It never writes into the study
directory. Still frames only: nothing here claims to have watched motion.
"""
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
SRC = ROOT / "captures/vine-wind-retile-2026-09-13"
OUT = ROOT / "captures/fable-vine-wind-review-2026-09-13"
OUT.mkdir(parents=True, exist_ok=True)

CASES = [
    ("interior", "full", [338, 339, 340, 341, 342], [796, 797, 798, 799, 800]),
    ("interior", "growth", [338, 339, 340, 341, 342], [796, 797, 798, 799, 800]),
    ("vertex", "full", [315, 316, 317, 318, 319], [896, 897, 898, 899, 900]),
    ("right", "full", [315, 316, 317, 318, 319], [627, 628, 629, 630, 631]),
    ("vertex", "growth", [338, 339, 340, 341, 342], [796, 797, 798, 799, 800]),
    ("right", "growth", [338, 339, 340, 341, 342], [796, 797, 798, 799, 800]),
]
SCALE = 8


def load(case, mode, label, i):
    return np.asarray(
        Image.open(SRC / f"{case}-{mode}-{label}" / f"frame_{i:04d}.png").convert("RGB")
    ).astype(np.float64)


def stack(case, mode, label):
    return np.stack([load(case, mode, label, i) for i in range(1200)])  # (1200,128,256,3)


def luma(rgb):
    return rgb @ np.array([0.2126, 0.7152, 0.0722])


def bbox(mask, pad=3):
    ys, xs = np.nonzero(mask)
    return (
        max(ys.min() - pad, 0),
        min(ys.max() + pad + 1, mask.shape[0]),
        max(xs.min() - pad, 0),
        min(xs.max() + pad + 1, mask.shape[1]),
    )


def sheet(case, mode, old, new, frames, tag, box):
    y0, y1, x0, x1 = box
    rows = []
    for i in frames:
        a = old[i, y0:y1, x0:x1]
        b = new[i, y0:y1, x0:x1]
        d = np.clip(np.abs(a - b) * 4, 0, 255)
        sep = np.full((a.shape[0], 2, 3), 64.0)
        rows.append(np.concatenate([a, sep, b, sep, d], axis=1))
    hsep = np.full((2, rows[0].shape[1], 3), 64.0)
    img = rows[0]
    for r in rows[1:]:
        img = np.concatenate([img, hsep, r], axis=0)
    im = Image.fromarray(img.astype(np.uint8))
    im.save(OUT / f"{case}-{mode}-{tag}-native.png")
    im.resize((im.width * SCALE, im.height * SCALE), Image.NEAREST).save(
        OUT / f"{case}-{mode}-{tag}-x{SCALE}.png"
    )


def temporal(series):
    """Independent per-variant temporal observations over 1200 frames."""
    l1 = np.abs(np.diff(series, axis=0)).sum(axis=(1, 2, 3)) / 255.0
    # spike: how much one boundary exceeds the mean of its two neighbours
    spike = np.abs(l1[1:-1] - (l1[:-2] + l1[2:]) / 2)
    per_pixel_jump = np.abs(np.diff(series, axis=0)).max(axis=3)  # (1199,H,W)
    lum = luma(series)
    # sign flips of the per-pixel luma derivative: a crude high-frequency oscillation count
    d = np.diff(lum, axis=0)
    flips = ((d[1:] * d[:-1]) < -1e-6).sum()
    return {
        "adjacent_l1_mean": float(l1.mean()),
        "adjacent_l1_max": float(l1.max()),
        "adjacent_l1_max_boundary": int(l1.argmax()),
        "adjacent_l1_spike_max": float(spike.max()),
        "adjacent_l1_spike_boundary": int(spike.argmax() + 1),
        "per_pixel_jump_max_8bit": float(per_pixel_jump.max()),
        "per_pixel_jump_max_boundary": int(
            np.unravel_index(per_pixel_jump.argmax(), per_pixel_jump.shape)[0]
        ),
        "luma_derivative_sign_flips": int(flips),
        "lit_pixels_mean": float((lum > 2.55).sum(axis=(1, 2)).mean()),
        "lit_pixels_ever": int((lum.max(axis=0) > 2.55).sum()),
        "peak_luma_8bit": float(lum.max()),
        "luma_sum_mean": float(lum.sum(axis=(1, 2)).mean()),
    }


def lean(series, box, rows):
    """Horizontal luma-weighted centroid of a band of rows, per frame, in pixels."""
    y0, y1, x0, x1 = box
    band = luma(series[:, y0 + rows[0] : y0 + rows[1], x0:x1])
    xs = np.arange(x0, x1)
    w = band.sum(axis=1)  # (1200, W)
    tot = w.sum(axis=1)
    cx = (w * xs).sum(axis=1) / np.where(tot > 0, tot, 1)
    cx[tot == 0] = np.nan
    return cx


def main():
    report = {}
    for case, mode, peak, worst in CASES:
        old = stack(case, mode, "old")
        new = stack(case, mode, "new")
        ever = (luma(old).max(axis=0) > 0) | (luma(new).max(axis=0) > 0)
        box = bbox(ever)
        sheet(case, mode, old, new, peak, "peak", box)
        sheet(case, mode, old, new, worst, "worst", box)
        only_new = int(((luma(new).max(axis=0) > 0) & (luma(old).max(axis=0) == 0)).sum())
        only_old = int(((luma(old).max(axis=0) > 0) & (luma(new).max(axis=0) == 0)).sum())
        entry = {
            "bbox_y0_y1_x0_x1": [int(v) for v in box],
            "pixels_lit_only_in_new": only_new,
            "pixels_lit_only_in_old": only_old,
            "old": temporal(old),
            "new": temporal(new),
        }
        if mode == "full":
            # top twelve rows of the lit envelope: the tip region, where lean shows most
            cx_old = lean(old, box, (0, 12))
            cx_new = lean(new, box, (0, 12))
            still = np.nanmedian(cx_old[:120])  # first 120 frames are exactly quiet per study
            entry["tip_centroid_px"] = {
                "quiet_reference": float(still),
                "old_max_excursion": float(np.nanmax(np.abs(cx_old - still))),
                "new_max_excursion": float(np.nanmax(np.abs(cx_new - still))),
                "old_at_peak": float(cx_old[peak[2]] - still),
                "new_at_peak": float(cx_new[peak[2]] - still),
                "quiet_frames_identical": int(
                    (np.abs(old[:120] - new[:120]).max(axis=(1, 2, 3)) == 0).sum()
                ),
            }
        report[f"{case}-{mode}"] = entry
        print(case, mode, json.dumps(entry, indent=1))
        sys.stdout.flush()
    (OUT / "temporal-observations.json").write_text(json.dumps(report, indent=1))


if __name__ == "__main__":
    main()
