"""FEED-RIG arm A, the flipbook (harness only, `design/handoffs/feeding-motion-rig-2026-10-09.md`).

    blender -b -P scripts/blender/feed_flipbook.py -- OUT.json [--vox 0.125]

The frondgrazer's feeding clip (`animal_models.fg_feed_clip`, the same armature and action
the bake samples into `frondgrazer.rig`) re-voxelized at every sampled frame with the bake's
own rule (`bake_voxel_models.voxelize_animal`): each part moved rigidly by its bone's
deformation (`animal_models.fg_feed_pose`, the renderer's blend), then voxelized.

- **Per bin**, one bout on a timeline at `FG_FEED_FPS`: the reach (the clip weight eased in
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

from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import animal_models as A  # noqa: E402
import bake_voxel_models as B  # noqa: E402

ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []


def smooth(x):
    x = min(max(x, 0.0), 1.0)
    return x * x * (3 - 2 * x)


def main():
    out = ARGS[0]
    vox = float(ARGS[ARGS.index("--vox") + 1]) if "--vox" in ARGS else 0.125
    with open(B.SOURCE) as fh:
        src = json.load(fh, parse_constant=float)
    f = next(f for f in src["founders"] if f["name"] == "frondgrazer")
    l0 = A.REF["frondgrazer"][1]
    B.reset_scene()
    clip = A.fg_feed_clip()
    loop = len(clip["frames"])
    levels = round(A.FG_FEED_EASE_S * A.FG_FEED_FPS)
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
    for n in range(B.BINS):
        length, _, objs = B.build_founder_bin(f, n, vox)
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
                    m = swap @ ((Vector(co[i:i + 3]) - offset) / s)
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
        print(f"bin {n}: {report[-1]}", flush=True)
        if n == B.BINS - 1:
            grid = [[keep(frame(j, k)) for j in range(loop)] for k in range(levels + 1)]
    doc = {"vox": vox, "fps": A.FG_FEED_FPS, "loop_frames": loop, "levels": levels,
           "ease_s": A.FG_FEED_EASE_S, "frames": frames, "grid": grid, "bins": report}
    with open(out, "w") as fh:
        json.dump(doc, fh)
    print(f"{len(frames)} distinct adult frames over {(levels) * loop + 1} (k, j) -> {out}", flush=True)


if __name__ == "__main__":
    main()
