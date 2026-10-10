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


# --- model-pass helpers (2026-10-01): tubes, lofted ribbons, solids of revolution ------

def tube(pts, r0, r1, mat, seg=8):
    """A smooth tapering tube through a polyline: cylinders joined by spheres."""
    n = len(pts) - 1
    for k in range(n):
        cyl(pts[k], pts[k + 1], r0 + (r1 - r0) * k / n, r0 + (r1 - r0) * (k + 1) / n, mat=mat, seg=seg)
        if k:
            rr = r0 + (r1 - r0) * k / n
            ell(pts[k], (rr * 0.95, rr * 0.95, rr * 0.95), mat)


def bezier(p0, p1, p2, n):
    """n points along a quadratic Bezier."""
    p0, p1, p2 = Vector(p0), Vector(p1), Vector(p2)
    return [p0 * (1 - t) ** 2 + p1 * 2 * t * (1 - t) + p2 * t * t for t in (k / (n - 1) for k in range(n))]


def _closed(bm, rings, cap=True):
    """Quads between consecutive rings (equal lengths), capped at both ends."""
    m = len(rings[0])
    for a, b in zip(rings, rings[1:]):
        for k in range(m):
            bm.faces.new((a[k], a[(k + 1) % m], b[(k + 1) % m], b[k]))
    if cap:
        bm.faces.new(list(reversed(rings[0])))
        bm.faces.new(rings[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)


def ribbon(pts, widths, mat, th=0.012, up=(0, 0, 1)):
    """A flat leaf blade along a path: width widths[k] at pts[k], lying across `up`."""
    pts = [Vector(p) for p in pts]
    bm = bmesh.new()
    rings = []
    for k, p in enumerate(pts):
        t = (pts[min(k + 1, len(pts) - 1)] - pts[max(k - 1, 0)]).normalized()
        side = t.cross(Vector(up))
        if side.length < 1e-6:
            side = t.cross(Vector((1, 0, 0)))
        side.normalize()
        nrm = side.cross(t).normalized()
        w = max(widths[k], 0.004) / 2
        rings.append([bm.verts.new(p + side * sx * w + nrm * sz * th / 2)
                      for sx, sz in ((1, 1), (-1, 1), (-1, -1), (1, -1))])
    _closed(bm, rings)
    return mk("ribbon", bm, mat)


def revolve(c, prof, mat, seg=24, rot=None):
    """A closed solid of revolution about the vertical through c: prof is the outline as
    (radius, height) pairs, walked as a closed polygon (radii > 0)."""
    c = Vector(c)
    bm = bmesh.new()
    rings = []
    for k in range(seg):
        a = 2 * math.pi * k / seg
        rings.append([bm.verts.new(Vector((rr * math.cos(a), rr * math.sin(a), zz))) for rr, zz in prof])
    m = len(prof)
    for k in range(seg):
        a, b = rings[k], rings[(k + 1) % seg]
        for j in range(m):
            bm.faces.new((a[j], a[(j + 1) % m], b[(j + 1) % m], b[j]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bmesh.ops.transform(bm, matrix=Matrix.Translation(c) @ (rot or Matrix.Identity(4)), verts=bm.verts)
    return mk("revolve", bm, mat)


def join(objs, name="joined"):
    """Merge parts of one material into one object (the bake tests each object once)."""
    objs = [o for o in objs if o is not None]
    if not objs:
        return None
    mat = objs[0].active_material.name
    bm = bmesh.new()
    for ob in objs:
        bm.from_mesh(ob.data)
        ORG.remove(ob)
        me = ob.data
        bpy.data.objects.remove(ob, do_unlink=True)
        bpy.data.meshes.remove(me)
    return mk(name, bm, mat)


class Collect:
    """`with Collect() as parts:` gathers every object built inside the block."""

    def __enter__(self):
        self.n = len(ORG)
        self.objs = []
        return self.objs

    def __exit__(self, *exc):
        self.objs.extend(ORG[self.n:])
        return False


# --- plants ----------------------------------------------------------------------

def turf_leaf(x, y, ang, lean, ln, w, z0, mat, rng):
    """A springturf leaf: a broad strap rising from the crown, arching outward to a
    ragged, slightly cupped tip."""
    dx, dy = math.cos(ang), math.sin(ang)
    p0 = (x + dx * w * 0.3, y + dy * w * 0.3, z0)
    p1 = (x + dx * ln * lean * 0.35, y + dy * ln * lean * 0.35, z0 + ln * 0.75)
    p2 = (x + dx * ln * lean, y + dy * ln * lean, z0 + ln * (0.85 + 0.1 * rng.random()))
    path = bezier(p0, p1, p2, 6)
    ribbon(path, [w * (0.8 + 0.25 * k / 5) for k in range(6)], mat, th=w * 0.25,
           up=(dx, dy, 0))


def springturf(x, y, h, r, layers, i, rng):
    """Artbook pick (springturf.png): a low spreading clump; a back rank of tall broad
    mint-teal straps fanning out to ragged pale tips (p2), a front rank of shorter dark
    teal straps overlapping their bases (p0), violet showing deep between them."""
    n_back = 7 if r < 0.2 else 11
    ell((x, y, h * 0.12), (r * 0.45, r * 0.3, h * 0.18), "violet")
    ph = rng.uniform(-0.1, 0.1)
    for k in range(n_back):  # the tall rank, all round the crown
        a = 2 * math.pi * k / n_back + ph
        ln = h * rng.uniform(0.8, 1.0)
        turf_leaf(x, y, a, r * 0.9 / ln, ln, r * 0.16, 0.0, "p2", rng)
    n_front = max(4, n_back - 5)
    for k in range(n_front):  # the short dark rank, outside the tall one's bases
        a = 2 * math.pi * (k + 0.5) / n_front + ph
        ln = h * 0.55
        turf_leaf(x, y, a, r * 0.75 / ln, ln, r * 0.22, 0.0, "p0", rng)


def velvetpad(x, y, h, r, layers, i, rng):
    """Artbook pick (velvetpad-provisional.png, provisional): a spreading violet pad with
    a raised purple perimeter lip (plum), irregular cobalt moss tufts (p1) packed over
    it, thin cyan branching channels between the tufts (teal), and smooth round
    blue-to-cyan young buds (p2) in the channels. Built in the order the bake should
    prefer where parts share a cell (the first part holding a cell's centre keeps it):
    buds, moss, channels, lip, pad."""
    zt = min(h, 0.11)
    tufts = []
    for _ in range(40):
        a, d = rng.uniform(0, 2 * math.pi), r * 0.8 * math.sqrt(rng.random())
        c = (x + math.cos(a) * d, y + math.sin(a) * d * 0.86)
        if all(math.hypot(c[0] - t[0], c[1] - t[1]) > r * 0.3 for t in tufts):
            tufts.append(c)
    with Collect() as buds:
        for _ in range(18):
            a, d = rng.uniform(0, 2 * math.pi), r * 0.8 * math.sqrt(rng.random())
            bx, by = x + math.cos(a) * d, y + math.sin(a) * d * 0.86
            if any(math.hypot(bx - t[0], by - t[1]) < r * 0.14 for t in tufts):
                continue
            s = r * rng.uniform(0.04, 0.1)
            revolve((bx, by, zt * 0.35), [(0.002, 0.0), (s, 0.0), (s, s * 0.5), (0.002, s * 0.55)], "p2", seg=12)
    join(buds, "buds")
    with Collect() as moss:
        for cx, cy in tufts:
            s = r * rng.uniform(0.2, 0.3)
            ell((cx, cy, zt * 0.45), (s, s * rng.uniform(0.7, 1.0), zt * 0.5), "p1")
    join(moss, "moss")
    with Collect() as chans:  # channels: branching lines from the centre toward the lip
        for dx, dy in ring_dirs(5, rng.random() * 6.3):
            mid = (x + dx * r * 0.45 + rng.uniform(-0.1, 0.1) * r, y + dy * r * 0.4, zt * 0.36)
            cyl((x, y, zt * 0.36), mid, 0.004, mat="teal", seg=4)
            for sg in (-1, 1):
                end = (mid[0] + (dx - sg * dy * 0.6) * r * 0.35, mid[1] + (dy + sg * dx * 0.6) * r * 0.3, zt * 0.36)
                cyl(mid, end, 0.003, mat="teal", seg=4)
    join(chans, "channels")
    with Collect() as lip:  # the perimeter lip, a ring of overlapping lobes
        for k in range(24):
            a = 2 * math.pi * k / 24
            ell((x + math.cos(a) * r * 0.92, y + math.sin(a) * r * 0.8, zt * 0.2),
                (r * 0.12, r * 0.12, zt * 0.3), "plum")
    join(lip, "lip")
    ell((x, y, zt * 0.25), (r * 0.98, r * 0.86, zt * 0.36), "violet")  # the pad


def stonecushion(x, y, h, r, layers, i, rng):
    """Artbook pick (stonecushion.png): an alien mound of plump pale-lilac lobes (lilac),
    a ring round the rock and a few on top, some lobes puckered round a cyan pore (p2),
    over a dark magenta-violet root mat spreading on the rock (detritus)."""
    top = 0.25
    box((x, y, top / 2), (r * 2.6, r * 2.2, top), "rock")  # the rock lip it lives on
    (_, a0, b0, r0, _, _), (_, a1, b1, r1, _, _) = layers
    with Collect() as mat:  # the root mat: branching dark strands over the rock
        for dx, dy in ring_dirs(9, rng.random() * 6.3):
            cyl((x + dx * r * 0.5, y + dy * r * 0.45, top + 0.006),
                (x + dx * r * 1.1, y + dy * r * 0.95, top + 0.006), 0.012, 0.005, mat="detritus", seg=5)
    join(mat, "rootmat")
    lobes = []
    n = 6
    for k, (dx, dy) in enumerate(ring_dirs(n, rng.random() * 6.3)):
        s = r * r0 * rng.uniform(0.42, 0.55)
        lobes.append(((x + dx * r * r0 * 0.58, y + dy * r * r0 * 0.52, top + h * b0 * 0.4),
                      (s * 1.25, s * 0.8, h * b0 * 0.42)))
    for dx, dy in ring_dirs(2, rng.random() * 6.3):
        s = r * r1 * 0.6
        lobes.append(((x + dx * r * r1 * 0.3, y + dy * r * r1 * 0.3, top + h * (a1 + b1) * 0.45),
                      (s * 1.2, s * 0.8, h * (b1 - a1) * 0.6)))
    for k, (c, s) in enumerate(lobes):
        # each lobe lies along the ring (tangential), as the pick's plump rolls do
        ang = math.atan2(c[1] - y, c[0] - x) + math.pi / 2 + rng.uniform(-0.3, 0.3)
        ell(c, s, "lilac", rot=Matrix.Rotation(ang, 4, "Z"))
        if k % 3 == 0:  # a puckered pore on the lobe's upper side
            ell((c[0], c[1], c[2] + s[2] * 0.85), (s[0] * 0.18, s[0] * 0.18, s[2] * 0.2), "p2")


def glowcap_cap(x, y, z, h, r, scale=1.0, tilt=0.25):
    """The selected glowcap (glowcap-on-vaulttree.png): a dark violet stalk (detritus),
    a broad tilted purple cap (violet) with flat dark spots (p0), paler lilac spots
    (lilac) and magenta speckles near the crown, and a cyan brim and underside (p2).
    The cap is held to the crown radius."""
    rc = r * 1.5 * scale
    cap = Vector((x, y, z + h * 0.62))
    rot = Matrix.Rotation(tilt, 4, "X")
    tube([(x, y, z), (x + 0.005, y, z + h * 0.3), (x, y + 0.005, cap.z)], r * 0.36 * scale, r * 0.26 * scale,
         "detritus", seg=8)
    d = rc * 0.6
    revolve(cap, [(rc * 0.98, 0.0), (rc * 0.7, d * 0.55), (0.02, d), (0.02, d * 0.85),
                  (rc * 0.6, d * 0.35), (rc * 0.94, -0.004)], "violet", seg=20, rot=rot)
    revolve(cap, [(rc * 0.3, d * 0.05), (rc * 0.99, -0.004), (rc * 1.0, -0.02), (rc * 0.3, d * 0.05 - 0.02)], "p2",
            seg=20, rot=rot)
    for (u, v, s, mat) in ((0.3, 0.6, 0.18, "p0"), (-0.4, 0.4, 0.14, "p0"), (0.1, -0.5, 0.15, "p0"),
                           (-0.5, -0.3, 0.12, "lilac"), (0.55, 0.0, 0.1, "lilac"), (0.35, 0.3, 0.04, "magenta")):
        rr = math.hypot(u, v) * 0.85
        zs = d * (1 - rr) if rr > 0.7 else d * (1 - 0.45 * rr / 0.7)  # on the cap's outline
        p = cap + rot @ Vector((u * rc * 0.85, v * rc * 0.85, zs))
        ell(p, (s * rc, s * rc, 0.008), mat, rot=rot)


def glowcap(x, y, h, r, layers, i, rng, log=None):
    """log = (length_m, height_m) of the dead wood the colony grows on, or None (the log
    and the mycelium in its split are world scenery, not the glowcap)."""
    z = 0.0
    if log is not None:
        length, dia = log
        cyl((x - length / 2, y, dia / 2), (x + length / 2, y, dia / 2), dia / 2, mat="bark", seg=12)
        box((x, y - dia * 0.3, dia * 0.55), (length * 0.8, dia * 0.08, dia * 0.2), "split")
        z = dia
    glowcap_cap(x, y, z, h, r, tilt=rng.uniform(0.15, 0.35))
    if log is not None and log[0] > 0.6:
        glowcap_cap(x + 0.18, y + 0.03, z, h * 0.7, r * 0.7)


def _scaled(p0, p1, f):
    if f == 1.0:
        return tuple(p1)
    return tuple(p0[k] + (p1[k] - p0[k]) * f for k in range(3))


# Foliage states (design/handoffs/foliage-states-implementation-2026-10-08.md; Wrysk's
# round-2 picks, scripts/blender/foliage_variants.py round 2). A builder's `spec` cuts the
# accepted model; None (or {}) is the accepted model itself, with the same random draws in
# the same order, so a stand keeps its phase across its states.
#
# bloomcrown: `basal` per-strap length factors for the 8 basal straps (0 drops one);
#   `vanes` per-vane length factors by rank of the vane's tip height (rank 0 = the lowest
#   tip, the one a browser reaches first); `pod` False drops the pod; `seedling` the length
#   factor of a seedling's rosette straps; `fallen` ground cells (i, j) beside the stem
#   that hold a fallen crown voxel, drawn here as a vane piece lying flat and marked
#   `ob["fallen"]` (the bake adds the cell itself, `bake_voxel_models.py`).
# umbrellafrond: `tiers` {role: (fronds kept: "all" | "alt" | n, rib length factor,
#   pinnae True | False (bare ribs)) or None (no fronds)}, role "lowest", "middle" or
#   "top" by the tier's place (two tiers are lowest and top; a seedling's one tier is
#   lowest); a role not named is full.
BLOOMCROWN_STATES = [
    ("full", {}),
    # round-1 browsed 2's stubbed rosette under a crown whose 4 lowest vanes are cut to 0.4
    ("browsed", {"basal": [0.3] * 8, "vanes": [0.4] * 4 + [1.0] * 6, "seedling": 0.3}),
    # stem, pod and bloom, and two fallen pink voxels beside the stem
    ("stripped", {"basal": [0.0] * 8, "vanes": [0.0] * 10, "seedling": 0.3,
                  "fallen": [(1, 0), (-1, -1)]}),
]
RIBS = ("all", 1.0, False)
UMBRELLAFROND_STATES = [
    ("full", {}),
    ("browsed A", {"tiers": {"lowest": RIBS}}),
    ("browsed B", {"tiers": {"lowest": ("all", 0.4, False), "middle": RIBS}}),
    ("stripped", {"tiers": {"lowest": ("all", 0.4, False), "middle": ("all", 0.45, False),
                            "top": ("all", 0.5, False)}}),
]
# VAULT-SHAPE round 3 (2026-10-10): the vaulttree's shelf count follows its foliage, full to
# thin: 22, 17 and 12 shelves (the pick's twelve plus the gap-filling rule's extras, placed
# after the pick's, so every state holds the thinner states' shelves cell for cell).
VAULTTREE_STATES = [
    ("b22", {"shelf_fill": 22 / 12}),
    ("b17", {"shelf_fill": 17 / 12}),
    ("b12", {"shelf_fill": 1.0}),
]
FOLIAGE_STATES = {"bloomcrown": BLOOMCROWN_STATES, "umbrellafrond": UMBRELLAFROND_STATES,
                  "vaulttree": VAULTTREE_STATES}


def bloomcrown(x, y, h, r, layers, i, rng, spec=None):
    """Artbook pick (bloomcrown-pink-preferred.png): a violet stem; a basal ring of
    periwinkle straps with cyan tips (p0); a crown of ten hot-pink square vanes (p2)
    flaring up and out round a pale blush pod (p1, the core) capped orange (warm, the
    bloom). Each part in its profile band. `spec`: a foliage state's cuts
    (`BLOOMCROWN_STATES`)."""
    spec = spec or {}
    if i == 0:  # seedling: a little ring of basal straps
        f = spec.get("seedling", 1.0)
        for dx, dy in ring_dirs(7, rng.random()):
            p0 = (x + dx * r * 0.12, y + dy * r * 0.12, 0)
            p1 = (x + dx * r, y + dy * r, h)
            if f > 0:
                strap(p0, _scaled(p0, p1, f), 0.03, "p0")
        return
    trunk = next(l for l in layers if l[0] == T)
    basal = next(l for l in layers if l[0] == F and l[1] == 0.0)
    crown = next(l for l in layers if l[0] == F and l[1] > 0.0)
    za, zb = crown[1] * h, crown[2] * h
    cyl((x, y, 0), (x, y, za + 0.25 * (zb - za)), max(trunk[3] * r * 0.55, 0.018),
        max(trunk[3] * r * 0.45, 0.015), mat="violet", seg=8)
    bf = spec.get("basal", [1.0] * 8)
    for k, (dx, dy) in enumerate(ring_dirs(8, rng.random())):  # basal straps, flaring out of the stem foot
        p0 = (x + dx * r * 0.08, y + dy * r * 0.08, 0.0)
        p1 = (x + dx * basal[3] * r, y + dy * basal[3] * r, basal[2] * h)
        if bf[k] > 0:
            strap(p0, _scaled(p0, p1, bf[k]), 0.035, "p0")
    # pod and bloom
    pc = (x, y, za + 0.42 * (zb - za))
    if spec.get("pod", True):
        ell(pc, (0.24 * r, 0.24 * r, 0.24 * (zb - za)), "p1")
    ell((x, y, za + 0.64 * (zb - za)), (0.15 * r, 0.15 * r, 0.06 * (zb - za)), "warm")
    vanes = []
    for k, (dx, dy) in enumerate(ring_dirs(10, rng.random())):  # vanes
        reach = crown[3] * r * (0.85 + 0.15 * ((k * 7) % 3) / 2)
        top = zb - (zb - za) * 0.12 * ((k * 3) % 4) / 3
        vanes.append(((x + dx * r * 0.12, y + dy * r * 0.12, za + 0.05 * (zb - za)),
                      (x + dx * reach, y + dy * reach, top)))
    vf = spec.get("vanes", [1.0] * 10)
    # by tip height, lowest first (stable: ties keep ring order)
    for rank, k in enumerate(sorted(range(10), key=lambda k: (vanes[k][1][2], k))):
        if vf[rank] > 0:
            p0, p1 = vanes[k]
            strap(p0, _scaled(p0, p1, vf[rank]), 0.04, "p2")
    for gi, gj in spec.get("fallen", []):
        # a vane piece lying flat in ground cell (gi, gj) of a 0.125 m grid around the stem
        # column; its voxel is added by the bake, not voxelized
        vox = 0.125
        cx, cy = x + gi * vox, y + gj * vox
        with Collect() as piece:
            strap((cx - 0.05, cy - 0.025, 0.012), (cx + 0.05, cy + 0.025, 0.018), 0.04, "p2")
        for ob in piece:
            ob["fallen"] = True


def frond(x, y, ang, zb, za, rr, top=False, n=14, keep=1.0, pinnae=True):
    """One umbrellafrond frond (artbook pick, 2026-09-24): a pale rib (p2) rising from
    the node and arching down to its tip, lacy teal pinnae (p1) along both sides. Cut to
    the first `keep` of its length from the node, with or without its pinnae (a foliage
    state's bare ribs and stubs); keep 1 with pinnae is the accepted frond."""
    dx, dy = math.cos(ang), math.sin(ang)
    drop = zb - za
    if top:  # the crowning tier rises before it curls over
        ctrl = (x + dx * rr * 0.35, y + dy * rr * 0.35, zb + drop * 0.9)
    else:
        ctrl = (x + dx * rr * 0.55, y + dy * rr * 0.55, zb + drop * 0.35)
    path = bezier((x, y, zb), ctrl, (x + dx * rr, y + dy * rr, za), n)
    m = n if keep >= 1.0 else max(2, int(round(n * keep)))
    if m == n:
        rib = path[::2] + ([path[-1]] if (n - 1) % 2 else [])
    else:
        rib = path[:m:2] + ([path[m - 1]] if (m - 1) % 2 else [])
        if len(rib) < 2:
            rib = [path[0], path[m - 1]]
    tube(rib, 0.016, 0.006 + 0.010 * (1 - min(keep, 1.0)), "p2", seg=6)
    if not pinnae:
        return
    with Collect() as blade:
        widths = [0.07 * math.sin(math.pi * (0.15 + 0.8 * k / (n - 1))) + 0.02 for k in range(n)]
        ribbon(path[:m], widths[:m], "p1")
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
                ribbon([q, mid, end], [0.045, 0.04, 0.012], "p1", th=0.01)
    join(blade, "frond")


def _kept(which, n=8):
    if which == "all":
        return set(range(n))
    if which == "alt":
        return set(range(0, n, 2))
    return {int(round(j * n / which)) % n for j in range(which)}


def _tier_rule(spec, k, count):
    """A tier's (fronds kept, rib length, pinnae) or None under `spec`, by its role."""
    role = "top" if count > 1 and k == count - 1 else ("lowest" if k == 0 else "middle")
    return (spec.get("tiers") or {}).get(role, ("all", 1.0, True))


def umbrellafrond(x, y, h, r, layers, i, rng, spec=None):
    """Artbook pick (umbrellafrond.png): a straight segmented violet stem with collars and
    a flared foot; three regular tiers of eight straight lacy teal fronds with pale ribs,
    each tier a node with a teal bud, widest at the bottom. Tiers sit in the profile's
    foliage bands (node at the band top, frond tips at its bottom). `spec`: a foliage
    state's cuts (`UMBRELLAFROND_STATES`)."""
    spec = spec or {}
    tiers = [l for l in layers if l[0] in (F, MAT)]
    phase = rng.uniform(0, math.pi / 8)
    if i == 0:  # seedling: one low tier of fronds on a stub
        cyl((x, y, 0), (x, y, h * 0.8), 0.012, mat="violet", seg=6)
        w = _tier_rule(spec, 0, 1)
        if w is not None:
            kept = _kept(w[0], 6)
            for k, (dx, dy) in enumerate(ring_dirs(6, phase)):
                if k in kept:
                    frond(x, y, math.atan2(dy, dx), h * 0.8, 0.0, r, n=8, keep=w[1], pinnae=w[2])
        return
    top_node = (tiers[-1][1] + 0.3 * (tiers[-1][2] - tiers[-1][1])) * h
    rs = max(0.03 * h, 0.02)
    # stem: a hexagonal shaft tapering a little, a flared stepped foot
    cyl((x, y, 0), (x, y, top_node), rs, rs * 0.8, mat="violet", seg=6)
    cyl((x, y, 0), (x, y, 0.05 * h), rs * 2.4, rs * 2.0, mat="plum", seg=6)
    cyl((x, y, 0.05 * h), (x, y, 0.1 * h), rs * 1.8, rs * 1.5, mat="plum", seg=6)
    nodes = []
    for k, (kind, a, b, rf, share, p) in enumerate(tiers):
        last = k == len(tiers) - 1
        zb = top_node if last else b * h
        nodes.append(zb)
        # the collar under each node, a node bud, and the tier's fronds
        rk = rs * (1 - 0.2 * zb / h)
        cyl((x, y, zb - 0.05 * h), (x, y, zb - 0.01 * h), rk * 1.45, rk * 1.2, mat="plum", seg=6)
        ell((x, y, zb + 0.012), (rk * 0.9, rk * 0.9, 0.025), "p1")
        w = _tier_rule(spec, k, len(tiers))
        if w is None:
            continue
        kept = _kept(w[0])
        for j, (dx, dy) in enumerate(ring_dirs(8, phase + k * math.pi / 8)):
            if j in kept:
                frond(x, y, math.atan2(dy, dx), zb, a * h, rf * r, top=last, keep=w[1], pinnae=w[2])
    # collars between the tiers (the pick's segment joints)
    prev = 0.1 * h
    for zb in nodes:
        zm = (prev + zb - 0.05 * h) / 2
        if zm - prev > 0.08 * h:
            rk = rs * (1 - 0.2 * zm / h)
            cyl((x, y, zm - 0.015 * h), (x, y, zm + 0.015 * h), rk * 1.3, rk * 1.15, mat="plum", seg=6)
        prev = zb


# Vaulttree shelves from the artbook pick (vaulttree.png, 1024 x 1536): centre x and top
# y of each canopy shelf in pick pixels, and its half-width. Read off the image. The
# pick's tree stands from y 1530 (ground) to y 15 (crown top); a shelf's height is its
# pick height as a fraction of that (2026-10-01: the shelves span 0.55-1.0 of height,
# the drape's bulk ends at ~0.25-0.35), as the profile's bands now are.
VAULT_SHELVES = [
    (620, 15, 190), (790, 60, 170), (230, 90, 140), (160, 150, 105), (650, 205, 55),
    (330, 220, 60), (870, 270, 115), (270, 300, 75), (180, 380, 85), (640, 500, 70),
    (210, 520, 180), (800, 540, 160),
]
VAULT_AXIS_PX, VAULT_HALF_PX = 512.0, 475.0
VAULT_GROUND_PX, VAULT_TOP_PX = 1530.0, 15.0
# Shelf fill factor: 1 is the pick's twelve shelves. Only VAULT-SHAPE's scratch candidate D
# sets it (the bake reads a source.json species' optional `shelf_fill`).
VAULT_FILL = 1.0
# Shared lean (VAULT-SHAPE round 2, scratch bakes only; None in every committed bake):
# (tilt, (dx, dy)): every variant's trunk leans along the unit (dx, dy) by tilt x height at
# the trunk's split, instead of the pick's 0.07 x height in a direction drawn per variant.
VAULT_LEAN = None
VAULT_GOLDEN = math.pi * (3 - math.sqrt(5))  # the golden angle: shelf k's azimuth step


def vault_shelf(c, rs, rng, drape_to=None):
    """One canopy shelf: a domed dark-teal cap (p1) over a drooping mint rim (p2), a fan
    of fine twigs under it, and cyan tendril clusters (teal, the drape) hanging from it."""
    cx, cy, ztop = c
    d = rs * 0.42  # dome rise
    revolve((cx, cy, ztop), [(0.02, 0.0), (rs * 0.55, -d * 0.3), (rs * 0.88, -d * 0.8), (rs * 0.86, -d * 0.8 - 0.03),
                             (rs * 0.5, -d * 0.3 - 0.03), (0.02, -0.035)], "p1", seg=20)
    revolve((cx, cy, ztop), [(rs * 0.86, -d * 0.78), (rs * 1.0, -d * 1.1), (rs * 0.97, -d * 1.1 - 0.03),
                             (rs * 0.84, -d * 0.8 - 0.03)], "p2", seg=20)
    with Collect() as twigs:
        for dx, dy in ring_dirs(4, rng.random() * 6.3):
            cyl((cx, cy, ztop - d * 1.4 - 0.04), (cx + dx * rs * 0.6, cy + dy * rs * 0.6, ztop - d * 0.7), 0.012,
                0.006, mat="violet", seg=5)
    join(twigs, "twigs")
    if drape_to is None:
        return
    with Collect() as drapes:
        for _ in range(rng.randint(1, 2)):
            a, dd = rng.uniform(0, 2 * math.pi), rs * rng.uniform(0.35, 0.9)
            bx, by = cx + math.cos(a) * dd, cy + math.sin(a) * dd
            # the pick's strands end in the lower part of the drape band (bulk ~0.25-0.35)
            z1 = rng.uniform(drape_to[0], drape_to[0] + 0.4 * (drape_to[1] - drape_to[0]))
            for _ in range(rng.randint(2, 4)):
                ox, oy = rng.uniform(-0.02, 0.02), rng.uniform(-0.02, 0.02)
                cyl((bx + ox, by + oy, ztop - d * 0.8), (bx + ox, by + oy, z1 + rng.uniform(0, 0.12)), 0.006,
                    mat="teal", seg=4)
    join(drapes, "drapes")


def vaulttree(x, y, h, r, layers, i, rng, spec=None):
    """Artbook pick (vaulttree.png): an asymmetric leaning trunk of twisted violet plates
    with one dark-teal plate, a leader and arching violet limbs, a canopy of separate
    domed dark-teal shelves with mint rims at many heights (the pick's shelves at the
    pick's heights, 0.55-1.0 of height, the profile's foliage band), long fine cyan
    tendrils (the drape) hanging from the shelves down into the drape band.

    Round crown (VAULT-SHAPE round 3, 2026-10-10): the pick is a front view, so the
    lineup put each shelf at the pick's x offset and drew only a narrow depth (0.45 r)
    for it, a crown about 2.1 x 1.5 m at r 1. Each shelf now keeps the pick's distance
    from the axis, height and size, at its own azimuth round the axis (the golden angle
    from a phase drawn per variant, jittered), so the crown is as deep as it is wide.
    `spec`: a foliage state (`VAULTTREE_STATES`), its `shelf_fill` the shelf count over
    the pick's twelve."""
    fill = (spec or {}).get("shelf_fill", VAULT_FILL)
    if i == 0:  # seedling: one tiny shelf on a stub
        cyl((x, y, 0), (x, y, h * 0.7), 0.012, mat="violet", seg=5)
        vault_shelf((x, y, h), r * 0.7, rng)
        return
    trunk = next(l for l in layers if l[0] == T)
    fol = next(l for l in layers if l[0] == F)
    drape = next((l for l in layers if l[0] == D), None)
    zs = trunk[2] * h  # where the trunk splits into limbs
    rt = trunk[3] * r  # trunk radius: the profile's trunk radius
    lean_a = rng.uniform(-0.6, 0.6)  # drawn even under VAULT_LEAN: the shelves keep their draws
    lean = Vector((math.cos(lean_a), math.sin(lean_a), 0)) * 0.07 * h
    if VAULT_LEAN is not None:
        tilt, (ldx, ldy) = VAULT_LEAN
        lean = Vector((ldx, ldy, 0)) * tilt * h
    axis = lambda z: Vector((x, y, z)) + lean * (z / zs) ** 1.4  # noqa: E731
    # trunk: three twisted plates round a core, flaring at the foot
    nz = 9
    for s, (mat, ph) in enumerate((("plum", 3.4), ("violet", 1.4), ("detritus", -0.7))):
        pts = []
        for k in range(nz):
            z = zs * k / (nz - 1)
            foot = max(0.0, 1 - z / (0.18 * h))
            off = rt * (0.45 + 0.9 * foot)
            a = ph + 1.4 * z / zs
            pts.append(axis(z) + Vector((math.cos(a), math.sin(a), 0)) * off)
        tube(pts, rt * (0.75 if mat != "detritus" else 0.6), rt * 0.45, mat, seg=8)
    tube([axis(zs * k / 4) for k in range(5)], rt * 0.8, rt * 0.55, "violet", seg=8)
    top = axis(zs)
    # canopy: the pick's shelves at the pick's heights (all of them for the adult, the
    # four highest for a juvenile); the foliage band 0.55-1.0 is the pick's shelf span
    za, zb = fol[1] * h, fol[2] * h
    shelves = VAULT_SHELVES if drape is not None else VAULT_SHELVES[:4]
    phase = rng.uniform(0, 2 * math.pi)
    crowning = []

    def place(k, px, py, ph):
        rs = r * ph / VAULT_HALF_PX * (fol[3])
        ztop = max(za + rs * 0.42 * 1.1, h * (VAULT_GROUND_PX - py) / (VAULT_GROUND_PX - VAULT_TOP_PX))
        d = abs(px - VAULT_AXIS_PX) / VAULT_HALF_PX * r
        a = phase + k * VAULT_GOLDEN + rng.uniform(-0.3, 0.3)
        ox, oy = math.cos(a) * d, math.sin(a) * d
        ex = math.hypot(ox, oy) + rs - r
        if ex > 0:  # keep each shelf inside the crown radius
            s = max(0.0, (math.hypot(ox, oy) - ex) / max(math.hypot(ox, oy), 1e-6))
            ox, oy = ox * s, oy * s
        c = Vector((x + ox, y + oy, ztop))
        crowning.append((ox, oy))
        # limb: from the leader, a little below the shelf, arching out and up to its base
        base = c - Vector((0, 0, rs * 0.4 + 0.06))
        zo = max(zs, min(base.z - 0.15 * h, zs + 0.6 * (base.z - zs)))
        o = top + Vector((0, 0, zo - zs))
        mid = o.lerp(base, 0.55) + Vector((0, 0, 0.05 * h))
        tube(bezier(o, mid, base, 5), 0.06 if k < 2 or ph > 150 else 0.045, 0.02, "violet", seg=6)
        vault_shelf(c, rs, rng, None if drape is None else (drape[1] * h, drape[2] * h))
    for k, (px, py, ph) in enumerate(shelves):
        place(k, px, py, ph)
    if drape is not None and fill > 1.0:
        # VAULT-SHAPE candidate D (2026-10-10; scratch bakes only, VAULT_FILL is 1 in every
        # committed bake): more shelves inside the same crown radius. Each gap between two
        # of the pick's shelves (by height) gets one more, mirrored across the axis, at the
        # gap's middle height and the two shelves' mean size; the largest gaps first,
        # round(12 x (VAULT_FILL - 1)) of them. Placed after the pick's shelves, so those
        # keep their random draws.
        order = sorted(VAULT_SHELVES, key=lambda sh: sh[1])
        gaps = sorted(zip(order, order[1:]), key=lambda ab: ab[0][1] - ab[1][1])
        n = round(len(VAULT_SHELVES) * (fill - 1.0))
        for k, (a, b) in enumerate(gaps[:n]):
            place(len(VAULT_SHELVES) + k, 2 * VAULT_AXIS_PX - a[0], (a[1] + b[1]) / 2, (a[2] + b[2]) / 2)
    # the leader to the crowning shelf
    cx, cy = crowning[0]
    tube([top, top + Vector((0, 0, (zb - zs) * 0.5)), Vector((x + cx * 0.5, y + cy * 0.5, zb - 0.1))],
         rt * 0.55, 0.03, "violet", seg=6)


def lanternberry(x, y, h, r, layers, i, rng):
    """Artbook pick (lanternberry-blue.png): five violet stems fanning from one base,
    paired blue leaflets (p1) up each stem from the trunk band and a leaf tuft at each
    tip, a basal leaf tuft, and one deep-plum lantern (warm accent: plum closed, orange
    inside when ripe) hanging from a hook under each stem tip; magenta seams on the
    lanterns (not voxelized: smaller than a voxel)."""
    if i == 0:  # seedling: a little tuft of blue leaflets
        rosette(x, y, 0, h, r, 7, "p1", phase=rng.random())
        return
    trunk = next(l for l in layers if l[0] == T)
    zl = trunk[2] * h  # leaflets start above the bare-stem band
    rot = rng.uniform(-0.3, 0.3)
    fan = [(-0.85, 0.62), (-0.42, 0.88), (0.02, 1.0), (0.48, 0.84), (0.86, 0.6)]
    for dx, dy in ring_dirs(7, rng.random()):  # basal tuft
        ribbon(bezier((x, y, 0), (x + dx * r * 0.2, y + dy * r * 0.2, 0.06), (x + dx * r * 0.32, y + dy * r * 0.32, 0.1), 4),
               [0.015, 0.03, 0.035, 0.01], "p1", th=0.008)
    for k, (fx, fz) in enumerate(fan):
        a = rot + fx * 0.35 + rng.uniform(-0.15, 0.15)
        side = Vector((math.cos(a), math.sin(a), 0)) * fx
        tip = Vector((x, y, 0)) + side * r * 0.92 + Vector((0, 0, h * fz * (0.94 + 0.06 * rng.random())))
        base = Vector((x, y, 0)) + side * 0.02
        path = bezier(base, base.lerp(tip, 0.5) + Vector((0, 0, h * 0.05)) - side * r * 0.15, tip, 8)
        tube(path, 0.02, 0.011, "violet", seg=6)
        with Collect() as leaves:
            for j in range(2, 8):
                p = path[j]
                if p.z < zl:
                    continue
                t = (path[min(j + 1, 7)] - path[j - 1]).normalized()
                n = t.cross(Vector((0, 0, 1))).normalized()
                for sg in (-1, 1):
                    e = p + (n * sg * 0.9 + t * 0.6).normalized() * 0.11
                    ribbon([p, p.lerp(e, 0.5) + Vector((0, 0, 0.015)), e], [0.014, 0.045, 0.008], "p1", th=0.008)
            for dd in ((-0.6, 0.0), (0.0, 0.0), (0.6, 0.0)):  # the tip tuft
                e = tip + Vector((dd[0] * 0.08, 0, 0.13))
                ribbon([tip, tip.lerp(e, 0.5), e], [0.014, 0.05, 0.008], "p1", th=0.008)
        join(leaves, "leaves")
        # the hook and its lantern, under the tip on the outer side
        hook = path[6]
        out = Vector((side.x, side.y, 0)).normalized() if side.length > 0.01 else Vector((1, 0, 0))
        lp = hook + out * 0.06 - Vector((0, 0, 0.05))
        tube([hook, hook + out * 0.05 + Vector((0, 0, 0.015)), lp + Vector((0, 0, 0.03))], 0.006, 0.005, "violet", seg=5)
        ell(lp, (0.035, 0.035, 0.042), "warm")
        for s in range(4):  # seams
            ang = s * math.pi / 2
            sv = Vector((math.cos(ang), math.sin(ang), 0)) * 0.036
            cyl(lp + sv + Vector((0, 0, 0.03)), lp + sv * 1.15 - Vector((0, 0, 0.035)), 0.003, mat="magenta", seg=4)


def siphonreed(x, y, h, r, layers, i, rng):
    """Artbook pick (siphonreed.png): four slim teal stems (p1) from one rooted base,
    each swelling into a pale celadon upper shaft (lilac) under a rounded pink fan head
    (p2); short blue roots (p1) splayed at the foot. Heights stagger as in the pick."""
    n = 4 if h >= 0.9 else 3
    heights = [1.0, 0.9, 0.82, 0.68][:n]
    offs = [(-0.45, 0.2), (0.05, -0.15), (0.5, 0.25), (0.85, -0.2)][:n]
    ph = rng.uniform(0, 2 * math.pi)
    rot = Matrix.Rotation(ph, 3, "Z")
    for k in range(n):
        o = rot @ Vector((offs[k][0] * r, offs[k][1] * r, 0))
        hh = h * heights[k] * 0.93
        base = Vector((x, y, 0)) + o * 0.3
        top = Vector((x, y, hh)) + o
        sw0 = base.lerp(top, 0.62)
        cyl(base, sw0, 0.011, 0.009, mat="p1", seg=6)
        # the swollen upper shaft
        cyl(sw0, sw0.lerp(top, 0.25), 0.009, 0.03, mat="lilac", seg=8)
        cyl(sw0.lerp(top, 0.25), sw0.lerp(top, 0.8), 0.03, 0.024, mat="lilac", seg=8)
        cyl(sw0.lerp(top, 0.8), top, 0.024, 0.008, mat="lilac", seg=8)
        # the fan head: three rounded lobes opening upward
        hr = 0.075 * (0.7 + 0.3 * heights[k])
        for la in (-0.7, 0.0, 0.7):
            dv = rot @ Vector((math.sin(la), 0.0, 0.0))
            c = top + Vector((dv.x * hr * 0.6, dv.y * hr * 0.6, hr * 0.45 - abs(la) * hr * 0.25))
            ell(c, (hr * 0.55, hr * 0.45, hr * 0.32), "p2")
    for dx, dy in ring_dirs(6, rng.random()):  # roots
        tube([(x, y, 0.02), (x + dx * r * 0.5, y + dy * r * 0.5, 0.01), (x + dx * r * 0.9, y + dy * r * 0.9, 0.0)],
             0.008, 0.004, "p1", seg=5)
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
