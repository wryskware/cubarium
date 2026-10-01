"""Animal models from the artbook picks (model pass MP-A, 2026-10-01).

    blender -b -P scripts/blender/animal_models.py -- --sheets OUT [--only NAME[,NAME]]

The Blender builders the voxel bake (`bake_voxel_models.py`) takes its animals from: the
two simulated founders (frondgrazer, littershredder) and five unsimulated specimens
(chorister, ripple-snail, capgnawer, seedporter, bellwing). Each moves its signed-off
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
from mathutils import Matrix, Vector

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
         "a_glow", "a_membrane", "a_fruit", "a_tendril", "magenta"]
# On an animal, these are accents (the bake's accent rule: an accent claims the cells whose
# centres lie inside it, or the one cell its centre is in): sense patches and eyes, the
# parts smaller than a voxel that must not vanish into the body.
ACCENTS = {"magenta", "a_eye"}

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
    },
    # plum bell (lighter toward the mouth), lilac wings, cyan leading edges, mauve wing
    # roots, peach tendril, magenta rim and underside
    "bellwing": {
        "a_main": ("#5E1764", "#2E0D3E"), "a_membrane": ("#A580F2", "#C19BFC"),
        "a_glow": ("#5ABCEF", "#95E7FC"), "a_limb": ("#9C547D", "#9C547D"),
        "a_tendril": ("#EEB4A7", "#F1C4B9"), "magenta": ("#C0099E", "#E20CAF"),
    },
}

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

def tube(pts, r0, r1, mat, seg=8):
    """Cylinders through a polyline, radius r0 -> r1, with a sphere at each inner joint."""
    n = len(pts) - 1
    for k in range(n):
        cyl(pts[k], pts[k + 1], r0 + (r1 - r0) * k / n, r0 + (r1 - r0) * (k + 1) / n, mat=mat, seg=seg)
        if k:
            rr = r0 + (r1 - r0) * k / n
            ell(pts[k], (rr, rr, rr), mat)


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
    loft(spine, [c("a_main"), c("a_dark"), c("a_pale")], body_zone, nring=90, nseg=24)
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
    # magenta eye under them each side, the cyan muzzle at the front
    ell((0.58, 0, 0.55), (0.15, 0.1, 0.11), c("a_main"), rot=ry(8))
    for sy in (-1, 1):
        ell((0.53, sy * 0.045, 0.63), (0.27, 0.055, 0.07), c("a_head"), rot=ry(17) @ rx(sy * 8))
        ell((0.565, sy * 0.075, 0.55), (0.08, 0.022, 0.05), c("a_eye"), rot=ry(10))
    ell((0.73, 0, 0.485), (0.08, 0.06, 0.042), c("a_glow"), rot=ry(8))

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

def frondgrazer():
    tops = [(0.42, 0.20), (0.39, 0.26), (0.30, 0.35), (0.19, 0.375), (0.075, 0.345), (0.0, 0.31),
            (-0.09, 0.32), (-0.225, 0.35), (-0.315, 0.32), (-0.39, 0.23), (-0.42, 0.18)]
    edge = 0.17                              # the shield's lower edge, its widest line
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
                ell((xx, sy * (surface(xx, z) + 0.002), z), (r, 0.004, r * 1.4), c("magenta"))
    # six short stout legs: dark band over two-toed hooves
    for hx, fx in ((0.25, 0.33), (-0.03, -0.03), (-0.31, -0.33)):
        for sy in (-1, 1):
            hip = (hx, sy * 0.12, 0.18)
            knee = ((hx + fx) / 2, sy * 0.15, 0.1)
            foot = (fx, sy * 0.155, 0.05)
            tube([hip, knee, foot], 0.048, 0.04, c("a_limb"), seg=12)
            cyl((fx, sy * 0.155, 0.032), (fx, sy * 0.155, 0.055), 0.042, 0.042, mat=c("a_dark"), seg=12)
            for toe in (-1, 1):
                ell((fx + 0.012, sy * 0.155 + toe * 0.019, 0.02), (0.03, 0.02, 0.022), c("a_foot"))
    # the folded neck, rising out of the shield's front to the transverse cropping scoop
    neck = [(0.29, 0, 0.2), (0.34, 0, 0.24), (0.40, 0, 0.29), (0.45, 0, 0.35), (0.48, 0, 0.385)]
    tube(neck, 0.065, 0.045, c("a_limb"), seg=14)
    # the scoop: a wide trumpet lip across the neck's top, its pale lining facing up
    ell((0.49, 0, 0.4), (0.055, 0.1, 0.03), c("a_limb"))
    ell((0.493, 0, 0.418), (0.042, 0.088, 0.014), c("a_head"))
    # two corner stalks, each ending in an eye: magenta rim, dark plum lens
    for sy in (-1, 1):
        tube([(0.49, sy * 0.09, 0.41), (0.5, sy * 0.12, 0.44), (0.505, sy * 0.15, 0.465)],
             0.008, 0.006, c("a_limb"), seg=6)
        ell((0.507, sy * 0.155, 0.47), (0.018, 0.018, 0.018), c("a_glow"))
        ell((0.515, sy * 0.162, 0.472), (0.012, 0.012, 0.012), c("a_eye"))


# --- the table ------------------------------------------------------------------------------

# name: (builder, reference length, width, height in metres). The founders' references are
# their adult bodies (`assets/voxel-models/source.json`); a specimen is baked at its own.
REF = {
    "chorister": (chorister, 1.4, 0.35, 0.6),
    "frondgrazer": (frondgrazer, 0.75, 0.375, 0.375),
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
    return objs


# --- contact sheets ---------------------------------------------------------------------------

PICKS = {
    "chorister": "chorister.png", "frondgrazer": "frondgrazer.png",
    "littershredder": "littershredder.png", "ripple-snail": "ripple-snail-organic.png",
    "capgnawer": "capgnawer-rose-bark.png", "seedporter": "seedporter.png", "bellwing": "bellwing.png",
}
# The pick's view: camera azimuth (degrees from +x, counter-clockwise seen from above; -90
# looks from -y at the left flank) and elevation.
VIEWS = {
    "chorister": (-80.0, 8.0), "frondgrazer": (100.0, 8.0), "littershredder": (115.0, 20.0),
    "ripple-snail": (-60.0, 22.0), "capgnawer": (20.0, 22.0), "seedporter": (0.0, 5.0),
    "bellwing": (0.0, 4.0),
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
        cells = B.voxelize_animal(objs, vox)
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
        t = 1.0 if top <= 0 else round(k / top * 3) / 3
        key = f"vox:{name}:{slot}:{t:.2f}"
        L.PAL[key] = _mix(lo, hi, t)
        objs.append(box(((i + 0.5) * vox, (j + 0.5) * vox, (k + 0.5) * vox), (vox, vox, vox), key))
    return objs


def main():
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if "--sheets" in args:
        out = args[args.index("--sheets") + 1]
        only = set(filter(None, (args[args.index("--only") + 1] if "--only" in args else "").split(",")))
        sheets(out, only)


if __name__ == "__main__":
    main()
