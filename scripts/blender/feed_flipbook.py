"""FEED-RIG arm A, the flipbook (harness only, `design/handoffs/feeding-motion-rig-2026-10-09.md`).

    blender -b -P scripts/blender/feed_flipbook.py -- OUT.json [--vox 0.125] [--founder NAME]

A rigged founder's feeding clip (`--founder frondgrazer`, the default:
`animal_models.fg_feed_clip`; `--founder chorister`, the Veilhound: `ch_feed_clip`; the same
armature and action the bake samples into `<name>.rig`) re-voxelized at every sampled frame
with the bake's own rule (`bake_voxel_models.voxelize_animal`): each part moved rigidly by
its bone's deformation (`animal_models.fg_feed_pose`, the renderer's blend; a part split
across a joint, `animal_models.bone_ahead`, vertex by vertex by its bones), then voxelized.

- **Per bin** (a founder's growth bins, or a specimen's young and adult bins), one bout on a timeline at `FG_FEED_FPS`: the reach (the clip weight eased in
  over `FG_FEED_EASE_S`), one loop, the return; its frame count, distinct frames and the
  cells the distinct frames hold: what a flipbook would ship.
- **For the harness** (`examples/voxel_feed_capture.rs`), the adult bin at every clip frame
  `j` and every weight step `k` of the ease (`w = smoothstep(k / levels)`): the frame an
  animal at weight step `k` and clip frame `j` draws. Distinct frames are stored once.

Not a shipped format: the JSON lives with the capture's outputs.
"""

import json
import math
import os
import sys
import time

from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import animal_models as A  # noqa: E402
import bake_voxel_models as B  # noqa: E402

ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []


def smooth(x):
    x = min(max(x, 0.0), 1.0)
    return x * x * (3 - 2 * x)


# founder: its clip, its frame rate
CLIPS = {"frondgrazer": (A.fg_feed_clip, A.FG_FEED_FPS), "chorister": (A.ch_feed_clip, A.CH_FEED_FPS)}


def bins_of(name, vox):
    """Per bin, a builder of (length, its objects): a founder's growth bins as the bake
    builds them, or a specimen's young and adult bins."""
    with open(B.SOURCE) as fh:
        src = json.load(fh, parse_constant=float)
    f = next((f for f in src["founders"] if f["name"] == name), None)
    if f is not None:
        return [lambda n=n: B.build_founder_bin(f, n, vox)[::2] for n in range(B.BINS)]
    _, l0, w0, h0 = A.REF[name]

    def specimen(s):
        B.reset_scene()
        return l0 * s, A.build(name, 0.5 * vox, 0.5 * vox, l0 * s, w0 * s, h0 * s)
    return [lambda s=s: specimen(s) for s in sorted(B.SPECIMEN_YOUNG.get(name, [])) + [1.0]]


def main():
    out = ARGS[0]
    vox = float(ARGS[ARGS.index("--vox") + 1]) if "--vox" in ARGS else 0.125
    name = ARGS[ARGS.index("--founder") + 1] if "--founder" in ARGS else "frondgrazer"
    clip_of, fps = CLIPS[name]
    l0 = A.REF[name][1]
    builders = bins_of(name, vox)
    B.reset_scene()
    clip = clip_of()
    loop = len(clip["frames"])
    levels = round(A.FG_FEED_EASE_S * fps)
    t0 = time.time()
    swap = A.SWAP_YZ.to_3x3()
    bone_index = {n: i for i, (n, _) in enumerate(clip["bones"])}

    frames, seen = [], {}

    def keep(cells):
        key = tuple(sorted(cells))
        if key not in seen:
            seen[key] = len(frames)
            frames.append([list(c) for c in key])
        return seen[key]

    report = []
    grid = None
    for n, build in enumerate(builders):
        length, objs = build()
        s = length / l0
        offset = Vector((0.5 * vox, 0.5 * vox, 0.0))
        rest = []
        for ob in objs:
            co = [0.0] * (3 * len(ob.data.vertices))
            ob.data.vertices.foreach_get("co", co)
            rest.append(co)

        def voxelized(j, w):
            pose = A.fg_feed_pose(clip, j, w)
            for ob, co in zip(objs, rest):
                d = pose[bone_index[ob.get("bone", "root")]]
                moved = []
                for i in range(0, len(co), 3):
                    v = Vector(co[i:i + 3])
                    if "bone_cut_x" in ob:
                        d = pose[bone_index[A.part_bone(ob, v)]]
                    m = swap @ ((v - offset) / s)
                    p = swap @ (d @ m)
                    moved.extend(p * s + offset)
                ob.data.vertices.foreach_set("co", moved)
                ob.data.update()
            raw = B.voxelize_animal(objs, vox, 0)
            return tuple((i, k, jj, mat, 0x30 if B.animal_role(mat) == "accent" else 0)
                         for (i, jj, k), mat in raw.items())

        cache = {}

        def frame(j, k):
            if k == 0:
                j = 0
            if (j, k) not in cache:
                cache[(j, k)] = voxelized(j, smooth(k / levels))
            return cache[(j, k)]

        # one bout from clip frame 0: reach, a loop, return
        timeline = [frame(t % loop, t) for t in range(1, levels + 1)]
        timeline += [frame(t % loop, levels) for t in range(levels + 1, levels + loop + 1)]
        timeline += [frame(t % loop, levels - (t - levels - loop))
                     for t in range(levels + loop + 1, 2 * levels + loop + 1)]
        distinct = {tuple(sorted(c)) for c in timeline}
        report.append({"bin": n, "length_m": round(length, 4), "rest_cells": len(frame(0, 0)),
                       "timeline_frames": len(timeline), "distinct": len(distinct),
                       "distinct_cells": sum(len(c) for c in distinct)})
        print(f"bin {n}: {report[-1]} ({time.time() - t0:.0f} s)", flush=True)
        if n == len(builders) - 1:
            grid = [[keep(frame(j, k)) for j in range(loop)] for k in range(levels + 1)]
    doc = {"founder": name, "vox": vox, "fps": fps, "loop_frames": loop, "levels": levels,
           "ease_s": A.FG_FEED_EASE_S, "frames": frames, "grid": grid, "bins": report}
    with open(out, "w") as fh:
        json.dump(doc, fh)
    print(f"{len(frames)} distinct adult frames over {(levels) * loop + 1} (k, j) -> {out}", flush=True)


if __name__ == "__main__":
    main()
