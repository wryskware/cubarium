#!/usr/bin/env python3
"""Motion and readability measurements for a rain-response review capture
(art/studies/rain-response-review). Reads a capture directory written by
`rain_review_capture` (`old/`, `new/`, optional `restart/`) and reports, for a crop:

  static   mean |new − old| over the crop per frame (how much the candidate changes
           a still frame), and the count of pixels changed by ≥ 16/255;
  flicker  mean |D_t − D_{t−1}| with D = new − old, i.e. the frame-to-frame motion
           the candidate ADDS on top of the streaks (which cancel in D); and the same
           for old alone (the streaks and wind), for scale;
  spectrum the dominant frequency of the candidate's added motion (FFT of the crop
           mean of D over the window, at 60 fps);
  restart  where a `restart/` stream exists: |restart − new| per frame from its start,
           and the first frame at which the two streams become byte-identical.

  motion.py CAPTURE --face F --cx CX --cy CY [--radius 12] [--from A --to B] [--label X]

Prints one JSON object (also written to CAPTURE/motion-<label>.json). Rendering-free.
"""
import argparse
import json
import math
import os
import sys

from PIL import Image

FACE_ORIGIN = {4: (64, 0), 3: (0, 64), 0: (64, 64), 1: (128, 64), 2: (192, 64)}
FPT = 3


def crop_box(face, cx, cy, radius):
    ox, oy = FACE_ORIGIN[face]
    u, v = cx * 4 + 2, cy * 4 + 2
    return (ox + max(0, u - radius), oy + max(0, v - radius), ox + min(64, u + radius), oy + min(64, v + radius))


def load(path, box):
    im = Image.open(path).convert("RGB").crop(box)
    return [p for px in im.getdata() for p in px]


def frame_range(capture, sub):
    names = sorted(n for n in os.listdir(os.path.join(capture, sub)) if n.startswith("frame_"))
    nums = [int(n[6:11]) for n in names]
    return nums


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("--face", type=int, required=True)
    ap.add_argument("--cx", type=int, required=True)
    ap.add_argument("--cy", type=int, required=True)
    ap.add_argument("--radius", type=int, default=12)
    ap.add_argument("--from", dest="lo", type=int)
    ap.add_argument("--to", dest="hi", type=int)
    ap.add_argument("--label", default="crop")
    a = ap.parse_args()
    box = crop_box(a.face, a.cx, a.cy, a.radius)
    nums = frame_range(a.capture, "new")
    if a.lo is not None:
        nums = [n for n in nums if a.lo * FPT <= n <= a.hi * FPT + FPT - 1]
    n_px = (box[2] - box[0]) * (box[3] - box[1])
    prev_d = None
    prev_old = None
    static_sum, static_changed16, static_max = 0.0, 0, 0
    flick_new, flick_old, flick_max_new, flick_max_old = 0.0, 0.0, 0, 0
    means = []
    steps = 0
    for n in nums:
        name = f"frame_{n:05}.png"
        old = load(os.path.join(a.capture, "old", name), box)
        new = load(os.path.join(a.capture, "new", name), box)
        d = [x - y for x, y in zip(new, old)]
        static_sum += sum(abs(x) for x in d) / len(d)
        static_changed16 += sum(1 for i in range(0, len(d), 3) if max(abs(d[i]), abs(d[i + 1]), abs(d[i + 2])) >= 16)
        static_max = max(static_max, max(abs(x) for x in d))
        means.append(sum(d) / len(d))
        if prev_d is not None:
            dd = [x - y for x, y in zip(d, prev_d)]
            flick_new += sum(abs(x) for x in dd) / len(dd)
            flick_max_new = max(flick_max_new, max(abs(x) for x in dd))
            do = [x - y for x, y in zip(old, prev_old)]
            flick_old += sum(abs(x) for x in do) / len(do)
            flick_max_old = max(flick_max_old, max(abs(x) for x in do))
            steps += 1
        prev_d, prev_old = d, old
    frames = len(nums)
    # Dominant added-motion frequency: a plain DFT of the crop-mean difference at 60 fps.
    spectrum = None
    if frames >= 60:
        m = [x - sum(means) / frames for x in means]
        best = (0.0, 0.0)
        for k in range(1, frames // 2):
            re = sum(v * math.cos(2 * math.pi * k * i / frames) for i, v in enumerate(m))
            im = sum(v * math.sin(2 * math.pi * k * i / frames) for i, v in enumerate(m))
            power = re * re + im * im
            if power > best[1]:
                best = (k * 60.0 / frames, power)
        spectrum = {"dominant_hz": round(best[0], 2)}
    restart = None
    if os.path.isdir(os.path.join(a.capture, "restart")):
        rn = frame_range(a.capture, "restart")
        rn = [n for n in rn if n in set(nums)] if a.lo is not None else rn
        first_same = None
        first_diff = None
        worst = 0
        series = []
        for n in rn:
            name = f"frame_{n:05}.png"
            new = load(os.path.join(a.capture, "new", name), box)
            r = load(os.path.join(a.capture, "restart", name), box)
            d = max(abs(x - y) for x, y in zip(new, r))
            series.append(d)
            if first_diff is None:
                first_diff = d
            worst = max(worst, d)
            if d == 0 and first_same is None:
                first_same = n
            if d != 0:
                first_same = None
        restart = {
            "start_frame": rn[0] if rn else None,
            "first_frame_max_channel_diff": first_diff,
            "worst_channel_diff": worst,
            "identical_from_frame": first_same,
            "seconds_to_identical": None if first_same is None or not rn else round((first_same - rn[0]) / 60.0, 2),
            "diff_by_tick": [max(series[i:i + FPT]) for i in range(0, len(series), FPT)][:40],
        }
    out = {
        "capture": a.capture, "label": a.label, "crop": {"face": a.face, "cx": a.cx, "cy": a.cy, "radius": a.radius, "pixels": n_px},
        "frames": frames, "fps": 60,
        "static": {"mean_abs_diff": round(static_sum / max(frames, 1), 3), "pixels_changed_ge16_per_frame": round(static_changed16 / max(frames, 1), 1), "max_channel_diff": static_max},
        "flicker": {"added_by_candidate_mean": round(flick_new / max(steps, 1), 3), "added_by_candidate_max": flick_max_new, "original_mean": round(flick_old / max(steps, 1), 3), "original_max": flick_max_old},
        "spectrum": spectrum,
        "restart": restart,
    }
    with open(os.path.join(a.capture, f"motion-{a.label}.json"), "w") as f:
        json.dump(out, f, indent=1)
    print(json.dumps(out))


if __name__ == "__main__":
    sys.exit(main())
