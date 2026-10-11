"""Bake the voxel organism models the presenter stamps (package V).

    cargo run -p cubarium --example voxel_model_source
    blender -b -P scripts/blender/bake_voxel_models.py [-- [--only NAME[,NAME]] [--out DIR]]

RIG-STATES round 2: add `--sleep-only` to port the committed model and rig without
voxelizing or writing any `.cvm` or manifest. Existing feed keys are copied exactly.
RIG-STATES round 3: the Veilhound, lanternjaw and perched seedporter (`seedporter-stand`)
end their asleep track on a sleep pose (`SLEEP_POSES`, `animal_models.posed_sleep_rig`).

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
facing +x, one pose, centred on the anchor cell, from `animal_models.py`'s builders (model
pass MP-A); the unsimulated specimens (`SPECIMENS`) as one adult model each, under the
manifest's `specimens` key.

The loftstrider uses Wrysk's calf-l38 rig blended linearly to the adult across the
five bins, at the original founder length keys. Its six landmark directions are
checked after voxelization at every grid size; coarse-grid aliases claim the nearest
cell in the authored direction, like the existing sub-voxel accent fallback.

Foliage states (`design/handoffs/foliage-states-implementation-2026-10-08.md`): a species
with states in `organism_lineup.FOLIAGE_STATES` (bloomcrown 3, umbrellafrond 4, vaulttree 5) is
baked in every state at every step and variant (a state's `shelf_fill` or `shelves` may be a
list, one per variant), the builder's cuts applied to the smaller steps too
(an umbrellafrond's two tiers are its lowest and top; a seedling's one tier is its
lowest). The file's state table carries each state's name and the total foliage fraction
it starts below (`source.json`'s `foliage_state_below`, the species config's). A stripped
bloomcrown's fallen crown voxels are added by hand at their ground cells, tagged fallen
foliage of the crown's layer, so the presenter paints them crown pink.

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
import animal_models as A  # noqa: E402  (import-safe: main() is guarded)

REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


ROOT = arg("--out", os.path.join(REPO, "assets", "voxel-models"))
SOURCE = arg("--source", os.path.join(ROOT, "source.json"))
ONLY = set(filter(None, (arg("--only") or "").split(",")))
# Scratch previews only (VAULT-SHAPE 2026-10-10): bake a plant's full-grown size step and
# nothing smaller. Never for a library the presenter grows stands through.
ADULT_ONLY = "--adult-only" in ARGS

# Placeholders (design/backlog.md §1).
VARIANTS = 3
BINS = 5
SAMPLES = 4000  # how finely the growth curve is walked to find the size steps

# The model palette, by the builders' material names. Anything else a builder draws
# (the stonecushion's rock lip, the siphonreed's pool, the glowcap's log) is scenery
# the world already draws, and is not voxelized.
PALETTE = ["plum", "violet", "detritus", "lilac", "p0", "p1", "p2", "teal", "magenta",
           "warm", "starved",
           # the animals' part slots (model pass MP-A; their colours are per animal)
           "a_main", "a_dark", "a_pale", "a_mark", "a_limb", "a_foot", "a_head", "a_eye",
           "a_glow", "a_membrane", "a_fruit", "a_tendril",
           # the lanternjaw's lure orbs (an accent: `animal_models.ACCENTS`)
           "a_lure"]

# What each builder material is on a plant: trunk, foliage, drape, accent, or None
# (scenery). Anything unlisted is foliage.
PLANT_ROLE = {"plum": "trunk", "violet": "trunk", "detritus": "trunk", "warm": "accent",
              "magenta": "accent", "bark": None, "split": None, "rock": None,
              "water": None, "film": None, "grey": None}
PLANT_ROLE_BY_SPECIES = {
    # D1: the cap is the glowcap's tissue, its stalk the mycelium's stem. The model pass
    # (2026-10-01) paints the cap's magenta speckles as cap tissue, not an accent cell.
    "glowcap": {"violet": "foliage", "p2": "foliage", "magenta": "foliage"},
    # D11: the teal filaments are the drape layer.
    "vaulttree": {"teal": "drape"},
    # The lanterns' magenta seams are smaller than a voxel: drawn, not voxelized.
    "lanternberry": {"magenta": None},
    # The violet showing between the leaves is the clump's own (mat) tissue.
    "springturf": {"violet": "foliage"},
    # The pad and its raised lip are the mat itself.
    "velvetpad": {"violet": "foliage", "plum": "foliage"},
}
# On an animal (`animal_models.py`): its sense patches and eyes are accents; everything
# else is body.
ANIMAL_ROLE = {slot: "accent" for slot in A.ACCENTS}

# The unsimulated animals, baked as specimens: one adult model each, manifest key
# `specimens`, at the builders' reference sizes (`animal_models.REF`). Placeholder sizes,
# length x width x height in metres:
#   chorister     1.4 x 0.35 x 0.6, + 0.8 m tail     reimagining §Chorister (11 x 3 x 5 v + 6.5)
#   ripple-snail  0.19 x 0.19 x 0.125                reimagining §Ripple-snail, roster 1.5 x 1.5 v
#   capgnawer     0.25 x 0.21 x 0.19                 reimagining §Capgnawer 0.25 x 0.19; width off the pick
#   seedporter    upright as pictured: 0.5 head to pelvis, 0.62 across the arms, 0.92 tall
#                 with the coiled tail               reimagining §Seedporter 0.5 x 0.25 body; pick proportions
#                 (the climbing pose, Wrysk 2026-10-01)
#   seedporter-stand / -ground / -glide   the same body posed (Wrysk 2026-10-01): perched,
#                 0.31 x 0.37, 1.04 tall, its tail hanging 0.26 below its feet (`A.BELOW_GROUND`);
#                 on all fours 1.14 x 0.31 x 0.67; gliding 1.24 long, 0.76 across the arms, 0.35 deep
#   bellwing      0.375 wingspan, 0.4 tall with the tendril (0.31 without)   roster 2 x 3 v, D14 wingspan 3 v
#   loftstrider   2.2 x 2.4 across the legs and fronds, 3.4 tall: underside 1.2, eyes 1.5,
#                 mouth 2.8 m             D15 heights; the pick's radial plan (Wrysk 2026-10-01)
SPECIMENS = ["chorister", "ripple-snail", "capgnawer", "seedporter", "bellwing", "loftstrider",
             "seedporter-stand", "seedporter-ground", "seedporter-glide"]

# A founder's other poses on its one rig (`animal_models.FG_POSES`), baked in the founder's
# own size bins from its source body, under the manifest's `specimens` key, so a young body
# poses at its own size: pose name -> founder. The frondgrazer's hunker (Wrysk 2026-10-06).
FOUNDER_POSES = {"frondgrazer-hunker": "frondgrazer"}

# A specimen that is a live founder with young drawn smaller (roster R3): extra bins at
# these fractions of the adult's size, below the adult bin, so the presenter's nearest-
# length pick draws a young body small. The chorister's newborn is
# (body_min / body_max)^(1/3) = 0.46 of the adult's length (the lanternjaw's hatchling
# ratio, which its table keeps by volume).
SPECIMEN_YOUNG = {"chorister": [0.46]}

# Founders `source.json` lists (since it was regenerated, 71afcbb0) that are still baked as
# their specimen, one adult bin at the builder's reference size, as the presenter draws them
# today: the founder path would scale the bellwing to its sim body (0.125 x 0.125 x
# 0.0625 m), squashing the picked model (0.375 m wingspan) to 3 cells (FEED-ROLLOUT,
# 2026-10-10). The other four newly listed founders are committed as specimens too, so a
# full bake keeps them as they are until each gets its own founder bins on purpose.
AS_SPECIMEN = {"bellwing", "capgnawer", "ripple-snail", "seedporter", "chorister"}

# A founder's other life stage, baked as a specimen of one bin at its built (true) size:
# name -> `animal_models.lanternjaw_larva` kind. The lanternjaw's rider larva, the
# leechfork (Wrysk 2026-10-09, D16), drawn while it rides a host
# (`crates/cubarium/src/voxel/figure.rs`, the rider branch of `specs`). Its belly row is
# k = 0, its one-voxel body on the anchor's row; `floor=None` as on the candidates' sheet.
LARVAE = {"lanternjaw-larva": "leechfork"}

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


def build_plant(name, h, r, stage_i, stage, rng, vox, spec=None):
    """The plant's parts, built at the cell centre; `spec` a foliage state's cuts (only for
    a species in `L.FOLIAGE_STATES`). A part marked `fallen` is left out: the bake adds
    its cell by hand (`fallen_cells`)."""
    reset_scene()
    x = y = 0.5 * vox
    fn = getattr(L, name)
    if spec:
        fn(x, y, h, r, lineup_layers(stage), stage_i, rng, spec=spec)
    else:
        fn(x, y, h, r, lineup_layers(stage), stage_i, rng)
    objs = [ob for ob in L.ORG if "fallen" not in ob]
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
    return A.build(name, x, y, adult[0] * s, adult[1] * s, adult[2] * s)


def animal_role(slot):
    """An animal part's role by its palette slot: accent, body ("trunk"), or None for a
    material the palette lacks (not voxelized)."""
    return ANIMAL_ROLE.get(slot, "trunk") if slot in PALETTE else None


class BakeBVH:
    """A BVH independent of BMesh's (notably UV-sphere) face enumeration.

    Sort polygons by their oriented loop (starting at its smallest vertex index)
    and material index. Pass the original loops to Blender: coordinates, winding,
    loop starts, normals and tessellation of nonplanar quads remain authored.
    Only BVH insertion order changes. Returned indices address `mesh.polygons`.

    Equal float32 nearest distances choose the smallest canonical face index,
    then the lexicographically smallest hit position (two triangles of one face).
    Across parts the existing authored build order breaks equal scores; accents
    still overwrite in build order. Boundary rules remain: dot == 0 is outside,
    distance == half a voxel is included, and floor assigns an accent on a grid
    plane to the positive-side cell. No geometric epsilon or coordinate snapping.
    """

    def __init__(self, vertices, mesh):
        faces = []
        for p in mesh.polygons:
            loop = tuple(p.vertices)
            start = loop.index(min(loop))
            faces.append((loop[start:] + loop[:start], p.material_index, loop, p.index))
        faces.sort()
        self.original = [f[3] for f in faces]
        self.tree = BVHTree.FromPolygons(vertices, [f[2] for f in faces])

    def find_nearest(self, point):
        hit = self.tree.find_nearest(point)
        if hit[0] is None:
            return hit
        # The API takes a float32 radius and squares it. Widen only the search
        # radius enough to include the rounded nearest distance; selection uses
        # exact returned distances, so this is not an occupancy tolerance.
        radius = max(hit[3] * (1 + 2 ** -22), 2 ** -63)
        hits = self.tree.find_nearest_range(point, radius)
        loc, nor, index, dist = min(
            [hit, *hits], key=lambda h: (h[3], h[2], tuple(h[0])))
        return loc, nor, self.original[index], dist


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
        bvh = BakeBVH(vs, me)
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


def fallen_cells(cells, spec, h, stage, vox):
    """`cells` (as `cells_of` writes them) with the state's fallen crown voxels added at
    their ground cells (dx, dz) = `spec["fallen"]`: the crown's own material (p2, the
    vanes') and the **fallen** tag of the crown's foliage layer (0x40 | i), which the
    presenter paints at the top of the layer's ramp. A cell already held is left as it is.
    The caller passes no spec for a seedling (stage 0): it has no crown to drop."""
    if not spec or not spec.get("fallen"):
        return cells
    crown = plant_tag("foliage", h, h, stage) & 0x0F
    taken = {(c[0], c[1], c[2]) for c in cells}
    out = list(cells)
    for i, j in spec["fallen"]:
        if (i, 0, j) in taken:
            print(f"  fallen cell {(i, j)} is held, left out", flush=True)
            continue
        out.append((i, 0, j, PALETTE.index("p2"), 0x40 | crown))
    return out


# --- the file -----------------------------------------------------------------------

def write_cvm(path, kind, steps, states=None):
    """steps: [(height_m, radius_m, variants)], cells [(dx, dy, dz, material, tag)]. With
    `states` [(name, start)], state 0 first, each variant is one cell list per state;
    without, each variant is one cell list and the file has the one state "full"."""
    if states is None:
        states = [("full", 1.0)]
        steps = [(h, r, [[cells] for cells in variants]) for h, r, variants in steps]
    used = sorted({c[3] for _, _, variants in steps for per in variants for cells in per for c in cells})
    local = {g: n for n, g in enumerate(used)}
    b = bytearray(b"CVM2")
    b += struct.pack("<BB", kind, len(used))
    for g in used:
        name = PALETTE[g].encode()
        b += struct.pack("<B", len(name)) + name
    assert 1 <= len(states) <= 255
    b += struct.pack("<B", len(states))
    for name, start in states:
        enc = name.encode()
        b += struct.pack("<B", len(enc)) + enc + struct.pack("<d", start)
    b += struct.pack("<H", len(steps))
    for h, r, variants in steps:
        b += struct.pack("<ddB", h, r, len(variants))
        for per in variants:
            assert len(per) == len(states), "one cell list per state"
            for cells in per:
                b += struct.pack("<I", len(cells))
                for dx, dy, dz, m, tag in cells:
                    b += struct.pack("<bbbBB", dx, dy, dz, local[m], tag)
    with open(path, "wb") as f:
        f.write(b)
    return len(b)


def state_table(sp):
    """[(name, start, spec)]: the species' foliage states, the full model first, each
    starting below its threshold in `foliage_state_below`; one full state without."""
    states = L.FOLIAGE_STATES.get(sp["name"])
    below = list(sp.get("foliage_state_below") or [])
    if not states:
        return [("full", 1.0, None)]
    if len(below) != len(states) - 1:
        raise ValueError(f"{sp['name']}: {len(states)} baked states but foliage_state_below "
                         f"{below} names {len(below)} thresholds")
    return [(name, 1.0 if k == 0 else below[k - 1], spec) for k, (name, spec) in enumerate(states)]


def bake_plant(sp, vox, out_dir):
    name = sp["name"]
    roles = dict(PLANT_ROLE)
    roles.update(PLANT_ROLE_BY_SPECIES.get(name, {}))

    def role_of(mat):
        return roles.get(mat, "foliage")

    table = state_table(sp)
    L.VAULT_FILL = float(sp.get("shelf_fill", 1.0))  # VAULT-SHAPE scratch candidates only
    # VAULT-SHAPE round 2 scratch: an optional `lean` {"tilt", "dir_deg", "turns"} gives every
    # variant the same lean (organism_lineup.VAULT_LEAN); with "turns", each variant is
    # baked four times, copy v x 4 + j leaning the shared way turned by j quarter turns as
    # the presenter's `model::turn` turns (dx, dz) = (x, y), so a stand drawn turned by q
    # from copy (4 - q) mod 4 leans the shared way in the world. Absent: unchanged.
    lean = sp.get("lean")
    copies = 4 if lean and lean.get("turns") else 1

    def lean_of(j):
        if not lean:
            return None
        a = math.radians(float(lean.get("dir_deg", 0.0)))
        dx, dz = math.cos(a), math.sin(a)
        for _ in range(j):
            dx, dz = dz, -dx
        return (float(lean["tilt"]), (dx, dz))
    steps, meta = [], []
    for (kh, kr), t in size_steps(sp, vox)[-1 if ADULT_ONLY else 0:]:
        prof = sp["profile"]
        si = stage_index(prof, t)
        stage = prof[si]
        h = dimension(prof, sp["crown_height_m"], t)
        r = dimension(prof, sp["crown_radius_m"], t)
        variants = []
        for v in range(VARIANTS * copies):
            per_state = []
            L.VAULT_LEAN = lean_of(v % copies)
            for _, _, spec in table:
                for key in ("shelf_fill", "shelves"):  # one per variant (vaulttree b0-2)
                    if spec and isinstance(spec.get(key), list):
                        per = spec[key]
                        spec = dict(spec, **{key: per[(v // copies) % len(per)]})
                objs = build_plant(name, h, r, si, stage, random.Random(f"{name}:{v // copies}"), vox, spec)
                cells = voxelize(objs, vox, role_of)

                def tag_of(mat, k, h=h, stage=stage):
                    return plant_tag(role_of(mat), (k + 0.5) * vox, h, stage)

                per_state.append(fallen_cells(cells_of(cells, tag_of), spec if si else None, h, stage, vox))
            variants.append(per_state)
        steps.append((kh * vox, kr * vox / 2, variants))
        meta.append({"height_m": kh * vox, "radius_m": kr * vox / 2, "stage": si,
                     "built_at": {"t": round(t, 4), "height_m": round(h, 4), "radius_m": round(r, 4)},
                     "cells": [len(per[0]) for per in variants]}
                    | ({"state_cells": [[len(c) for c in per] for per in variants]}
                       if len(table) > 1 else {}))
    file = f"{name}.cvm"
    size = write_cvm(os.path.join(out_dir, file), 0, steps, [(n, s) for n, s, _ in table])
    return file, size, meta, [{"name": n, "start": s} for n, s, _ in table]


def voxelize_animal(objs, vox, floor=0, bones=None):
    """{(i, j, k): slot}: `voxelize`'s rule for an animal, whose lofts carry several
    materials on one closed mesh (a flank, a back stripe, a belly): a cell is in or out by
    the whole mesh, and takes the material of the face nearest it. A single-material accent
    part claims cells as `voxelize`'s accents do. Cells below `floor` (the ground, k = 0)
    are dropped; `floor=None` keeps a pose's parts that hang below its perch. With a dict
    `bones`, each cell's rig bone (its part's `ob["bone"]`, the root's when it has none, or
    by the cell's position for a part split across a joint: `animal_models.part_bone`) is
    recorded there the way its material is: from the part that claimed it."""
    parts, accents, nested = [], [], []
    for ob in objs:
        me = ob.data
        vs = [v.co.copy() for v in me.vertices]
        if not vs:
            continue
        slots = [A.slot_of(m.name) for m in me.materials]
        if not any(animal_role(s) for s in slots):
            continue
        bvh = BakeBVH(vs, me)
        lo = Vector(min(v[k] for v in vs) for k in range(3))
        hi = Vector(max(v[k] for v in vs) for k in range(3))
        face_slot = [slots[p.material_index] for p in me.polygons]
        bone = ob
        if ob.get("nested"):
            nested.append((bvh, lo, hi, slots[0], bone))
        elif len(slots) == 1 and animal_role(slots[0]) == "accent":
            accents.append((bvh, lo, hi, slots[0], bone))
        else:
            parts.append((bvh, lo, hi, face_slot, bone))
    half = vox / 2
    cells = {}
    if parts:
        lo = Vector(min(p[1][k] for p in parts) for k in range(3))
        hi = Vector(max(p[2][k] for p in parts) for k in range(3))
        rng = [range(math.floor(lo[k] / vox) - 1, math.ceil(hi[k] / vox) + 1) for k in range(3)]
        for i in rng[0]:
            for j in rng[1]:
                for k in rng[2]:
                    if floor is not None and k < floor:
                        continue
                    c = Vector(((i + 0.5) * vox, (j + 0.5) * vox, (k + 0.5) * vox))
                    best, best_d, best_bone = None, 1e9, None
                    for bvh, plo, phi, face_slot, bone in parts:
                        if any(c[a] < plo[a] - half or c[a] > phi[a] + half for a in range(3)):
                            continue
                        loc, nor, index, dist = bvh.find_nearest(c)
                        if loc is None:
                            continue
                        slot = face_slot[index]
                        if animal_role(slot) is None:
                            continue
                        # inside: the part the centre is deepest in (a thin tendril through
                        # a bell does not take the bell's cell), in the material of its face
                        # nearest the centre; outside, within half a voxel: nearest first
                        d = -1.0 - dist if (c - loc).dot(nor) < 0 else dist
                        if d <= half and d < best_d:
                            best, best_d, best_bone = slot, d, bone
                    if best is not None:
                        cells[(i, j, k)] = best
                        if bones is not None:
                            bones[(i, j, k)] = A.part_bone(best_bone, c)
    for bvh, plo, phi, slot, bone in accents:
        inside = []
        for i in range(math.floor(plo[0] / vox), math.ceil(phi[0] / vox)):
            for j in range(math.floor(plo[1] / vox), math.ceil(phi[1] / vox)):
                k0 = math.floor(plo[2] / vox)
                for k in range(k0 if floor is None else max(floor, k0), math.ceil(phi[2] / vox)):
                    c = Vector(((i + 0.5) * vox, (j + 0.5) * vox, (k + 0.5) * vox))
                    loc, nor, _, _ = bvh.find_nearest(c)
                    if loc is not None and (c - loc).dot(nor) < 0:
                        inside.append((i, j, k))
        if not inside:
            mid = (plo + phi) / 2
            key = (math.floor(mid[0] / vox), math.floor(mid[1] / vox), math.floor(mid[2] / vox))
            if floor is None or key[2] >= floor:
                inside.append(key)
        for key in inside:
            cells[key] = slot
            if bones is not None:
                c = Vector(((key[0] + 0.5) * vox, (key[1] + 0.5) * vox, (key[2] + 0.5) * vox))
                bones[key] = A.part_bone(bone, c)
    nested_cells(nested, vox, cells, bones)
    if bones is not None:
        own_cells(objs, vox, cells, bones)
    return cells


def nested_cells(nested, vox, cells, bones):
    """The nested parts (`animal_models.nested`: a retractable neck's segment inside the
    body at rest): each held cell hidden inside the body (all six faces against held cells)
    whose centre is inside a nested part takes the part's material and, with a dict `bones`,
    its bone. Nothing is added or removed and no outer cell changes, so the resting body
    draws as it did; the clip slides those cells out with their bone."""
    if not nested:
        return
    held = set(cells)
    half = vox / 2
    hidden = [key for key in cells
              if all((key[0] + di, key[1] + dj, key[2] + dk) in held
                     for di, dj, dk in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)))]
    for key in hidden:
        c = Vector(((key[0] + 0.5) * vox, (key[1] + 0.5) * vox, (key[2] + 0.5) * vox))
        for bvh, plo, phi, slot, ob in nested:
            if any(c[a] < plo[a] - half or c[a] > phi[a] + half for a in range(3)):
                continue
            loc, nor, _, _ = bvh.find_nearest(c)
            if loc is not None and (c - loc).dot(nor) < 0:
                cells[key] = slot
                if bones is not None:
                    bones[key] = A.part_bone(ob, c)
                break


def own_cells(objs, vox, cells, bones):
    """A rig's own-cell parts (`animal_models.own_cell`): each group left without a cell
    of its material on its bone claims one, as an eye claims its accent cell. The cell its
    centre is in, or, while that holds another bone's part, the next one ahead (+x, the
    way the model faces), so the claimed cell sits in front of the body. The groups go
    from the most central (across, y) out, and one whose cell another group has taken
    steps sideways toward its own side, so a pair beside a middle part stays a pair."""
    groups = {}
    for ob in objs:
        if "own_cell" in ob:
            groups.setdefault(ob["own_cell"], []).append(ob)
    if not groups:
        return
    centre = {}
    for g, obs in groups.items():
        vs = [v.co for ob in obs for v in ob.data.vertices]
        lo = Vector(min(v[k] for v in vs) for k in range(3))
        hi = Vector(max(v[k] for v in vs) for k in range(3))
        centre[g] = (lo + hi) / 2
    mid = sum((p.y for p in centre.values())) / len(centre)
    claimed, todo = {}, []
    for g in sorted(groups, key=lambda g: abs(centre[g].y - mid)):
        ob, p = groups[g][0], centre[g]
        slot, bone = A.slot_of(ob.data.materials[0].name), A.part_bone(ob, p)
        have = [key for key in cells if cells[key] == slot and bones.get(key) == bone]
        if have:
            claimed.update((key, g) for key in have)
        else:
            todo.append((g, p, slot, bone))
    for g, p, slot, bone in todo:
        i, j, k = (math.floor(p[a] / vox) for a in range(3))
        while (i, j, k) in cells and bones.get((i, j, k)) != bone and (i, j, k) not in claimed:
            i += 1
        side = 1 if p.y >= mid else -1
        while (i, j, k) in claimed:
            j += side
        claimed[(i, j, k)] = g
        cells[(i, j, k)], bones[(i, j, k)] = slot, bone


def animal_cells(objs, vox, floor=0):
    """An animal's cells, as `cells_of` writes them: tag accent or body."""
    cells = voxelize_animal(objs, vox, floor)
    return cells_of(cells, lambda mat, k: 0x30 if animal_role(mat) == "accent" else 0x00)


LOFTSTRIDER_PARTS = {"eyes": ({"a_eye"}, 0.0), "tusks": ({"a_head"}, 0.0),
                    "fronds": ({"a_membrane", "a_glow"}, 30.0), "feet": ({"a_foot"}, 30.0)}


def loftstrider_direction(i, j, vox, az):
    if math.hypot(i * vox, j * vox) < 0.09:
        return None
    angle = math.degrees(math.atan2(j, i)) - az
    a = angle % 60.0
    if a <= 15.0 + 1e-9 or a >= 45.0 - 1e-9:
        return round(angle / 60.0) % 6
    return None


def loftstrider_sectors(cells, vox):
    """Count surviving part directions, as in the calf sheet, on the anchor-centred grid."""
    counts = {}
    for part, (slots, az) in LOFTSTRIDER_PARTS.items():
        seen = {loftstrider_direction(i, j, vox, az)
                for (i, j, _), slot in cells.items() if slot in slots}
        seen.discard(None)
        counts[part] = len(seen)
    return counts


def loftstrider_cells(objs, vox, bones=None):
    """Preserve six authored directions when small landmarks alias on a coarse grid.

    Like the accent fallback, a missing landmark claims one cell. Choose the closest
    cell to its mesh in its authored direction, without displacing another landmark.
    Mesh proportions stay untouched; a fine grid that already retains them is unchanged.
    """
    cells = voxelize_animal(objs, vox, 0, bones)
    landmark_slots = set().union(*(s for s, _ in LOFTSTRIDER_PARTS.values()))
    for part, (slots, az) in LOFTSTRIDER_PARTS.items():
        seen = {loftstrider_direction(i, j, vox, az)
                for (i, j, _), slot in cells.items() if slot in slots}
        for direction in sorted(set(range(6)) - seen):
            candidates = []
            for ob in objs:
                me = ob.data
                face_slots = [A.slot_of(m.name) for m in me.materials]
                if not set(face_slots) & slots:
                    continue
                vs = [ob.matrix_world @ v.co for v in me.vertices]
                mid = sum(vs, Vector()) / len(vs) - Vector((vox / 2, vox / 2, 0))
                angle = math.degrees(math.atan2(mid.y, mid.x)) - az
                if round(angle / 60) % 6 != direction:
                    continue
                bvh = BakeBVH(vs, me)
                lo = [math.floor(min(v[k] for v in vs) / vox) - 1 for k in range(3)]
                hi = [math.ceil(max(v[k] for v in vs) / vox) + 1 for k in range(3)]
                for i in range(lo[0], hi[0]):
                    for j in range(lo[1], hi[1]):
                        if loftstrider_direction(i, j, vox, az) != direction:
                            continue
                        for k in range(max(0, lo[2]), hi[2]):
                            key = (i, j, k)
                            if cells.get(key) in landmark_slots:
                                continue
                            c = Vector(((i + .5) * vox, (j + .5) * vox, (k + .5) * vox))
                            _, _, face, dist = bvh.find_nearest(c)
                            if face is not None and face_slots[me.polygons[face].material_index] in slots:
                                candidates.append((dist, key, face_slots[me.polygons[face].material_index],
                                                   A.part_bone(ob, c)))
            if not candidates:
                raise ValueError(f"no voxel for loftstrider {part} direction {direction}")
            _, key, slot, bone = min(candidates, key=lambda x: x[:3])
            cells[key] = slot
            if bones is not None:
                bones[key] = bone
    return cells


def ls_parts(n):
    """The loftstrider's part sizes in bin `n`: Wrysk's calf-l38 blended to the adult."""
    return A.ls_blend(A.LS_CALVES_R2["calf-l38"], n / (BINS - 1))


def build_founder_bin(f, n, vox):
    """Keep founder length keys; the chosen loftstrider rig blends in absolute metres."""
    adult = (f["adult_length_m"], f["adult_width_m"], f["adult_height_m"])
    t = n / (BINS - 1)
    s0 = (f["body_min"] / f["body_max"]) ** (1.0 / 3.0)
    s = s0 + (1.0 - s0) * t
    if f["name"] == "loftstrider":
        reset_scene()
        objs = A.build_loftstrider(ls_parts(n), 0.5 * vox, 0.5 * vox)
    else:
        objs = build_animal(f["name"], s, adult, vox)
    return adult[0] * s, adult[1] * s, objs


# The founders with a rig (FEED-RIG, FEED-ROLLOUT): the builder's clips, sampled. Each is
# baked with a `<name>.rig` beside its `.cvm` (`write_rig`), named under the manifest's
# `rigs`. A rig whose bins are one build scaled has one rig for every bin (`RIGS`, at the
# reference size); the loftstrider's bins are each their own build, and so its rig
# (`BIN_RIGS`, the bin's part sizes, in its own metres).
RIGS = {"frondgrazer": lambda: A.feed_rig(A.fg_feed_clip()),
        "chorister": lambda: A.feed_rig(A.ch_feed_clip()),
        "lanternjaw": lambda: A.feed_rig(A.lj_feed_clip()),
        # a specimen of one bin (its young draw the adult's)
        "bellwing": lambda: A.feed_rig(A.bw_feed_clip()),
        # the small ground feeders: the littershredder's founder bins, the capgnawer and
        # ripple-snail specimens of one bin
        "littershredder": lambda: A.feed_rig(A.lt_feed_clip()),
        "capgnawer": lambda: A.feed_rig(A.cg_feed_clip()),
        "ripple-snail": lambda: A.feed_rig(A.rs_feed_clip()),
        # the seedporter's pose models it crops in (specimens, one bin each)
        "seedporter-stand": lambda: A.feed_rig(A.sp_feed_clip("stand")),
        "seedporter-ground": lambda: A.feed_rig(A.sp_feed_clip("ground"))}
BIN_RIGS = {"loftstrider": lambda n, cells, vox: A.ls_rig_clips(ls_parts(n), cells, vox)}
# Founders whose shared rig takes per-bin state clips (RIG-STATES): (the builder's
# `fn(shared, cells, bones, scale, pose_cells)`, the pose model whose committed cells it
# reads per bin). The frondgrazer's sleep, hunker and idle are whole voxels of each bin.
STATE_RIGS = {"frondgrazer": (A.fg_state_rig, "frondgrazer-hunker")}
SLEEP_PORTS = ("chorister", "lanternjaw", "seedporter-ground", "bellwing")
for _name in SLEEP_PORTS:
    STATE_RIGS[_name] = (lambda *args, name=_name: A.sleep_state_rig(*args, legs=A.sleep_legs(name)), None)
# RIG-STATES round 3: the ports whose asleep track ends on a sleep pose (`posed_sleep_rig`)
SLEEP_POSES = {"chorister": (A.ch_sleep_plan, True), "lanternjaw": (A.lj_sleep_plan, True),
               "seedporter-stand": (A.sp_sleep_plan, False)}
SLEEP_PORTS += ("seedporter-stand",)
for _name, (_plan, _ground) in SLEEP_POSES.items():
    STATE_RIGS[_name] = (lambda shared, cells, bones, scale, pose=None, plan=_plan, ground=_ground:
                         A.posed_sleep_rig(shared, cells, bones, scale, plan, ground), None)


def model_cells(path):
    """Each bin's cells of a committed animal `.cvm` (its first variant and state; the
    layout is `write_cvm`'s), as the voxelizer's keys (i, j, k up)."""
    with open(path, "rb") as fh:
        b = fh.read()
    assert b[:4] == b"CVM2", path
    at = 5                                   # the magic and the kind
    n, at = b[at], at + 1                    # materials: name each
    for _ in range(n):
        at += 1 + b[at]
    n, at = b[at], at + 1                    # states: name and start each
    for _ in range(n):
        at += 1 + b[at] + 8
    states = n
    steps, = struct.unpack_from("<H", b, at)
    at += 2
    bins = []
    for _ in range(steps):
        at += 16                             # height, radius
        nv, at = b[at], at + 1
        first = None
        for _ in range(nv * states):
            nc, = struct.unpack_from("<I", b, at)
            at += 4
            if first is None:
                first = {(dx, dz, dy) for dx, dy, dz, _, _ in struct.iter_unpack("<bbbBB", b[at:at + 5 * nc])}
            at += 5 * nc
        bins.append(first)
    assert at == len(b), path
    return bins


def write_rig(path, bins):
    """`<name>.rig` (CRG3; `crates/cubarium/src/voxel/rig.rs` has the layout): the bones,
    the turn symmetry (`symmetry`: order and each bone's image; none when absent) and the
    clips, and per size bin its scale, each clip's aim (`aim`, the bite height it reaches
    for, metres; NaN when absent) and lower (`lower`, the share of the `asleep` track it
    holds the body at; 0 when absent), its bones' heads, each clip's sampled frames and each
    cell's bone. `bins`: (length, scale, rig, cells), every rig with the same bones and
    clips."""
    first = bins[0][2]
    b = bytearray(b"CRG3")
    b += struct.pack("<B", len(first["bones"]))
    for name, parent in first["bones"]:
        enc = name.encode()
        b += struct.pack("<B", len(enc)) + enc
        b += struct.pack("<B", 255 if parent is None else parent)
    order, image = first.get("symmetry") or (0, [])
    b += struct.pack("<B", order)
    if order >= 2:
        assert len(image) == len(first["bones"])
        b += bytes(image)
    b += struct.pack("<B", len(first["clips"]))
    for clip in first["clips"]:
        enc = clip["state"].encode()
        b += struct.pack("<B", len(enc)) + enc
        b += struct.pack("<BfH", clip["mode"], clip["fps"], len(clip["frames"]))
    b += struct.pack("<B", len(bins))
    for length, scale, rig, cells in bins:
        assert rig["bones"] == first["bones"] and len(rig["clips"]) == len(first["clips"])
        b += struct.pack("<df", length, scale)
        for clip in rig["clips"]:
            aim = clip.get("aim")
            b += struct.pack("<ff", float("nan") if aim is None else aim, clip.get("lower", 0.0))
        for h in rig["heads"]:
            b += struct.pack("<3f", *h)
        for clip, c0 in zip(rig["clips"], first["clips"]):
            assert len(clip["frames"]) == len(c0["frames"])
            for keys in clip["frames"]:
                for r, d in keys:
                    # A committed rig's quaternion tuple is copied without a matrix
                    # round trip: even the feed clip's float bits must stay unchanged.
                    q = r.to_quaternion() if hasattr(r, "to_quaternion") else r
                    b += struct.pack("<7f", *q, *d)
        b += struct.pack("<I", len(cells))
        for dx, dy, dz, bone in cells:
            b += struct.pack("<bbbB", dx, dy, dz, bone)
    with open(path, "wb") as fh:
        fh.write(b)
    return len(b)


def rig_cells(raw, bones, names):
    """The bone of each cell `cells_of` writes, in its offsets (dx, dy up, dz)."""
    index = {n: i for i, (n, _) in enumerate(names)}
    return [(i, k, j, index[bones[(i, j, k)]]) for (i, j, k) in sorted(raw, key=lambda c: (c[2], c[1], c[0]))]


def read_sleep_source(path):
    """Committed CRG2/3 bins, preserving raw feed keys. Undo this port's children on
    repeat bakes so editing sleep is idempotent and never resamples feeding."""
    with open(path, "rb") as fh:
        data = fh.read()
    at = 4
    assert data[:4] in (b"CRG2", b"CRG3"), path

    def take(fmt):
        nonlocal at
        values = struct.unpack_from(fmt, data, at)
        at += struct.calcsize(fmt)
        return values

    def name():
        nonlocal at
        n, = take("<B")
        text = data[at:at + n].decode()
        at += n
        return text

    n, = take("<B")
    bones = []
    for _ in range(n):
        text, parent = name(), take("<B")[0]
        bones.append((text, None if parent == 255 else parent))
    if data[:4] == b"CRG3":
        order, = take("<B")
        assert order == 0, "sleep ports have no aim symmetry"
    nc, = take("<B")
    clips = []
    for _ in range(nc):
        text = name()
        mode, fps, frames = take("<BfH")
        clips.append(dict(state=text, mode=mode, fps=fps, count=frames))
    keep = [i for i, (text, _) in enumerate(bones) if not text.startswith("sleep_")]
    assert keep == list(range(len(keep))), "sleep-only children follow the feed bones"
    bins = []
    for _ in range(take("<B")[0]):
        length, scale = take("<df")
        aims = [take("<ff") for _ in clips] if data[:4] == b"CRG3" else [(float("nan"), 0.0)] * nc
        heads = [Vector(take("<3f")) for _ in bones]
        own = []
        for clip, (aim, lower) in zip(clips, aims):
            frames = []
            for _ in range(clip["count"]):
                keys = [take("<7f") for _ in bones]
                if clip["state"] != "asleep":
                    assert all(keys[i] == (1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                               for i in range(len(keep), n)), "sleep children must inherit their feed parent"
                frames.append([(keys[i][:4], Vector(keys[i][4:])) for i in keep])
            if clip["state"] != "asleep":
                own.append(dict(state=clip["state"], mode=clip["mode"], fps=clip["fps"],
                                aim=None if math.isnan(aim) else aim, lower=lower, frames=frames))
        cells = {}
        for _ in range(take("<I")[0]):
            i, k, j, owner = take("<bbbB")
            while owner not in keep:
                owner = bones[owner][1]
            cells[(i, j, k)] = bones[owner][0]
        bins.append((length, scale, dict(bones=bones[:len(keep)], heads=heads[:len(keep)], clips=own), cells))
    assert at == len(data), path
    return bins


def bake_sleep_only(name, vox, out_dir):
    """Port only the rig side file of the exact committed cells and feed rig."""
    fn, pose = STATE_RIGS[name]
    assert pose is None, "the round-2 sleep ports"
    cells = model_cells(os.path.join(out_dir, f"{name}.cvm"))
    source = read_sleep_source(os.path.join(out_dir, f"{name}.rig"))
    assert len(cells) == len(source)
    bins = []
    for n, ((length, scale, shared, bones), raw) in enumerate(zip(source, cells)):
        assert raw == set(bones), f"{name} bin {n}: committed model/rig cells differ"
        own, note = fn(shared, raw, bones, scale)
        print(f"{vox} m {name} bin {n}: {note}", flush=True)
        bins.append((length, scale, raw, bones, own))
    return write_rig_of(name, out_dir, bins, vox, states_done=True)


def bake_animal(f, vox, out_dir):
    """The founder's `.cvm` in its size bins, and its `.rig` when it has one: (file,
    bytes, per-bin detail, rig file or None)."""
    name = f["name"]
    steps, meta = [], []
    rigged = name in RIGS or name in BIN_RIGS
    rig_bins = []
    for n in range(BINS):
        length, width, objs = build_founder_bin(f, n, vox)
        bones = {} if rigged else None
        raw = loftstrider_cells(objs, vox, bones) if name == "loftstrider" else voxelize_animal(objs, vox, 0, bones)
        if name in BIN_RIGS:
            # the bin is its own build, in metres
            rig_bins.append((length, 1.0 / vox, raw, bones, BIN_RIGS[name](n, bones, vox)))
        elif rigged:
            # the bin is the reference build scaled by length / reference length
            rig_bins.append((length, length / A.REF[name][1] / vox, raw, bones, None))
        counts = loftstrider_sectors(raw, vox) if name == "loftstrider" else None
        if counts is not None:
            if any(count != 6 for count in counts.values()):
                raise ValueError(f"{name} bin {n} at {vox} m lost a part: {counts}")
            print(f"{name} bin {n} at {vox} m: {counts}", flush=True)
        cells = cells_of(raw, lambda mat, k: 0x30 if animal_role(mat) == "accent" else 0x00)
        steps.append((length, width, [cells]))
        meta.append({"length_m": round(length, 4), "cells": len(cells)} |
                    ({"sectors": counts} if counts is not None else {}))
    file = f"{name}.cvm"
    size = write_cvm(os.path.join(out_dir, file), 1, steps)
    rig = None
    if rigged:
        rig = write_rig_of(name, out_dir, rig_bins, vox)
    return file, size, meta, rig


def write_rig_of(name, out_dir, rig_bins, vox, states_done=False):
    """`<name>.rig` from its bins' (length, scale, cells, cell bones, the bin's own rig or
    None for the shared one); the file's name."""
    shared = None
    if any(r is None for *_, r in rig_bins):
        reset_scene()
        shared = RIGS[name]()
    rig = f"{name}.rig"
    if name in STATE_RIGS and not states_done:
        fn, pose = STATE_RIGS[name]
        poses = model_cells(os.path.join(out_dir, f"{pose}.cvm")) if pose else [None] * len(rig_bins)
        assert len(poses) == len(rig_bins), f"{pose}: {len(poses)} bins"
        staged = []
        for n, (length, scale, raw, bones, r) in enumerate(rig_bins):
            own, note = fn(r or shared, raw, bones, scale, poses[n])
            print(f"{vox} m {name} bin {n}: {note}", flush=True)
            staged.append((length, scale, raw, bones, own))
        rig_bins = staged
    bins = [(length, scale, r or shared, rig_cells(raw, bones, (r or shared)["bones"]))
            for length, scale, raw, bones, r in rig_bins]
    if name in SLEEP_PORTS:
        # Keep the feed skeleton, but discard sleep children empty in every bin.
        used = {c[3] for *_, cells in bins for c in cells}
        # and every bone a used one hangs from (a sleep pose's chain)
        for b in list(used):
            while bins[0][2]["bones"][b][1] is not None:
                b = bins[0][2]["bones"][b][1]
                used.add(b)
        keep = [i for i, (bone, _) in enumerate(bins[0][2]["bones"])
                if i in used or not bone.startswith("sleep_")]
        index = {old: new for new, old in enumerate(keep)}
        trimmed = []
        for length, scale, r, cells in bins:
            own = dict(r, bones=[(r["bones"][i][0], None if r["bones"][i][1] is None
                                 else index[r["bones"][i][1]]) for i in keep],
                       heads=[r["heads"][i] for i in keep],
                       clips=[dict(c, frames=[[keys[i] for i in keep] for keys in c["frames"]])
                              for c in r["clips"]])
            trimmed.append((length, scale, own, [(x, y, z, index[b]) for x, y, z, b in cells]))
        bins = trimmed
    rig_size = write_rig(os.path.join(out_dir, rig), bins)
    first = bins[0][2]
    counts = [{n: sum(1 for c in cells if c[3] == i) for i, (n, _) in enumerate(first["bones"])}
              for _, _, _, cells in bins]
    frames = {c["state"]: len(c["frames"]) for c in first["clips"]}
    print(f"{vox} m {name}: rig {len(first['bones'])} bones, frames {frames}, {rig_size} bytes,"
          f" cells per bone by bin {counts}", flush=True)
    return rig


def bake_specimen(name, vox, out_dir):
    """One adult bin at the builder's reference size (the placeholder table above), and
    any young bins `SPECIMEN_YOUNG` lists below it, ascending by length."""
    _, l0, w0, h0 = A.REF[name]
    floor = None if name in A.BELOW_GROUND else 0
    steps, meta = [], []
    rigged = name in RIGS
    rig_bins = []
    for s in sorted(SPECIMEN_YOUNG.get(name, [])) + [1.0]:
        reset_scene()
        objs = A.build(name, 0.5 * vox, 0.5 * vox, l0 * s, w0 * s, h0 * s)
        bones = {} if rigged else None
        raw = voxelize_animal(objs, vox, floor, bones)
        cells = cells_of(raw, lambda mat, k: 0x30 if animal_role(mat) == "accent" else 0x00)
        if rigged:
            rig_bins.append((l0 * s, s / vox, raw, bones, None))
        steps.append((l0 * s, w0 * s, [cells]))
        meta.append({"length_m": round(l0 * s, 4), "cells": len(cells)})
    file = f"{name}.cvm"
    size = write_cvm(os.path.join(out_dir, file), 1, steps)
    rig = None
    if rigged:
        rig = write_rig_of(name, out_dir, rig_bins, vox)
    return file, size, meta, rig


def bake_larva(name, vox, out_dir):
    """One bin: the larva at its built size, its length and width the built extent."""
    reset_scene()
    objs = A.build_larva(LARVAE[name], 0.5 * vox, 0.5 * vox, vox)
    lo = [min((ob.matrix_world @ v.co)[k] for ob in objs for v in ob.data.vertices) for k in range(2)]
    hi = [max((ob.matrix_world @ v.co)[k] for ob in objs for v in ob.data.vertices) for k in range(2)]
    length, width = hi[0] - lo[0], hi[1] - lo[1]
    cells = animal_cells(objs, vox, None)
    file = f"{name}.cvm"
    size = write_cvm(os.path.join(out_dir, file), 1, [(length, width, [cells])])
    return file, size, [{"length_m": round(length, 4), "cells": len(cells)}]


def main():
    with open(SOURCE) as fh:
        src = json.load(fh, parse_constant=float)
    for sp in src["species"]:
        for st in sp["profile"]:
            st["wood_fraction_max"] = float(st["wood_fraction_max"])  # "inf" -> inf
    if "--sleep-only" in ARGS:
        assert ONLY and ONLY <= set(SLEEP_PORTS), f"--sleep-only --only {','.join(SLEEP_PORTS)}"
        for vox in src["voxel_sizes_m"]:
            out_dir = os.path.join(ROOT, str(round(vox * 1000)))
            for name in sorted(ONLY):
                bake_sleep_only(name, vox, out_dir)
        return
    for vox in src["voxel_sizes_m"]:
        out_dir = os.path.join(ROOT, str(round(vox * 1000)))
        os.makedirs(out_dir, exist_ok=True)
        manifest_path = os.path.join(out_dir, "manifest.json")
        manifest = {"format": "CVM2", "voxel_m": vox, "palette": PALETTE,
                    "variants": VARIANTS, "bins": BINS, "species": {}, "founders": {},
                    "detail": {}}
        if ONLY and os.path.exists(manifest_path):
            with open(manifest_path) as fh:
                manifest = json.load(fh)
            manifest["palette"] = PALETTE
            manifest["format"] = "CVM2"
        manifest.setdefault("specimens", {})
        for sp in src["species"]:
            if (ONLY and sp["name"] not in ONLY) or not hasattr(L, sp["name"]):
                continue  # no builder yet: no model, and the presenter falls back
            file, size, meta, states = bake_plant(sp, vox, out_dir)
            manifest["species"][sp["name"]] = file
            manifest["detail"][sp["name"]] = {"bytes": size, "steps": meta} | (
                {"states": states} if len(states) > 1 else {})
            print(f"{vox} m {sp['name']}: {len(meta)} steps, {size} bytes", flush=True)
        for f in src["founders"]:
            if (ONLY and f["name"] not in ONLY) or f["name"] not in A.REF or f["name"] in AS_SPECIMEN:
                continue
            file, size, meta, rig = bake_animal(f, vox, out_dir)
            manifest["founders"][f["name"]] = file
            if rig:
                manifest.setdefault("rigs", {})[f["name"]] = rig
            if f["name"] in SPECIMENS:
                manifest["specimens"][f["name"]] = file  # gallery alias; never overwrite the bins
            manifest["detail"][f["name"]] = {"bytes": size, "bins": meta}
            print(f"{vox} m {f['name']}: {len(meta)} bins, {size} bytes", flush=True)
        for name, base in FOUNDER_POSES.items():
            f = next((f for f in src["founders"] if f["name"] == base), None)
            if (ONLY and name not in ONLY) or f is None or name not in A.REF:
                continue
            file, size, meta, _ = bake_animal(dict(f, name=name), vox, out_dir)
            manifest["specimens"][name] = file
            manifest["detail"][name] = {"bytes": size, "bins": meta}
            print(f"{vox} m {name}: {base}'s pose, {len(meta)} bins, {size} bytes", flush=True)
        for name in SPECIMENS:
            if (ONLY and name not in ONLY) or name not in A.REF or (name not in AS_SPECIMEN and any(
                    f["name"] == name for f in src["founders"])):
                continue
            file, size, meta, rig = bake_specimen(name, vox, out_dir)
            manifest["specimens"][name] = file
            if rig:
                manifest.setdefault("rigs", {})[name] = rig
            manifest["detail"][name] = {"bytes": size, "bins": meta}
            print(f"{vox} m {name}: specimen, {[m['cells'] for m in meta]} cells, {size} bytes", flush=True)
        for name in LARVAE:
            if ONLY and name not in ONLY:
                continue
            file, size, meta = bake_larva(name, vox, out_dir)
            manifest["specimens"][name] = file
            manifest["detail"][name] = {"bytes": size, "bins": meta}
            print(f"{vox} m {name}: larva, {meta[0]['cells']} cells, {meta[0]['length_m']} m, {size} bytes", flush=True)
        with open(manifest_path, "w") as fh:
            json.dump(manifest, fh, indent=1)
            fh.write("\n")


if __name__ == "__main__":
    main()
