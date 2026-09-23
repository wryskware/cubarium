"""Organism scale lineup: every organism in the roster, by life stage, at true size.

Blocks, not art. Each plant is built from its layer table (band, radius, kind), each
animal from its dossier body plan, inside a box the size of the panel world
(20 m ring x 3 m deep x 9 m tall, 0.125 m voxels). Two versions:

  sim     what the model holds today (flora crown ranges in 0.25 m reference voxels,
          scaled to metres; fauna adult dims from `body.rs`)
  ladder  the size ladder Wrysk chose on 2026-09-21 (organism-scale-and-roster §4,
          organism-anatomy §3-4), plus the planned species

Each version is also voxelized at 0.125 m. Panel renders use the presenter's oblique
projection: 12 px per voxel across and up, 6 px per voxel of depth, as on the panel.

    blender -b -P scripts/blender/organism_lineup.py -- OUT_DIR
"""

import math
import os
import random
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector
from mathutils.bvhtree import BVHTree

VOX = 0.125
OUT = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "/tmp/lineup"

# --- sizes -----------------------------------------------------------------------

# crates/cubarium-voxel-flora/src/lib.rs: crown_height_voxels / crown_radius_voxels,
# in 0.25 m reference voxels (FloraConfig::for_voxel_size keeps them in metres).
SIM_REF = 0.25
SIM_PLANTS = {
    "springturf": ([0.5, 1.0], [0.5, 1.0]),
    "velvetpad": ([0.5, 1.0], [1.0, 2.0]),
    "stonecushion": ([0.5, 0.5], [0.5, 1.0]),
    "glowcap": ([0.5, 0.5], [0.5, 0.5]),
    "bloomcrown": ([1.0, 3.0], [0.5, 1.5]),
    "umbrellafrond": ([2.0, 5.0], [1.0, 2.5]),
}
# organism-anatomy-2026-09-21.md §3, in 0.125 m voxels.
LADDER_PLANTS = {
    "springturf": ([1.0, 1.5], [1.0, 2.0]),
    "velvetpad": ([1.0, 1.0], [1.5, 3.0]),
    "stonecushion": ([1.0, 1.5], [1.0, 1.25]),
    "glowcap": ([1.0, 2.0], [0.5, 0.5]),
    "bloomcrown": ([3.0, 8.0], [1.0, 2.5]),
    "umbrellafrond": ([8.0, 16.0], [3.0, 6.0]),
    "vaulttree": ([18.0, 28.0], [5.0, 8.0]),
    "lanternberry": ([5.0, 9.0], [2.0, 3.5]),
    "siphonreed": ([6.0, 12.0], [0.5, 1.0]),
}
# Adult L x W x H in metres. sim: body.rs; ladder: organism-anatomy §4 / roster §4.
SIM_ANIMALS = {
    "frondgrazer": (0.375, 0.1875, 0.1875),
    "littershredder": (0.19, 0.0625, 0.0625),
}
LADDER_ANIMALS = {
    "frondgrazer": (0.75, 0.375, 0.375),
    "littershredder": (0.375, 0.125, 0.125),
}
NEWBORN = (0.005 / 0.05) ** (1.0 / 3.0)  # length ~ body^(1/3), body 0.005 -> 0.05

# Layer profiles: (wood_fraction_max, height cap m, [(kind, a, b, r, share, p)]).
# Shared species match the code (lib.rs profiles); new species are anatomy §3.
F, T, D, MAT = "foliage", "trunk", "drape", "mat"
PROFILES = {
    "springturf": [(9, None, [(MAT, 0, 1, 1, 1, 0.3)])],
    "velvetpad": [(9, None, [(MAT, 0, 1, 1, 1, 0.6)])],
    "stonecushion": [(9, None, [(F, 0, 0.6, 1, 0.7, 0.2), (F, 0.6, 1, 0.6, 0.3, 0.2)])],
    "glowcap": [(9, None, [(F, 0, 1, 1, 1, 0.5)])],
    "bloomcrown": [
        (0.2, 0.125, [(F, 0, 1, 1, 1, 0.4)]),
        (0.5, None, [(T, 0, 0.4, 0.15, 0, 0), (F, 0, 0.25, 0.7, 0.4, 0.4), (F, 0.5, 1, 1, 0.6, 0.3)]),
        (9, None, [(T, 0, 0.5, 0.15, 0, 0), (F, 0, 0.15, 0.6, 0.25, 0.4), (F, 0.55, 1, 1, 0.75, 0.3)]),
    ],
    "umbrellafrond": [
        (0.15, 0.125, [(F, 0, 1, 1, 1, 0.4)]),
        (0.5, None, [(T, 0, 0.5, 0.15, 0, 0), (F, 0.5, 0.65, 1, 0.6, 0.4), (F, 0.85, 1, 0.6, 0.4, 0.5)]),
        (9, None, [(T, 0, 0.4, 0.2, 0, 0), (F, 0.4, 0.55, 1, 0.4, 0.4), (F, 0.65, 0.8, 0.8, 0.35, 0.4),
                   (F, 0.9, 1, 0.5, 0.25, 0.5)]),
    ],
    "vaulttree": [
        (0.1, 0.125, [(F, 0, 1, 1, 1, 0.4)]),
        (0.4, None, [(T, 0, 0.6, 0.15, 0, 0), (F, 0.6, 1, 1, 1, 0.4)]),
        (9, None, [(T, 0, 0.5, 0.15, 0, 0), (T, 0.5, 0.7, 0.6, 0, 0.85), (F, 0.7, 1, 1, 0.85, 0.45),
                   (D, 0.35, 0.5, 0.7, 0.15, 0.9)]),
    ],
    "lanternberry": [
        (0.2, 0.125, [(F, 0, 1, 1, 1, 0.5)]),
        (9, None, [(T, 0, 0.3, 0.4, 0, 0.7), (F, 0.3, 1, 1, 1, 0.5)]),
    ],
    "siphonreed": [(9, None, [(F, 0, 1, 1, 1, 0.7)])],
}


def stage_of(species, wf):
    for i, (wmax, cap, layers) in enumerate(PROFILES[species]):
        if wf <= wmax:
            return i, cap, layers
    raise ValueError(species)


def plant_size(version, species, wf):
    if version == "sim":
        (h0, h1), (r0, r1) = SIM_PLANTS[species]
        unit = SIM_REF
    else:
        (h0, h1), (r0, r1) = LADDER_PLANTS[species]
        unit = VOX
    i, cap, layers = stage_of(species, wf)
    h = (h0 + (h1 - h0) * wf) * unit
    r = (r0 + (r1 - r0) * wf) * unit
    seed = PROFILES[species][0]
    if version == "ladder" and seed[1] is not None:
        # Growth fix (organism-large-animals §2, agreed 2026-09-23): a seedling is a
        # rosette <= 0.125 m tall and wide; after it, height and radius grow linearly
        # from the seedling to the adult maximum.
        hs = rs = 0.125
        if i == 0:
            h, r = min(h, hs), min(r, rs)
        else:
            t = (wf - seed[0]) / (1.0 - seed[0])
            h, r = hs + (h1 * unit - hs) * t, rs + (r1 * unit - rs) * t
    elif cap is not None:
        h = min(h, cap)
    return i, h, r, layers


# --- materials -----------------------------------------------------------------

PAL = {
    "floor": "#2A2148", "bg": "#0C0818", "wire": "#6F5FA8", "grid": "#3B3066",
    "plum": "#2A0E4A", "violet": "#3A1A7A", "detritus": "#510B6D", "lilac": "#B99BE6",
    "p0": "#1E2798", "p1": "#2B6AD0", "p2": "#42C5F8", "teal": "#248CA8",
    "magenta": "#FF2AFC", "warm": "#FF9B50", "rock": "#4F4670", "water": "#1F4E8C",
    "film": "#B8A640", "grey": "#8C8AA0", "human": "#D8D2E4", "white": "#F2F0F8",
    "label": "#E6DDF8", "sublabel": "#9D90C4", "bark": "#3A2150", "split": "#8E6CC0",
}
_mats = {}


def lin(h):
    h = h.lstrip("#")
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    return tuple(x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c)


def M(key):
    if key not in _mats:
        m = bpy.data.materials.new(key)
        m.diffuse_color = (*lin(PAL[key]), 1.0)
        _mats[key] = m
    return _mats[key]


# --- geometry helpers (all geometry baked in world coordinates) --------------------

COL = None      # current collection
ORG = []        # objects of the organism being built (voxelized)
OVL = []        # overlay objects (never voxelized)


def mk(name, bm, mat, overlay=False):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    ob.data.materials.append(M(mat))
    COL.objects.link(ob)
    (OVL if overlay else ORG).append(ob)
    return ob


def cyl(p0, p1, r0, r1=None, mat="violet", seg=10, overlay=False):
    p0, p1 = Vector(p0), Vector(p1)
    d = p1 - p0
    if d.length < 1e-6:
        return None
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=seg, radius1=r0,
                          radius2=r0 if r1 is None else r1, depth=d.length)
    q = Vector((0, 0, 1)).rotation_difference(d.normalized())
    m = Matrix.Translation((p0 + p1) / 2) @ q.to_matrix().to_4x4()
    bmesh.ops.transform(bm, matrix=m, verts=bm.verts)
    return mk("cyl", bm, mat, overlay)


def ell(c, r, mat, rot=None, overlay=False):
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=16, v_segments=10, radius=1.0)
    m = Matrix.Translation(Vector(c)) @ (rot or Matrix.Identity(4)) @ Matrix.Diagonal((*r, 1))
    bmesh.ops.transform(bm, matrix=m, verts=bm.verts)
    return mk("ell", bm, mat, overlay)


def box(c, s, mat, rot=None, overlay=False):
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    m = Matrix.Translation(Vector(c)) @ (rot or Matrix.Identity(4)) @ Matrix.Diagonal((*s, 1))
    bmesh.ops.transform(bm, matrix=m, verts=bm.verts)
    return mk("box", bm, mat, overlay)


def strap(p0, p1, w, mat):
    """A leaf or vane: a square-section rod of width w (voxelizes to >= 1 voxel)."""
    return cyl(p0, p1, w / 2, mat=mat, seg=4)


def label(text, x, y, z, size=0.14, mat="label"):
    cu = bpy.data.curves.new("label", "FONT")
    cu.body = text
    cu.size = size
    cu.align_x = "CENTER"
    cu.materials.append(M(mat))
    ob = bpy.data.objects.new("label:" + text.split("\n")[0], cu)
    ob.location = (x, y, z)
    ob.rotation_euler = (math.radians(90), 0, 0)
    COL.objects.link(ob)
    return ob


def ring_dirs(n, phase=0.0):
    return [(math.cos(phase + 2 * math.pi * k / n), math.sin(phase + 2 * math.pi * k / n)) for k in range(n)]


def rosette(x, y, z0, z1, rr, n, mat, phase=0.0, w=0.03):
    """Leaves from a buried crown leaning out to radius rr at height z1."""
    for dx, dy in ring_dirs(n, phase):
        strap((x + dx * rr * 0.12, y + dy * rr * 0.12, z0), (x + dx * rr, y + dy * rr, z1), w, mat)


# --- plants ----------------------------------------------------------------------

def springturf(x, y, h, r, layers, i, rng):
    n = 7 if r < 0.2 else 11
    rosette(x, y, 0.0, h, r, n, "p2", phase=rng.random())
    rosette(x, y, 0.0, h * 0.6, r * 0.7, max(4, n - 4), "p0", phase=rng.random())


def velvetpad(x, y, h, r, layers, i, rng):
    ell((x, y, h * 0.3), (r, r * 0.85, h * 0.45), "teal")
    for _ in range(14):
        a, d = rng.uniform(0, 2 * math.pi), r * math.sqrt(rng.random()) * 0.8
        ell((x + math.cos(a) * d, y + math.sin(a) * d * 0.85, h * 0.65), (0.03, 0.03, 0.025), "teal")


def stonecushion(x, y, h, r, layers, i, rng):
    top = 0.25
    box((x, y, top / 2), (r * 2.6, r * 2.2, top), "rock")  # the rock lip it lives on
    (_, a0, b0, r0, _, _), (_, a1, b1, r1, _, _) = layers
    ell((x, y, top), (r * r0, r * r0 * 0.9, h * b0), "lilac")
    ell((x, y, top + h * a1 * 0.8), (r * r1, r * r1 * 0.9, h * (b1 - a1) * 1.4), "lilac")


def glowcap_cap(x, y, z, h, r, scale=1.0):
    """The saddle crown on a short stalk, cyan lip underneath."""
    cyl((x, y, z), (x, y, z + h * 0.45), r * 0.35 * scale, mat="detritus")
    ell((x, y, z + h * 0.7), (r * 1.5 * scale, r * 1.0 * scale, h * 0.28), "violet")
    ell((x, y, z + h * 0.52), (r * 1.3 * scale, r * 0.85 * scale, h * 0.08), "p2")


def glowcap(x, y, h, r, layers, i, rng, log=None):
    """log = (length_m, height_m) of the dead wood the colony grows on, or None."""
    z = 0.0
    if log is not None:
        length, dia = log
        cyl((x - length / 2, y, dia / 2), (x + length / 2, y, dia / 2), dia / 2, mat="bark", seg=12)
        box((x, y - dia * 0.3, dia * 0.55), (length * 0.8, dia * 0.08, dia * 0.2), "split")
        z = dia
    glowcap_cap(x, y, z, h, r)
    if log is not None and log[0] > 0.6:
        glowcap_cap(x + 0.18, y + 0.03, z, h * 0.7, r * 0.7)
        ell((x - 0.14, y, z + 0.03), (0.025, 0.025, 0.03), "violet")  # button


def bloomcrown(x, y, h, r, layers, i, rng):
    if i == 0:
        rosette(x, y, 0, h, r, 7, "p1", phase=rng.random())
        return
    for kind, a, b, rf, share, p in layers:
        if kind == T:
            cyl((x, y, 0), (x, y, b * h), max(rf * r, 0.014), mat="violet")
        elif a == 0.0:  # basal rosette
            rosette(x, y, 0, b * h, rf * r, 8, "p1", phase=rng.random())
        else:  # crown of vanes around a core
            za, zb = a * h, b * h
            ell((x, y, za + 0.35 * (zb - za)), (0.28 * r, 0.28 * r, 0.25 * (zb - za)), "p1")
            for dx, dy in ring_dirs(6, rng.random()):
                strap((x + dx * r * 0.15, y + dy * r * 0.15, za + 0.1 * (zb - za)),
                      (x + dx * r * rf, y + dy * r * rf, zb), 0.035, "p2")
            if i == 2:
                for dx, dy in ring_dirs(4, rng.random()):
                    strap((x, y, za + 0.3 * (zb - za)), (x + dx * r * 0.5, y + dy * r * 0.5, zb * 0.97), 0.03, "p0")
            ell((x, y, za + 0.55 * (zb - za)), (0.07 * r + 0.02, 0.07 * r + 0.02, 0.05), "warm")  # bloom


def frond_tier(x, y, za, zb, rr, n, phase, up=False):
    for dx, dy in ring_dirs(n, phase):
        mid = (x + dx * rr * 0.55, y + dy * rr * 0.55, zb + (zb - za) * 0.15)
        end = (x + dx * rr, y + dy * rr, zb if up else za)
        strap((x, y, zb), mid, 0.045, "p1")
        strap(mid, end, 0.04, "p1")


def umbrellafrond(x, y, h, r, layers, i, rng):
    if i == 0:
        frond_tier(x, y, 0, h, r, 6, rng.random())
        return
    tiers = [l for l in layers if l[0] == F]
    for kind, a, b, rf, share, p in layers:
        if kind == T:
            cyl((x, y, 0), (x, y, b * h), max(rf * r * 0.5, 0.02), mat="violet")
    top = tiers[-1][2] * h
    cyl((x, y, 0), (x, y, top), max(r * 0.05, 0.015), mat="violet")
    for k, (kind, a, b, rf, share, p) in enumerate(tiers):
        frond_tier(x, y, a * h, b * h, rf * r, 8 - k, rng.random(), up=(k == len(tiers) - 1))


def vaulttree(x, y, h, r, layers, i, rng):
    if i == 0:
        rosette(x, y, 0, h, r, 6, "p1", phase=rng.random())
        return
    trunk = layers[0]
    cyl((x, y, 0), (x, y, trunk[2] * h), max(trunk[3] * r * 0.5, 0.03), max(trunk[3] * r * 0.35, 0.025), mat="plum")
    if i == 1:
        (_, a, b, _, _, _) = layers[1]
        cyl((x, y, trunk[2] * h), (x, y, b * h * 0.95), 0.03, mat="plum")
        for dx, dy in ring_dirs(3, rng.random()):
            ell((x + dx * r * 0.5, y + dy * r * 0.5, (a + 0.4 * (b - a)) * h), (0.42 * r, 0.42 * r, 0.2 * (b - a) * h), "p1")
        ell((x, y, (a + 0.8 * (b - a)) * h), (0.35 * r, 0.35 * r, 0.15 * (b - a) * h), "p1")
        return
    # adult: arching limbs with sky between, lobes at their ends, drapes under the lobes
    z0, zl = 0.5 * h, 0.7 * h
    top_lobe = (x, y, 0.9 * h)
    cyl((x, y, z0), top_lobe, 0.035, mat="plum")
    ell(top_lobe, (0.3 * r, 0.3 * r, 0.05 * h), "p1")
    for dx, dy in ring_dirs(5, rng.random()):
        elbow = (x + dx * r * 0.6, y + dy * r * 0.6, zl)
        tip = (x + dx * r * 0.68, y + dy * r * 0.68, 0.82 * h)
        cyl((x, y, z0), elbow, 0.045, 0.03, mat="plum")
        cyl(elbow, tip, 0.03, 0.02, mat="plum")
        ell(tip, (0.32 * r, 0.32 * r, 0.06 * h), "p1")
        for k in range(3):
            ax, ay = tip[0] + rng.uniform(-0.15, 0.15) * r, tip[1] + rng.uniform(-0.15, 0.15) * r
            cyl((ax, ay, 0.72 * h), (ax, ay, rng.uniform(0.35, 0.45) * h), 0.008, mat="teal", seg=4)


def lanternberry(x, y, h, r, layers, i, rng):
    if i == 0:
        rosette(x, y, 0, h, r, 7, "p1", phase=rng.random())
        return
    fan = [(-0.85, 0.1), (-0.35, -0.2), (0.3, 0.25), (0.8, -0.1)]
    for k, (fx, fy) in enumerate(fan):
        tip = Vector((x + fx * r, y + fy * r, h * (0.88 + 0.12 * rng.random())))
        base = Vector((x + fx * 0.04, y + fy * 0.04, 0))
        cyl(base, tip, 0.02, 0.012, mat="violet")
        for j in range(7):
            p = base.lerp(tip, 0.3 + 0.1 * j)
            side = 1 if j % 2 else -1
            box((p.x + side * 0.03, p.y, p.z), (0.05, 0.03, 0.012), "p1")
        cyl(tip, tip - Vector((0, 0, 0.04)), 0.006, mat="violet", seg=4)
        ell(tip - Vector((0, 0, 0.075)), (0.03, 0.03, 0.04), "warm")


def siphonreed(x, y, h, r, layers, i, rng):
    n = 3 if h < 1.0 else 5
    for k in range(n):
        a, d = rng.uniform(0, 2 * math.pi), r * math.sqrt(rng.random())
        px, py = x + math.cos(a) * d, y + math.sin(a) * d
        hh = h * rng.uniform(0.7, 1.0)
        cyl((px, py, 0), (px, py, hh), 0.012, mat="p1", seg=6)
        ell((px, py, hh + 0.015), (0.022, 0.022, 0.03), "p2")
    # the water it needs, flush with the ground (a pool edge)
    box((x, y, 0.03), (2 * r + 0.3, 2 * r + 0.25, 0.06), "water")


def glassfilm(x, y, rng):
    box((x, y, 0.05), (0.5, 0.35, 0.1), "rock")
    box((x, y, 0.103), (0.46, 0.31, 0.006), "film")


# --- animals ---------------------------------------------------------------------

def frondgrazer(x, y, L, W, H, overlays=True):
    ell((x, y, 0.62 * H), (0.5 * L, 0.5 * W, 0.28 * H), "plum")
    for sx in (0.26, -0.24):
        ell((x + sx * L, y, 0.68 * H), (0.2 * L, 0.46 * W, 0.3 * H), "violet")
    for sx in (0.3, 0.0, -0.3):
        for sy in (-1, 1):
            hip = (x + sx * L, y + sy * 0.35 * W, 0.45 * H)
            knee = (x + sx * L, y + sy * 0.62 * W, 0.35 * H)
            foot = (x + sx * L, y + sy * 0.66 * W, 0.0)
            cyl(hip, knee, 0.06 * W, mat="plum", seg=6)
            cyl(knee, foot, 0.05 * W, mat="plum", seg=6)
    cyl((x + 0.42 * L, y, 0.45 * H), (x + 0.58 * L, y, 0.72 * H), 0.1 * W, mat="violet", seg=8)
    box((x + 0.6 * L, y, 0.78 * H), (0.1 * L, 0.45 * W, 0.12 * H), "lilac")
    for sy in (-1, 1):
        box((x + 0.26 * L, y + sy * 0.2 * W, 0.97 * H), (0.08 * L, 0.1 * W, 0.04 * H), "magenta")
    if overlays:
        # mouth band: [0, 1.33 H] above the standing surface, 0.25 L ahead (decision 2)
        x0, x1, z1 = x + 0.5 * L, x + 0.75 * L, 1.33 * H
        for a, b in [((x0, y, 0), (x0, y, z1)), ((x1, y, 0), (x1, y, z1)), ((x0, y, z1), (x1, y, z1))]:
            cyl(a, b, 0.006, mat="magenta", seg=4, overlay=True)


def littershredder(x, y, L, W, H):
    for k in range(5):
        f = 1.0 - 0.1 * k
        ell((x + (0.38 - 0.19 * k) * L, y, 0.5 * H * f), (0.13 * L, 0.5 * W * f, 0.5 * H * f), "p0")
    for sy in (-1, 1):
        cyl((x + 0.45 * L, y + sy * 0.2 * W, 0.45 * H), (x + 0.62 * L, y + sy * 0.4 * W, 0.0), 0.04 * W + 0.004, mat="lilac", seg=4)
    box((x + 0.19 * L, y, 0.98 * H), (0.05 * L, 0.15 * W, 0.04 * H), "magenta")


def bellwing(x, y, z):
    cyl((x, y, z), (x, y, z + 0.09), 0.04, 0.018, mat="plum", seg=12)
    cyl((x, y, z - 0.005), (x, y, z + 0.005), 0.042, mat="magenta", seg=12)
    cyl((x, y, z), (x + 0.02, y, z - 0.035), 0.005, mat="lilac", seg=4)
    for sx, sweep in ((0.02, 0.35), (-0.02, -0.35)):
        for sy in (-1, 1):
            p0 = Vector((x + sx, y + sy * 0.02, z + 0.07))
            p1 = p0 + Vector((sweep * 0.3, sy * 0.16, 0.01))
            strap(p0, p1, 0.02, "lilac")
    # flight band 4-20 voxels, drawn beside it
    cyl((x + 0.22, y, 4 * VOX), (x + 0.22, y, 20 * VOX), 0.006, mat="p2", seg=4, overlay=True)
    for zz in (4 * VOX, 20 * VOX):
        cyl((x + 0.17, y, zz), (x + 0.27, y, zz), 0.006, mat="p2", seg=4, overlay=True)


def lanternjaw(x, y):
    """Mesopredator at coyote scale, 8 x 2 x 3.5 v (organism-large-animals §4)."""
    ell((x - 0.08, y, 0.3), (0.36, 0.12, 0.12), "detritus")
    ell((x + 0.34, y, 0.3), (0.15, 0.115, 0.13), "plum")
    box((x + 0.36, y, 0.2), (0.22, 0.16, 0.05), "lilac")
    for hx in (-0.34, -0.12):
        for sy in (-1, 1):
            cyl((x + hx, y + sy * 0.08, 0.26), (x + hx + 0.06, y + sy * 0.12, 0.0), 0.022, mat="plum", seg=6)
    for hx in (0.18,):
        for sy in (-1, 1):
            cyl((x + hx, y + sy * 0.08, 0.26), (x + hx + 0.05, y + sy * 0.11, 0.0), 0.02, mat="plum", seg=6)
    cyl((x + 0.4, y, 0.42), (x + 0.52, y, 0.58), 0.008, mat="plum", seg=4)
    ell((x + 0.54, y, 0.6), (0.03, 0.03, 0.03), "p2")


def loftstrider(x, y, s, feeding):
    """Canopy browser, 'stilt column' (organism-large-animals §3). s = length scale."""
    ell((x, y, 1.375 * s), (0.75 * s, 0.275 * s, 0.225 * s), "plum")
    ell((x, y, 1.17 * s), (0.6 * s, 0.05 * s, 0.03 * s), "lilac")
    legs = [((0.55, 0), (0.8, 0), (0.85, 0)), ((-0.55, 0), (-0.8, 0), (-0.85, 0)),
            ((0.0, 0.2), (0.0, 0.45), (0.0, 0.55)), ((0.0, -0.2), (0.0, -0.45), (0.0, -0.55))]
    for (hx, hy), (kx, ky), (fx, fy) in legs:
        hip, knee, foot = (x + hx * s, y + hy * s, 1.2 * s), (x + kx * s, y + ky * s, 0.7 * s), (x + fx * s, y + fy * s, 0.0)
        cyl(hip, knee, 0.035 * s, mat="plum", seg=6)
        cyl(knee, foot, 0.03 * s, mat="plum", seg=6)
        ell((foot[0], foot[1], 0.015 * s), (0.08 * s, 0.08 * s, 0.015 * s), "detritus")
        if hy:
            cyl((foot[0], foot[1], 0.12 * s), (foot[0], foot[1] + 0.12 * s * (1 if hy > 0 else -1), 0.2 * s), 0.02 * s, 0.004, mat="lilac", seg=4)
    base = 1.5 * s
    top = 3.5 * s if feeding else 2.2 * s
    cx = x + 0.35 * s
    seg = (top - base) / 3
    for k, rad in enumerate((0.075, 0.06, 0.045)):
        z0, z1 = base + k * seg, base + (k + 1) * seg + 0.05 * s
        cyl((cx, y, z0), (cx, y, z1), rad * s, mat="plum", seg=10)
        cyl((cx, y, z1 - 0.04 * s), (cx, y, z1), (rad + 0.008) * s, mat="lilac", seg=10)
    cyl((cx, y, top - 0.1 * s), (cx, y, top - 0.06 * s), 0.07 * s, mat="magenta", seg=12)
    for dx, dy in ring_dirs(6):
        strap((cx, y, top), (cx + dx * 0.12 * s, y + dy * 0.12 * s, top + 0.14 * s), 0.02 * s, "lilac")
    for sy in (-1, 1):
        box((x + 0.66 * s, y + sy * 0.1 * s, 1.45 * s), (0.06 * s, 0.06 * s, 0.05 * s), "magenta")
    # mouth band 0.8 m .. 3.5 m (scaled), around the column
    x0, x1, z0, z1 = cx - 0.35 * s, cx + 0.35 * s, 0.8 * s, 3.5 * s
    for a, b in [((x0, y, z0), (x0, y, z1)), ((x1, y, z0), (x1, y, z1)), ((x0, y, z0), (x1, y, z0)), ((x0, y, z1), (x1, y, z1))]:
        cyl(a, b, 0.006, mat="magenta", seg=4, overlay=True)


def chorister(x, y, s=1.0):
    """Pack hunter, 11 x 3 x 5 v + tail (organism-large-animals §5)."""
    ell((x, y, 0.5 * s), (0.7 * s, 0.17 * s, 0.14 * s), "violet")
    for hx in (0.45, -0.45):
        for sy in (-1, 1):
            hip, foot = (x + hx * s, y + sy * 0.12 * s, 0.45 * s), (x + (hx + 0.08) * s, y + sy * 0.18 * s, 0.0)
            knee = (x + (hx - 0.08) * s, y + sy * 0.18 * s, 0.25 * s)
            cyl(hip, knee, 0.035 * s, mat="plum", seg=6)
            cyl(knee, foot, 0.03 * s, mat="plum", seg=6)
    cyl((x - 0.65 * s, y, 0.52 * s), (x - 1.45 * s, y, 0.6 * s), 0.05 * s, 0.015 * s, mat="violet", seg=8)
    for sy in (-1, 1):
        box((x + 0.78 * s, y + sy * 0.07 * s, 0.5 * s), (0.26 * s, 0.035 * s, 0.13 * s), "lilac")
    ell((x + 0.6 * s, y, 0.34 * s), (0.1 * s, 0.08 * s, 0.07 * s), "p2")


def proxy(kind, x, y):
    """Planned animals with a ladder size and a one-line role, no body plan yet."""
    if kind == "capgnawer":        # 2 x 1.5 v
        ell((x, y, 0.094), (0.125, 0.09, 0.094), "grey")
    elif kind == "ripple snail":   # 1.5 x 1.5 v
        ell((x, y, 0.0), (0.094, 0.08, 0.1875), "grey")
    elif kind == "lanternjaw":     # 5 x 2 v
        ell((x - 0.05, y, 0.13), (0.26, 0.12, 0.12), "grey")
        ell((x + 0.24, y, 0.14), (0.08, 0.09, 0.1), "grey")
    elif kind == "seedporter":     # 4 x 2 v, long tail, climbs trunks
        ell((x, y, 0.14), (0.2, 0.09, 0.11), "grey")
        cyl((x - 0.18, y, 0.14), (x - 0.34, y, 0.3), 0.025, 0.01, mat="grey", seg=6)


# --- references ------------------------------------------------------------------

def post(x, y):
    for k in range(8):
        box((x, y, (k + 0.5) * VOX), (0.06, 0.06, VOX), "white" if k % 2 == 0 else "magenta", overlay=True)
    label("1 m", x, y - 0.1, 1.05, 0.1, "sublabel")


def human(x, y):
    for sy in (-0.09, 0.09):
        cyl((x, y + sy, 0), (x, y + sy, 0.85), 0.065, 0.055, mat="human", overlay=True)
    ell((x, y, 1.18), (0.12, 0.2, 0.34), "human", overlay=True)
    ell((x, y, 1.62), (0.1, 0.09, 0.12), "human", overlay=True)
    for sy in (-0.27, 0.27):
        cyl((x, y + sy, 1.45), (x, y + sy * 1.1, 0.8), 0.04, mat="human", overlay=True)


def world_box(ox, title, sub):
    W, Dp, Hh = 20.0, 3.0, 9.0
    box((ox + W / 2, Dp / 2, -0.01), (W, Dp, 0.02), "floor", overlay=True)
    corners = [(ox + a * W, b * Dp, c * Hh) for a in (0, 1) for b in (0, 1) for c in (0, 1)]
    for p in corners:
        for q in corners:
            if p < q and sum(1 for u, v in zip(p, q) if u != v) == 1:
                cyl(p, q, 0.015, mat="wire", seg=6, overlay=True)
    for zz in range(1, 9):
        cyl((ox, Dp, zz), (ox + W, Dp, zz), 0.006, mat="grid", seg=4, overlay=True)
        label(f"{zz} m", ox - 0.3, Dp, zz - 0.05, 0.14, "sublabel")
    label(title, ox + 0.3 + len(title) * 0.07, Dp, 8.45, 0.3)
    label(sub, ox + 0.3 + len(sub) * 0.035, Dp, 8.05, 0.15, "sublabel")


# --- layout ----------------------------------------------------------------------

def snap(v):
    return (math.floor(v / VOX) + 0.5) * VOX


def plant_items(version, species, stages, **kw):
    fn = globals()[species]
    items = []
    for name, wf in stages:
        i, h, r, layers = plant_size(version, species, wf)

        def build(x, y, i=i, h=h, r=r, layers=layers, wf=wf):
            fn(x, y, h, r, layers, i, random.Random(f"{species}:{wf}:{version}"), **kw)
        items.append((f"{name}\n{h / VOX:.3g} v", 2 * r, build))
    return items


def animal_items(version, species):
    L, W, H = (SIM_ANIMALS if version == "sim" else LADDER_ANIMALS)[species]
    fn = globals()[species]
    items = []
    for name, s in (("newborn", NEWBORN), ("adult", 1.0)):
        items.append((f"{name}\n{L * s / VOX:.2g}x{H * s / VOX:.2g} v", L * s * (1.6 if species == "frondgrazer" else 1.3),
                      lambda x, y, s=s: fn(x, y, L * s, W * s, H * s)))
    return items


def rows(version):
    lad = version == "ladder"
    three = [("seedling", 0.1), ("juvenile", 0.35), ("adult", 1.0)]
    front = [
        ("springturf", plant_items(version, "springturf", [("young", 0.1), ("full", 1.0)])),
        ("velvetpad", plant_items(version, "velvetpad", [("full", 1.0)])),
        ("stonecushion", plant_items(version, "stonecushion", [("full", 1.0)])),
    ]
    if lad:
        front += [
            ("glowcap", plant_items(version, "glowcap", [("section log", 0.2)], log=(0.5, 1.5 * VOX))
             + plant_items(version, "glowcap", [("vaulttree fall", 1.0)], log=(1.0, 2.5 * VOX))),
            ("glassfilm", [("", 0.5, lambda x, y: glassfilm(x, y, None))]),
        ]
    else:
        front += [("glowcap (no log)", plant_items(version, "glowcap", [("", 1.0)]))]
    front += [
        ("littershredder", animal_items(version, "littershredder")),
        ("frondgrazer", animal_items(version, "frondgrazer")),
    ]
    if lad:
        front += [
            ("bellwing", [("hovering 4 v", 0.45, lambda x, y: bellwing(x - 0.08, y, 4 * VOX))]),
            ("capgnawer*", [("2x1.5 v", 0.3, lambda x, y: proxy("capgnawer", x, y))]),
            ("ripple snail*", [("1.5x1.5 v", 0.25, lambda x, y: proxy("ripple snail", x, y))]),
            ("lanternjaw", [("8x3.5 v", 1.1, lambda x, y: lanternjaw(x, y))]),
            ("seedporter*", [("4x2 v", 0.7, lambda x, y: proxy("seedporter", x, y))]),
            ("chorister pack (for decision)", [("11x5 v + tail", 3.6, lambda x, y: [
                chorister(x - 0.6, y - 0.35), chorister(x + 0.25, y + 0.1), chorister(x + 1.0, y + 0.5)])]),
        ]
    back = [
        ("bloomcrown", plant_items(version, "bloomcrown", three)),
        ("umbrellafrond", plant_items(version, "umbrellafrond", three)),
    ]
    if lad:
        back.insert(1, ("lanternberry", plant_items(version, "lanternberry", [("seedling", 0.1), ("adult", 1.0)])))
        back.insert(2, ("siphonreed (in water)", plant_items(version, "siphonreed", [("young", 0.2), ("adult", 1.0)])))
        back.append(("vaulttree", plant_items(version, "vaulttree", three)))
        back.append(("loftstrider (new)", [("newborn, resting", 1.0, lambda x, y: loftstrider(x, y, NEWBORN, False)),
                                           ("adult, feeding: reach 28 v", 1.9, lambda x, y: loftstrider(x, y, 1.0, True))]))
    return front, back


def voxelize(objs):
    """Cells whose centre lies inside a part or within half a voxel of its surface."""
    parts = []
    for ob in objs:
        me = ob.data
        vs = [v.co.copy() for v in me.vertices]
        if not vs:
            continue
        bvh = BVHTree.FromPolygons(vs, [tuple(p.vertices) for p in me.polygons])
        lo = Vector(min(v[k] for v in vs) for k in range(3))
        hi = Vector(max(v[k] for v in vs) for k in range(3))
        parts.append((bvh, lo, hi, ob.active_material.name))
    if not parts:
        return {}
    lo = Vector(min(p[1][k] for p in parts) for k in range(3))
    hi = Vector(max(p[2][k] for p in parts) for k in range(3))
    cells = {}
    rng = [range(math.floor(lo[k] / VOX) - 1, math.ceil(hi[k] / VOX) + 1) for k in range(3)]
    half = VOX / 2
    for i in rng[0]:
        for j in rng[1]:
            for k in rng[2]:
                if k < 0:
                    continue
                c = Vector(((i + 0.5) * VOX, (j + 0.5) * VOX, (k + 0.5) * VOX))
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
    return cells


def emit_voxels(cells, name):
    by_mat = {}
    for key, mat in cells.items():
        by_mat.setdefault(mat, []).append(key)
    for mat, keys in by_mat.items():
        bm = bmesh.new()
        for i, j, k in keys:
            m = Matrix.Translation(((i + 0.5) * VOX, (j + 0.5) * VOX, (k + 0.5) * VOX)) @ Matrix.Diagonal((VOX, VOX, VOX, 1))
            bmesh.ops.create_cube(bm, size=1.0, matrix=m)
        me = bpy.data.meshes.new(f"vox:{name}:{mat}")
        bm.to_mesh(me)
        bm.free()
        ob = bpy.data.objects.new(me.name, me)
        ob.data.materials.append(M(mat))
        COL.objects.link(ob)


def build_box(version, ox, vox, stats):
    global COL
    COL = bpy.data.collections.new(f"{version}{'-voxels' if vox else ''}")
    bpy.context.scene.collection.children.link(COL)
    title = "SIM TODAY" if version == "sim" else "SIZE LADDER (accepted 2026-09-23, growth fix applied)"
    sub = ("sizes on main: flora ranges x 0.25 m, bodies from body.rs.  magenta frame = grazer mouth band"
           if version == "sim" else "anatomy §3-4 + roster §4.  * grey = planned animal, size only.  magenta frame = mouth band.  cyan line = bellwing flight band 4-20 v")
    world_box(ox, title + (" — voxelized 0.125 m" if vox else ""), sub)
    front, back = rows(version)
    for row, y in ((front, 0.8), (back, 2.05)):
        x = ox + 0.5
        for gname, items in row:
            gx0, gtop = x, 0.0
            for sub_name, width, build in items:
                cx, cy = snap(x + width / 2), snap(y)
                ORG.clear()
                build(cx, cy)
                top = max((v.co.z for ob in ORG for v in ob.data.vertices), default=0.0)
                gtop = max(gtop, top)
                if vox:
                    objs = list(ORG)
                    cells = voxelize(objs)
                    stats[(version, gname.split(chr(10))[0], sub_name.split(chr(10))[0])] = len(cells)
                    emit_voxels(cells, gname)
                    for ob in objs:
                        bpy.data.objects.remove(ob, do_unlink=True)
                if sub_name:
                    text = sub_name.replace(chr(10), " ")
                    if row is front:
                        label(text, cx, 0.12, 0.0, 0.075, "sublabel")
                    else:
                        label(text, cx, cy, top + 0.08, 0.075, "sublabel")
                x += max(width, 0.2) + 0.15
            gname1 = gname.replace(chr(10), " ")
            if row is front:
                label(gname1, (gx0 + x - 0.15) / 2, 0.42, 0.0, 0.1)
            else:
                label(gname1, (gx0 + x - 0.15) / 2, y, gtop + 0.25, 0.12)
            x += 0.3
        stats[(version, "row width", "front" if row is front else "back")] = round(x - ox, 2)
    post(ox + 19.1, 0.8)
    human(ox + 19.55, 0.8)
    label("1.75 m", ox + 19.55, 0.7, 1.85, 0.1, "sublabel")


# --- scene, cameras, renders -------------------------------------------------------

def setup_scene():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    sc = bpy.context.scene
    sc.render.engine = "BLENDER_WORKBENCH"
    sh = sc.display.shading
    sh.light = "STUDIO"
    sh.color_type = "MATERIAL"
    sh.show_cavity = True
    sh.show_object_outline = True
    sh.object_outline_color = (0.02, 0.01, 0.05)
    sh.show_shadows = True
    sh.shadow_intensity = 0.35
    sc.display.render_aa = "8"
    sc.view_settings.view_transform = "Standard"
    if sc.world is None:
        sc.world = bpy.data.worlds.new("World")
    sc.world.color = lin(PAL["bg"])
    return sc


def camera(name, cx, tilt_deg, scale, oblique=False):
    cam = bpy.data.cameras.new(name)
    cam.type = "ORTHO"
    cam.ortho_scale = scale
    cam.clip_end = 200
    ob = bpy.data.objects.new(name, cam)
    bpy.context.scene.collection.objects.link(ob)
    t = math.radians(tilt_deg)
    target = Vector((cx, 1.5, 5.25 if oblique else 4.2))
    view = Vector((0, math.cos(t), -math.sin(t)))
    ob.location = target - view * 60
    ob.rotation_euler = (math.radians(90) - t, 0, 0)
    return ob


def render(cam, path, w, h):
    sc = bpy.context.scene
    sc.camera = cam
    sc.render.resolution_x, sc.render.resolution_y = w, h
    sc.render.resolution_percentage = 100
    sc.render.filepath = path
    bpy.ops.render.render(write_still=True)


def main():
    os.makedirs(OUT, exist_ok=True)
    sc = setup_scene()
    stats = {}
    boxes = [("sim", 0.0, False), ("ladder", 24.0, False), ("sim", 48.0, True), ("ladder", 72.0, True)]
    for version, ox, vox in boxes:
        build_box(version, ox, vox, stats)

    # the .blend keeps true geometry and a few orbitable cameras
    cams = []
    for version, ox, vox in boxes:
        tag = f"{version}{'-vox' if vox else ''}"
        cams.append((tag, camera(f"panel-{tag}", ox + 10, 0, 20.0, oblique=True),
                     camera(f"tilt30-{tag}", ox + 10, 30, 20.6)))
    persp = bpy.data.cameras.new("overview")
    pob = bpy.data.objects.new("overview", persp)
    sc.collection.objects.link(pob)
    pob.location = (34, -26, 14)
    pob.rotation_euler = (math.radians(68), 0, math.radians(-12))
    sc.camera = cams[1][2]
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT, "organism_lineup.blend"))

    for tag, _, tcam in cams:
        render(tcam, os.path.join(OUT, f"tilt30-{tag}.png"), 3840, 2160)

    # Presenter projection: shear every object so depth y lifts it by y/2, then look
    # straight on. 12 px per voxel across and up, 6 per voxel of depth: the panel.
    shear = Matrix.Identity(4)
    shear[2][1] = 0.5
    root = bpy.data.objects.new("oblique", None)
    sc.collection.objects.link(root)
    for ob in list(sc.objects):
        if ob.type in {"MESH", "FONT"}:
            ob.parent = root
            ob.matrix_parent_inverse = shear
    for tag, pcam, _ in cams:
        render(pcam, os.path.join(OUT, f"panel-{tag}.png"), 1920, 1080)
        render(pcam, os.path.join(OUT, f"panel-{tag}-2x.png"), 3840, 2160)

    with open(os.path.join(OUT, "stats.txt"), "w") as f:
        for k, v in stats.items():
            f.write(f"{' / '.join(k)}: {v}\n")


if __name__ == "__main__":
    main()
