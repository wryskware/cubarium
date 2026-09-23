"""Bake the voxel organism models the presenter stamps (package V).

    cargo run -p cubarium --example voxel_model_source
    blender -b -P scripts/blender/bake_voxel_models.py [-- [--only NAME[,NAME]] [--out DIR]]

Reads **only** `assets/voxel-models/source.json` for numbers (crown ranges, stage
profiles, founder bodies, voxel sizes; `design/handoffs/voxel-organism-models-2026-09-23.md`
§1) and reuses `organism_lineup.py`'s builders for shapes. Writes, per voxel size,
`assets/voxel-models/<voxel_mm>/<name>.cvm` and a `manifest.json`. The binary layout is
in `crates/cubarium/src/voxel/model.rs`'s header.

Plants: one model per **size step** along package L's growth rule, from the seedling to
the adult: a new step wherever the crown height crosses a whole voxel or the crown radius
a half voxel (a velvetpad grows only sideways, and would otherwise be one model). Each
step is built at the height, radius and profile stage of the middle of its span of
`wood / wood_max`, in `VARIANTS` variants whose random phase is fixed per variant, so a
stand keeps its phase as it grows. Every cell carries a material and a tag: trunk,
foliage layer i, drape i (i = the stage's foliage index, by the band the cell's height
falls in) or accent.

Animals: each founder in `BINS` size bins, newborn to adult (length ~ body^(1/3)),
facing +x, one pose, centred on the anchor cell.

Voxelization is the lineup's rule — a cell whose centre lies inside a part or within
half a voxel of its surface — for everything but the accents (bloom, fruit, sense patch).
Every accent is smaller than a voxel: under the half-voxel rule a 6 cm fruit grew into a
2 x 2 x 2 block, and under first-come it vanished inside the crown or shield it sits on.
So an accent claims exactly the cells whose centres lie inside it, or, when it holds
none, the one cell its centre is in, over whatever else is there.
"""

import json
import math
import os
import random
import struct
import sys

import bpy
from mathutils import Matrix, Vector
from mathutils.bvhtree import BVHTree

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import organism_lineup as L  # noqa: E402  (import-safe: main() is guarded)

REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


ROOT = arg("--out", os.path.join(REPO, "assets", "voxel-models"))
SOURCE = arg("--source", os.path.join(ROOT, "source.json"))
ONLY = set(filter(None, (arg("--only") or "").split(",")))

# Placeholders (design/backlog.md §1).
VARIANTS = 3
BINS = 5
SAMPLES = 4000  # how finely the growth curve is walked to find the size steps

# The model palette, by the builders' material names. Anything else a builder draws
# (the stonecushion's rock lip, the siphonreed's pool, the glowcap's log) is scenery
# the world already draws, and is not voxelized.
PALETTE = ["plum", "violet", "detritus", "lilac", "p0", "p1", "p2", "teal", "magenta",
           "warm", "starved"]

# What each builder material is on a plant: trunk, foliage, drape, accent, or None
# (scenery). Anything unlisted is foliage.
PLANT_ROLE = {"plum": "trunk", "violet": "trunk", "detritus": "trunk", "warm": "accent",
              "magenta": "accent", "bark": None, "split": None, "rock": None,
              "water": None, "film": None, "grey": None}
PLANT_ROLE_BY_SPECIES = {
    # D1: the cap is the glowcap's tissue, its stalk the mycelium's stem.
    "glowcap": {"violet": "foliage", "p2": "foliage"},
    # D11: the teal filaments are the drape layer.
    "vaulttree": {"teal": "drape"},
}
# On an animal: magenta is the sense patch / inherited accent; everything else is body.
ANIMAL_ROLE = {"magenta": "accent"}

KIND = {"Foliage": L.F, "Trunk": L.T, "Drape": L.D, "Mat": L.MAT}


# --- growth rule (flora's SpeciesConfig::crown_dimension_m, from the dump) ------------

def stage_index(profile, t):
    for i, st in enumerate(profile):
        if st["wood_fraction_max"] >= t:
            return i
    return len(profile) - 1


def capped_seedling(profile):
    first = profile[0]
    cap, w0 = first["height_m_max"], first["wood_fraction_max"]
    if cap is None or not (math.isfinite(cap) and cap >= 0 and math.isfinite(w0) and 0 < w0 < 1):
        return None
    return w0, cap


def dimension(profile, rng, t):
    linear = rng[0] + t * (rng[1] - rng[0])
    cs = capped_seedling(profile)
    if cs is None:
        return linear
    w0, cap = cs
    if stage_index(profile, t) == 0:
        return min(linear, cap)
    start = min(cap, rng[1])
    s = min(max((t - w0) / (1 - w0), 0.0), 1.0)
    return start + s * (rng[1] - start)


def half_up(v):
    return int(math.floor(v + 0.5))


def size_steps(sp, vox):
    """[(key, t_mid)]: the size steps along the growth curve, each with the middle of
    the wood fraction it spans. key = (height voxels, radius half-voxels)."""
    prof = sp["profile"]
    spans = []
    for n in range(SAMPLES + 1):
        t = n / SAMPLES
        h = dimension(prof, sp["crown_height_m"], t)
        r = dimension(prof, sp["crown_radius_m"], t)
        key = (max(1, half_up(h / vox)), max(1, half_up(2 * r / vox)))
        if spans and spans[-1][0] == key:
            spans[-1][2] = t
        else:
            spans.append([key, t, t])
    return [(key, (a + b) / 2) for key, a, b in spans]


# --- building -----------------------------------------------------------------------

def reset_scene():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    for me in list(bpy.data.meshes):
        bpy.data.meshes.remove(me)
    if L.COL is None or L.COL.name not in bpy.data.collections:
        L.COL = bpy.data.collections.new("bake")
        bpy.context.scene.collection.children.link(L.COL)
    L.ORG.clear()
    L.OVL.clear()


def lineup_layers(stage):
    return [(KIND[l["kind"]], l["band"][0], l["band"][1], l["radius"], l["share"], l["porosity"])
            for l in stage["layers"]]


def build_plant(name, h, r, stage_i, stage, rng, vox):
    reset_scene()
    x = y = 0.5 * vox
    fn = getattr(L, name)
    fn(x, y, h, r, lineup_layers(stage), stage_i, rng)
    objs = list(L.ORG)
    if name == "stonecushion":
        # The lineup draws the rock lip it lives on and sits the cushion on its top. In
        # the world the stand's support face *is* that rock: drop the lip and set the
        # cushion on the face.
        top = 0.25
        for ob in objs:
            ob.data.transform(Matrix.Translation((0, 0, -top)))
    return objs


def build_animal(name, s, adult, vox):
    reset_scene()
    x = y = 0.5 * vox
    fn = getattr(L, name)
    Lm, Wm, Hm = (adult[0] * s, adult[1] * s, adult[2] * s)
    if name == "frondgrazer":
        fn(x, y, Lm, Wm, Hm, overlays=False)
    else:
        fn(x, y, Lm, Wm, Hm)
    return list(L.ORG)


def voxelize(objs, vox, role_of):
    """{(i, j, k): material}: the lineup's rule, and accents as the module header says."""
    parts, accents = [], []
    for ob in objs:
        mat = ob.active_material.name
        role = role_of(mat)
        if role is None:
            continue
        me = ob.data
        vs = [v.co.copy() for v in me.vertices]
        if not vs:
            continue
        bvh = BVHTree.FromPolygons(vs, [tuple(p.vertices) for p in me.polygons])
        lo = Vector(min(v[k] for v in vs) for k in range(3))
        hi = Vector(max(v[k] for v in vs) for k in range(3))
        (accents if role == "accent" else parts).append((bvh, lo, hi, mat))
    half = vox / 2
    cells = {}
    if parts:
        lo = Vector(min(p[1][k] for p in parts) for k in range(3))
        hi = Vector(max(p[2][k] for p in parts) for k in range(3))
        rng = [range(math.floor(lo[k] / vox) - 1, math.ceil(hi[k] / vox) + 1) for k in range(3)]
        for i in rng[0]:
            for j in rng[1]:
                for k in rng[2]:
                    if k < 0:
                        continue
                    c = Vector(((i + 0.5) * vox, (j + 0.5) * vox, (k + 0.5) * vox))
                    best, best_d = None, 1e9
                    for bvh, plo, phi, mat in parts:
                        if any(c[a] < plo[a] - half or c[a] > phi[a] + half for a in range(3)):
                            continue
                        loc, nor, _, dist = bvh.find_nearest(c)
                        if loc is None:
                            continue
                        d = -1.0 if (c - loc).dot(nor) < 0 else dist
                        if d <= half and d < best_d:
                            best, best_d = mat, d
                    if best is not None:
                        cells[(i, j, k)] = best
    for bvh, plo, phi, mat in accents:
        inside = []
        for i in range(math.floor(plo[0] / vox), math.ceil(phi[0] / vox)):
            for j in range(math.floor(plo[1] / vox), math.ceil(phi[1] / vox)):
                for k in range(max(0, math.floor(plo[2] / vox)), math.ceil(phi[2] / vox)):
                    c = Vector(((i + 0.5) * vox, (j + 0.5) * vox, (k + 0.5) * vox))
                    loc, nor, _, _ = bvh.find_nearest(c)
                    if loc is not None and (c - loc).dot(nor) < 0:
                        inside.append((i, j, k))
        if not inside:
            mid = (plo + phi) / 2
            key = (math.floor(mid[0] / vox), math.floor(mid[1] / vox), math.floor(mid[2] / vox))
            if key[2] >= 0:
                inside.append(key)
        for key in inside:
            cells[key] = mat
    return cells


def plant_tag(role, zc, h, stage):
    """The cell's tag byte: trunk, accent, or the foliage/drape layer whose band holds
    the cell's height (else the nearest band)."""
    if role == "trunk":
        return 0x00
    if role == "accent":
        return 0x30
    bearing = []
    fi = 0
    for l in stage["layers"]:
        if l["kind"] == "Trunk":
            continue
        bearing.append((fi, l))
        fi += 1
    want_drape = role == "drape"
    cands = [(i, l) for i, l in bearing if (l["kind"] == "Drape") == want_drape] or bearing
    best, best_d = None, None
    for i, l in cands:
        a, b = l["band"][0] * h, l["band"][1] * h
        d = 0.0 if a <= zc <= b else min(abs(zc - a), abs(zc - b))
        if best_d is None or d < best_d:
            best, best_d = (i, l), d
    i, l = best
    return (0x20 if l["kind"] == "Drape" else 0x10) | (i & 0x0F)


def cells_of(cells, tag_of):
    out = []
    for (i, j, k), mat in sorted(cells.items(), key=lambda kv: (kv[0][2], kv[0][1], kv[0][0])):
        tag = tag_of(mat, k)
        if tag is None:
            continue
        if not all(-128 <= v <= 127 for v in (i, j, k)):
            raise ValueError(f"cell {(i, j, k)} does not fit an i8 offset")
        # Blender (x right, y away, z up) -> world (dx, dy up, dz away).
        out.append((i, k, j, PALETTE.index(mat), tag))
    return out


# --- the file -----------------------------------------------------------------------

def write_cvm(path, kind, steps):
    """steps: [(height_m, radius_m, [cells, ...])], cells [(dx, dy, dz, material, tag)]."""
    used = sorted({c[3] for _, _, variants in steps for cells in variants for c in cells})
    local = {g: n for n, g in enumerate(used)}
    b = bytearray(b"CVM1")
    b += struct.pack("<BB", kind, len(used))
    for g in used:
        name = PALETTE[g].encode()
        b += struct.pack("<B", len(name)) + name
    b += struct.pack("<H", len(steps))
    for h, r, variants in steps:
        b += struct.pack("<ddB", h, r, len(variants))
        for cells in variants:
            b += struct.pack("<I", len(cells))
            for dx, dy, dz, m, tag in cells:
                b += struct.pack("<bbbBB", dx, dy, dz, local[m], tag)
    with open(path, "wb") as f:
        f.write(b)
    return len(b)


def bake_plant(sp, vox, out_dir):
    name = sp["name"]
    roles = dict(PLANT_ROLE)
    roles.update(PLANT_ROLE_BY_SPECIES.get(name, {}))

    def role_of(mat):
        return roles.get(mat, "foliage")

    steps, meta = [], []
    for (kh, kr), t in size_steps(sp, vox):
        prof = sp["profile"]
        si = stage_index(prof, t)
        stage = prof[si]
        h = dimension(prof, sp["crown_height_m"], t)
        r = dimension(prof, sp["crown_radius_m"], t)
        variants = []
        for v in range(VARIANTS):
            objs = build_plant(name, h, r, si, stage, random.Random(f"{name}:{v}"), vox)
            cells = voxelize(objs, vox, role_of)

            def tag_of(mat, k, h=h, stage=stage):
                return plant_tag(role_of(mat), (k + 0.5) * vox, h, stage)

            variants.append(cells_of(cells, tag_of))
        steps.append((kh * vox, kr * vox / 2, variants))
        meta.append({"height_m": kh * vox, "radius_m": kr * vox / 2, "stage": si,
                     "built_at": {"t": round(t, 4), "height_m": round(h, 4), "radius_m": round(r, 4)},
                     "cells": [len(c) for c in variants]})
    file = f"{name}.cvm"
    size = write_cvm(os.path.join(out_dir, file), 0, steps)
    return file, size, meta


def bake_animal(f, vox, out_dir):
    name = f["name"]
    adult = (f["adult_length_m"], f["adult_width_m"], f["adult_height_m"])
    s0 = (f["body_min"] / f["body_max"]) ** (1.0 / 3.0)

    def role_of(mat):
        return ANIMAL_ROLE.get(mat, "trunk") if mat in PALETTE else None

    steps, meta = [], []
    for n in range(BINS):
        s = s0 + (1.0 - s0) * n / (BINS - 1)
        objs = build_animal(name, s, adult, vox)
        cells = voxelize(objs, vox, role_of)
        cells = cells_of(cells, lambda mat, k: 0x30 if role_of(mat) == "accent" else 0x00)
        steps.append((adult[0] * s, adult[1] * s, [cells]))
        meta.append({"length_m": round(adult[0] * s, 4), "cells": len(cells)})
    file = f"{name}.cvm"
    size = write_cvm(os.path.join(out_dir, file), 1, steps)
    return file, size, meta


def main():
    with open(SOURCE) as fh:
        src = json.load(fh, parse_constant=float)
    for sp in src["species"]:
        for st in sp["profile"]:
            st["wood_fraction_max"] = float(st["wood_fraction_max"])  # "inf" -> inf
    for vox in src["voxel_sizes_m"]:
        out_dir = os.path.join(ROOT, str(round(vox * 1000)))
        os.makedirs(out_dir, exist_ok=True)
        manifest_path = os.path.join(out_dir, "manifest.json")
        manifest = {"format": "CVM1", "voxel_m": vox, "palette": PALETTE,
                    "variants": VARIANTS, "bins": BINS, "species": {}, "founders": {},
                    "detail": {}}
        if ONLY and os.path.exists(manifest_path):
            with open(manifest_path) as fh:
                manifest = json.load(fh)
        for sp in src["species"]:
            if (ONLY and sp["name"] not in ONLY) or not hasattr(L, sp["name"]):
                continue  # no builder yet: no model, and the presenter falls back
            file, size, meta = bake_plant(sp, vox, out_dir)
            manifest["species"][sp["name"]] = file
            manifest["detail"][sp["name"]] = {"bytes": size, "steps": meta}
            print(f"{vox} m {sp['name']}: {len(meta)} steps, {size} bytes", flush=True)
        for f in src["founders"]:
            if (ONLY and f["name"] not in ONLY) or not hasattr(L, f["name"]):
                continue
            file, size, meta = bake_animal(f, vox, out_dir)
            manifest["founders"][f["name"]] = file
            manifest["detail"][f["name"]] = {"bytes": size, "bins": meta}
            print(f"{vox} m {f['name']}: {len(meta)} bins, {size} bytes", flush=True)
        with open(manifest_path, "w") as fh:
            json.dump(manifest, fh, indent=1)
            fh.write("\n")


main()
