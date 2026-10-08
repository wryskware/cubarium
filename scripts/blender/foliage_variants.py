"""Browsed-foliage candidates, round 1 (design/handoffs/foliage-variants-and-calf-round1-2026-10-07.md,
package A). **Candidates for Wrysk to pick, not a bake**: nothing here writes
`assets/voxel-models/`.

    blender -b -P scripts/blender/foliage_variants.py -- OUT_DIR [--only NAME[,NAME]]

For bloomcrown and umbrellafrond, at the bake's adult step (variant 0's random phase), it
builds the accepted model (full) and three browsed and three stripped candidates. Each
candidate is the accepted builder's own geometry with parts removed or shortened, so the
parts and palette are the accepted ones; nothing new is drawn. Browsed goes from the
bottom up (browser height first) and keeps the crown (bloomcrown) or the top tier
(umbrellafrond); stripped is the stem plus a remnant. The candidates differ in how much is
removed and where; none is preferred.

Writes, under OUT_DIR:

- `panels/<species>-<state>-{mesh,vox}.png`: the Blender model and its 0.125 m voxels, one
  fixed camera per species (fitted to the full model) so the states compare at one scale;
- `models/125/`: a copy of the shipped 0.125 m library whose `<species>.cvm` adult step
  holds the candidates as its variants, in CANDIDATE order (variant 0 = full), for
  `voxel_specimens --gpu --models OUT_DIR/models --candidates SPECIES:0,1,...`;
- `candidates.json`: per species, the variant index, label and cell counts by tag.

`scripts/blender/foliage_variants_sheets.py` composes the labelled contact sheets.
Headless; no windows.
"""

import json
import math
import os
import random
import shutil
import sys

import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
ARGV = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = ARGV[0] if ARGV and not ARGV[0].startswith("--") else os.path.join(
    REPO, "art", "gen", "runs", "2026-10-07-foliage-variants")
ONLY = set(filter(None, (ARGV[ARGV.index("--only") + 1] if "--only" in ARGV else "").split(",")))

# plant_sheets.py minus its main(): the bake (growth rule, builders, voxelizer, tags, the
# .cvm writer) and the sheet colours (a mirror of colours.rs recipe()), cvm reader, painter.
_ps = os.path.join(HERE, "plant_sheets.py")
_saved = sys.argv
sys.argv = [sys.argv[0]]
P = {"__name__": "plant_sheets", "__file__": _ps}
exec(compile(open(_ps).read().rsplit("\nmain()", 1)[0], _ps, "exec"), P)
sys.argv = _saved
B, L = P["B"], P["L"]
VOX = 0.125

# --- the candidates --------------------------------------------------------------------
#
# A spec is a dict of knobs on the accepted builder; {} is the accepted model itself.
#
# bloomcrown (8 basal straps = foliage layer 0, the rosette; pod + bloom + 10 vanes = layer
# 1, the crown):
#   basal: per-strap length factors, strap k of the 8 (0 drops it, <1 is a chewed stub);
#   vanes: per-vane length factors, by rank of the vane's tip height (rank 0 = lowest tip,
#          the one a browser reaches first);
#   pod:   False drops the pod.
# umbrellafrond (three tiers of eight fronds, bottom-up = foliage layers 0, 1, 2):
#   tiers: per tier, (fronds kept: "all" | "alt" (every other) | int n (n spread round),
#          rib length factor, pinnae: True | False (bare ribs));
#   tier None drops the tier's fronds (its collar and node bud stay, they are stem).

FULL = ("full", "full (accepted model)", {})

CANDIDATES = {
    "bloomcrown": [
        FULL,
        ("browsed-1", "browsed 1: rosette nibbled (4 straps gone, 4 chewed)",
         {"basal": [0.0, 0.6] * 4}),
        ("browsed-2", "browsed 2: rosette grazed to stubs",
         {"basal": [0.3] * 8}),
        ("browsed-3", "browsed 3: rosette gone, 3 lowest vanes chewed",
         {"basal": [0.0] * 8, "vanes": [0.5, 0.5, 0.5] + [1.0] * 7}),
        ("stripped-1", "stripped 1: stem + pod + bloom",
         {"basal": [0.0] * 8, "vanes": [0.0] * 10}),
        ("stripped-2", "stripped 2: stem + pod + vane and strap stubs",
         {"basal": [0.25] * 8, "vanes": [0.35] * 10}),
        ("stripped-3", "stripped 3: stem + pod + top tuft (3 highest vanes)",
         {"basal": [0.0] * 8, "vanes": [0.0] * 7 + [0.7] * 3}),
    ],
    "umbrellafrond": [
        FULL,
        ("browsed-1", "browsed 1: lower tier thinned (every other frond)",
         {"tiers": [("alt", 1.0, True), ("all", 1.0, True), ("all", 1.0, True)]}),
        ("browsed-2", "browsed 2: lower tier gone",
         {"tiers": [None, ("all", 1.0, True), ("all", 1.0, True)]}),
        ("browsed-3", "browsed 3: lower tier gone, middle tier bare ribs",
         {"tiers": [None, ("all", 1.0, False), ("all", 1.0, True)]}),
        ("stripped-1", "stripped 1: stem + bare-rib top tier",
         {"tiers": [None, None, ("all", 1.0, False)]}),
        ("stripped-2", "stripped 2: stem + small top tuft (4 short fronds)",
         {"tiers": [None, None, (4, 0.6, True)]}),
        ("stripped-3", "stripped 3: stem + rib stubs on every tier",
         {"tiers": [("all", 0.4, False), ("all", 0.45, False), ("all", 0.5, False)]}),
    ],
}

# Views: (elevation, azimuth) in degrees. Three-quarter from a little above, like the
# panel's 30 degree camera but turned, so rosette, stem and crown all show.
VIEW = (24, 30)
PANEL_PX = 720


# --- builders: the accepted ones with knobs ---------------------------------------------

def _scaled(p0, p1, f):
    return tuple(p0[k] + (p1[k] - p0[k]) * f for k in range(3))


def bloomcrown(x, y, h, r, layers, i, rng, spec):
    """L.bloomcrown (the artbook pick) with parts removed or shortened per `spec`. The
    random draws are the same, in the same order, so spec {} is L.bloomcrown."""
    T, F = L.T, L.F
    assert i != 0, "candidates are adult"
    trunk = next(l for l in layers if l[0] == T)
    basal = next(l for l in layers if l[0] == F and l[1] == 0.0)
    crown = next(l for l in layers if l[0] == F and l[1] > 0.0)
    za, zb = crown[1] * h, crown[2] * h
    L.cyl((x, y, 0), (x, y, za + 0.25 * (zb - za)), max(trunk[3] * r * 0.55, 0.018),
          max(trunk[3] * r * 0.45, 0.015), mat="violet", seg=8)
    bf = spec.get("basal", [1.0] * 8)
    for k, (dx, dy) in enumerate(L.ring_dirs(8, rng.random())):
        p0 = (x + dx * r * 0.08, y + dy * r * 0.08, 0.0)
        p1 = (x + dx * basal[3] * r, y + dy * basal[3] * r, basal[2] * h)
        if bf[k] > 0:
            L.strap(p0, _scaled(p0, p1, bf[k]), 0.035, "p0")
    pc = (x, y, za + 0.42 * (zb - za))
    if spec.get("pod", True):
        L.ell(pc, (0.24 * r, 0.24 * r, 0.24 * (zb - za)), "p1")
    L.ell((x, y, za + 0.64 * (zb - za)), (0.15 * r, 0.15 * r, 0.06 * (zb - za)), "warm")
    vanes = []
    for k, (dx, dy) in enumerate(L.ring_dirs(10, rng.random())):
        reach = crown[3] * r * (0.85 + 0.15 * ((k * 7) % 3) / 2)
        top = zb - (zb - za) * 0.12 * ((k * 3) % 4) / 3
        vanes.append(((x + dx * r * 0.12, y + dy * r * 0.12, za + 0.05 * (zb - za)),
                      (x + dx * reach, y + dy * reach, top)))
    vf = spec.get("vanes", [1.0] * 10)
    # rank by tip height, lowest first (stable: ties keep ring order)
    order = sorted(range(10), key=lambda k: (vanes[k][1][2], k))
    for rank, k in enumerate(order):
        f = vf[rank]
        if f > 0:
            p0, p1 = vanes[k]
            L.strap(p0, _scaled(p0, p1, f), 0.04, "p2")


def frond(x, y, ang, zb, za, rr, top=False, n=14, keep=1.0, pinnae=True):
    """L.frond, cut to the first `keep` of its length from the node, with or without its
    pinnae. keep 1, pinnae True is L.frond."""
    dx, dy = math.cos(ang), math.sin(ang)
    drop = zb - za
    if top:
        ctrl = (x + dx * rr * 0.35, y + dy * rr * 0.35, zb + drop * 0.9)
    else:
        ctrl = (x + dx * rr * 0.55, y + dy * rr * 0.55, zb + drop * 0.35)
    path = L.bezier((x, y, zb), ctrl, (x + dx * rr, y + dy * rr, za), n)
    m = n if keep >= 1.0 else max(2, int(round(n * keep)))
    if m == n:
        rib = path[::2] + ([path[-1]] if (n - 1) % 2 else [])
    else:
        rib = path[:m:2] + ([path[m - 1]] if (m - 1) % 2 else [])
        if len(rib) < 2:
            rib = [path[0], path[m - 1]]
    L.tube(rib, 0.016, 0.006 + 0.010 * (1 - min(keep, 1.0)), "p2", seg=6)
    if not pinnae:
        return
    with L.Collect() as blade:
        widths = [0.07 * math.sin(math.pi * (0.15 + 0.8 * k / (n - 1))) + 0.02 for k in range(n)]
        L.ribbon(path[:m], widths[:m], "p1")
        side = Vector((-dy, dx, 0))
        for k in range(2, min(n - 1, m - 1)):
            s = k / (n - 1)
            q = path[k]
            t = (path[k + 1] - path[k - 1]).normalized()
            ln = rr * (0.38 * (1 - s) + 0.09)
            for sg in (-1, 1):
                d = (t * 0.55 + side * sg).normalized()
                end = q + d * ln - Vector((0, 0, ln * 0.35))
                mid = q + d * ln * 0.5 - Vector((0, 0, ln * 0.08))
                L.ribbon([q, mid, end], [0.045, 0.04, 0.012], "p1", th=0.01)
    L.join(blade, "frond")


def _kept(which, n=8):
    if which == "all":
        return set(range(n))
    if which == "alt":
        return set(range(0, n, 2))
    return {int(round(j * n / which)) % n for j in range(which)}


def umbrellafrond(x, y, h, r, layers, i, rng, spec):
    """L.umbrellafrond (the artbook pick) with fronds removed, shortened or bared per
    `spec`. Same random draw, so spec {} is L.umbrellafrond."""
    F, MAT = L.F, L.MAT
    assert i != 0, "candidates are adult"
    tiers = [l for l in layers if l[0] in (F, MAT)]
    phase = rng.uniform(0, math.pi / 8)
    want = spec.get("tiers", [("all", 1.0, True)] * len(tiers))
    top_node = (tiers[-1][1] + 0.3 * (tiers[-1][2] - tiers[-1][1])) * h
    rs = max(0.03 * h, 0.02)
    L.cyl((x, y, 0), (x, y, top_node), rs, rs * 0.8, mat="violet", seg=6)
    L.cyl((x, y, 0), (x, y, 0.05 * h), rs * 2.4, rs * 2.0, mat="plum", seg=6)
    L.cyl((x, y, 0.05 * h), (x, y, 0.1 * h), rs * 1.8, rs * 1.5, mat="plum", seg=6)
    nodes = []
    for k, (kind, a, b, rf, share, p) in enumerate(tiers):
        last = k == len(tiers) - 1
        zb = top_node if last else b * h
        nodes.append(zb)
        rk = rs * (1 - 0.2 * zb / h)
        L.cyl((x, y, zb - 0.05 * h), (x, y, zb - 0.01 * h), rk * 1.45, rk * 1.2, mat="plum", seg=6)
        L.ell((x, y, zb + 0.012), (rk * 0.9, rk * 0.9, 0.025), "p1")
        w = want[k]
        if w is None:
            continue
        which, keep, pinnae = w
        kept = _kept(which)
        for j, (dx, dy) in enumerate(L.ring_dirs(8, phase + k * math.pi / 8)):
            if j in kept:
                frond(x, y, math.atan2(dy, dx), zb, a * h, rf * r, top=last, keep=keep, pinnae=pinnae)
    prev = 0.1 * h
    for zb in nodes:
        zm = (prev + zb - 0.05 * h) / 2
        if zm - prev > 0.08 * h:
            rk = rs * (1 - 0.2 * zm / h)
            L.cyl((x, y, zm - 0.015 * h), (x, y, zm + 0.015 * h), rk * 1.3, rk * 1.15, mat="plum", seg=6)
        prev = zb


BUILDERS = {"bloomcrown": bloomcrown, "umbrellafrond": umbrellafrond}


# --- rendering ----------------------------------------------------------------------

def fixed_camera(pts, elev, azim, aspect):
    """An ortho camera fitted to `pts` (the full model), reused for every state: (ortho
    scale, location, rotation)."""
    lo = Vector(min(p[k] for p in pts) for k in range(3))
    hi = Vector(max(p[k] for p in pts) for k in range(3))
    c = (lo + hi) / 2
    e, a = math.radians(elev), math.radians(azim)
    view = Matrix.Rotation(a, 3, "Z") @ Vector((0, math.cos(e), -math.sin(e)))
    right = Matrix.Rotation(a, 3, "Z") @ Vector((1, 0, 0))
    up = right.cross(view).normalized()
    w = max(abs((p - c).dot(right)) for p in pts) * 2
    hh = max(abs((p - c).dot(up)) for p in pts) * 2
    return (max(w / aspect, hh) * 1.12, c - view * 100, (math.radians(90) - e, 0, a))


def shoot(spec, path, aspect):
    """Render from a camera made from `spec` (fixed_camera's), removed afterwards: the
    builders clear the scene between states."""
    scale, loc, rot = spec
    cam = bpy.data.cameras.new("cam")
    cam.type = "ORTHO"
    cam.ortho_scale = scale
    cam.clip_end = 500
    ob = bpy.data.objects.new("cam", cam)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = loc
    ob.rotation_euler = rot
    sc = bpy.context.scene
    sc.camera = ob
    sc.render.resolution_x, sc.render.resolution_y = int(PANEL_PX * aspect), PANEL_PX
    sc.render.resolution_percentage = 100
    sc.render.filepath = path
    bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(ob, do_unlink=True)


def points(objs):
    return [ob.matrix_world @ v.co for ob in objs for v in ob.data.vertices]


def main():
    os.makedirs(os.path.join(OUT, "panels"), exist_ok=True)
    src = json.load(open(B["SOURCE"]), parse_constant=float)
    for sp in src["species"]:
        for st in sp["profile"]:
            st["wood_fraction_max"] = float(st["wood_fraction_max"])
    P["setup"]()
    # The scratch library: the shipped one, species files replaced below.
    lib_src = os.path.join(REPO, "assets", "voxel-models", "125")
    lib_dst = os.path.join(OUT, "models", "125")
    os.makedirs(lib_dst, exist_ok=True)
    for f in os.listdir(lib_src):
        shutil.copy(os.path.join(lib_src, f), os.path.join(lib_dst, f))
    report = {}
    for sp in src["species"]:
        name = sp["name"]
        if name not in CANDIDATES or (ONLY and name not in ONLY):
            continue
        prof = sp["profile"]
        (kh, kr), t = B["size_steps"](sp, VOX)[-1]
        si = B["stage_index"](prof, t)
        stage = prof[si]
        h = B["dimension"](prof, sp["crown_height_m"], t)
        r = B["dimension"](prof, sp["crown_radius_m"], t)
        roles = dict(B["PLANT_ROLE"])
        roles.update(B["PLANT_ROLE_BY_SPECIES"].get(name, {}))
        role = lambda m: roles.get(m, "foliage")  # noqa: E731
        aspect = 0.8
        built = []  # (key, label, mesh objs points, cells)
        cam = vcam = None
        for key, label, spec in CANDIDATES[name]:
            P["clear"]()
            B["reset_scene"]()
            rng = random.Random(f"{name}:0")
            BUILDERS[name](0.5 * VOX, 0.5 * VOX, h, r, B["lineup_layers"](stage), si, rng, spec)
            objs = [ob for ob in L.ORG if role(ob.active_material.name) is not None]
            cells = B["voxelize"](objs, VOX, role)

            def tag_of(mat, k, h=h, stage=stage):
                return B["plant_tag"](role(mat), (k + 0.5) * VOX, h, stage)

            cvm_cells = B["cells_of"](cells, tag_of)
            if key == "full":
                # The accepted builder must give the same cells: the candidates are cut
                # from exactly the shipped model.
                P["clear"]()
                B["reset_scene"]()
                ref_objs = B["build_plant"](name, h, r, si, stage, random.Random(f"{name}:0"), VOX)
                ref = B["cells_of"](B["voxelize"](ref_objs, VOX, role), tag_of)
                assert sorted(ref) == sorted(cvm_cells), f"{name}: full candidate != accepted builder"
                shipped = P["read_cvm"](os.path.join(lib_src, f"{name}.cvm"))[-1][2][0]
                same = sorted((c[0], c[1], c[2], B["PALETTE"][c[3]], c[4]) for c in cvm_cells) == sorted(shipped)
                print(f"{name}: full == shipped adult variant 0: {same}", flush=True)
                P["clear"]()
                B["reset_scene"]()
                BUILDERS[name](0.5 * VOX, 0.5 * VOX, h, r, B["lineup_layers"](stage), si,
                               random.Random(f"{name}:0"), spec)
                objs = [ob for ob in L.ORG if role(ob.active_material.name) is not None]
            for ob in objs:
                P["paint"](ob, name, h, role)
            if cam is None:
                cam = fixed_camera(points(objs), VIEW[0], VIEW[1], aspect)
            shoot(cam, os.path.join(OUT, "panels", f"{name}-{key}-mesh.png"), aspect)
            P["clear"]()
            vobjs = P["voxel_objs"](name, [(c[0], c[1], c[2], B["PALETTE"][c[3]], c[4]) for c in cvm_cells], VOX)
            if vcam is None:
                vcam = fixed_camera(points(vobjs), VIEW[0], VIEW[1], aspect)
            shoot(vcam, os.path.join(OUT, "panels", f"{name}-{key}-vox.png"), aspect)
            tags = {}
            for c in cvm_cells:
                kind = P["tag_kind"](c[4])
                k = kind if kind in ("trunk", "accent") else f"{kind}{c[4] & 0x0F}"
                tags[k] = tags.get(k, 0) + 1
            built.append((key, label, cvm_cells, tags))
            print(f"{name} {key}: {len(cvm_cells)} cells {tags}", flush=True)
        # the scratch .cvm: shipped steps, the adult step's variants = the candidates
        steps = []
        for sh, sr, variants in P["read_cvm"](os.path.join(lib_src, f"{name}.cvm")):
            steps.append((sh, sr, [[(c[0], c[1], c[2], B["PALETTE"].index(c[3]), c[4]) for c in v]
                                   for v in variants]))
        steps[-1] = (steps[-1][0], steps[-1][1], [b[2] for b in built])
        B["write_cvm"](os.path.join(lib_dst, f"{name}.cvm"), 0, steps)
        report[name] = {"adult_step": {"height_m": kh * VOX, "radius_m": kr * VOX / 2,
                                       "built_at": {"t": t, "height_m": h, "radius_m": r}},
                        "variants": [{"index": n, "key": b[0], "label": b[1], "cells": len(b[2]),
                                      "by_tag": b[3]} for n, b in enumerate(built)]}
    with open(os.path.join(OUT, "candidates.json"), "w") as fh:
        json.dump(report, fh, indent=1)
        fh.write("\n")


main()
