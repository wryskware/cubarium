"""Animal models from the artbook picks (model pass MP-A, 2026-10-01).

    blender -b -P scripts/blender/animal_models.py -- --sheets OUT [--only NAME[,NAME]] [--view AZ,EL]

The Blender builders the voxel bake (`bake_voxel_models.py`) takes its animals from: the
three simulated founders (frondgrazer, littershredder, and the lanternjaw, added
2026-10-01 from its pick) and nine unsimulated specimens
(chorister, ripple-snail, capgnawer, seedporter, bellwing, and the loftstrider, added
2026-10-01 from its pick; the seedporter's stand, ground and glide poses, 2026-10-01, the
pick's own pose being its climbing one). Each moves its signed-off
blockout (`animal_bodies_r2.py`: CH v3, RS-D2 v2, CG-D2 v2, SP-AB2, FG/LT v2, BW-A) to
its pick in `art/gen/artbook/` (README table; `design/handoffs/voxel-model-pass-2026-10-01.md`).
Blockout fidelity: primitives, tubes and lofts, every part, count, proportion and colour
zone the pick shows; no sculpting, no textures.

A builder draws one animal at its **reference size** (`REF`), at the origin, facing +x,
standing on z = 0; `build()` scales it to the asked length, width and height and moves it
to the anchor. Every part is drawn in a **part slot** (`a_main`, `a_dark`, ...: names in
the model palette, appended to `model::PALETTE` in `crates/cubarium/src/voxel/model.rs`);
what each slot means, and its colour, is per animal (`COATS`). The Blender material is
`<animal>:<slot>` so a render shows the animal's own colours; the bake keeps the slot.

`COATS` holds each slot's ramp `(low, high)` (belly → back, base → tip), eyedropped from
the pick on 2026-10-01 (median of a 9 x 9 patch, or single pixels on thin parts). The
renderer's copy is `crates/cubarium/src/voxel/colours.rs` (`animal`, `specimen`).

`--sheets OUT` writes one contact sheet per animal: the pick | this builder rendered from
the pick's angle | the adult voxel model at 0.125 m, cubes coloured by the same ramps.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Quaternion, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import organism_lineup as L  # noqa: E402  (import-safe: main() is guarded)

cyl, ell, box = L.cyl, L.ell, L.box


def rx(deg):
    return Matrix.Rotation(math.radians(deg), 4, "X")


def ry(deg):
    return Matrix.Rotation(math.radians(deg), 4, "Y")


def rz(deg):
    return Matrix.Rotation(math.radians(deg), 4, "Z")


# --- colours ---------------------------------------------------------------------------
#
# Part slots (palette names). Their meaning per animal is in the comment beside each coat.
SLOTS = ["a_main", "a_dark", "a_pale", "a_mark", "a_limb", "a_foot", "a_head", "a_eye",
         "a_glow", "a_membrane", "a_fruit", "a_tendril", "magenta", "a_lure"]
# On an animal, these are accents (the bake's accent rule: an accent claims the cells whose
# centres lie inside it, or the one cell its centre is in): sense patches and eyes, the
# parts smaller than a voxel that must not vanish into the body.
ACCENTS = {"magenta", "a_eye", "a_lure"}

COATS = {
    # main flank, dark back, pale countershade, rosettes, legs (dark shin -> thigh), claws,
    # crest lobes, eye (magenta), muzzle (cyan)
    "chorister": {
        "a_main": ("#6C55D3", "#6C55D5"), "a_dark": ("#312675", "#241B56"),
        "a_pale": ("#B9A8E2", "#B9A8E2"), "a_mark": ("#261F5B", "#261F5B"),
        "a_limb": ("#30256E", "#6C55D5"), "a_foot": ("#2A2061", "#342878"),
        "a_head": ("#B7AAE1", "#EBDDFC"), "a_eye": ("#D70FE3", "#FD22FE"),
        "a_glow": ("#55C5FB", "#52C4FC"),
    },
    # coral shield, hoof band, cream belly skirt, wine saddles, legs and neck, hooves,
    # scoop lining, stalk-eye lens, stalk-eye rim, shoulder sensor spots (magenta)
    "frondgrazer": {
        "a_main": ("#EE7156", "#FB8162"), "a_dark": ("#58342D", "#5D3B31"),
        "a_pale": ("#BB7A5E", "#DF9F7D"), "a_mark": ("#872935", "#902C3A"),
        "a_limb": ("#9B4341", "#D96862"), "a_foot": ("#C0927C", "#CA9B86"),
        "a_head": ("#FED3AC", "#FED3AC"), "a_eye": ("#300919", "#4E162B"),
        "a_glow": ("#EE699E", "#F370A2"), "magenta": ("#C83C66", "#D8456F"),
    },
    # gold plates, brown bands, pale underside / legs / feelers, claws (dark tip -> gold
    # base), knee joints, the sensor spot (magenta)
    "littershredder": {
        "a_main": ("#F4AD31", "#FED24E"), "a_dark": ("#5A2D10", "#693819"),
        "a_pale": ("#E6C690", "#FEEEC0"), "a_foot": ("#6D3101", "#E5A537"),
        "a_limb": ("#C06F0E", "#C06F0E"), "magenta": ("#A30E57", "#A30E57"),
    },
    # azure shell, dark foot, pale mantle edge, growth-ridge grooves, palp stalks, palp tips,
    # the dark front opening
    "ripple-snail": {
        "a_main": ("#416EE4", "#5081F2"), "a_dark": ("#1B234C", "#252D5A"),
        "a_pale": ("#7FA1F4", "#96B5FC"), "a_mark": ("#172B6B", "#172B6B"),
        "a_limb": ("#5E75CE", "#5F5BB9"), "a_glow": ("#FC56CE", "#FC57D0"),
        "a_head": ("#0A0D2A", "#0A0D2A"),
    },
    # rose-mauve mantle, bark streaks, fringe, pale spots, body and legs, toes, mandibles,
    # mouth slit, sensor patches (light pink, magenta slot)
    "capgnawer": {
        "a_main": ("#9C607A", "#A76982"), "a_mark": ("#6B4E63", "#78596E"),
        "a_pale": ("#EFA8C1", "#F8D6EC"), "a_glow": ("#E3BCF8", "#ECCAFB"),
        "a_limb": ("#B4768E", "#B7778F"), "a_foot": ("#BD92BF", "#F0D1F0"),
        "a_head": ("#BB95BB", "#F5E5F4"), "a_dark": ("#010000", "#010000"),
        "magenta": ("#FAB6E4", "#FEC7EF"),
    },
    # moss skin, open mouth, pale underside and face, dorsal stripe, claws, eyes, flank
    # membranes, the carried fruit, cheek sensor dots (magenta)
    "seedporter": {
        "a_main": ("#5C6A28", "#889335"), "a_dark": ("#361D13", "#361D13"),
        "a_pale": ("#BEB55E", "#EADE97"), "a_mark": ("#21361D", "#2D441F"),
        "a_foot": ("#191817", "#30321A"), "a_eye": ("#0C0707", "#0C0707"),
        "a_membrane": ("#DBDA7B", "#EDEAA5"), "a_fruit": ("#802B51", "#D6658A"),
        "magenta": ("#B5376A", "#C13E7B"),
        # the pose models' cheek dots: not in the bake's palette, so Blender only; at
        # 0.125 m the face is one cell, and it shows the eyes
        "a_cheek": ("#B5376A", "#C13E7B"),
        # and their claws, Blender only too: a claw drawn in voxels is a black cell
        # beyond the hand or foot
        "a_claw": ("#191817", "#30321A"),
    },
    # violet plates (shadow -> lit), dark joints and underside, legs and striking arms, claw
    # tips, fangs and razor blades (pale), eyes (magenta), lure orbs (cyan), lure holders.
    # Sampled 2026-10-01 from the pick (k-means of its opaque pixels).
    "lanternjaw": {
        "a_main": ("#5E4070", "#9168A1"), "a_dark": ("#1B161B", "#2F2433"),
        "a_limb": ("#5E4070", "#865D9C"), "a_foot": ("#2A2530", "#4A4452"),
        "a_head": ("#B691BF", "#E5D6ED"), "a_eye": ("#B71984", "#E437C1"),
        "a_lure": ("#63EEFC", "#ADFAFD"), "a_mark": ("#4A4452", "#695787"),
    },
    # body (teal low -> violet high), neck, legs, feet, knee and hip bulbs and the pink head,
    # eyes (magenta), tusks (bone), frond blades (lilac), frond edges (cyan), pupils and the
    # mouth hole. Sampled 2026-10-01 from the reference (k-means per body region), then
    # pushed brighter and more saturated to match its vividness (Wrysk).
    "loftstrider": {
        "a_main": ("#22B4D2", "#8A62E6"), "a_pale": ("#A07CE0", "#D2A8F4"),
        "a_limb": ("#18A2C8", "#8C5CDC"), "a_foot": ("#2ED2D8", "#6CF2E8"),
        "a_mark": ("#D466B8", "#F79AD6"), "a_eye": ("#F02E9E", "#FF4CBA"),
        "a_head": ("#EEDAB4", "#FFF0CC"), "a_membrane": ("#C6AEFF", "#EADCFF"),
        "a_glow": ("#00C6F2", "#36E8FF"), "a_dark": ("#1A0E1A", "#3A1C2A"),
        # the eyes' slit pupils: smaller than a voxel, and not in the bake's palette, so
        # they are drawn in Blender only and never cover an eye's magenta cell
        "a_pupil": ("#120810", "#120810"),
    },
    # plum bell (lighter toward the mouth), lilac wings, cyan leading edges, mauve wing
    # roots, peach tendril, magenta rim and underside
    "bellwing": {
        "a_main": ("#5E1764", "#2E0D3E"), "a_membrane": ("#A580F2", "#C19BFC"),
        "a_glow": ("#5ABCEF", "#95E7FC"), "a_limb": ("#9C547D", "#9C547D"),
        "a_tendril": ("#EEB4A7", "#F1C4B9"), "magenta": ("#C0099E", "#E20CAF"),
    },
}

# the seedporter's poses wear its coat
for _pose in ("stand", "ground", "glide"):
    COATS[f"seedporter-{_pose}"] = COATS["seedporter"]
# and the frondgrazer's hunker wears its own
COATS["frondgrazer-hunker"] = COATS["frondgrazer"]

CUR = [None]


def _mix(a, b, t=0.5):
    a, b = a.lstrip("#"), b.lstrip("#")
    ca = [int(a[i:i + 2], 16) for i in (0, 2, 4)]
    cb = [int(b[i:i + 2], 16) for i in (0, 2, 4)]
    return "#" + "".join(f"{round(x + (y - x) * t):02X}" for x, y in zip(ca, cb))


def use(name):
    CUR[0] = name
    for slot, (lo, hi) in COATS[name].items():
        L.PAL[f"{name}:{slot}"] = _mix(lo, hi)


def c(slot):
    if slot not in COATS[CUR[0]]:
        raise KeyError(f"{CUR[0]} has no {slot}")
    return f"{CUR[0]}:{slot}"


def slot_of(material):
    """`frondgrazer:a_main` -> `a_main` (what the bake writes)."""
    return material.rsplit(":", 1)[-1]


# --- geometry helpers ---------------------------------------------------------------------

def tube(pts, r0, r1, mat, seg=8, bones=None):
    """Cylinders through a polyline, radius r0 -> r1, with a sphere at each inner joint.
    `bones[k]`, when given, is the rig bone segment k belongs to; the joint sphere at the
    start of segment k goes with it (a sphere on its own pivot stays put as it turns)."""
    n = len(pts) - 1
    for k in range(n):
        start = len(L.ORG)
        cyl(pts[k], pts[k + 1], r0 + (r1 - r0) * k / n, r0 + (r1 - r0) * (k + 1) / n, mat=mat, seg=seg)
        if k:
            rr = r0 + (r1 - r0) * k / n
            ell(pts[k], (rr, rr, rr), mat)
        if bones:
            bone_of(start, bones[k])


def bone_of(start, name):
    """Every organism part built since `start` belongs to rig bone `name` (the voxelizer
    records each cell's bone from its part, as it records the material)."""
    for ob in L.ORG[start:]:
        ob["bone"] = name


def bone_ahead(start, x, name):
    """Every organism part built since `start`: what lies ahead of `x` (reference metres,
    +x forward) belongs to rig bone `name`, what lies behind to the part's own bone. For a
    part that is one mesh across a joint (a loft); the voxelizer reads each cell's bone,
    and the flipbook each vertex's, by position (`part_bone`). `build` carries `x` to the
    built size."""
    for ob in L.ORG[start:]:
        ob["bone_cut_x"] = x
        ob["bone_ahead"] = name


def bone_nearest(start, segs):
    """Every organism part built since `start` (one mesh across several bones: a loft up a
    neck, a frond's blade): each point belongs to the bone of `segs` ((name, (head, tail)),
    metres) whose segment is nearest it. The voxelizer reads each cell's bone by position
    (`part_bone`); `build_loftstrider` carries the segments to the anchor."""
    for ob in L.ORG[start:]:
        ob["bone_names"] = ",".join(n for n, _ in segs)
        ob["bone_segs"] = [v for _, (h, t) in segs for v in (*h, *t)]


def seg_dist(p, h, t):
    """Distance from point `p` to the segment `h`–`t`."""
    p, h, t = Vector(p), Vector(h), Vector(t)
    d = t - h
    u = 0.0 if d.length_squared < 1e-12 else max(0.0, min(1.0, (p - h).dot(d) / d.length_squared))
    return (p - (h + d * u)).length


def part_bone(ob, p):
    """The rig bone of part `ob` at point `p` (the part's own coordinates): the nearest of
    its `bone_nearest` segments, else its `bone_ahead` bone ahead of its cut, else its own
    bone, else the root."""
    if "bone_segs" in ob:
        names = ob["bone_names"].split(",")
        v = list(ob["bone_segs"])
        return min(names, key=lambda n: seg_dist(p, v[6 * names.index(n):6 * names.index(n) + 3],
                                                 v[6 * names.index(n) + 3:6 * names.index(n) + 6]))
    if "bone_cut_x" in ob and p[0] > ob["bone_cut_x"]:
        return ob["bone_ahead"]
    return ob.get("bone", "root")


def sheet(pts, mat, th=0.006):
    """A flat membrane through a polygon of 3D points: a thin slab, both faces."""
    bm = bmesh.new()
    nrm = (Vector(pts[1]) - Vector(pts[0])).cross(Vector(pts[2]) - Vector(pts[0]))
    nrm = nrm.normalized() if nrm.length > 1e-9 else Vector((0, 0, 1))
    top = [bm.verts.new(Vector(p) + nrm * th / 2) for p in pts]
    bot = [bm.verts.new(Vector(p) - nrm * th / 2) for p in pts]
    bm.faces.new(top)
    bm.faces.new(list(reversed(bot)))
    n = len(pts)
    for k in range(n):
        bm.faces.new((top[k], top[(k + 1) % n], bot[(k + 1) % n], bot[k]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return L.mk("sheet", bm, mat)


def catmull(pts, n):
    """Resample a polyline of tuples through a Catmull-Rom spline to n points."""
    P = [pts[0]] + list(pts) + [pts[-1]]
    out = []
    segs = len(pts) - 1
    for k in range(n):
        u = k / (n - 1) * segs
        i = min(int(u), segs - 1)
        t = u - i
        p0, p1, p2, p3 = P[i], P[i + 1], P[i + 2], P[i + 3]
        out.append(tuple(0.5 * ((2 * b) + (-a + c_) * t + (2 * a - 5 * b + 4 * c_ - d) * t * t
                                + (-a + 3 * b - 3 * c_ + d) * t ** 3) for a, b, c_, d in zip(p0, p1, p2, p3)))
    return out


def loft(stations, mats, zone, nring=72, nseg=20):
    """One closed smooth mesh through stations (x, y, z, ry, rz_up[, rz_down]): an
    elliptical ring per station, its upper and lower half-heights apart. zone(f, s, p)
    picks the material index per face from its fraction along, the sine of its ring angle
    (1 = top) and its centre."""
    st = [s if len(s) == 6 else (*s, s[4]) for s in stations]
    S = catmull(st, nring)
    bm = bmesh.new()
    rings = []
    for i, (px, py, pz, ry_, ru, rd) in enumerate(S):
        a = Vector(S[max(i - 1, 0)][:3])
        b = Vector(S[min(i + 1, len(S) - 1)][:3])
        t = (b - a).normalized()
        ref = Vector((0, 0, 1)) if abs(t.z) < 0.9 else Vector((1, 0, 0))
        side = t.cross(ref).normalized()
        up = side.cross(t).normalized()
        if abs(t.z) < 0.9 and up.z < 0:
            up, side = -up, -side
        c0 = Vector((px, py, pz))
        ring = []
        for k in range(nseg):
            ang = 2 * math.pi * k / nseg
            sn = math.sin(ang)
            ring.append(bm.verts.new(c0 + side * max(ry_, 1e-4) * math.cos(ang)
                                     + up * max(ru if sn > 0 else rd, 1e-4) * sn))
        rings.append(ring)
    for i in range(len(rings) - 1):
        for k in range(nseg):
            f = bm.faces.new((rings[i][k], rings[i][(k + 1) % nseg], rings[i + 1][(k + 1) % nseg], rings[i + 1][k]))
            cen = sum((v.co for v in f.verts), Vector()) / 4
            f.material_index = zone(i / (len(rings) - 1), math.sin(2 * math.pi * (k + 0.5) / nseg), cen)
            f.smooth = True
    for ring, rev in ((rings[0], True), (rings[-1], False)):
        cen = bm.verts.new(sum((v.co for v in ring), Vector()) / len(ring))
        for k in range(nseg):
            tri = (cen, ring[(k + 1) % nseg], ring[k]) if rev else (cen, ring[k], ring[(k + 1) % nseg])
            f = bm.faces.new(tri)
            f.material_index = zone(0.0 if rev else 1.0, 0.0, cen.co)
            f.smooth = True
    # outward normals: the voxelizer's inside test reads them
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.normal_update()
    me = bpy.data.meshes.new("loft")
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new("loft", me)
    for m in mats:
        ob.data.materials.append(L.M(m))
    L.COL.objects.link(ob)
    L.ORG.append(ob)
    return ob


def helix(p0, axis_len, turns, radius, wire, mat, seg=6, n=48, phase=0.0):
    """A 3D corkscrew hanging down from p0: `turns` turns over `axis_len`."""
    pts = []
    for k in range(n + 1):
        t = k / n
        a = phase + 2 * math.pi * turns * t
        pts.append((p0[0] + radius * math.cos(a), p0[1] + radius * math.sin(a), p0[2] - axis_len * t))
    tube(pts, wire, wire * 0.8, mat, seg=seg)


# --- chorister · CH-A2 v3 violet stalker (1.4 x 0.35 m, shoulder 0.6 m, tail 0.8 m) ----------
#
# Pick: art/gen/artbook/chorister.png, side view facing right. Measured off the pick at
# 0.00322 m/px along the body and 0.00227 m/px up it (the pick's legs are longer than the
# 0.6 m shoulder allows: the shoulder is kept, so the height is read 0.7x the length).

def chorister():
    def body_zone(f, s, p):
        if s > 0.55:
            return 1                        # dark back
        if s < -0.2 and (p.x < -0.6 or -0.45 < p.x < 0.36):
            return 2                        # pale countershade (flank stripe, tail underside)
        return 0
    spine = [  # x, y, z centre, half-width, half-height up, down
        (0.50, 0, 0.51, 0.08, 0.11, 0.11), (0.42, 0, 0.51, 0.1, 0.13, 0.13),
        (0.28, 0, 0.50, 0.15, 0.165, 0.17), (0.08, 0, 0.465, 0.14, 0.16, 0.16),
        (-0.12, 0, 0.44, 0.12, 0.14, 0.135), (-0.31, 0, 0.44, 0.1, 0.1, 0.1),
        (-0.50, 0, 0.435, 0.095, 0.083, 0.083), (-0.73, 0, 0.456, 0.05, 0.048, 0.048),
        (-0.99, 0, 0.475, 0.028, 0.025, 0.025), (-1.21, 0, 0.48, 0.014, 0.012, 0.012),
        (-1.38, 0, 0.49, 0.003, 0.003, 0.003)]
    start = len(L.ORG)
    loft(spine, [c("a_main"), c("a_dark"), c("a_pale")], body_zone, nring=90, nseg=24)
    # the rig (FEED-RIG round 2): the loft is one mesh, so its front, ahead of the
    # shoulders, is the neck bone's by position; the rest of it is the root's
    bone_ahead(start, CH_NECK_CUT_X, "neck")
    # rosettes: dark rings on the back and upper flanks, along the body and the tail root
    S = catmull([s[:5] for s in spine], 40)
    for idx, ang in ((6, 0.95), (8, 1.25), (10, 0.75), (12, 1.15), (14, 0.8), (16, 1.3), (18, 0.9),
                     (20, 0.6), (22, 1.05), (24, 0.75), (26, 0.95)):
        px, py, pz, ry_, rz_ = S[idx]
        for sy in (-1, 1):
            p = Vector((px, py + sy * ry_ * math.cos(ang), pz + rz_ * math.sin(ang)))
            n = Vector((0, sy * math.cos(ang), math.sin(ang)))
            rot = Vector((0, 0, 1)).rotation_difference(n).to_matrix().to_4x4()
            rr = 0.026 if ry_ > 0.08 else 0.016
            ell(p, (rr, rr * 0.8, 0.008), c("a_mark"), rot=rot)
            ell(p + n * 0.004, (rr * 0.45, rr * 0.36, 0.006), c("a_dark"), rot=rot)
    # head: a rounded skull, the two lilac crest lobes swept back over the neck, a large
    # magenta eye under them each side, the cyan muzzle at the front; all on the head bone
    start = len(L.ORG)
    ell((0.58, 0, 0.55), (0.15, 0.1, 0.11), c("a_main"), rot=ry(8))
    for sy in (-1, 1):
        ell((0.53, sy * 0.045, 0.63), (0.27, 0.055, 0.07), c("a_head"), rot=ry(17) @ rx(sy * 8))
        ell((0.565, sy * 0.075, 0.55), (0.08, 0.022, 0.05), c("a_eye"), rot=ry(10))
    ell((0.73, 0, 0.485), (0.08, 0.06, 0.042), c("a_glow"), rot=ry(8))
    bone_of(start, "head")

    def leg_zone(f, s, p):
        return 0 if f < 0.4 else 1
    legs = [  # (x, y, z, r) from hip to the ankle; then the paw
        [(0.40, 0.08, 0.44, 0.085), (0.56, 0.14, 0.22, 0.045), (0.68, 0.15, 0.05, 0.032)],
        [(0.14, 0.09, 0.40, 0.085), (0.205, 0.15, 0.2, 0.045), (0.205, 0.16, 0.06, 0.032)],
        [(-0.50, 0.08, 0.40, 0.1), (-0.36, 0.14, 0.23, 0.055), (-0.66, 0.15, 0.07, 0.032)],
    ]
    for leg in legs:
        for sy in (-1, 1):
            pts = [(x, sy * yy, z, r, r) for x, yy, z, r in leg]
            loft(pts, [c("a_main"), c("a_limb")], leg_zone, nring=30, nseg=12)
            ax, ay, az = pts[-1][:3]
            # a long paw running forward, three claws
            cyl((ax, ay, az), (ax + 0.1, ay, 0.018), 0.026, 0.02, mat=c("a_limb"), seg=10)
            for k in (-1, 0, 1):
                cyl((ax + 0.09, ay + k * 0.016, 0.018), (ax + 0.14, ay + k * 0.022, 0.006), 0.009, 0.003,
                    mat=c("a_foot"), seg=6)


# --- frondgrazer · coral saddle shield (0.75 x 0.375 x 0.375 m) ----------------------------
#
# Pick: art/gen/artbook/frondgrazer.png, side view facing left. 0.00139 m/px along the
# shield, 0.00116 m/px up it (the shield is kept at the founder's 0.375 m height).

# The hunker pose (Wrysk 2026-10-06, HUNKER: "author the new pose after as well"): the same
# rig, three changes and nothing else. The shield, rigid with its sensor spots, comes down
# until its rim (the lower edge, its widest line) is on the ground line all round; the neck
# folds back into the shield's front so the scoop sits flush with the front opening; the
# eye stalks fold inward beside the scoop. Readings taken where the pose said nothing
# (open questions for Wrysk): the whole shield drops by the same amount (shoulders and
# rump alike), its pale belly below the rim goes under the ground line, and the legs fold
# flat under the shield, hooves drawn in 3.5 cm under the rim, so they are hidden.
FG_POSES = {"rest": dict(drop=0.0, tuck=False), "hunker": dict(drop=0.17, tuck=True)}


def frondgrazer(pose="rest"):
    drop, tuck = FG_POSES[pose]["drop"], FG_POSES[pose]["tuck"]
    tops = [(0.42, 0.20), (0.39, 0.26), (0.30, 0.35), (0.19, 0.375), (0.075, 0.345), (0.0, 0.31),
            (-0.09, 0.32), (-0.225, 0.35), (-0.315, 0.32), (-0.39, 0.23), (-0.42, 0.18)]
    tops = [(x, t - drop) for x, t in tops]
    edge = 0.17 - drop                       # the shield's lower edge, its widest line
    stations = []
    for x, top in tops:
        end = abs(x) > 0.4
        half = 0.07 if end else (0.15 if abs(x) > 0.35 else 0.19)
        stations.append((x, 0.0, edge, half, max(top - edge, 0.02), 0.03 if end else 0.06))

    def saddle(x, s):
        # the two wine saddles: a rounded patch over each hump, down to a quarter of the
        # way above the shield's edge
        return any(((x - x0) / ax) ** 2 + ((s - 1) / a_s) ** 2 < 1
                   for x0, ax, a_s in ((0.2, 0.085, 0.75), (-0.245, 0.075, 0.8)))

    def shield_zone(f, s, p):
        if s < -0.05:
            return 1                         # the cream belly skirt under the shield
        if abs(p.x - 0.01) < 0.006:
            return 2                         # the seam between the two shield masses
        return 2 if saddle(p.x, s) else 0
    loft(stations, [c("a_main"), c("a_pale"), c("a_mark")], shield_zone, nring=160, nseg=48)

    def surface(x, z):
        top = dict(tops)
        xs = sorted(top)
        for a, b in zip(xs, xs[1:]):
            if a <= x <= b:
                t = (x - a) / (b - a)
                h = top[a] + t * (top[b] - top[a])
                break
        half = 0.15 if abs(x) > 0.35 else 0.19
        u = min(max((z - edge) / max(h - edge, 0.02), 0.0), 1.0)
        return half * math.sqrt(1 - u * u)
    # shoulder sensor spots: a cluster at the front and the rear of each flank
    for x0 in (0.262, -0.34):
        for dx, z, r in ((0.0, 0.235, 0.011), (0.0, 0.212, 0.006), (-0.012, 0.21, 0.006), (-0.008, 0.193, 0.006)):
            xx = x0 + (dx if x0 > 0 else -dx)
            for sy in (-1, 1):
                ell((xx, sy * (surface(xx, z - drop) + 0.002), z - drop), (r, 0.004, r * 1.4), c("magenta"))
    # six short stout legs: dark band over two-toed hooves
    for hx, fx in ((0.25, 0.33), (-0.03, -0.03), (-0.31, -0.33)):
        for sy in (-1, 1):
            hip = (hx, sy * 0.12, 0.18)
            knee = ((hx + fx) / 2, sy * 0.15, 0.1)
            fy = 0.155
            if drop:
                # folded flat under the lowered shield, the hooves drawn in under its rim
                fy = 0.12
                hip, knee = (hx, sy * 0.09, 0.06), ((hx + fx) / 2, sy * 0.11, 0.05)
            foot = (fx, sy * fy, 0.05)
            tube([hip, knee, foot], 0.048, 0.04, c("a_limb"), seg=12)
            cyl((fx, sy * fy, 0.032), (fx, sy * fy, 0.055), 0.042, 0.042, mat=c("a_dark"), seg=12)
            for toe in (-1, 1):
                ell((fx + 0.012, sy * fy + toe * 0.019, 0.02), (0.03, 0.02, 0.022), c("a_foot"))
    if tuck:
        # the neck folded back into the shield's front: the scoop flush with the front
        # opening, its trough turned in so only its outer surface shows; the eye stalks
        # folded inward beside it, the eyes against the scoop's lip
        tube([(0.27, 0, 0.05), (0.32, 0, 0.06), (0.36, 0, 0.065)], 0.06, 0.045, c("a_limb"), seg=14)
        ell((0.375, 0, 0.065), (0.03, 0.09, 0.05), c("a_limb"))
        ell((0.36, 0, 0.065), (0.014, 0.078, 0.04), c("a_head"))
        for sy in (-1, 1):
            tube([(0.385, sy * 0.08, 0.09), (0.375, sy * 0.06, 0.1), (0.365, sy * 0.045, 0.104)],
                 0.008, 0.006, c("a_limb"), seg=6)
            # the eye folded in, its lens turned against the scoop: drawn without its lens
            # (an accent claims a cell even where it is hidden, and the lens would show)
            ell((0.363, sy * 0.042, 0.105), (0.012, 0.012, 0.012), c("a_glow"))
        return
    # the folded neck, rising out of the shield's front to the transverse cropping scoop;
    # its lower two segments on the rig's first neck bone, its upper two on the second
    neck = FG_NECK
    tube(neck, 0.065, 0.045, c("a_limb"), seg=14, bones=["neck_a", "neck_a", "neck_b", "neck_b"])
    # the scoop: a wide trumpet lip across the neck's top, its pale lining facing up
    start = len(L.ORG)
    ell((0.49, 0, 0.4), (0.055, 0.1, 0.03), c("a_limb"))
    ell((0.493, 0, 0.418), (0.042, 0.088, 0.014), c("a_head"))
    bone_of(start, "head")
    # two corner stalks, each ending in an eye: magenta rim, dark plum lens
    for sy in (-1, 1):
        start = len(L.ORG)
        tube([(0.49, sy * 0.09, 0.41), (0.5, sy * 0.12, 0.44), (0.505, sy * 0.15, 0.465)],
             0.008, 0.006, c("a_limb"), seg=6)
        ell((0.507, sy * 0.155, 0.47), (0.018, 0.018, 0.018), c("a_glow"))
        ell((0.515, sy * 0.162, 0.472), (0.012, 0.012, 0.012), c("a_eye"))
        bone_of(start, "eye_l" if sy > 0 else "eye_r")


def frondgrazer_hunker():
    frondgrazer("hunker")


# --- the frondgrazer's rig and feeding clip (FEED-RIG, 2026-10-09) ---------------------
#
# `design/handoffs/feeding-motion-rig-2026-10-09.md`: one armature and one action, the one
# source of the feeding motion for the flipbook (each sampled frame re-voxelized) and for
# the renderer's bones (`crates/cubarium/src/voxel/rig.rs`). Every part the rest builder
# draws belongs to one bone (`bone_of`; a part with none is the root's). The clip is a
# placeholder graze for judging the method; Wrysk directs the real choreography later.
#
# The action is the **loop** (two bites and a chew) in the reached posture. The reach and
# the return are the renderer's clip weight easing 0 -> 1 and back over `FG_FEED_EASE_S`
# (each bone's turn eased from none, about its own pivot), so an animal can enter and leave
# the loop anywhere in it and two grazers keep their own phases.

FG_NECK = [(0.29, 0, 0.2), (0.34, 0, 0.24), (0.40, 0, 0.29), (0.45, 0, 0.35), (0.48, 0, 0.385)]
# name, parent index, head, tail (reference metres, Blender axes)
FG_BONES = [
    ("root", None, (0.0, 0.0, 0.2), (0.1, 0.0, 0.2)),
    ("neck_a", 0, FG_NECK[0], FG_NECK[2]),
    ("neck_b", 1, FG_NECK[2], FG_NECK[4]),
    ("head", 2, FG_NECK[4], (0.545, 0.0, 0.4)),
    ("eye_l", 3, (0.49, 0.09, 0.41), (0.505, 0.15, 0.465)),
    ("eye_r", 3, (0.49, -0.09, 0.41), (0.505, -0.15, 0.465)),
]
FG_FEED_FPS = 30
FG_FEED_LOOP_S = 1.2
FG_FEED_EASE_S = 0.4
# Reached: the scoop's lip on the ground line about one voxel (0.125 m) ahead of the front
# feet (their hooves' toes, x 0.372), the scoop level as at rest. Its lip is 0.065 m ahead of
# the head's pivot and its underside 0.015 m below it.
FG_FRONT_FOOT_X = 0.372
FG_LIP = (0.065, -0.015)
# The loop's key poses: (seconds, the pivot's dip (+ up, metres), the scoop's tip (+
# forward, degrees)): two bites of 0.35 s (dip 3 cm, tip 15 deg), a chew of 0.5 s (lift
# 4 cm, tilted back 8 deg: "a little").
FG_FEED_KEYS = [(0.0, 0.0, 0.0), (0.175, -0.03, 15.0), (0.35, 0.0, 0.0), (0.525, -0.03, 15.0),
                (0.70, 0.0, 0.0), (0.95, 0.04, -8.0), (1.20, 0.0, 0.0)]
# Blender (x forward, y side, z up) <-> the model's (x forward, y up, z side).
SWAP_YZ = Matrix(((1, 0, 0, 0), (0, 0, 1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))


def fg_feed_angles(dip, tip_deg):
    """Each bone's turn about the side axis, radians counter-clockwise in the (x, z) plane
    (+ lifts the nose), relative to its parent, for the reached pose dipped by `dip` and
    tipped forward by `tip_deg`: the neck a two-bone reach to the head's pivot, the head
    turned so the scoop is tipped by `tip_deg` from level, the eye stalks counter-turned
    so the eyes stay level."""
    p0, p2, p4 = (Vector((p[0], p[2])) for p in (FG_NECK[0], FG_NECK[2], FG_NECK[4]))
    target = Vector((FG_FRONT_FOOT_X + 0.125 - FG_LIP[0], -FG_LIP[1] + dip))
    l1, l2 = (p2 - p0).length, (p4 - p2).length
    to = target - p0
    dist = min(max(to.length, abs(l1 - l2) + 1e-6), l1 + l2 - 1e-6)
    base = math.atan2(to.y, to.x)
    bend = math.acos(max(-1.0, min(1.0, (l1 * l1 + dist * dist - l2 * l2) / (2 * l1 * dist))))
    # the joint on the forward-up side of the reach, as a folded neck carries it
    joints = [p0 + Vector((math.cos(base + s * bend), math.sin(base + s * bend))) * l1 for s in (1, -1)]
    joint = max(joints, key=lambda j: j.x + j.y)
    ang = lambda v: math.atan2(v.y, v.x)  # noqa: E731
    a1 = ang(joint - p0) - ang(p2 - p0)
    a2 = ang(target - joint) - ang(p4 - p2) - a1
    tip = math.radians(tip_deg)
    head = -tip - (a1 + a2)
    return [0.0, a1, a2, head, tip, tip]


def fg_feed_armature():
    """The armature and its feeding action, in the current scene at reference size,
    standing at the origin as `frondgrazer()` draws: the action keyed at the loop's key
    poses (Bezier, eased), at `FG_FEED_FPS` frames a second."""
    sc = bpy.context.scene
    sc.render.fps = FG_FEED_FPS
    arm = bpy.data.armatures.new("fg_rig")
    ob = bpy.data.objects.new("fg_rig", arm)
    sc.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.mode_set(mode="EDIT")
    for name, parent, head, tail in FG_BONES:
        eb = arm.edit_bones.new(name)
        eb.head, eb.tail = Vector(head), Vector(tail)
        if parent is not None:
            eb.parent = arm.edit_bones[FG_BONES[parent][0]]
    bpy.ops.object.mode_set(mode="OBJECT")
    ob.animation_data_create()
    ob.animation_data.action = bpy.data.actions.new("fg_feed")
    for t, dip, tip in FG_FEED_KEYS:
        frame = t * FG_FEED_FPS
        for (name, _, _, _), a in zip(FG_BONES, fg_feed_angles(dip, tip)):
            pb = ob.pose.bones[name]
            pb.rotation_mode = "QUATERNION"
            # the turn about the side axis through the bone's head, in armature space
            # (Blender's +y; a counter-clockwise (x, z) turn is a negative turn about +y),
            # as the bone's own basis: Q^-1 R Q, its rest orientation Q
            q = pb.bone.matrix_local.to_quaternion()
            r = Matrix.Rotation(-a, 4, "Y").to_quaternion()
            pb.rotation_quaternion = q.inverted() @ r @ q
            pb.keyframe_insert("rotation_quaternion", frame=frame)
    return ob


def fg_feed_clip():
    """The feeding clip sampled at `FG_FEED_FPS` over its loop, in the model's axes and
    reference metres (anchor at the origin): each bone's name, parent and pivot (its
    head), and per frame per bone its turn about its pivot relative to its parent (a 3x3
    rotation) and the pivot's displacement (metres; zero for this armature's pure turns)."""
    return sample_clip(fg_feed_armature(), FG_BONES, round(FG_FEED_LOOP_S * FG_FEED_FPS), FG_FEED_FPS)


def rig_armature(name, bones, fps):
    """An armature of `bones` ((name, parent index, head, tail), reference metres, Blender
    axes) in the current scene, with an empty action, its bones in edit order."""
    sc = bpy.context.scene
    sc.render.fps = fps
    arm = bpy.data.armatures.new(name)
    ob = bpy.data.objects.new(name, arm)
    sc.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.mode_set(mode="EDIT")
    for bname, parent, head, tail in bones:
        eb = arm.edit_bones.new(bname)
        eb.head, eb.tail = Vector(head), Vector(tail)
        if parent is not None:
            eb.parent = arm.edit_bones[bones[parent][0]]
    bpy.ops.object.mode_set(mode="OBJECT")
    ob.animation_data_create()
    ob.animation_data.action = bpy.data.actions.new(name + "_feed")
    return ob


def sample_clip(ob, bones, n, fps):
    """Armature `ob`'s action sampled at frames 0..n-1 (and removed): in the model's axes
    and reference metres (anchor at the origin), each bone's name, parent and pivot (its
    head), and per frame per bone its turn about its pivot relative to its parent (a 3x3
    rotation) and the pivot's displacement (metres)."""
    sc = bpy.context.scene
    heads = [SWAP_YZ.to_3x3() @ Vector(h) for _, _, h, _ in bones]
    frames = []
    for f in range(n):
        sc.frame_set(f)
        bpy.context.view_layer.update()
        deform = [ob.pose.bones[name].matrix @ ob.data.bones[name].matrix_local.inverted()
                  for name, _, _, _ in bones]
        keys = []
        for i, (_, parent, _, _) in enumerate(bones):
            local = deform[i] if parent is None else deform[parent].inverted() @ deform[i]
            m = SWAP_YZ @ local @ SWAP_YZ
            r = m.to_3x3()
            d = r @ heads[i] + m.translation - heads[i]
            keys.append((r, d))
        frames.append(keys)
    bpy.data.objects.remove(ob, do_unlink=True)
    return {"bones": [(name, parent) for name, parent, _, _ in bones], "heads": heads,
            "fps": fps, "frames": frames}


def fg_feed_pose(clip, frame, w):
    """Each bone's deformation at sampled `frame` with clip weight `w` (0 rest, 1 the
    clip), model axes, reference metres: each bone's turn eased from none by a slerp and
    its displacement by `w`, about its own pivot, composed down the chain. The renderer's
    `rig::AnimalRig::pose` is the same sum."""
    out = []
    for i, (_, parent) in enumerate(clip["bones"]):
        r, d = clip["frames"][frame][i]
        h = clip["heads"][i]
        q = Quaternion().slerp(r.to_quaternion(), w)
        rw = q.to_matrix().to_4x4()
        local = Matrix.Translation(h + d * w) @ rw @ Matrix.Translation(-h)
        out.append(local if parent is None else out[parent] @ local)
    return out


# --- the Veilhound's rig and feeding clip (FEED-RIG round 2, 2026-10-09) -----------------
#
# `design/handoffs/feeding-motion-rig-2026-10-09.md` §Round 2: the chorister (Veilhound)
# feeding at a carcass on the ground; a placeholder, Wrysk directs the real clip. Three
# bones: the root (body, legs, tail), the neck (the spine loft's front, ahead of the
# shoulders, by position: `bone_ahead`), the head (skull, crest lobes, eyes, muzzle).
#
# As the frondgrazer's: the action is the **loop** in the reached posture, and the reach and
# return are the renderer's clip weight (`FG_FEED_EASE_S`). The reach: the muzzle's tip on
# the ground line about one voxel ahead of the front paws' claws. The neck and head alone
# are 0.41 m from the shoulder to the muzzle's tip, and that point is 0.75 m from it: with
# the root (and so the legs) still, the neck's pivot has to move. It moves the least it
# must, straight toward the point (the neck and head then nearly in line), and holds that
# displacement through the loop; the loop is the two turns (a two-bone reach to the tip's
# target) and the head's twist about its own length.

CH_NECK_CUT_X = 0.40                      # the shoulders: the front legs' hip joints
CH_TIP = (0.809, 0.0, 0.474)              # the muzzle's tip (its ellipsoid's front, ry(8))
CH_BONES = [
    ("root", None, (0.0, 0.0, 0.45), (0.1, 0.0, 0.45)),
    ("neck", 0, (CH_NECK_CUT_X, 0.0, 0.51), (0.50, 0.0, 0.52)),
    ("head", 1, (0.50, 0.0, 0.52), CH_TIP),
]
CH_FEED_FPS = 30
CH_FEED_LOOP_S = 1.8
# The front paws' claws reach x 0.82 (ankle 0.68 + 0.14); the target one voxel ahead,
# its tip 2 cm above the ground line. The reach leaves the chain 5 % short of straight so
# the loop's targets bend it one way.
CH_FRONT_CLAW_X = 0.82
CH_TARGET = (CH_FRONT_CLAW_X + 0.125, 0.02)
CH_SLACK = 0.95
# The loop's key poses: (seconds, the tip's target moved (x back -, z up +) metres, the
# head's twist degrees): tear (back and up 4 cm, a twist) 0.4 s, dip 0.3 s, the other
# side's tear 0.4 s, the swallow (lifted 8 cm) 0.4 s, dip 0.3 s.
CH_FEED_KEYS = [(0.0, 0.0, 0.0, 0.0), (0.4, -0.028, 0.028, 15.0), (0.7, 0.0, 0.0, 0.0),
                (1.1, -0.028, 0.028, -15.0), (1.5, 0.0, 0.08, 0.0), (1.8, 0.0, 0.0, 0.0)]


def ch_neck_shift():
    """The neck pivot's displacement (x, z) the reach needs: toward the target, by what
    the straight chain is short of it (with `CH_SLACK`)."""
    pn = Vector((CH_BONES[1][2][0], CH_BONES[1][2][2]))
    ph = Vector((CH_BONES[2][2][0], CH_BONES[2][2][2]))
    tip = Vector((CH_TIP[0], CH_TIP[2]))
    reach = ((ph - pn).length + (tip - ph).length) * CH_SLACK
    to = Vector(CH_TARGET) - pn
    return to.normalized() * max(to.length - reach, 0.0)


def ch_feed_angles(dx, dz):
    """The neck's and the head's turns about the side axis, radians counter-clockwise in
    the (x, z) plane (+ lifts the nose), each relative to its parent, putting the muzzle's
    tip on the target moved by (dx, dz), the neck's pivot shifted by `ch_neck_shift`."""
    shift = ch_neck_shift()
    pn = Vector((CH_BONES[1][2][0], CH_BONES[1][2][2]))
    ph = Vector((CH_BONES[2][2][0], CH_BONES[2][2][2]))
    tip = Vector((CH_TIP[0], CH_TIP[2]))
    p0 = pn + shift
    target = Vector(CH_TARGET) + Vector((dx, dz))
    l1, l2 = (ph - pn).length, (tip - ph).length
    to = target - p0
    dist = min(max(to.length, abs(l1 - l2) + 1e-6), l1 + l2 - 1e-6)
    base = math.atan2(to.y, to.x)
    bend = math.acos(max(-1.0, min(1.0, (l1 * l1 + dist * dist - l2 * l2) / (2 * l1 * dist))))
    joints = [p0 + Vector((math.cos(base + s * bend), math.sin(base + s * bend))) * l1 for s in (1, -1)]
    joint = max(joints, key=lambda j: j.y)  # the neck arched, its joint on the upper side
    ang = lambda v: math.atan2(v.y, v.x)  # noqa: E731
    a1 = ang(joint - p0) - ang(ph - pn)
    a2 = ang(target - joint) - ang(tip - ph) - a1
    return a1, a2


def ch_feed_armature():
    """The chorister's armature and its feeding action, in the current scene at reference
    size, standing at the origin as `chorister()` draws: keyed at `CH_FEED_KEYS` (Bezier,
    eased), the neck's pivot displaced by `ch_neck_shift` throughout."""
    ob = rig_armature("ch_rig", CH_BONES, CH_FEED_FPS)
    sx, sz = ch_neck_shift()
    shift = Vector((sx, 0.0, sz))
    head_axis = (Vector(CH_TIP) - Vector(CH_BONES[2][2])).normalized()
    for t, dx, dz, twist in CH_FEED_KEYS:
        frame = t * CH_FEED_FPS
        a1, a2 = ch_feed_angles(dx, dz)
        turns = {"root": Matrix.Identity(3),
                 "neck": Matrix.Rotation(-a1, 3, "Y"),
                 # the twist about the head's own length, then its pitch
                 "head": Matrix.Rotation(-a2, 3, "Y") @ Matrix.Rotation(math.radians(twist), 3, head_axis)}
        for name, _, _, _ in CH_BONES:
            pb = ob.pose.bones[name]
            pb.rotation_mode = "QUATERNION"
            # a turn about the bone's head in armature space (relative to its parent), as
            # the bone's own basis: Q^-1 R Q, its rest orientation Q; a displacement D as
            # its location Q^-1 D
            q = pb.bone.matrix_local.to_quaternion()
            pb.rotation_quaternion = q.inverted() @ turns[name].to_quaternion() @ q
            pb.keyframe_insert("rotation_quaternion", frame=frame)
            if name == "neck":
                pb.location = q.inverted() @ shift
                pb.keyframe_insert("location", frame=frame)
    return ob


def ch_feed_clip():
    """The chorister's feeding clip sampled at `CH_FEED_FPS` over its loop (`sample_clip`)."""
    return sample_clip(ch_feed_armature(), CH_BONES, round(CH_FEED_LOOP_S * CH_FEED_FPS), CH_FEED_FPS)


# --- littershredder · long tapered crawler (0.375 x 0.125 x 0.125 m) -----------------------
#
# Pick: art/gen/artbook/littershredder.png (the second, tapered version), side view facing
# left: a blunt plough, six banded plates and a tail cone, seven leg pairs, two feelers,
# one magenta spot on the second plate. 0.00051 m/px along, 0.00044 m/px up.

def littershredder():
    kx, kz = 0.00051, 0.00044

    def X(px):
        return (392 - px) * kx

    def Z(py):
        return (380 - py) * kz
    # the plough: a blunt wedge, low at its front lip and highest at its rear
    plough = [(30, 315, 335, 0.03), (60, 270, 338, 0.045), (110, 190, 340, 0.055), (170, 120, 336, 0.06),
              (230, 98, 330, 0.0625), (285, 108, 322, 0.06)]
    loft([(X(px), 0.0, (Z(t) + Z(b)) / 2, w, (Z(t) - Z(b)) / 2) for px, t, b, w in plough],
         [c("a_main")], lambda f, s, p: 0, nring=40, nseg=20)
    # the plates: gold, each with a brown band across its rear third, tapering to the tail
    plates = [(330, 112, 316, 0.06), (430, 135, 313, 0.057), (525, 155, 310, 0.052),
              (622, 182, 305, 0.046), (692, 210, 297, 0.038), (735, 232, 288, 0.03)]
    lx = 0.032
    for px, t, b, w in plates:
        zc, rzz = (Z(t) + Z(b)) / 2, (Z(t) - Z(b)) / 2
        ell((X(px) + 0.1 * lx, 0, zc), (0.9 * lx, w, rzz), c("a_main"))
        ell((X(px) - 0.55 * lx, 0, zc), (0.45 * lx, w * 0.93, rzz * 0.93), c("a_dark"))
    cyl((X(745), 0, Z(258)), (X(768), 0, Z(250)), 0.012, 0.002, mat=c("a_main"), seg=10)   # tail cone
    # the pale underside the plates overlap
    ell((X(470), 0, 0.026), (0.13, 0.04, 0.014), c("a_pale"))
    # seven pairs of short legs: pale, a gold knee, a hooked claw
    for px in (160, 230, 300, 395, 495, 590, 690):
        x0 = X(px)
        for sy in (-1, 1):
            hip = (x0, sy * 0.035, 0.028)
            knee = (x0 + 0.005, sy * 0.056, 0.018)
            ankle = (x0 + 0.009, sy * 0.06, 0.009)
            tube([hip, knee], 0.0075, 0.007, c("a_pale"), seg=8)
            ell(knee, (0.007, 0.007, 0.007), c("a_limb"))
            tube([knee, ankle], 0.0068, 0.006, c("a_pale"), seg=8)
            tube([ankle, (x0 + 0.006, sy * 0.061, 0.0)], 0.0055, 0.0015, c("a_foot"), seg=6)
    # two pale feelers curling down from under the plough's lip
    for sy in (-1, 1):
        tube([(X(70), sy * 0.012, Z(330)), (X(45), sy * 0.016, Z(352)), (X(32), sy * 0.018, Z(372)),
              (X(38), sy * 0.018, Z(384))], 0.004, 0.0025, c("a_pale"), seg=6)
    # one magenta sensor spot on the second plate, near its top
    ell((X(307), 0.0, Z(118)), (0.008, 0.006, 0.004), c("magenta"))


# --- ripple-snail · organic azure dome (0.19 x 0.19 x 0.125 m) -----------------------------
#
# Pick: art/gen/artbook/ripple-snail-organic.png, 3/4 view facing right: a matte dome with
# two shallow growth ridges, a pale wavy mantle edge, a dark spreading foot, an open front
# with two pink-tipped palps. 0.00035 m/px.

def ripple_snail():
    # the dark foot, spreading under the shell and trailing behind it
    ell((-0.03, 0, 0.009), (0.12, 0.104, 0.011), c("a_dark"))
    ell((-0.115, 0.015, 0.006), (0.04, 0.07, 0.007), c("a_dark"))
    ell((-0.07, -0.08, 0.005), (0.04, 0.035, 0.006), c("a_dark"))
    # the pale mantle edge: a flared, wavy skirt under the shell's rim
    ell((-0.008, 0, 0.037), (0.1, 0.096, 0.01), c("a_pale"))
    for k in range(26):
        a = 2 * math.pi * k / 26
        ell((-0.008 + 0.1 * math.cos(a), 0.096 * math.sin(a), 0.034 + 0.005 * math.sin(3 * a)),
            (0.012, 0.02, 0.0045), c("a_pale"), rot=rz(math.degrees(a)))
    # the dome, and its two growth ridges (grooves) near the top
    ell((-0.012, 0, 0.045), (0.088, 0.084, 0.078), c("a_main"))
    for zf in (0.62, 0.88):
        rr = math.sqrt(1 - zf ** 2)
        z = 0.045 + 0.078 * zf
        for k in range(28):
            a0, a1 = 2 * math.pi * k / 28, 2 * math.pi * (k + 1) / 28
            cyl((-0.012 + 0.0885 * rr * math.cos(a0), 0.0845 * rr * math.sin(a0), z),
                (-0.012 + 0.0885 * rr * math.cos(a1), 0.0845 * rr * math.sin(a1), z), 0.0028,
                mat=c("a_mark"), seg=6)
    # the open front: a dark opening under the rim, two palps reaching forward and down
    ell((0.076, 0, 0.033), (0.016, 0.034, 0.011), c("a_head"))
    for sy, reach in ((-1, 1.0), (1, 0.85)):
        root = (0.07, sy * 0.014, 0.034)
        mid = (0.095, sy * 0.024 * reach, 0.022)
        tip = (0.112 * reach + 0.012 * (1 - reach), sy * 0.03, 0.011)
        tube([root, mid, tip], 0.0065, 0.006, c("a_limb"), seg=8)
        ell(tip, (0.014, 0.009, 0.0075), c("a_glow"), rot=rz(sy * 18))


# --- capgnawer · rose-mauve bark mantle (0.25 x 0.21 x 0.19 m) ------------------------------
#
# Pick: art/gen/artbook/capgnawer-rose-bark.png, front view a little to one side: two full
# rounded mantle lobes with a central cleft, bark streaks, pale spots and a short pale
# fringe; under the cleft six mandibles in three stacked pairs round a vertical mouth, four
# light pink sensor patches (large upper, small lower); six stout legs on pale toes.

def capgnawer():
    for sy in (-1, 1):
        cy, cz = sy * 0.058, 0.085
        start = len(L.ORG)

        def zone(f, s, p):
            # bark streaks: fine wavy bands running down the lobe from the cleft
            u = 110 * p.x + 12 * s + 2.5 * math.sin(70 * p.z + 40 * p.x)
            return 1 if math.sin(u) > 0.6 and s > -0.2 else 0
        stations = [(x, cy, cz, 0.08 * w, 0.09 * w, 0.02) for x, w in
                    ((0.125, 0.25), (0.11, 0.7), (0.07, 0.95), (0.0, 1.0), (-0.07, 0.95), (-0.11, 0.7),
                     (-0.125, 0.25))]
        loft(stations, [c("a_main"), c("a_mark")], zone, nring=110, nseg=64)
        # pale spots, scattered over the lobe
        for k in range(14):
            a = 0.35 + 2.4 * ((k * 0.618) % 1.0)          # up the lobe, cleft to outer edge
            x = 0.1 * (((k * 0.382) % 1.0) * 2 - 1)
            w = math.sqrt(max(0.0, 1 - (x / 0.125) ** 2))
            p = Vector((x, cy + sy * 0.08 * w * math.cos(a), cz + 0.09 * w * math.sin(a)))
            r = 0.006 if k % 3 else 0.009
            ell(p, (r, r, r * 0.6), c("a_glow"))
        # the short pale fringe round the lobe's outer and front edges
        for k in range(90):
            a = 2 * math.pi * k / 90
            ex, ey = 0.122 * math.cos(a), 0.08 * math.sin(a)
            if sy * ey < -0.035:
                continue                                    # the cleft side has none
            root = Vector((ex, cy + ey, cz - 0.004))
            out = Vector((math.cos(a), math.sin(a), 0)).normalized()
            cyl(root, root + out * 0.016 + Vector((0, 0, -0.01)), 0.003, 0.0012, mat=c("a_pale"), seg=4)
        # tilt the lobe outward about its inner edge, so the cleft stays high
        pivot = Matrix.Translation((0, sy * 0.005, cz))
        tilt = pivot @ rx(-sy * 16) @ pivot.inverted()
        for ob in L.ORG[start:]:
            ob.data.transform(tilt)
    ell((0.0, 0, 0.065), (0.11, 0.09, 0.055), c("a_limb"))           # the body under the mantle
    # the face under the cleft
    ell((0.105, 0, 0.062), (0.035, 0.05, 0.035), c("a_limb"))
    box((0.139, 0, 0.063), (0.006, 0.005, 0.042), c("a_dark"))       # the vertical mouth
    for z in (0.078, 0.064, 0.05):
        for s2 in (-1, 1):
            ell((0.138, s2 * 0.009, z), (0.008, 0.007, 0.0065), c("a_head"))
    for s2 in (-1, 1):
        ell((0.13, s2 * 0.032, 0.078), (0.007, 0.012, 0.011), c("magenta"))
        ell((0.133, s2 * 0.027, 0.048), (0.005, 0.007, 0.007), c("magenta"))
    # six short stout legs, four pale toes each
    for px in (0.07, -0.01, -0.09):
        for s2 in (-1, 1):
            foot = (px + 0.012, s2 * 0.105, 0.016)
            tube([(px, s2 * 0.06, 0.05), (px + 0.006, s2 * 0.09, 0.03), foot], 0.022, 0.018, c("a_limb"), seg=10)
            for t in range(4):
                ell((px + 0.03, s2 * 0.105 + (t - 1.5) * 0.009, 0.007), (0.009, 0.0055, 0.007), c("a_foot"))


# --- seedporter · moss-green glider, the selected image (upright, as pictured) ---------------
#
# Pick: art/gen/artbook/seedporter.png (not the compact-head alternate or the orchid study),
# front view: tall cranium, large dark eyes, open mouth, magenta cheek dots, a dorsal
# stripe; six limbs (small fruit-handling arms holding a fruit at the chest, larger arms
# carrying the flank membranes, rear gripping feet) and a long coiled prehensile tail.
# Built in the pick's own pose and proportions: 0.00128 m/px (0.5 m head to pelvis).

def seedporter():
    k = 0.00128

    def P(px, py, x=0.0):
        return Vector((x, (px - 250) * k, (760 - py) * k))
    hy = (235 - 250) * k
    # head: the tall cranium, a pale lower face and muzzle, the open mouth, the eyes
    ell(P(235, 115, 0.0), (0.07, 0.079, 0.092), c("a_main"))
    ell(P(248, 205, 0.02), (0.05, 0.058, 0.045), c("a_pale"))
    ell(P(255, 215, 0.05), (0.025, 0.032, 0.023), c("a_pale"))
    ell(P(255, 233, 0.07), (0.008, 0.018, 0.008), c("a_dark"))
    for ex, sl in ((218, 1), (285, -1)):
        ell(P(ex, 180, 0.05), (0.012, 0.028, 0.02), c("a_eye"), rot=rx(sl * 20))
        for d in range(3):  # three cheek dots under each eye
            ell(P(ex - 9 + d * 6, 197 + d * 2, 0.058), (0.003, 0.003, 0.003), c("magenta"))
    stripe = [Vector((0.065 * math.cos(a), hy, P(235, 115).z + 0.095 * math.sin(a)))
              for a in (math.radians(d) for d in (70, 90, 110, 140, 170))]
    tube(stripe, 0.008, 0.008, c("a_mark"), seg=6)
    # neck and torso: green back and sides, pale front
    tube([P(240, 250), P(235, 285)], 0.024, 0.026, c("a_main"), seg=10)
    ell(P(225, 320, -0.005), (0.045, 0.058, 0.07), c("a_main"))
    ell(P(225, 320, 0.012), (0.036, 0.048, 0.062), c("a_pale"))
    ell(P(232, 392, -0.005), (0.04, 0.046, 0.065), c("a_main"))
    ell(P(232, 392, 0.01), (0.032, 0.036, 0.058), c("a_pale"))
    tube([P(238, 260, -0.035), P(228, 330, -0.042), P(235, 420, -0.036)], 0.009, 0.009, c("a_mark"), seg=6)
    # the small fruit-handling arms, and the fruit held at the chest
    for sh, el_, hand in (((200, 258), (150, 300), (176, 274)), ((262, 268), (215, 300), (192, 282))):
        tube([P(*sh, 0.01), P(*el_, 0.045), P(*hand, 0.075)], 0.01, 0.008, c("a_main"), seg=8)
    ell(P(184, 272, 0.085), (0.026, 0.026, 0.028), c("a_fruit"))
    # the big arms, the membranes they carry down to the flanks, the hands' claws
    arms = {
        -1: ([(175, 240), (100, 165), (60, 95)], [(60, 95), (100, 165), (175, 240), (180, 325), (110, 305), (38, 215)],
             [((60, 95), (28, 50)), ((60, 95), (55, 40)), ((60, 95), (88, 45)), ((60, 95), (18, 95))]),
        1: ([(295, 248), (385, 262), (450, 190)], [(450, 190), (385, 262), (295, 248), (290, 335), (330, 405),
                                                   (415, 370), (448, 300)],
            [((450, 190), (482, 150)), ((450, 190), (500, 170)), ((450, 190), (505, 205)), ((450, 190), (470, 238))]),
    }
    for _, (arm, mem, claws) in arms.items():
        tube([P(*q, 0.005) for q in arm], 0.015, 0.011, c("a_main"), seg=8)
        sheet([tuple(P(*q, -0.012)) for q in mem], c("a_membrane"), th=0.005)
        for a, b in claws:
            tube([P(*a, 0.005), P(*b, 0.01)], 0.006, 0.002, c("a_foot"), seg=5)
    # the rear gripping feet
    for leg, claws in (([(190, 410), (140, 370), (150, 450), (165, 482)], [(150, 505), (172, 512), (190, 500)]),
                       ([(265, 432), (310, 495), (330, 540), (335, 575)], [(325, 610), (348, 607), (362, 590)])):
        tube([P(*q, 0.01) for q in leg], 0.024, 0.012, c("a_main"), seg=8)
        for q in claws:
            tube([P(*leg[-1], 0.012), P(*q, 0.015)], 0.006, 0.002, c("a_foot"), seg=5)
    # the long prehensile tail, coiling at its end, its dorsal stripe on the outside
    tail = [(250, 445), (266, 525), (282, 610), (260, 690), (205, 738), (130, 742), (68, 700), (52, 630),
            (82, 572), (135, 556), (172, 588), (165, 632), (130, 645), (112, 618)]
    pts = [P(*q, -0.02) for q in catmull(tail, 60)]
    n = len(pts)
    for i in range(n - 1):
        r = 0.022 + (0.006 - 0.022) * i / (n - 1)
        cyl(pts[i], pts[i + 1], r, r, mat=c("a_main"), seg=8)
    centre = P(118, 640, -0.02)
    for i in range(18, n - 1):
        r = 0.022 + (0.006 - 0.022) * i / (n - 1)
        o = [(p - centre).normalized() * r * 0.75 for p in (pts[i], pts[i + 1])]
        cyl(pts[i] + o[0], pts[i + 1] + o[1], r * 0.45, r * 0.45, mat=c("a_mark"), seg=6)
    # the pale underside of the tail, on the inside of the coil
    for i in range(18, n - 1, 2):
        r = 0.022 + (0.006 - 0.022) * i / (n - 1)
        ell(pts[i] - (pts[i] - centre).normalized() * r * 0.6, (r * 0.5, r * 0.5, r * 0.5), c("a_pale"))


# --- seedporter poses · stand, ground, glide (Wrysk 2026-10-01) ------------------------------
#
# One body in three poses, from Wrysk's picks among the Qwen 2.1 edits of the artbook image
# (art/gen/artbook/seedporter-{stand,ground,glide}.png; the rest in
# art/gen/runs/2026-10-01-seedporter-poses/). Wrysk's anatomy, which the pictures do not
# keep: the big clawed arms are the front pair; the membranes run from their wrists to the
# hind legs' knees; the small fruit-handling arms are the middle pair. Proportions are the
# pick builder's (0.5 m head to pelvis); the limbs keep their lengths in every pose (`ik`).

SP_ARM, SP_SMALL, SP_LEG = (0.16, 0.16), (0.07, 0.07), (0.13, 0.13)
SP_SIDE = {1: "l", -1: "r"}  # +y is the body's left (x forward, z up)
SP_HEAD = 1.25  # the head's scale: the pictures' heads are as wide as the shoulders


def _dir(az, el):
    a, e = math.radians(az), math.radians(el)
    return Vector((math.cos(e) * math.cos(a), math.cos(e) * math.sin(a), math.sin(e)))


def ik(root, target, a, b, pole):
    """Two-bone limb from root toward target, bending toward pole: (joint, end). Out of
    reach, the limb straightens toward the target."""
    root, target, pole = Vector(root), Vector(target), Vector(pole)
    d = target - root
    n = min(d.length, a + b - 1e-4)
    u = d.normalized()
    x = (a * a - b * b + n * n) / (2 * n)
    h = math.sqrt(max(a * a - x * x, 0.0))
    p = (pole - u * pole.dot(u)).normalized()
    return root + u * x + p * h, root + u * n


def sp_digits(root, d, up, n, length, claw, r):
    """n fingers or toes fanned about `up`, from root along d, each ending in a dark claw
    hooking down (against `up`)."""
    d, up = Vector(d).normalized(), Vector(up).normalized()
    for k in range(n):
        dk = Matrix.Rotation(math.radians((k - (n - 1) / 2) * 18), 3, up) @ d
        tip = root + dk * length
        tube([root, root + dk * length * 0.55 + up * 0.006, tip], r, r * 0.75, c("a_main"), seg=6)
        tube([tip, tip + dk * claw * 0.6 - up * claw * 0.8], r * 0.7, 0.0015, c("a_claw"), seg=5)


def sp_web(edge, free, th=0.006):
    """A membrane between its attached edge and its free edge (both run wrist -> knee): a
    ruled slab, both resampled to the same count, thickened along the boundary's normal."""
    n, rows = 24, 4
    A = [Vector(p) for p in catmull([tuple(q) for q in edge], n)]
    B = [Vector(p) for p in catmull([tuple(q) for q in free], n)]
    loop = A + B[::-1]
    nrm = Vector()
    for i, p in enumerate(loop):  # Newell's normal of the boundary
        q = loop[(i + 1) % len(loop)]
        nrm += Vector(((p.y - q.y) * (p.z + q.z), (p.z - q.z) * (p.x + q.x), (p.x - q.x) * (p.y + q.y)))
    nrm = nrm.normalized() * th / 2
    bm = bmesh.new()
    grid = [[A[i].lerp(B[i], r / rows) for i in range(n)] for r in range(rows + 1)]
    top = [[bm.verts.new(p + nrm) for p in row] for row in grid]
    bot = [[bm.verts.new(p - nrm) for p in row] for row in grid]
    for r in range(rows):
        for i in range(n - 1):
            bm.faces.new((top[r][i], top[r][i + 1], top[r + 1][i + 1], top[r + 1][i]))
            bm.faces.new((bot[r][i], bot[r + 1][i], bot[r + 1][i + 1], bot[r][i + 1]))
    rim = ([(0, i) for i in range(n)] + [(r, n - 1) for r in range(1, rows + 1)]
           + [(rows, i) for i in range(n - 2, -1, -1)] + [(r, 0) for r in range(rows - 1, -1, -1)])
    for (r0, i0), (r1, i1) in zip(rim, rim[1:] + rim[:1]):
        if (r0, i0) != (r1, i1):
            bm.faces.new((top[r0][i0], bot[r0][i0], bot[r1][i1], top[r1][i1]))
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-6)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return L.mk("web", bm, c("a_membrane"))


def sp_head(H, F, tilt, mouth_open):
    """The head about its cranium centre H, the face toward F: the tall cranium tipped back
    by `tilt` degrees, pale face and muzzle, big black almond eyes slanting up and out,
    three magenta dots under each, the mouth, the dark stripe over the top."""
    F = Vector(F).normalized()
    S = Vector((0, 0, 1)).cross(F).normalized()
    U = F.cross(S)

    def at(f, s, u):
        return H + F * f + S * s + U * u

    def frame(t, roll=0.0):
        t, ro = math.radians(t), math.radians(roll)
        f2, u2 = F * math.cos(t) + U * math.sin(t), U * math.cos(t) - F * math.sin(t)
        s2, u3 = S * math.cos(ro) + u2 * math.sin(ro), u2 * math.cos(ro) - S * math.sin(ro)
        return Matrix(((f2.x, s2.x, u3.x, 0), (f2.y, s2.y, u3.y, 0), (f2.z, s2.z, u3.z, 0), (0, 0, 0, 1)))

    k = SP_HEAD
    cr = at(0.01 * k, 0, 0.025 * k)
    ell(cr, (0.078 * k, 0.07 * k, 0.1 * k), c("a_main"), rot=frame(tilt))
    ell(at(0.04 * k, 0, -0.04 * k), (0.05 * k, 0.056 * k, 0.048 * k), c("a_pale"), rot=frame(0))
    ell(at(0.062 * k, 0, -0.07 * k), (0.022 * k, 0.03 * k, 0.022 * k), c("a_pale"), rot=frame(0))
    ell(at(0.082 * k, 0, -0.08 * k), (0.008 * k, 0.016 * k, (0.006 + 0.006 * mouth_open) * k), c("a_dark"), rot=frame(0))
    for s in (-1, 1):
        ell(at(0.07 * k, s * 0.036 * k, -0.012 * k), (0.016 * k, 0.03 * k, 0.022 * k), c("a_eye"), rot=frame(0, -s * 25))
        for d in range(3):
            ell(at((0.08 - d * 0.004) * k, s * (0.024 + d * 0.009) * k, (-0.044 - d * 0.004) * k), (0.0035 * k,) * 3,
                c("a_cheek"))
    t = math.radians(tilt)
    f2, u2 = F * math.cos(t) + U * math.sin(t), U * math.cos(t) - F * math.sin(t)
    tube([cr + f2 * 0.078 * k * math.cos(math.radians(a)) + u2 * 0.1 * k * math.sin(math.radians(a))
          for a in (40, 65, 90, 115, 140, 165, 190)], 0.009 * k, 0.008 * k, c("a_mark"), seg=6)


def sp_body(spec):
    """Draw the seedporter from a pose spec (see SP_POSES); its rig's frame (FEED-ROLLOUT,
    `sp_feed_clip`): the head's centre `H`, face `F` and up `U`, the neck's point (the
    head's pivot), each small arm's shoulder, elbow, wrist and pole by side, the fruit,
    every point where the built parts stand. Its parts are on rig bones: the head's on
    `head`, each small arm's upper half on `arm_<side>` and its lower half and fingers on
    `hand_<side>`, the fruit on `hand_l`, the rest on the root."""
    start = len(L.ORG)
    grip = []  # the big hands' and the hind feet's digits
    H, F = Vector(spec["head"]), _dir(*spec["face"])
    U = F.cross(Vector((0, 0, 1)).cross(F).normalized())
    nk = H - (F * 0.02 + U * 0.035) * SP_HEAD
    st = spec["spine"] + [(nk.x, nk.y, nk.z, 0.026, 0.026)]  # tail base -> neck -> into the head
    axis = [Vector(p[:3]) for p in catmull([s[:3] for s in st], 120)]

    def zone(f, s, p):
        i = min(range(len(axis)), key=lambda k: (axis[k] - p).length)
        t = axis[min(i + 1, len(axis) - 1)] - axis[max(i - 1, 0)]
        belly = Vector((t.z, 0.0, -t.x)).normalized()  # the spine's tangent turned down/forward
        d = p - axis[i]
        cosb = d.dot(belly) / max(d.length, 1e-9)
        return 1 if cosb > 0.2 else (2 if cosb < -0.93 else 0)
    loft([(x, y, z, ry, rz) for x, y, z, ry, rz in st], [c("a_main"), c("a_pale"), c("a_mark")], zone,
         nring=48, nseg=20)
    n0 = len(L.ORG)
    sp_head(H, F, spec["tilt"], spec["mouth"])
    bone_of(n0, "head")
    frame = {"H": H, "F": F, "U": U, "neck": nk, "arms": {}}
    for s in (-1, 1):
        def m(v):
            return Vector((v[0], s * v[1], v[2]))
        # the big front arms, and their hands
        sh = m(spec["shoulder"])
        el, wr = ik(sh, m(spec["hand"]), *SP_ARM, m(spec["arm_pole"]))
        tube([sh, el, wr], 0.016, 0.011, c("a_main"), seg=8)
        n0 = len(L.ORG)
        sp_digits(wr, m(_dir(*spec["fingers"])), m(spec["finger_up"]), 4, 0.045, 0.022, 0.0055)
        grip += L.ORG[n0:]
        # the small middle arms, holding the fruit
        ss = m(spec["small_shoulder"])
        se, sw = ik(ss, m(spec["small_hand"]), *SP_SMALL, m(spec["small_pole"]))
        side = SP_SIDE[s]
        tube([ss, se, sw], 0.011, 0.008, c("a_main"), seg=8, bones=[f"arm_{side}", f"hand_{side}"])
        n0 = len(L.ORG)
        sp_digits(sw, (spec["fruit"] - sw).normalized(), Vector((0, 0, 1)), 3, 0.022, 0.01, 0.004)
        bone_of(n0, f"hand_{side}")
        frame["arms"][side] = (ss, se, sw, m(spec["small_pole"]))
        # the hind legs, and their feet
        hp = m(spec["hip"])
        kn, an = ik(hp, m(spec["ankle"]), *SP_LEG, m(spec["leg_pole"]))
        tube([hp, kn], 0.026, 0.017, c("a_main"), seg=8)
        tube([kn, an], 0.016, 0.012, c("a_main"), seg=8)
        n0 = len(L.ORG)
        sp_digits(an, m(_dir(*spec["toes"])), m(spec["toe_up"]), 4, 0.05, 0.022, 0.0065)
        grip += L.ORG[n0:]
        # the membrane: wrist, along the arm, down the flank, the thigh to the knee; its
        # free edge back from the knee to the wrist
        side = [m(q) for q in spec["flank"]]
        edge = [wr, el, sh] + side + [hp, kn]
        free = spec["free"](kn, wr, s)
        sp_web(edge, [wr] + free + [kn])
    # the fruit rides in the hands (FEED-ROLLOUT: on the left hand's bone; the clip moves
    # both hands alike)
    n0 = len(L.ORG)
    ell(spec["fruit"], (0.028, 0.028, 0.03), c("a_fruit"))
    bone_of(n0, "hand_l")
    frame["fruit"] = Vector(spec["fruit"])
    # the long prehensile tail, its dorsal stripe along the outside
    pts = [Vector(p) for p in catmull([st[0][:3]] + spec["tail"], 70)]
    n = len(pts)
    r0, r1 = spec.get("tail_r", (0.026, 0.005))
    for i in range(n - 1):
        r = r0 + (r1 - r0) * i / (n - 1)
        cyl(pts[i], pts[i + 1], r, r, mat=c("a_main"), seg=8)
        o = spec["tail_up"](pts[i], i / (n - 1))
        t = (pts[i + 1] - pts[i]).normalized()
        o = (o - t * o.dot(t)).normalized() * r * 0.75
        cyl(pts[i] + o, pts[i + 1] + o, r * 0.45, r * 0.45, mat=c("a_mark"), seg=6)
    if spec.get("feet_on_ground") is not None:
        # z = 0 under the hands and feet (the perch), the tail hanging below it; the feet
        # stand that far above it, inside the ground row of cells
        low = min(v.co.z for ob in grip if ob.type == "MESH" and slot_of(ob.data.materials[0].name) != "a_claw"
                  for v in ob.data.vertices)
        shift = Vector((0, 0, spec["feet_on_ground"] - low))
        for ob in L.ORG[start:]:
            ob.data.transform(Matrix.Translation(shift))
        frame = sp_shifted(frame, shift)
    return frame


def sp_shifted(frame, d):
    """`sp_body`'s frame moved by `d` (the perch's lift)."""
    out = {k: (v + d if k in ("H", "neck", "fruit") else v) for k, v in frame.items() if k != "arms"}
    out["arms"] = {k: (a + d, b + d, w + d, pole) for k, (a, b, w, pole) in frame["arms"].items()}
    return out


def _spiral(centre, plane, a0, a1, r0, r1, n=24):
    """Points on a spiral about centre in a plane (two unit axes), angle a0 -> a1 degrees."""
    u, v = (Vector(q) for q in plane)
    out = []
    for k in range(n + 1):
        t = k / n
        a = math.radians(a0 + (a1 - a0) * t)
        r = r0 + (r1 - r0) * t
        out.append(tuple(Vector(centre) + u * r * math.cos(a) + v * r * math.sin(a)))
    return out


def _sag(down, depth, bumps=0.0):
    """A free edge between knee and wrist, hanging `depth` along `down` at its middle, with
    `bumps` deep scallops between three slack points."""
    def free(kn, wr, s):
        out = []
        dv = Vector((down[0], s * down[1], down[2]))
        for t in (0.8, 0.6, 0.4, 0.2):  # wrist -> knee
            out.append(kn.lerp(wr, t) + dv * (depth * math.sin(math.pi * t) + bumps * abs(math.sin(2.5 * math.pi * t))))
        return out
    return free


SP_POSES = {
    # perched upright on a branch (the branch is not drawn), eating the fruit; the big arms
    # straight down to the perch outside the crouched knees, the membranes folded between
    # them; the tail hanging below the perch into a coil seen from the front. The perch is
    # z = 0 (Wrysk 2026-10-01: ground level under its feet), so the tail's cells go below it.
    "stand": dict(
        spine=[(-0.03, 0, 0.43, 0.03, 0.03), (0.0, 0, 0.47, 0.045, 0.042), (0.015, 0, 0.56, 0.036, 0.034),
               (0.03, 0, 0.65, 0.05, 0.045), (0.035, 0, 0.72, 0.03, 0.028)],
        head=(0.05, 0, 0.87), face=(0, -8), tilt=12, mouth=1.0,
        shoulder=(0.03, 0.055, 0.69), hand=(0.1, 0.13, 0.39), arm_pole=(-0.3, 1, 0),
        fingers=(15, -50), finger_up=(0.5, 0, 1),
        small_shoulder=(0.06, 0.035, 0.6), small_hand=(0.14, 0.028, 0.735), small_pole=(0, 1, -0.6),
        fruit=Vector((0.15, 0, 0.74)),
        hip=(-0.005, 0.045, 0.45), ankle=(0.05, 0.08, 0.39), leg_pole=(1, 0.5, 0.6),
        toes=(10, -30), toe_up=(0.4, 0, 1),
        flank=[(0.025, 0.05, 0.64), (0.012, 0.038, 0.55)],
        free=_sag((-0.5, 0.8, -0.3), 0.07, 0.015),
        tail=[(-0.08, 0, 0.37), (-0.1, -0.015, 0.27), (-0.085, -0.01, 0.17)]
             + _spiral((-0.07, 0.085, 0.085), ((0, 1, 0), (0, 0, 1)), 180, 520, 0.085, 0.022),
        tail_up=lambda p, f: (p - Vector((-0.07, 0.085, 0.085))) if f > 0.4 else Vector((-1, 0, 0)),
        # 0.1 m keeps the cells of the look Wrysk approved (2026-10-01, r6): the face row
        # pale | eye | pale, the feet in the ground row
        feet_on_ground=0.1,
    ),
    # walking on all fours, the big arms as front legs, the small arms holding the fruit at
    # the chest, the membranes draped along the flanks; the tail behind, curling up into a
    # coil seen from the side
    "ground": dict(
        spine=[(-0.3, 0, 0.29, 0.03, 0.03), (-0.24, 0, 0.29, 0.045, 0.042), (-0.09, 0, 0.29, 0.036, 0.034),
               (0.06, 0, 0.295, 0.05, 0.046), (0.12, 0, 0.34, 0.03, 0.028)],
        head=(0.2, 0, 0.49), face=(0, -8), tilt=45, mouth=0.0,
        shoulder=(0.08, 0.055, 0.31), hand=(0.15, 0.09, 0.03), arm_pole=(-1, 0.4, 0),
        fingers=(25, -8), finger_up=(0, 0, 1),
        small_shoulder=(0.1, 0.035, 0.26), small_hand=(0.18, 0.026, 0.28), small_pole=(0, 1, -0.5),
        fruit=Vector((0.19, 0, 0.28)),
        hip=(-0.24, 0.045, 0.27), ankle=(-0.225, 0.11, 0.035), leg_pole=(1, 0.6, 0.5),
        toes=(20, -8), toe_up=(0, 0, 1),
        flank=[(0.06, 0.05, 0.295), (-0.09, 0.036, 0.29)],
        free=_sag((0, 0.3, -1), 0.1, 0.012),
        # thin, and level on the cell row z = 0.3125 so it is one voxel thick, ending in a
        # small upward curl
        tail=[(-0.4, 0, 0.3125), (-0.52, 0, 0.3125), (-0.64, 0, 0.3125), (-0.72, 0, 0.3125)]
             + _spiral((-0.75, 0, 0.375), ((1, 0, 0), (0, 0, 1)), -90, -400, 0.0625, 0.02),
        tail_r=(0.016, 0.004),
        tail_up=lambda p, f: (p - Vector((-0.75, 0, 0.375))) if f > 0.7 else Vector((0, 0, 1)),
    ),
    # gliding, level and belly down: the big arms spread wide and the knees out, the
    # membranes taut from wrist to knee; the small arms hold the fruit under the chest; the
    # tail streams straight out behind
    "glide": dict(
        spine=[(-0.21, 0, 0.11, 0.03, 0.028), (-0.15, 0, 0.11, 0.045, 0.038), (-0.05, 0, 0.11, 0.036, 0.032),
               (0.05, 0, 0.115, 0.05, 0.042), (0.12, 0, 0.13, 0.03, 0.028)],
        head=(0.22, 0, 0.21), face=(0, -25), tilt=25, mouth=0.5,
        shoulder=(0.08, 0.05, 0.135), hand=(0.24, 0.35, 0.21), arm_pole=(-1, 0, 0.2),
        fingers=(40, -5), finger_up=(0, 0, 1),
        small_shoulder=(0.1, 0.035, 0.09), small_hand=(0.155, 0.022, 0.055), small_pole=(0, 1, -0.3),
        fruit=Vector((0.165, 0, 0.05)),
        hip=(-0.15, 0.045, 0.1), ankle=(-0.27, 0.2, 0.08), leg_pole=(0.4, 1, 0.2),
        toes=(160, -10), toe_up=(0, 0, 1),
        flank=[(0.05, 0.048, 0.115), (-0.05, 0.035, 0.11)],
        free=_sag((-0.5, -0.6, 0), 0.025, 0.02),
        tail=[(-0.35, 0.02, 0.105), (-0.5, -0.03, 0.105), (-0.65, -0.05, 0.11), (-0.8, -0.01, 0.12),
              (-0.9, 0.03, 0.13)],
        tail_up=lambda p, f: Vector((0, 0, 1)),
    ),
}


def seedporter_stand():
    sp_body(SP_POSES["stand"])


def seedporter_ground():
    sp_body(SP_POSES["ground"])


def seedporter_glide():
    sp_body(SP_POSES["glide"])


# --- the seedporter's feeding rig and clip (FEED-ROLLOUT, 2026-10-10) ---------------------
#
# `design/handoffs/feed-rollout-2026-10-10.md` §Seedporter; a placeholder, Wrysk directs the
# real clip. It crops perched in a crown (`seedporter-stand`) and, at a vine face, on the
# ground (`seedporter-ground`); both pose models carry this rig. Bones: the root (body, big
# arms, legs, membranes, tail), the head (no jaw: the mouth is a dark patch), each small
# fruit arm an upper and a lower bone (the hand's fingers on the lower; the fruit rides on
# the left hand). The loop starts with the fruit at the mouth and the head dipped:
#
#   reach  0.4 s  both hands up and forward toward a fruit ahead and above (`SP_REACH_EL`
#                 above the forward line, as far as the 0.14 m arms reach: they are short
#                 of a body length), the head tipping up `SP_TIP_DEG`;
#   bring  0.4 s  back to the mouth, the head dipping to meet them (`sp_dip`: at least
#                 `SP_DIP_DEG`, more where the arms fall short of the mouth);
#   chew   0.5 s  two nods of `SP_CHEW_DEG`, the arms held in.
#
# The fingers have no bones: the hands cannot be seen to close. The arms are each solved
# per sampled frame (`ik`, the pose's own elbow pole) and keyed as turns.

SP_FEED_FPS = 30
SP_REACH_S, SP_BRING_S, SP_CHEW_S = 0.4, 0.4, 0.5
SP_FEED_LOOP_S = SP_REACH_S + SP_BRING_S + SP_CHEW_S
SP_REACH_EL = 45.0   # the reach's direction above the body's forward line, degrees
SP_TIP_DEG = 20.0    # the head tipped up at the reach
SP_DIP_DEG = 10.0    # the head dipped at the mouth, at least
SP_CHEW_DEG = 10.0   # each nod of the chew, up from the dip
SP_SLACK = 0.95      # the arm's most extension (shoulder to wrist over its two bones)


def sp_bones(fr):
    """The rig's bones (name, parent index, head, tail) from `sp_body`'s frame."""
    pelvis = fr["neck"] - fr["U"] * 0.3
    bones = [("root", None, tuple(pelvis), tuple(pelvis + Vector((0.1, 0, 0)))),
             ("head", 0, tuple(fr["neck"]), tuple(fr["H"]))]
    for side in ("l", "r"):
        ss, se, sw, _ = fr["arms"][side]
        n = len(bones)
        bones += [(f"arm_{side}", 0, tuple(ss), tuple(se)), (f"hand_{side}", n, tuple(se), tuple(sw))]
    return bones


def sp_mouth(fr, deg):
    """The point the fruit is brought to with the head pitched `deg` (+ up) about its
    pivot: just ahead of the mouth (`sp_head`'s mouth patch)."""
    k = SP_HEAD
    rest = fr["H"] + fr["F"] * (0.082 * k + 0.02) - fr["U"] * 0.08 * k
    return fr["neck"] + Matrix.Rotation(-math.radians(deg), 3, "Y") @ (rest - fr["neck"])


def sp_wrists(fr, fruit):
    """Each hand's wrist target with the fruit at `fruit`: the rest pose's wrist-to-fruit
    offset kept."""
    return {side: fruit + (sw - fr["fruit"]) for side, (_, _, sw, _) in fr["arms"].items()}


def sp_reachable(fr, fruit):
    return all((w - fr["arms"][side][0]).length <= SP_SLACK * sum(SP_SMALL)
               for side, w in sp_wrists(fr, fruit).items())


def sp_dip(fr):
    """The head's dip at the mouth: `SP_DIP_DEG`, or the least more (to 40 degrees) at
    which both hands reach the mouth."""
    for d in range(int(SP_DIP_DEG), 41):
        if sp_reachable(fr, sp_mouth(fr, -d)):
            return float(d)
    return 40.0


def sp_reach_point(fr):
    """Where the fruit is held at the reach: from between the shoulders, along the reach's
    direction, as far as the arms go."""
    mid = sum((a[0] for a in fr["arms"].values()), Vector()) / 2
    el = math.radians(SP_REACH_EL)
    d = Vector((math.cos(el), 0, math.sin(el)))
    return mid + d * SP_SLACK * sum(SP_SMALL) + (fr["fruit"] - sum((a[2] for a in fr["arms"].values()), Vector()) / 2)


def sp_feed_at(fr, dip, t):
    """The fruit's place and the head's pitch (+ up, degrees) at `t` seconds into the loop."""
    sm = lambda x: x * x * (3 - 2 * x)  # noqa: E731
    mouth, reach = sp_mouth(fr, -dip), sp_reach_point(fr)
    if t < SP_REACH_S:
        u = sm(t / SP_REACH_S)
        return mouth.lerp(reach, u), -dip + (SP_TIP_DEG + dip) * u
    t -= SP_REACH_S
    if t < SP_BRING_S:
        u = sm(t / SP_BRING_S)
        return reach.lerp(mouth, u), SP_TIP_DEG - (SP_TIP_DEG + dip) * u
    t -= SP_BRING_S
    return mouth, -dip + SP_CHEW_DEG * math.sin(2 * math.pi * t / SP_CHEW_S) ** 2


def sp_arm_turns(fr, side, wrist):
    """The small arm's two turns (armature space, each relative to its parent) putting its
    wrist on `wrist`, the elbow toward the pose's pole."""
    ss, se, sw, pole = fr["arms"][side]
    e, w = ik(ss, wrist, *SP_SMALL, pole)
    up = (se - ss).rotation_difference(e - ss)
    low = (sw - se).rotation_difference(up.inverted() @ (w - e))
    return up.to_matrix(), low.to_matrix()


def sp_feed_clip(pose):
    """The feeding loop of pose model `seedporter-<pose>` at reference size, sampled at
    `SP_FEED_FPS` (`sample_clip`)."""
    use(f"seedporter-{pose}")
    fr = sp_body(SP_POSES[pose])
    bones = sp_bones(fr)
    dip = sp_dip(fr)
    n = round(SP_FEED_LOOP_S * SP_FEED_FPS)
    ob = rig_armature(f"sp_{pose}_rig", bones, SP_FEED_FPS)
    for f in range(n):
        fruit, pitch = sp_feed_at(fr, dip, f / SP_FEED_FPS)
        turns = {"head": Matrix.Rotation(-math.radians(pitch), 3, "Y")}
        for side, wrist in sp_wrists(fr, fruit).items():
            turns[f"arm_{side}"], turns[f"hand_{side}"] = sp_arm_turns(fr, side, wrist)
        key_turns(ob, bones, f, turns, Vector())
    print(f"seedporter-{pose} feed: dip {dip:.0f} deg, reach {(sp_reach_point(fr) - fr['fruit']).length:.3f} m"
          f" from the rest fruit, mouth {(sp_mouth(fr, -dip) - fr['fruit']).length:.3f} m", flush=True)
    return sample_clip(ob, bones, n, SP_FEED_FPS)


# --- bellwing · BW-A with the thin corkscrew tendril (wingspan 0.375 m) ---------------------
#
# Pick: art/gen/artbook/bellwing.png, front view: the hanging plum bell with its magenta
# rim and underside, four lilac wings (a larger upper and a smaller lower pair) with cyan
# leading edges on mauve roots, and the fine muted-peach tendril ending in a 3D corkscrew.
# Original colours. 0.000556 m/px (wingspan 0.375 m); the tendril's tip on z = 0.

def bellwing():
    k = 0.000556

    def P(px, py, x=0.0):
        return Vector((x, (px - 385) * k, (745 - py) * k))
    rim_z = P(0, 540).z

    def bell_zone(f, s, p):
        return 1 if p.z < rim_z else 0
    prof = [(232, 4), (242, 40), (262, 70), (300, 82), (360, 90), (420, 98), (480, 112), (515, 132),
            (545, 155), (572, 160), (590, 150)]
    loft([(0.0, 0.0, P(0, py).z, r * k, r * k) for py, r in prof], [c("a_main"), c("magenta")], bell_zone,
         nring=50, nseg=28)
    # wings: root (px), tip (px), front (+x) or back pair, which side leads
    wings = [((300, 258), (40, 30), 0.012, 1), ((470, 285), (705, 150), 0.012, 1),
             ((300, 292), (90, 382), -0.012, -1), ((470, 320), (615, 440), -0.012, -1)]
    for root, tip, x, lead in wings:
        R, T = P(*root, x), P(*tip, x)
        d = T - R
        n = Vector((0, -d.z, d.y)).normalized()
        if n.z * lead < 0:
            n = -n
        w = 0.17 * d.length
        lead_pts = [R + d * t + n * w * math.sin(math.pi * t) ** 0.8 for t in (i / 12 for i in range(13))]
        trail_pts = [R + d * t - n * 0.55 * w * math.sin(math.pi * t) for t in (i / 12 for i in range(11, 0, -1))]
        sheet([tuple(p) for p in lead_pts + trail_pts], c("a_membrane"), th=0.003)
        tube(lead_pts[1:], 0.0035, 0.0025, c("a_glow"), seg=6)
        ell(R, (0.016, 0.018, 0.016), c("a_limb"))
    # the tendril: out of the bell's mouth, then a spindly corkscrew to its tip
    tube([P(385, 500), P(382, 560), P(392, 600)], 0.0025, 0.0022, c("a_tendril"), seg=6)
    top = P(392, 600)
    helix((top.x, top.y - 0.008, top.z), top.z, 3.2, 0.008, 0.002, c("a_tendril"), phase=0.0)


# --- the table ------------------------------------------------------------------------------

# --- lanternjaw · mantis scorpid with a forked lure tail (1.0 x 0.25 x 0.45 m) -------------
#
# Pick (Wrysk 2026-10-01): art/gen/artbook/lanternjaw.png, a front three-quarter view. A low plated scorpid: a helmeted head with two large
# magenta eyes and two small ones, pale curved fangs; a pair of mantis striking arms in front
# (thick femur raised, a long spined tibia curving down to a hook); six walking legs, knees
# high, dark joints, dark claw tips; six overlapping back plates over a dark body. The tail
# forks close to the body (Wrysk: the image only draws a second end beside the first) and the
# two branches arch forward side by side, each ending in a holder of three claws around a
# cyan lure orb. Wrysk's changes to the image (2026-10-01): the tails climb higher and reach
# further, the orbs hanging over and in front of the face; the body tapers and turns up into
# the tail early; the fangs are larger and vertical, a pale column in front of the face; the
# striking arms end in pale razor blades. Sizes are the founder's adult body (source.json):
# ~1.0 m from the blades to the tail root, the body 0.25 m wide (plates and legs reach past
# it). The tails' arch tops out near 0.6 m (5 voxels), above the 0.45 m height, so the curl
# reads.

# The orbs' cell column: 3 = one cell behind the fangs (Wrysk 2026-10-01), 2 = two cells
# behind (the variant he asked to see). `LJ_ORB_CELL=2` in the environment picks it.
LJ_ORB_CELL = int(os.environ.get("LJ_ORB_CELL", "3"))


def lanternjaw():
    # Voxel placement (the bake anchors the origin on a cell centre: cell (i, j, k) centres at
    # x = 0.125 i, y = 0.125 j, z = 0.125 k + 0.0625). The fangs contain the two centres of
    # cell column (4, 0, 0..1), the razor blades (4, +-2, 0), each orb the one cell
    # (LJ_ORB_CELL, +-1, 3), held from the cell behind it, open on top, in front and beside. The two tail branches run
    # on the cell rows y = +-0.125, thin enough that the row between them stays empty.
    def body_zone(f, s, p):
        return 0 if (p.x > 0.24 and s > -0.2) else 1    # violet helmet; the rest dark under plates
    body = [  # x, y, z centre, half-width, half-height up, down; tapering, the rear turning up
        (0.47, 0, 0.13, 0.035, 0.03, 0.03), (0.43, 0, 0.14, 0.10, 0.075, 0.055),
        (0.35, 0, 0.15, 0.125, 0.085, 0.06), (0.25, 0, 0.145, 0.125, 0.075, 0.06),
        (0.14, 0, 0.145, 0.135, 0.08, 0.06), (0.02, 0, 0.15, 0.13, 0.08, 0.062),
        (-0.10, 0, 0.16, 0.112, 0.074, 0.058), (-0.20, 0, 0.18, 0.09, 0.066, 0.052),
        (-0.28, 0, 0.21, 0.07, 0.058, 0.048), (-0.33, 0, 0.25, 0.058, 0.052, 0.045)]
    loft(body, [c("a_main"), c("a_dark")], body_zone, nring=60, nseg=20)
    # six overlapping back plates, narrowing and climbing with the body
    for x, hw, z, tilt in ((0.15, 0.15, 0.21, -14), (0.06, 0.152, 0.215, -14), (-0.03, 0.14, 0.222, -16),
                           (-0.12, 0.12, 0.232, -20), (-0.20, 0.098, 0.25, -26), (-0.27, 0.078, 0.278, -34)):
        ell((x, 0, z), (0.058, hw, 0.036), c("a_main"), rot=ry(tilt))
    # head: a central ridge, two large magenta eyes, two small ones
    ell((0.35, 0, 0.215), (0.1, 0.018, 0.026), c("a_main"), rot=ry(8))
    for sy in (-1, 1):
        ell((0.395, sy * 0.085, 0.18), (0.036, 0.03, 0.04), c("a_eye"))
        ell((0.435, sy * 0.028, 0.2), (0.014, 0.014, 0.014), c("a_eye"))
        # fangs: two large pale blades hanging vertically in front of the mouth, tips hooked in
        tube([(0.47, sy * 0.022, 0.24), (0.497, sy * 0.022, 0.135), (0.505, sy * 0.02, 0.035)], 0.04, 0.025,
             c("a_head"), seg=10)
        cyl((0.505, sy * 0.02, 0.035), (0.475, sy * 0.006, -0.005), 0.025, 0.003, mat=c("a_head"), seg=8)

    def jointed(pts, r0, r1, tip_r):
        """A thick armoured limb: straight segments with dark joint balls; the last segment
        is a long dark claw."""
        tube(pts[:-1], r0, r1, c("a_limb"), seg=10)
        for p in pts[1:-1]:
            ell(p, (r0 * 0.78,) * 3, c("a_dark"))
        cyl(pts[-2], pts[-1], tip_r, 0.003, mat=c("a_foot"), seg=8)

    for sy in (-1, 1):
        # striking arm: coxa under the head, a thick femur raised high and out, a tibia curving
        # down and forward into a pale razor blade with a hooked tip; pale razor spines inside
        hip, knee = (0.30, sy * 0.10, 0.10), (0.39, sy * 0.25, 0.29)
        tib = [knee, (0.47, sy * 0.275, 0.24), (0.52, sy * 0.268, 0.16)]
        tube([hip, knee], 0.056, 0.048, c("a_limb"), seg=10)
        ell(knee, (0.042,) * 3, c("a_dark"))
        tube(tib, 0.044, 0.034, c("a_limb"), seg=10)
        ell((0.515, sy * 0.255, 0.085), (0.035, 0.028, 0.085), c("a_head"), rot=ry(-10))
        cyl((0.51, sy * 0.25, 0.01), (0.475, sy * 0.23, 0.0), 0.02, 0.003, mat=c("a_head"), seg=8)
        T = catmull(tib, 21)
        for t in (0.2, 0.45, 0.7, 0.95):
            p = Vector(T[round(t * 20)])
            cyl(tuple(p), tuple(p + Vector((-0.024, -sy * 0.06, -0.014))), 0.016, 0.001, mat=c("a_head"), seg=6)
        # six walking legs: thick, knees above the plates, long dark claws splayed outward and
        # toward their own end
        for hx, dx in ((0.17, 0.08), (0.02, 0.0), (-0.14, -0.085)):
            jointed([(hx, sy * 0.12, 0.12), (hx + dx * 0.5, sy * 0.29, 0.28), (hx + dx, sy * 0.38, 0.11),
                     (hx + dx * 1.15, sy * 0.43, 0.0)], 0.046, 0.034, 0.032)

    ox = 0.125 * LJ_ORB_CELL  # the orbs' cell column (x of its centre)
    # tail: the rear turns up into one thick segment that forks at once; the two branches
    # climb, arch high over the back and come down in front, the orbs hanging over the face
    seg_tail([(-0.33, 0, 0.25), (-0.36, 0, 0.31)], 0.062, 0.058)
    for sy in (-1, 1):
        path = [(-0.36, sy * 0.03, 0.31), (-0.38, sy * 0.08, 0.40), (-0.34, sy * 0.11, 0.49),
                (-0.24, sy * 0.125, 0.53), (-0.10, sy * 0.125, 0.555), (0.06, sy * 0.125, 0.56),
                (ox - 0.205, sy * 0.125, 0.56), (ox - 0.14, sy * 0.125, 0.545)]
        seg_tail(catmull(path, 12), 0.056, 0.04)
        # the lure holder: the tip turns down behind the orb (Wrysk: the dark cell moves from
        # above the orb to just behind it), three claws gripping the orb's back half
        end = path[-1]
        hold = (ox - 0.11, sy * 0.125, 0.45)
        cyl(end, hold, 0.032, 0.028, mat=c("a_mark"), seg=10)
        orb = Vector((ox, sy * 0.125, 0.4375))
        ell(tuple(orb), (0.06,) * 3, c("a_lure"))
        for ang in (90, 210, 330):  # the claws grip the orb's back half only
            a = math.radians(ang)
            tip = orb + Vector((-0.045, math.cos(a) * 0.045, math.sin(a) * 0.045))
            cyl(hold, tuple(tip), 0.012, 0.003, mat=c("a_mark"), seg=6)


def seg_tail(pts, r0, r1):
    """A segmented tail: violet segments with a dark joint ring between each pair."""
    n = len(pts) - 1
    for k in range(n):
        ra, rb = r0 + (r1 - r0) * k / n, r0 + (r1 - r0) * (k + 1) / n
        a, b = Vector(pts[k]), Vector(pts[k + 1])
        cyl(tuple(a), tuple(b), ra, rb, mat=c("a_main"), seg=12)
        if k:
            d = (b - a).normalized() * 0.008
            cyl(tuple(a - d), tuple(a + d), ra * 1.06, ra * 1.06, mat=c("a_dark"), seg=12)


# --- lanternjaw larva · the rider stage: four candidates (2026-10-09) ---------------------------
#
# Brief: design/handoffs/lanternjaw-larva-candidates-2026-10-09.md. Candidates for Wrysk to
# pick from, not a decided look: the larva hatches from a buried egg, clamps onto a host's
# back and drinks from it, and drops off at about a quarter of adult body, from then on drawn
# as the adult's smallest bins. Today a rider draws as an interim block (`glyph_spec` in
# crates/cubarium/src/voxel/figure.rs).
#
# Each kind is built at its own true size (no `REF` scaling; how big to draw a rider is
# Wrysk's call), facing +x, its belly on z = 0 = the host's back, in the lanternjaw's slots.
# Voxel placement as in `lanternjaw()`: cell (i, j, k) centres at x = 0.125 i, y = 0.125 j,
# z = 0.125 k + 0.0625, and the bake's rule claims every cell whose centre is inside a part
# or within half a voxel (0.0625) of it, so each part is kept that far clear of the cells it
# must not take. A two-wide body is centred on y = 0.0625, between rows j = 0 and 1. Parts
# that bite into the host go below z = 0 on purpose: voxelized with `floor=None`, their
# cells (k = -1) stand in for the host's top cells. Every other part stays above z = 0.
#
#   tick   flat round-backed clamp of three overlapping plates, one voxel tall; two pale
#          hooked fangs dug in at the front; one short tail nub curling up at the back
#          under a single cyan lure dot (the twin tails not yet forked)
#   grub   four segments arched like an inchworm mid-step, head and tail on the host, the
#          middle raised; a pale head capsule biting in; a cyan glow at the tail tip
#   nymph  a miniature adult: oversized head and fangs, the striking arms folded tight,
#          legs clamped round the host's flanks, one unforked upright tail stub with a
#          small lure bud
#   leech  a limbless body along the host's spine, one voxel wide and four long; a pale
#          sucker mouth at the front; the back end lifted with two tiny lure buds, the twin
#          tails as two dots
LJ_LARVA_KINDS = ("tick", "grub", "nymph", "leech")

# Round 2 (Wrysk 2026-10-09: "either grub or tick. i like that tick is symmetric; thats
# important"): the tick and the grub made symmetric about the host's spine, two ways each,
# and nothing bites below the larva's belly (the engine can't cut a host's cells):
#
#   tick3, grub3  three voxels wide on rows j = -1..1, centred on the spine; the tick's one
#                 lure cell on the middle row, the grub's cyan foot across all three
#   tick2, grub2  two voxels wide, as in round 1, drawn half a voxel across so the pair of
#                 rows straddles the spine (`LJ_LARVA_SHIFT`); the lure across both rows
LJ_LARVA_R2 = ("tick3", "tick2", "grub3", "grub2")
# The voxels across a kind is drawn shifted (by its anchor, not its cells) to centre it.
LJ_LARVA_SHIFT = {"tick2": -0.5, "grub2": -0.5}


def _capsules(pts, r0, r1, mat):
    """A thin limb as one long ellipsoid per segment (radius r0 -> r1): closed and convex,
    with no end caps, whose rims can pass the bake's inside test from a cell beside a bend
    (a `tube` leg over the host's edge took a cell off its far side that way)."""
    n = len(pts) - 1
    for k in range(n):
        a, b = Vector(pts[k]), Vector(pts[k + 1])
        d = b - a
        r = r0 + (r1 - r0) * (k + 0.5) / n
        rot = Vector((1, 0, 0)).rotation_difference(d.normalized()).to_matrix().to_4x4()
        ell(tuple((a + b) / 2), (d.length / 2 + 0.6 * r, r, r), mat, rot=rot)


def lanternjaw_larva(kind, vox=None):
    if kind in ("tick3", "tick2"):
        # round 2's tick: round 1's three plates and dark body, the fangs ending on the
        # host's back in front of the plates (a pale cell each), the tail nub and lure on
        # the axis
        Y, hw = (0.0, 0.13) if kind == "tick3" else (0.0625, 0.108)
        ell((0.0, Y, 0.045), (0.165, hw - 0.005, 0.03), c("a_dark"))
        for x, rx_, tilt in ((0.12, 0.058, -6), (0.0, 0.074, -8), (-0.12, 0.058, -10)):
            ell((x, Y, 0.06 + (0.004 if x < 0 else 0)), (rx_, hw, 0.034), c("a_main"), rot=ry(tilt))
        for y in ((-0.125, 0.125) if kind == "tick3" else (0.0, 0.125)):
            tube([(0.17, y, 0.05), (0.21, y, 0.045), (0.245, y, 0.03)], 0.02, 0.016, c("a_head"), seg=8)
            cyl((0.245, y, 0.03), (0.225, y, 0.02), 0.016, 0.004, mat=c("a_head"), seg=8)
        tube(catmull([(-0.155, Y, 0.07), (-0.165, Y, 0.12), (-0.135, Y, 0.155)], 6),
             0.026, 0.02, c("a_main"), seg=8)
        ell((-0.125, Y, 0.1875), (0.04, 0.04 if kind == "tick3" else 0.1, 0.04), c("a_lure"))
        return
    if kind in ("grub3", "grub2"):
        # round 2's grub: round 1's arch, its head resting on the host (no mandibles below)
        Y, ry_ = (0.0, 0.13) if kind == "grub3" else (0.0625, 0.1)
        segs = [((-0.25, Y, 0.125), (0.055, ry_, 0.075)), ((-0.125, Y, 0.19), (0.07, ry_, 0.052)),
                ((0.0, Y, 0.19), (0.07, ry_, 0.052)), ((0.125, Y, 0.175), (0.058, ry_, 0.06))]
        for centre, r in segs:
            ell(centre, r, c("a_main"))
        # the dark joint rings (two wide only): they take no cell at two wide, and at three
        # wide they won stray cells above and beside the arch, unevenly
        if kind == "grub2":
            for x, z, tilt in ((-0.2, 0.175, -40), (-0.0625, 0.195, 0), (0.07, 0.19, 30)):
                cyl((x - 0.005 * math.cos(math.radians(tilt)), Y, z),
                    (x + 0.005 * math.cos(math.radians(tilt)), Y, z + 0.005 * math.sin(math.radians(tilt))),
                    0.064, 0.064, mat=c("a_dark"), seg=12)
        # the head capsule, short enough that the middle row takes no cell in front of it
        ell((0.13, Y, 0.065), (0.045, ry_ - 0.008, 0.05), c("a_head"))
        # the cyan tail foot across the whole width (a middle-row cell alone hides behind
        # the side cells)
        ell((-0.25, Y, 0.0625), (0.034, 0.13 if kind == "grub3" else 0.09, 0.03), c("a_lure"))
        return
    if kind == "tick":
        Y = 0.0625
        # the dark body under the plates, showing only at their edges
        ell((0.0, Y, 0.045), (0.165, 0.1, 0.03), c("a_dark"))
        # three overlapping plates, front to back, each over one cell column, the rear ones
        # lapping onto the one in front; the front and rear plates kept 0.0625 clear of the
        # columns beyond them
        for x, rx_, tilt in ((0.12, 0.058, -6), (0.0, 0.074, -8), (-0.12, 0.058, -10)):
            ell((x, Y, 0.06 + (0.004 if x < 0 else 0)), (rx_, 0.108, 0.034), c("a_main"), rot=ry(tilt))
        # two pale hooked fangs, one over each cell row, running down and forward from under
        # the front plate and dug into the host, tips hooked back: they cross from the front
        # plate's cell to the host cell in front of and below it on the diagonal between the
        # two, so they take that host cell and not the ones beside it
        for y in (0.0, 0.125):
            tube([(0.14, y, 0.05), (0.1875, y, 0.0), (0.24, y, -0.055)], 0.02, 0.016, c("a_head"), seg=8)
            cyl((0.24, y, -0.055), (0.215, y, -0.09), 0.016, 0.004, mat=c("a_head"), seg=8)
        # the one tail nub, curling up off the rear plate under the single lure dot
        tube(catmull([(-0.155, 0.0, 0.07), (-0.165, 0.0, 0.12), (-0.135, 0.0, 0.155)], 6),
             0.026, 0.02, c("a_main"), seg=8)
        ell((-0.125, 0.0, 0.1875), (0.04, 0.04, 0.04), c("a_lure"))
        return
    if kind == "grub":
        Y = 0.0625
        # four segments along an inchworm's arch: the tail segment standing on the host, two
        # raised, the front one bending down to the head; a dark joint ring between each pair
        segs = [((-0.25, Y, 0.125), (0.055, 0.1, 0.075), 0),
                ((-0.125, Y, 0.19), (0.07, 0.1, 0.052), 0),
                ((0.0, Y, 0.19), (0.07, 0.1, 0.052), 0),
                ((0.125, Y, 0.175), (0.058, 0.1, 0.06), 0)]
        for centre, r, tilt in segs:
            ell(centre, r, c("a_main"), rot=ry(tilt))
        for x, z, tilt in ((-0.2, 0.175, -40), (-0.0625, 0.195, 0), (0.07, 0.19, 30)):
            cyl((x - 0.005 * math.cos(math.radians(tilt)), Y, z),
                (x + 0.005 * math.cos(math.radians(tilt)), Y, z + 0.005 * math.sin(math.radians(tilt))),
                0.064, 0.064, mat=c("a_dark"), seg=12)
        # the pale head capsule under the front segment, its mandibles biting into the host
        ell((0.14, Y, 0.065), (0.055, 0.092, 0.05), c("a_head"))
        for y in (0.0, 0.125):
            tube([(0.16, y, 0.03), (0.165, y, -0.02), (0.15, y, -0.05)], 0.016, 0.006, c("a_head"), seg=6)
        # the cyan glow at the tail tip, across both rows
        ell((-0.25, Y, 0.0625), (0.034, 0.09, 0.03), c("a_lure"))
        return
    if kind == "nymph":
        # abdomen and thorax, dark underneath, a violet back plate over each
        ell((-0.105, 0.0, 0.045), (0.06, 0.048, 0.03), c("a_dark"))
        ell((-0.1, 0.0, 0.075), (0.06, 0.055, 0.035), c("a_main"), rot=ry(-10))
        ell((0.0, 0.0, 0.07), (0.055, 0.05, 0.045), c("a_main"))
        # the oversized head: a helmet three cells wide, two large magenta eyes on its upper
        # sides, and the pale fangs a column in front of the face
        ell((0.125, 0.0, 0.11), (0.06, 0.13, 0.08), c("a_main"))
        for sy in (-1, 1):
            ell((0.13, sy * 0.125, 0.1875), (0.03, 0.03, 0.035), c("a_eye"))
        tube(catmull([(0.19, 0.0, 0.2), (0.245, 0.0, 0.13), (0.255, 0.0, 0.05), (0.235, 0.0, 0.01)], 8),
             0.032, 0.012, c("a_head"), seg=10)
        for sy in (-1, 1):
            # a striking arm folded tight: the femur up the head's side, the tibia folded
            # down in front of it, ending in a pale blade
            tube([(0.07, sy * 0.06, 0.04), (0.15, sy * 0.13, 0.12), (0.21, sy * 0.125, 0.07)],
                 0.024, 0.02, c("a_limb"), seg=8)
            ell((0.235, sy * 0.125, 0.05), (0.025, 0.018, 0.04), c("a_head"), rot=ry(-15))
            # three legs, clamped round the host's flanks: out over its back, then over its
            # edge (on the diagonal between the cells, so they take neither neighbour) and down
            # its side, the dark claw tips hooked in below its top
            for x in (0.0, -0.0625, -0.125):
                _capsules([(x, sy * 0.04, 0.05), (x, sy * 0.14, 0.04), (x, sy * 0.1875, 0.0),
                           (x, sy * 0.235, -0.045)], 0.018, 0.012, c("a_limb"))
                cyl((x, sy * 0.235, -0.045), (x, sy * 0.225, -0.1), 0.014, 0.003, mat=c("a_foot"), seg=6)
        # the one unforked tail: an upright stub off the abdomen's rear, a small lure bud on top
        seg_tail([(-0.15, 0.0, 0.08), (-0.14, 0.0, 0.17), (-0.13, 0.0, 0.255)], 0.03, 0.025)
        ell((-0.125, 0.0, 0.3), (0.035, 0.035, 0.035), c("a_lure"))
        return
    if kind == "leechfork":
        # Wrysk 2026-10-09, a sketch on the leech: "leech but forked at the end like this".
        # The front end forks: the body runs one cell further, to where the sucker was, and
        # two prongs reach on the diagonals from there, one cell forward and one aside, their
        # tips pale (the mouth moved out to the two tips). The lifted rear and its two buds as
        # the leech's.
        tube([(0.11, 0.0, 0.06), (0.0, 0.0, 0.06), (-0.125, 0.0, 0.062)], 0.044, 0.04, c("a_main"), seg=12)
        cyl((-0.125, 0.0, 0.062), (-0.25, 0.0, 0.1875), 0.024, 0.022, mat=c("a_main"), seg=10)
        ell((-0.25, 0.0, 0.1875), (0.04, 0.04, 0.04), c("a_main"))
        # The two buds stand 0.25 apart, one row each, only on a grid of 0.125 or finer;
        # coarser, each sits on a cell boundary and both land on one side, so there they are
        # one cyan bud on the middle row of the lifted rear (Wrysk 2026-10-09).
        two_buds = vox is None or vox <= 0.125 + 1e-9
        for sy in (-1, 1):
            # thin enough, and from far enough back, to take neither the cell in front of the
            # fork nor the ones beside it: only the tip's cell
            tube([(0.14, 0.0, 0.06), (0.25, sy * 0.125, 0.06)], 0.014, 0.014, c("a_main"), seg=8)
            ell((0.25, sy * 0.125, 0.06), (0.03, 0.03, 0.03), c("a_head"))
            if two_buds:
                cyl((-0.25, 0.0, 0.19), (-0.25, sy * 0.1, 0.19), 0.012, 0.01, mat=c("a_mark"), seg=6)
                ell((-0.25, sy * 0.125, 0.1875), (0.03, 0.03, 0.03), c("a_lure"))
        if not two_buds:
            ell((-0.25, 0.0, 0.1875), (0.03, 0.03, 0.03), c("a_lure"))
        return
    if kind == "leech":
        # one voxel wide along the spine: the body lying flat for two cells behind the
        # mouth, then lifting off the host at the back
        # (the lift crosses on the diagonal between the cells, thin, so the cell under the
        # lifted end stays empty)
        tube([(0.07, 0.0, 0.06), (0.0, 0.0, 0.06), (-0.125, 0.0, 0.062)], 0.044, 0.04, c("a_main"), seg=12)
        cyl((-0.125, 0.0, 0.062), (-0.25, 0.0, 0.1875), 0.024, 0.022, mat=c("a_main"), seg=10)
        ell((-0.25, 0.0, 0.1875), (0.04, 0.04, 0.04), c("a_main"))
        # the pale sucker mouth at the front: a disc with a dark ring of a mouth
        ell((0.125, 0.0, 0.06), (0.05, 0.05, 0.048), c("a_head"))
        cyl((0.165, 0.0, 0.06), (0.177, 0.0, 0.06), 0.022, 0.022, mat=c("a_dark"), seg=10)
        # the twin tails as two tiny buds forking off the lifted end, one each side
        for sy in (-1, 1):
            cyl((-0.25, 0.0, 0.19), (-0.25, sy * 0.1, 0.19), 0.012, 0.01, mat=c("a_mark"), seg=6)
            ell((-0.25, sy * 0.125, 0.1875), (0.03, 0.03, 0.03), c("a_lure"))
        return
    raise ValueError(f"unknown lanternjaw larva kind {kind!r}")


COATS["lanternjaw-larva"] = COATS["lanternjaw"]


def build_larva(kind, x, y, vox=None):
    """Build the `kind` larva at its true size, its origin moved to (x, y) on z = 0 (as
    `build` moves an animal's anchor), for a bake at `vox` (None: no grid in mind). Returns
    the new objects."""
    use("lanternjaw-larva")
    start = len(L.ORG)
    lanternjaw_larva(kind, vox)
    objs = L.ORG[start:]
    m = Matrix.Translation((x, y, 0))
    for ob in objs:
        ob.data.transform(m)
        ob.data.update()
    return objs


# --- loftstrider · radial canopy browser (2.2 x 2.4 x 3.4 m) ---------------------------------
#
# Pick (Wrysk 2026-10-01): art/gen/artbook/loftstrider.png, a front view. Radially symmetric about a vertical axis, in sixes (Wrysk: the image does not keep
# these counts): an onion-shaped body on six long legs, six magenta eyes round its middle,
# each between two legs; a long neck up to a pink head with the mouth hole on top; round the
# mouth six bone tusks and six feeding fronds, alternating and equally spaced, the fronds in
# three sizes (`LS_FROND`). Front (+x) is
# an eye. Legs and fronds at 30 + 60 k degrees, eyes and tusks at 60 k. Sizes follow D15:
# underside ~1.2 m, eyes at ~1.5 m, mouth at ~2.8 m, frond crowns to ~3.4 m.
#
# Voxel placement (cell centres at x = 0.125 i, y = 0.125 j, z = 0.125 k + 0.0625): the twelve
# tusk and frond roots sit on the radius-0.25 m ring round the mouth, which falls on twelve
# distinct cells, so each keeps its own cell at 0.125 m.

def ribbon(path, widths, side, mat, th=0.02):
    """A thin closed blade along `path` (3D points), `widths[k]` across at each point, laid
    out along the unit vector `side`: a slab `th` thick."""
    bm = bmesh.new()
    pts = [Vector(p) for p in path]
    n = len(pts)
    rows = []
    for k, p in enumerate(pts):
        t = (pts[min(k + 1, n - 1)] - pts[max(k - 1, 0)]).normalized()
        nrm = t.cross(side).normalized()
        w = side * (widths[k] / 2)
        rows.append([bm.verts.new(p + sgn * w + h * nrm * th / 2) for sgn, h in ((-1, 1), (1, 1), (1, -1), (-1, -1))])
    for k in range(n - 1):
        a, b = rows[k], rows[k + 1]
        for q in range(4):
            bm.faces.new((a[q], a[(q + 1) % 4], b[(q + 1) % 4], b[q]))
    bm.faces.new(rows[0][::-1])
    bm.faces.new(rows[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return L.mk("ribbon", bm, mat)


# The fronds' sizes, from the root, one per frond going round (k = 0..5 at 30 + 60 k
# degrees), 1 being the reference's size. Wrysk 2026-10-01 chose them mixed: each of
# 100, 75 and 60 % twice, no two neighbours alike. `LS_FROND=0.75` (all alike) or six
# values in the environment override it for comparisons.
LS_FROND = [float(v) for v in os.environ.get("LS_FROND", "1,0.75,0.6,0.75,1,0.6").split(",")]
LS_FROND = LS_FROND * 6 if len(LS_FROND) == 1 else LS_FROND


# The rig's part sizes, each a linear factor on the adult's (`LS_ADULT`, all 1): the onion
# body (radius and height), the legs (hip, knee and ankle heights and the splay beyond the
# hip), the legs' thickness (tubes, bulbs and toes), the neck's length and thickness, the
# head (with the mouth and the tusk and frond root ring), the fronds (beyond the root ring),
# the tusks (on top of the head's factor), and the eyes. Every count and angle is the
# adult's; all factors but `tusk` at s (tusk 1) is the adult scaled by s.
LS_ADULT = dict(body=1.0, leg=1.0, leg_r=1.0, neck=1.0, neck_r=1.0, head=1.0, frond=1.0,
                tusk=1.0, eye=1.0)

# Calf candidates (round 1, `design/handoffs/foliage-variants-and-calf-round1-2026-10-07.md`
# package B; Wrysk 2026-10-07: "long legs, short neck, a big head, small fronds", same
# six-leg rig and counts, same coat). Not baked: Wrysk picks. Each keeps the body at the
# newborn's linear size, (body_min / body_max)^(1/3) = 0.1^(1/3) = 0.464 (the loftstrider's
# table is the frondgrazer's scaled by mass, the ratio kept), so the onion holds a tenth of
# the adult's volume; they differ in how far legs, head, neck, fronds and tusks depart.
LS_NEWBORN = 0.1 ** (1.0 / 3.0)
LS_CALVES = {
    # A, mild: the nearest to a scaled adult
    "calf-a": dict(body=LS_NEWBORN, leg=0.62, leg_r=0.60, neck=0.42, neck_r=0.55, head=0.62,
                   frond=0.45, tusk=0.40, eye=0.55),
    # B, middle: a foal's legs, a short neck, stub tusks
    "calf-b": dict(body=LS_NEWBORN, leg=0.72, leg_r=0.55, neck=0.30, neck_r=0.60, head=0.72,
                   frond=0.35, tusk=0.55, eye=0.60),
    # C, extreme: stilts, almost no neck, a near-adult head, nubs for tusks, fronds as buds
    "calf-c": dict(body=LS_NEWBORN, leg=0.85, leg_r=0.50, neck=0.12, neck_r=0.65, head=0.85,
                   frond=0.25, tusk=0.30, eye=0.70),
}


# Round 2 (Wrysk 2026-10-08: "im not liking the long legs. shorter legs actually work for me.
# like more chibi. and then i would pick the middle for the rest of the body shape."): B's
# body, neck, head, fronds, tusks, eyes and thicknesses, with legs at or below the body's
# scale (0.464). Leg thickness stays B's 0.55, so the shorter the leg the stubbier it reads.
# Not baked: Wrysk picks.
LS_CALVES_R2 = {
    f"calf-l{round(leg * 100)}": dict(LS_CALVES["calf-b"], leg=leg)
    for leg in (0.46, 0.38, 0.30)
}

# Round 3 (Wrysk 2026-10-08: "show me the spread legs for comparison"): l38 at its own
# spread next to two narrower ones, the feet pulled in under the body and the toes shorter
# so the six stubs stand apart. Not baked: Wrysk picks.
LS_CALVES_R3 = {
    "calf-l38": LS_CALVES_R2["calf-l38"],
    "calf-l38-s60": dict(LS_CALVES_R2["calf-l38"], splay=0.60, toe=0.60),
    "calf-l38-s30": dict(LS_CALVES_R2["calf-l38"], splay=0.30, toe=0.40),
}


def ls_blend(calf, t, ease=None):
    """The rig `t` of the way from `calf` (0) to the adult (1), each factor linearly, or for
    the factors named in `ease` ({factor: exponent}) along t ** exponent."""
    ease = ease or {}
    return {k: v + (LS_ADULT[k] - v) * t ** ease.get(k, 1.0) for k, v in calf.items()}


def loftstrider():
    loftstrider_rig(LS_ADULT)


def ls_frame(p):
    """The loftstrider's landmarks at the part sizes `p` (`LS_ADULT`'s keys), metres,
    anchor at the origin, standing on z = 0: the heights of the body, neck and head, the
    head's height map, and per leg and frond (k = 0..5, at 30 + 60 k degrees) its hip,
    knee and ankle and its centreline. `loftstrider_rig` builds on them and the rig's bones
    (`ls_bones`) are placed by them."""
    B, Lg, Lr, N, Nr, Hd, F, T, E = (p[k] for k in ("body", "leg", "leg_r", "neck", "neck_r",
                                                      "head", "frond", "tusk", "eye"))
    zb = 1.22 * Lg - 0.07 * B          # the body's underside: the hips sit 0.07 above it
    zt = zb + (1.96 - 1.15) * B        # the body's top, where the neck starts
    zn = zt + (2.62 - 1.96) * N        # the neck's top, where the pink head starts
    z_neck = zb + (1.98 - 1.15) * B

    def head_z(z):                     # an adult head height (2.62 up) on this head
        return zn + (z - 2.62) * Hd
    # optional (round 3 comparisons, absent = 1): `splay` scales the knee's and ankle's
    # reach beyond the hip, `toe` the toes' length from the ankle
    Sp = p.get("splay", 1.0)
    rh = 0.40 * B
    rk, ra = rh + 0.32 * Lg * Sp, rh + 0.52 * Lg * Sp
    legs, fronds = [], []
    for k in range(6):
        b = math.radians(30 + 60 * k)
        cb, sb = math.cos(b), math.sin(b)
        legs.append(((rh * cb, rh * sb, 1.22 * Lg), (rk * cb, rk * sb, 0.80 * Lg),
                     (ra * cb, ra * sb, 0.16 * Lg)))
        prof = [(0.22, 2.76), (0.36, 3.02), (0.56, 3.27), (0.82, 3.37), (1.04, 3.27), (1.17, 3.03),
                (1.2, 2.78), (1.14, 2.6)]
        fs = LS_FROND[k] * F
        prof = [(0.22 * Hd + (r - 0.22) * fs, head_z(2.76) + (z - 2.76) * fs) for r, z in prof]
        fronds.append([(r * cb, r * sb, z) for r, z in catmull(prof, 16)])
    return dict(zb=zb, zt=zt, zn=zn, z_neck=z_neck, head_z=head_z, legs=legs, fronds=fronds,
                mouth_z=head_z(2.835))


def loftstrider_rig(p):
    """The loftstrider at the part sizes `p` (`LS_ADULT`'s keys), in metres, standing on
    z = 0: the adult at `LS_ADULT`, a calf at its own sizes. Every part is on its rig bone
    (`ls_bones`; FEED-ROLLOUT): the column loft and each frond's blade by the nearest
    bone's segment (`bone_nearest`), the rest whole."""
    B, Lg, Lr, N, Nr, Hd, F, T, E = (p[k] for k in ("body", "leg", "leg_r", "neck", "neck_r",
                                                      "head", "frond", "tusk", "eye"))
    fr = ls_frame(p)
    zb, zt, zn, z_neck, head_z = fr["zb"], fr["zt"], fr["zn"], fr["z_neck"], fr["head_z"]
    segs = {name: (head, tail) for name, _, head, tail in ls_bones(p)}

    def zone(f, s, q):
        return 0 if q.z < z_neck else (1 if q.z < zn else 2)   # body, neck, pink head
    body = [(1.15, 0.05), (1.19, 0.30), (1.29, 0.47), (1.43, 0.555), (1.57, 0.56), (1.71, 0.50),
            (1.84, 0.38), (1.96, 0.26)]
    neck = [(2.14, 0.19), (2.40, 0.155), (2.60, 0.16)]
    head = [(2.71, 0.20), (2.78, 0.21), (2.83, 0.13)]
    column = ([(zb + (z - 1.15) * B, r * B) for z, r in body]       # one loft up the axis
              + [(zt + (z - 1.96) * N, r * Nr) for z, r in neck]
              + [(head_z(z), r * Hd) for z, r in head])
    start = len(L.ORG)
    loft([(0, 0, z, r, r, r) for z, r in column], [c("a_main"), c("a_pale"), c("a_mark")], zone,
         nring=90, nseg=36)
    bone_nearest(start, [(n, segs[n]) for n in ("root", "neck_a", "neck_b", "head")])
    # the mouth hole on top
    start = len(L.ORG)
    ell((0, 0, head_z(2.835)), (0.07 * Hd, 0.07 * Hd, 0.025 * Hd), c("a_dark"))
    bone_of(start, "head")
    z_eye = zb + (1.52 - 1.15) * B
    for k in range(6):
        a = math.radians(60 * k)            # eyes and tusks
        b = math.radians(30 + 60 * k)       # legs and fronds
        ca, sa, cb, sb = math.cos(a), math.sin(a), math.cos(b), math.sin(b)
        # eye: a magenta lens on the body's middle facing out, a dark vertical slit pupil
        re, rp = 0.53 * B, 0.53 * B + 0.055 * E
        ell((re * ca, re * sa, z_eye), (0.06 * E, 0.1 * E, 0.1 * E), c("a_eye"), rot=rz(60 * k))
        ell((rp * ca, rp * sa, z_eye), (0.012 * E, 0.014 * E, 0.075 * E), c("a_pupil"), rot=rz(60 * k))
        # tusk: from the head's rim, curving up and out
        tk = [(0.19, 2.73), (0.25, 2.83), (0.29, 2.95), (0.28, 3.06), (0.24, 3.12)]
        r0, z0 = tk[0]
        tk = [(Hd * (r0 + (r - r0) * T), head_z(z0 + (z - z0) * T)) for r, z in tk]
        start = len(L.ORG)
        tube([(r * ca, r * sa, z) for r, z in tk], 0.05 * Hd, 0.008 * Hd, c("a_head"), seg=10)
        bone_of(start, "head")
        # frond: rises from the rim, arcs outward and droops, its edges frilled cyan
        path = fr["fronds"][k]
        widths = [F * (0.07 + 0.3 * LS_FROND[k] * math.sin(math.pi * min(1.0, t / 15 * 1.12)))
                  for t in range(16)]
        side = Vector((-sb, cb, 0))
        start = len(L.ORG)
        ribbon(path, widths, side, c("a_membrane"), th=0.035 * F)
        for sgn in (-1, 1):
            edge = [tuple(Vector(q) + side * (sgn * w / 2)) for q, w in zip(path, widths)]
            tube(edge[1:], 0.025 * F, 0.016 * F, c("a_glow"), seg=6)
        bone_nearest(start, [(n, segs[n]) for n in (f"f{k}_a", f"f{k}_b", f"f{k}_c")])
        # leg: a hip bulb under the body, the thigh out and down to a bulbous knee, the shin
        # down to the ankle, three splayed toes (`toe`, optional, absent = 1: the toes'
        # length from the ankle); the hip and thigh on the thigh bone, the rest on the shin
        Tl = p.get("toe", 1.0)
        hip, knee, ankle = fr["legs"][k]
        start = len(L.ORG)
        ell(hip, (0.12 * Lr,) * 3, c("a_limb"))
        tube([hip, knee], 0.088 * Lr, 0.072 * Lr, c("a_limb"), seg=12)
        bone_of(start, f"thigh{k}")
        start = len(L.ORG)
        ell(knee, (0.1 * Lr,) * 3, c("a_mark"))
        tube([knee, ankle], 0.07 * Lr, 0.06 * Lr, c("a_limb"), seg=12)
        ell(ankle, (0.075 * Lr,) * 3, c("a_mark"))
        for dt in (-30, 0, 30):
            d = math.radians(30 + 60 * k + dt)
            toe = (ankle[0] + 0.17 * Lr * Tl * math.cos(d), ankle[1] + 0.17 * Lr * Tl * math.sin(d), 0.05 * Lr)
            cyl(ankle, toe, 0.06 * Lr, 0.05 * Lr, mat=c("a_foot"), seg=8)
            ell(toe, (0.1 * Lr, 0.07 * Lr, 0.05 * Lr), c("a_foot"), rot=rz(30 + 60 * k + dt))
        bone_of(start, f"shin{k}")


def build_loftstrider(p, x, y):
    """`loftstrider_rig(p)` at its own metres, its anchor at (x, y) on z = 0 (no `REF`
    rescale: a calf's part sizes are absolute). Returns the new organism objects."""
    use("loftstrider")
    start = len(L.ORG)
    loftstrider_rig(p)
    objs = L.ORG[start:]
    for ob in objs:
        ob.data.transform(Matrix.Translation((x, y, 0)))
        ob.data.update()
        if "bone_segs" in ob:
            pts = list(ob["bone_segs"])
            ob["bone_segs"] = [v + (x, y, 0.0)[i % 3] for i, v in enumerate(pts)]
    return objs


# --- the loftstrider's rig, feeding clip and rest pose (FEED-ROLLOUT, 2026-10-10) ---------
#
# `design/handoffs/feed-rollout-2026-10-10.md` §Loftstrider; placeholders, Wrysk directs the
# real ones. 34 bones (`ls_bones`): the root (the body and its eyes), a neck of two, the
# head (mouth and tusks), each frond a chain of three (its centreline in thirds), each leg
# a thigh (hip bulb and thigh) and a shin (knee, shin, ankle and toes). Every size bin is
# its own build (`ls_blend`), so the bake samples both clips per bin from that bin's
# landmarks (`ls_rig_clips`).
#
# The legs are solved here, every sampled frame, by a two-bone IK: the ankle planted where
# it stood, the knee on the upper side of the hip-ankle line in the vertical plane through
# them (as it stands); the result is keyed as plain turns, so the engine only plays bone
# transforms.
#
# **Feed** (state `cropping`, a loop; the reach and return are the clip weight): browsing a
# crown above the heading (+x), a fixed height and side. Held through the loop, the body
# leans `LS_LEAN_DEG` toward it about the far hips (which stay put) and rises as far as the
# legs allow (`LS_STRETCH`; the standing legs are already near straight, so it is a few
# cm), and the neck bends that way. Pair A (fronds 0 and 5, at 30 and 330 degrees, facing
# the crown) swings toward it and reaches up and out (`LS_REACH_S`), curls closed
# (`LS_CURL_S`) and arcs back over the head to the mouth (`LS_BRING_S`) while the head dips
# to meet it, then returns to rest (`LS_RETURN_S`) during the head's chew nod (`LS_CHEW_S`)
# while pair B (fronds 1 and 4, the next pair round) reaches; then B brings, A reaches.
#
# **Rest** (state `asleep`, a track: the clip weight walks its frames, first to last): the
# body lowers until its underside is on the ground, the legs folding spider-like, knees
# high, feet where they stood.

LS_FEED_FPS = 15
LS_REACH_S, LS_CURL_S, LS_BRING_S, LS_RETURN_S, LS_CHEW_S = 0.7, 0.2, 0.4, 0.7, 0.3
LS_PAIR_S = LS_REACH_S + LS_CURL_S + LS_BRING_S     # one pair's reach, grab and bring
LS_FEED_LOOP_S = 2 * LS_PAIR_S
LS_PAIRS = ((0, 5), (1, 4))
LS_LEAN_DEG = 6.0
LS_NECK_DEG = 6.0           # each neck bone, toward the crown
LS_DIP_DEG = 3.0            # each neck bone more as a frond delivers
LS_CHEW_DEG = 4.0           # the head's nod after a delivery
LS_STRETCH = 0.995          # the legs' most extension (hip to ankle over thigh + shin)
# each frond bone's elevation (degrees above level, the head's frame): reached, curled
LS_REACH = (50.0, 40.0, 25.0)
LS_CURL = (55.0, 5.0, -45.0)
LS_BRING_A = 100.0          # the frond's first bone, brought: just past upright
LS_REST_FRAMES = 12
# clip modes in the `.rig` (`crates/cubarium/src/voxel/rig.rs`)
CLIP_LOOP, CLIP_TRACK = 0, 1


def ls_bones(p):
    """The loftstrider's bones at part sizes `p`: (name, parent index, head, tail), metres,
    Blender axes, anchor at the origin, parents first."""
    fr = ls_frame(p)
    z_mid = (fr["z_neck"] + fr["zn"]) / 2
    out = [("root", None, (0, 0, fr["zb"]), (0, 0, fr["z_neck"])),
           ("neck_a", 0, (0, 0, fr["z_neck"]), (0, 0, z_mid)),
           ("neck_b", 1, (0, 0, z_mid), (0, 0, fr["zn"])),
           ("head", 2, (0, 0, fr["zn"]), (0, 0, fr["head_z"](2.84)))]
    for k, path in enumerate(fr["fronds"]):
        for i, part in enumerate("abc"):
            out.append((f"f{k}_{part}", 3 if i == 0 else len(out) - 1, path[5 * i], path[5 * i + 5]))
    for k, (hip, knee, ankle) in enumerate(fr["legs"]):
        out.append((f"thigh{k}", 0, hip, knee))
        out.append((f"shin{k}", len(out) - 1, knee, ankle))
    return out


def smooth(u):
    u = max(0.0, min(1.0, u))
    return u * u * (3 - 2 * u)


def ls_legs(bones, turn, shift, out):
    """Each leg's thigh and shin turns (relative to the parent, rest armature space) into
    `out`, the root turned by `turn` about its head and its head moved by `shift`: the
    ankle stays where it stood, the knee on the upper side of the hip-ankle line."""
    h0 = Vector(bones[0][2])
    by = {name: (Vector(h), Vector(t)) for name, _, h, t in bones}
    up = Vector((0, 0, 1))
    for k in range(6):
        hip0, knee0 = by[f"thigh{k}"]
        _, ankle = by[f"shin{k}"]
        l1, l2 = (knee0 - hip0).length, (ankle - knee0).length
        hip = h0 + shift + turn @ (hip0 - h0)
        to = ankle - hip
        d = min(max(to.length, abs(l1 - l2) + 1e-6), l1 + l2 - 1e-6)
        u = to.normalized()
        w = (up - u * up.dot(u)).normalized()
        a = math.acos(max(-1.0, min(1.0, (l1 * l1 + d * d - l2 * l2) / (2 * l1 * d))))
        knee = hip + (u * math.cos(a) + w * math.sin(a)) * l1
        thigh_w = (turn @ (knee0 - hip0)).rotation_difference(knee - hip).to_matrix() @ turn
        shin_w = (thigh_w @ (ankle - knee0)).rotation_difference(ankle - knee).to_matrix() @ thigh_w
        out[f"thigh{k}"] = turn.transposed() @ thigh_w
        out[f"shin{k}"] = thigh_w.transposed() @ shin_w


def ls_lean(bones):
    """The feed's held root turn and the root head's shift: `LS_LEAN_DEG` toward +x about
    the far hips' line (legs 2 and 3, which stay put), then up as far as every leg allows
    (`LS_STRETCH`), or down if leaning stretched one past it. Also the rise, metres."""
    h0 = Vector(bones[0][2])
    by = {name: (Vector(h), Vector(t)) for name, _, h, t in bones}
    turn = Matrix.Rotation(math.radians(LS_LEAN_DEG), 3, "Y")
    far = (by["thigh2"][0] + by["thigh3"][0]) / 2
    far.y = 0.0
    rise = 1e9
    for k in range(6):
        hip0, knee0 = by[f"thigh{k}"]
        _, ankle = by[f"shin{k}"]
        lmax = LS_STRETCH * ((knee0 - hip0).length + (ankle - knee0).length)
        v = far + turn @ (hip0 - far) - ankle
        rise = min(rise, -v.z + math.sqrt(max(0.0, v.z * v.z - (v.length_squared - lmax * lmax))))
    shift = far + turn @ (h0 - far) + Vector((0, 0, rise)) - h0
    return turn, shift, rise


def ls_frond_keys(bones, k, mouth_z):
    """Frond `k`'s key poses (swing about z degrees, each bone's elevation degrees): rest,
    reached, curled, brought (its tip just over the mouth, a two-bone reach for its outer
    bones, the joint on the upper side)."""
    names = [f"f{k}_{x}" for x in "abc"]
    by = {name: (Vector(h), Vector(t)) for name, _, h, t in bones}

    def rz_(v):                         # (radius, height) in the frond's own plane
        return Vector((math.hypot(v.x, v.y), v.z))
    segs = [(rz_(by[n][0]), rz_(by[n][1])) for n in names]
    lens = [(t - h).length for h, t in segs]
    elev = [math.degrees(math.atan2((t - h).y, (t - h).x)) for h, t in segs]
    b = 30 + 60 * k
    b = b - 360 if b > 180 else b
    swing = -0.5 * b
    root = segs[0][0]
    a = math.radians(LS_BRING_A)
    j1 = root + Vector((math.cos(a), math.sin(a))) * lens[0]
    target = Vector((0.0, mouth_z + 0.02))
    to = target - j1
    lb, lc = lens[1], lens[2]
    d = min(max(to.length, abs(lb - lc) + 1e-6), lb + lc - 1e-6)
    base = math.atan2(to.y, to.x)
    bend = math.acos(max(-1.0, min(1.0, (lb * lb + d * d - lc * lc) / (2 * lb * d))))
    joint = max((j1 + Vector((math.cos(base + s * bend), math.sin(base + s * bend))) * lb for s in (1, -1)),
                key=lambda j: j.y)
    bring = (0.0, LS_BRING_A, math.degrees(math.atan2((joint - j1).y, (joint - j1).x)),
             math.degrees(math.atan2((target - joint).y, (target - joint).x)))
    return elev, {"rest": (0.0, *elev), "reach": (swing, *LS_REACH), "curl": (swing, *LS_CURL),
                  "bring": bring}


def ls_frond_at(keys, u):
    """A frond's pose `u` seconds into its pair's cycle (`LS_FEED_LOOP_S` long)."""
    lerp = lambda a, b, w: tuple(x + (y - x) * smooth(w) for x, y in zip(a, b))  # noqa: E731
    t1 = LS_REACH_S
    t2 = t1 + LS_CURL_S
    t3 = t2 + LS_BRING_S
    t4 = t3 + LS_RETURN_S
    if u < t1:
        return lerp(keys["rest"], keys["reach"], u / LS_REACH_S)
    if u < t2:
        return lerp(keys["reach"], keys["curl"], (u - t1) / LS_CURL_S)
    if u < t3:
        return lerp(keys["curl"], keys["bring"], (u - t2) / LS_BRING_S)
    if u < t4:
        return lerp(keys["bring"], keys["rest"], (u - t3) / LS_RETURN_S)
    return keys["rest"]


def ls_dip_chew(u):
    """The neck's extra bend (dip, degrees) and the head's nod (degrees) `u` seconds into a
    pair's cycle: the dip comes in as the fronds arc to the mouth and goes during the chew."""
    t2 = LS_REACH_S + LS_CURL_S
    t3 = t2 + LS_BRING_S
    if t2 <= u < t3:
        return LS_DIP_DEG * smooth((u - t2) / LS_BRING_S), 0.0
    if t3 <= u < t3 + LS_CHEW_S:
        x = (u - t3) / LS_CHEW_S
        return LS_DIP_DEG * (1 - smooth(x)), LS_CHEW_DEG * math.sin(math.pi * x)
    return 0.0, 0.0


def key_turns(ob, bones, frame, turns, shift):
    """Key `turns` ({bone: 3x3, relative to its parent, rest armature space}; absent: none)
    at `frame`, the root's head moved by `shift`."""
    for name, _, _, _ in bones:
        pb = ob.pose.bones[name]
        pb.rotation_mode = "QUATERNION"
        q = pb.bone.matrix_local.to_quaternion()
        r = turns.get(name, Matrix.Identity(3)).to_quaternion()
        pb.rotation_quaternion = q.inverted() @ r @ q
        pb.keyframe_insert("rotation_quaternion", frame=frame)
        if name == "root":
            pb.location = q.inverted() @ shift
            pb.keyframe_insert("location", frame=frame)


def ls_feed_clip(p):
    """The feeding loop at part sizes `p`, sampled at `LS_FEED_FPS` (`sample_clip`)."""
    bones = ls_bones(p)
    fr = ls_frame(p)
    n = round(LS_FEED_LOOP_S * LS_FEED_FPS)
    ob = rig_armature("ls_rig", bones, LS_FEED_FPS)
    turn, shift, _ = ls_lean(bones)
    keys = {k: ls_frond_keys(bones, k, fr["mouth_z"]) for pair in LS_PAIRS for k in pair}
    for f in range(n):
        t = f / LS_FEED_FPS
        turns = {"root": turn}
        ls_legs(bones, turn, shift, turns)
        dip, chew = 0.0, 0.0
        for i, pair in enumerate(LS_PAIRS):
            u = (t - i * LS_PAIR_S) % LS_FEED_LOOP_S
            d, c_ = ls_dip_chew(u)
            dip, chew = dip + d, chew + c_
            for k in pair:
                elev, kk = keys[k]
                swing, *th = ls_frond_at(kk, u)
                b = math.radians(30 + 60 * k)
                s_ = Vector((-math.sin(b), math.cos(b), 0))
                dth = [math.radians(x - e) for x, e in zip(th, elev)]
                rel = [dth[0], dth[1] - dth[0], dth[2] - dth[1]]
                # + about the frond's side axis lowers it: a lift is a negative turn
                for x, a in zip("abc", rel):
                    turns[f"f{k}_{x}"] = Matrix.Rotation(-a, 3, s_)
                turns[f"f{k}_a"] = Matrix.Rotation(math.radians(swing), 3, "Z") @ turns[f"f{k}_a"]
        neck = Matrix.Rotation(math.radians(LS_NECK_DEG + dip), 3, "Y")
        turns["neck_a"], turns["neck_b"] = neck, neck
        turns["head"] = Matrix.Rotation(math.radians(chew), 3, "Y")
        key_turns(ob, bones, f, turns, shift)
    return sample_clip(ob, bones, n, LS_FEED_FPS)


def ls_rest_clip(p):
    """The rest pose at part sizes `p` as a track of `LS_REST_FRAMES`: standing (frame 0)
    to the body's underside on the ground (the last), evenly (the weight's ease shapes it)."""
    bones = ls_bones(p)
    drop = ls_frame(p)["zb"]
    ob = rig_armature("ls_rest", bones, LS_FEED_FPS)
    for f in range(LS_REST_FRAMES):
        shift = Vector((0, 0, -drop * f / (LS_REST_FRAMES - 1)))
        turns = {}
        ls_legs(bones, Matrix.Identity(3), shift, turns)
        key_turns(ob, bones, f, turns, shift)
    return sample_clip(ob, bones, LS_REST_FRAMES, LS_FEED_FPS)


def ls_rig_clips(p):
    """The loftstrider's rig at part sizes `p`: its bones, their heads (the model's axes,
    metres), and its clips by the state each plays in (`write_rig`'s input)."""
    feed, rest = ls_feed_clip(p), ls_rest_clip(p)
    return {"bones": feed["bones"], "heads": feed["heads"],
            "clips": [dict(state="cropping", mode=CLIP_LOOP, fps=feed["fps"], frames=feed["frames"]),
                      dict(state="asleep", mode=CLIP_TRACK, fps=rest["fps"], frames=rest["frames"])]}


def feed_rig(clip):
    """A one-clip rig (the frondgrazer's, the chorister's feeding loop) as `write_rig` reads
    a rig: its clip plays while the body crops."""
    return {"bones": clip["bones"], "heads": clip["heads"],
            "clips": [dict(state="cropping", mode=CLIP_LOOP, fps=clip["fps"], frames=clip["frames"])]}


# Models whose parts hang below their z = 0 (the perch under the stand pose's feet): the
# voxelizer keeps their cells below the ground row (offsets dy < 0).
BELOW_GROUND = {"seedporter-stand"}

# name: (builder, reference length, width, height in metres). The founders' references are
# their adult bodies (`assets/voxel-models/source.json`); a specimen is built and baked at
# its own (the bake's placeholder table), here as its built extent.
REF = {
    "chorister": (chorister, 1.4, 0.35, 0.6),
    "frondgrazer": (frondgrazer, 0.75, 0.375, 0.375),
    # the same body hunkered: the reference box of the standing body, so it scales with it
    "frondgrazer-hunker": (frondgrazer_hunker, 0.75, 0.375, 0.375),
    "littershredder": (littershredder, 0.375, 0.125, 0.125),
    "ripple-snail": (ripple_snail, 0.19, 0.19, 0.125),
    "capgnawer": (capgnawer, 0.25, 0.21, 0.19),
    "seedporter": (seedporter, 0.2, 0.63, 0.94),
    "seedporter-stand": (seedporter_stand, 0.31, 0.37, 1.04),
    "seedporter-ground": (seedporter_ground, 1.14, 0.31, 0.67),
    "seedporter-glide": (seedporter_glide, 1.24, 0.76, 0.35),
    "bellwing": (bellwing, 0.05, 0.375, 0.4),
    "lanternjaw": (lanternjaw, 1.0, 0.25, 0.45),
    "loftstrider": (loftstrider, 2.2, 2.4, 3.4),
}


def build(name, x, y, length, width, height):
    """Build `name` scaled to (length, width, height), its anchor at (x, y) on z = 0.
    Returns the new organism objects."""
    fn, l0, w0, h0 = REF[name]
    use(name)
    start = len(L.ORG)
    fn()
    m = Matrix.Translation((x, y, 0)) @ Matrix.Diagonal((length / l0, width / w0, height / h0, 1))
    objs = L.ORG[start:]
    for ob in objs:
        ob.data.transform(m)
        ob.data.update()
        if "bone_cut_x" in ob:
            ob["bone_cut_x"] = x + ob["bone_cut_x"] * length / l0
    return objs


# --- contact sheets ---------------------------------------------------------------------------

PICKS = {
    "chorister": "chorister.png", "frondgrazer": "frondgrazer.png",
    "frondgrazer-hunker": "frondgrazer.png",
    "littershredder": "littershredder.png", "ripple-snail": "ripple-snail-organic.png",
    "capgnawer": "capgnawer-rose-bark.png", "seedporter": "seedporter.png", "bellwing": "bellwing.png",
    "seedporter-stand": "seedporter-stand.png", "seedporter-ground": "seedporter-ground.png",
    "seedporter-glide": "seedporter-glide.png",
    "lanternjaw": "lanternjaw.png",
    "loftstrider": "loftstrider.png",
}
# The pick's view: camera azimuth (degrees from +x, counter-clockwise seen from above; -90
# looks from -y at the left flank) and elevation.
VIEWS = {
    "chorister": (-80.0, 8.0), "frondgrazer": (100.0, 8.0), "frondgrazer-hunker": (100.0, 8.0), "littershredder": (115.0, 20.0),
    "ripple-snail": (-60.0, 22.0), "capgnawer": (20.0, 22.0), "seedporter": (0.0, 5.0),
    "seedporter-stand": (0.0, 5.0), "seedporter-ground": (70.0, 12.0), "seedporter-glide": (180.0, 45.0),
    "bellwing": (0.0, 4.0), "lanternjaw": (35.0, 22.0),
    "loftstrider": (0.0, 8.0),
}


def ortho_camera(objs, az, el, aspect):
    lo = Vector((min(v[k] for ob in objs for v in ob.bound_box) for k in range(3)))
    hi = Vector((max(v[k] for ob in objs for v in ob.bound_box) for k in range(3)))
    pts = [Vector((a, b, cc)) for a in (lo.x, hi.x) for b in (lo.y, hi.y) for cc in (lo.z, hi.z)]
    centre = (lo + hi) / 2
    a, e = math.radians(az), math.radians(el)
    d = Vector((math.cos(e) * math.cos(a), math.cos(e) * math.sin(a), math.sin(e)))
    cam = bpy.data.cameras.new("sheet")
    cam.type = "ORTHO"
    ob = bpy.data.objects.new("sheet", cam)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = centre + d * 50
    ob.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
    bpy.context.view_layer.update()
    inv = ob.matrix_world.inverted()
    q = [inv @ p for p in pts]
    w = max(p.x for p in q) - min(p.x for p in q)
    h = max(p.y for p in q) - min(p.y for p in q)
    cam.ortho_scale = 1.12 * max(w, h * aspect)
    cam.clip_end = 200
    return ob


def sheets(out, only):
    import bake_voxel_models as B  # noqa: E402  (its main() is guarded)
    import subprocess
    os.makedirs(out, exist_ok=True)
    sc = L.setup_scene()
    sc.display.shading.show_specular_highlight = False
    sc.world.color = L.lin("#F4F2F8")
    sc.display.shading.object_outline_color = (0.05, 0.03, 0.08)
    vox = 0.125
    W, H = 900, 700
    for name in REF:
        if only and name not in only:
            continue
        B.reset_scene()
        _, l0, w0, h0 = REF[name]
        objs = build(name, 0.5 * vox, 0.5 * vox, l0, w0, h0)
        az, el = VIEWS[name]
        cam = ortho_camera(objs, az, el, W / H)
        L.render(cam, os.path.join(out, f"_{name}-model.png"), W, H)
        cells = B.voxelize_animal(objs, vox, None if name in BELOW_GROUND else 0)
        B.reset_scene()
        cubes = voxel_cubes(name, cells, vox)
        cam = ortho_camera(cubes, az, el, W / H)
        L.render(cam, os.path.join(out, f"_{name}-voxels.png"), W, H)
        top = max(k for (_, _, k) in cells) if cells else 0
        print(f"{name}: {len(cells)} cells at {vox} m, {span(cells)} voxels (L x W x H), top {top}", flush=True)
        subprocess.run(["python3", os.path.join(HERE, "animal_model_sheet.py"),
                        os.path.join(REPO, "art", "gen", "artbook", PICKS[name]),
                        os.path.join(out, f"_{name}-model.png"), os.path.join(out, f"_{name}-voxels.png"),
                        os.path.join(out, f"{name}.png"), name, f"{len(cells)} cells, {span(cells)} voxels"],
                       check=True)


def span(cells):
    if not cells:
        return "0 x 0 x 0"
    ks = list(cells)
    rng = [max(k[a] for k in ks) - min(k[a] for k in ks) + 1 for a in range(3)]
    return f"{rng[0]} x {rng[1]} x {rng[2]}"


def voxel_cubes(name, cells, vox):
    """The voxel model as cubes, each in its slot's ramp colour at its height band (the
    renderer's rule: `colours::band`, four bands)."""
    top = max((k for (_, _, k) in cells), default=0)
    objs = []
    for (i, j, k), mat in cells.items():
        slot = slot_of(mat)
        lo, hi = COATS[name].get(slot, COATS[name]["a_main"])
        t = 1.0 if top <= 0 else round(min(max(k, 0), top) / top * 3) / 3
        key = f"vox:{name}:{slot}:{t:.2f}"
        L.PAL[key] = _mix(lo, hi, t)
        objs.append(box(((i + 0.5) * vox, (j + 0.5) * vox, (k + 0.5) * vox), (vox, vox, vox), key))
    if name in BELOW_GROUND and cells:
        # the ground (perch) line, z = 0, through the model: a thin plate in front of it
        i0, i1 = min(c[0] for c in cells), max(c[0] for c in cells)
        j0, j1 = min(c[1] for c in cells), max(c[1] for c in cells)
        L.PAL["vox:ground"] = "#8A6F55"
        objs.append(box((((i0 + i1) / 2 + 0.5) * vox, ((j0 + j1) / 2 + 0.5) * vox, -0.01),
                        ((i1 - i0 + 3) * vox, (j1 - j0 + 3) * vox, 0.02), "vox:ground"))
    return objs


def main():
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if "--view" in args:  # --view AZ,EL: render every sheet from this view instead
        az, el = (float(v) for v in args[args.index("--view") + 1].split(","))
        for k in VIEWS:
            VIEWS[k] = (az, el)
    if "--sheets" in args:
        out = args[args.index("--sheets") + 1]
        only = set(filter(None, (args[args.index("--only") + 1] if "--only" in args else "").split(",")))
        sheets(out, only)


if __name__ == "__main__":
    main()
