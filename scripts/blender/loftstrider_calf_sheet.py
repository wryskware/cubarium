"""Loftstrider calf candidates: render tiles, then compose the sheets (round 1, package B of
`design/handoffs/foliage-variants-and-calf-round1-2026-10-07.md`).

    blender -b -P scripts/blender/loftstrider_calf_sheet.py -- --out DIR [--only NAME[,NAME]]
        [--round 2 [--blend NAME] [--ease FACTOR=EXP[,...]]]
    python3 scripts/blender/loftstrider_calf_compose.py DIR

Candidates only: nothing here is baked. Models (`animal_models.py`): `adult` (the reference
`loftstrider()`), `scaled` (the adult scaled by the newborn's 0.464, how the founders' size
bins are made today), the calves `LS_CALVES`, and each calf blended to the adult across
the five bins (`<calf>-bin<n>`, n = 1..3; bin 0 is the calf, bin 4 the adult). Each is
rendered orthographic from the side (elevation 0) and three-quarter (azimuth -45,
elevation 30), as its voxel model at 0.125 m (`bake_voxel_models.voxelize_animal`, cubes
in the renderer's band colours) and as the Blender model: `true` tiles share one camera
framed on the adult (ground aligned, true relative size), `fit` tiles are framed on the
model's own height. `meta.json` records each model's factors, cells, voxel span, and per
counted part (eyes, tusks, fronds, feet) its connected pieces (26-connected cells of the
part's slots) and how many of its six azimuths still hold a cell, so a count lost to the
0.125 m grid shows.
"""

import json
import math
import os
import sys

import bpy
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import organism_lineup as L  # noqa: E402
import animal_models as A  # noqa: E402
import bake_voxel_models as B  # noqa: E402

ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = ARGS[ARGS.index("--out") + 1]
ROUND = int(ARGS[ARGS.index("--round") + 1]) if "--round" in ARGS else 1
# round 2: only `--blend`'s calf gets bins; `--ease` adds a second blend of it (`<calf>e`)
# whose named factors move along t ** exponent
BLEND = ARGS[ARGS.index("--blend") + 1] if "--blend" in ARGS else None
EASE = {kv.split("=")[0]: float(kv.split("=")[1])
        for kv in (ARGS[ARGS.index("--ease") + 1].split(",") if "--ease" in ARGS else [])}
ONLY = set(filter(None, (ARGS[ARGS.index("--only") + 1] if "--only" in ARGS else "").split(",")))

VOX = 0.125
BINS = 5
W, H = 340, 420
VIEWS = {"side": (-90.0, 0.0), "q": (-45.0, 30.0)}
PARTS = {"eyes": {"a_eye"}, "tusks": {"a_head"}, "fronds": {"a_membrane", "a_glow"},
         "feet": {"a_foot"}}


def models():
    s0 = A.LS_NEWBORN
    out = {"adult": A.LS_ADULT, "scaled": {k: (1.0 if k == "tusk" else s0) for k in A.LS_ADULT}}
    if ROUND == 3:
        return {"adult": A.LS_ADULT, **A.LS_CALVES_R3}
    calves = A.LS_CALVES if ROUND == 1 else A.LS_CALVES_R2
    for name, calf in calves.items():
        out[name] = calf
        if ROUND == 1 or name == BLEND:
            for n in range(1, BINS - 1):
                out[f"{name}-bin{n}"] = A.ls_blend(calf, n / (BINS - 1))
        if name == BLEND and EASE:
            out[f"{name}e"] = calf
            for n in range(1, BINS - 1):
                out[f"{name}e-bin{n}"] = A.ls_blend(calf, n / (BINS - 1), EASE)
    return out


def view_dir(az, el):
    a, e = math.radians(az), math.radians(el)
    return Vector((math.cos(e) * math.cos(a), math.cos(e) * math.sin(a), math.sin(e)))


def frame(objs, az, el):
    """(target, vertical extent) fitting `objs` seen from (az, el)."""
    lo = Vector((min(v[k] for ob in objs for v in ob.bound_box) for k in range(3)))
    hi = Vector((max(v[k] for ob in objs for v in ob.bound_box) for k in range(3)))
    d = view_dir(az, el)
    up = d.cross(Vector((0, 0, 1))).cross(d).normalized() if el < 89 else Vector((0, 1, 0))
    pts = [Vector((a, b, c)) for a in (lo.x, hi.x) for b in (lo.y, hi.y) for c in (lo.z, hi.z)]
    vs = [p.dot(up) for p in pts]
    centre = (lo + hi) / 2
    # move the target along `up` to the middle of the projected extent
    target = centre + up * ((max(vs) + min(vs)) / 2 - centre.dot(up))
    return target, max(vs) - min(vs)


def camera(target, extent, az, el):
    cam = bpy.data.cameras.new("tile")
    cam.type = "ORTHO"
    cam.sensor_fit = "VERTICAL"
    cam.ortho_scale = extent
    cam.clip_end = 200
    ob = bpy.data.objects.new("tile", cam)
    bpy.context.scene.collection.objects.link(ob)
    d = view_dir(az, el)
    ob.location = target + d * 50
    ob.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
    return ob


def pieces(cells, slots):
    """26-connected groups of the cells whose slot is in `slots`."""
    left = {k for k, s in cells.items() if s in slots}
    n = 0
    while left:
        n += 1
        stack = [left.pop()]
        while stack:
            i, j, k = stack.pop()
            for di in (-1, 0, 1):
                for dj in (-1, 0, 1):
                    for dk in (-1, 0, 1):
                        q = (i + di, j + dj, k + dk)
                        if q in left:
                            left.remove(q)
                            stack.append(q)
    return n


# Each part's six azimuths (degrees from +x): eyes and tusks at 60 k, legs and fronds at
# 30 + 60 k.
AZ = {"eyes": 0.0, "tusks": 0.0, "fronds": 30.0, "feet": 30.0}


def sectors(cells, part):
    """How many of the part's six azimuths hold a cell of its slots within 15 degrees (cells
    on the axis do not count): six when every one survives the grid."""
    return B.loftstrider_sectors(cells, VOX)[part]


def build(p):
    B.reset_scene()
    return A.build_loftstrider(p, 0.5 * VOX, 0.5 * VOX)


def main():
    os.makedirs(os.path.join(OUT, "tiles"), exist_ok=True)
    sc = L.setup_scene()
    sc.display.shading.show_specular_highlight = False
    sc.world.color = L.lin("#F4F2F8")
    sc.display.shading.object_outline_color = (0.05, 0.03, 0.08)
    # the true-size frames: the adult's, padded, shared by every model
    adult = build(A.LS_ADULT)
    true = {v: frame(adult, *VIEWS[v]) for v in VIEWS}
    true = {v: (t, e * 1.12) for v, (t, e) in true.items()}
    meta_path = os.path.join(OUT, "meta.json")
    meta = json.load(open(meta_path)) if os.path.exists(meta_path) else {}
    for name, p in models().items():
        if ONLY and name not in ONLY and name.split("-bin")[0] not in ONLY:
            continue
        is_bin = "-bin" in name
        objs = build(p)
        cells = B.voxelize_animal(objs, VOX, 0)
        top = max(v[2] for ob in objs for v in ob.bound_box)
        fits = {v: frame(objs, *VIEWS[v]) for v in VIEWS}
        for v, (az, el) in VIEWS.items():
            framings = [("true", true[v])] + ([] if is_bin else [("fit", (fits[v][0], fits[v][1] * 1.12))])
            for tag, (tgt, ext) in framings:
                L.render(camera(tgt, ext, az, el), os.path.join(OUT, "tiles", f"{name}_bl_{v}_{tag}.png"), W, H)
        B.reset_scene()
        cubes = A.voxel_cubes("loftstrider", cells, VOX)
        for v, (az, el) in VIEWS.items():
            framings = [("true", true[v])] + ([] if is_bin else [("fit", (fits[v][0], fits[v][1] * 1.12))])
            for tag, (tgt, ext) in framings:
                L.render(camera(tgt, ext, az, el), os.path.join(OUT, "tiles", f"{name}_vox_{v}_{tag}.png"), W, H)
        meta[name] = {"factors": {k: round(x, 3) for k, x in p.items()}, "cells": len(cells),
                      "span": A.span(cells), "height_m": round(top, 3),
                      "pieces": {part: pieces(cells, slots) for part, slots in PARTS.items()},
                      "sectors": {part: sectors(cells, part) for part in PARTS}}
        print(f"{name}: {meta[name]}", flush=True)
        with open(meta_path, "w") as fh:
            json.dump(meta, fh, indent=1)


if __name__ == "__main__":
    main()
