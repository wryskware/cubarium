"""Animal body blockouts, round 2: four reimagined options from Astra's prose.

    blender -b -P scripts/blender/animal_bodies_r2.py -- OUT_DIR           rows, true scale, .blend, strips
    blender -b -P scripts/blender/animal_bodies_r2.py -- OUT_DIR --v2      rebuilt CH/RS/CG, colourways, v2 strips
    blender -b -P scripts/blender/animal_bodies_r2.py -- OUT_DIR --v3      the lofted chorister, its cells, strip, comparison
    blender -b -P scripts/blender/animal_bodies_r2.py -- --strip CODE OUT [--v2]  (internal: one strip's views)

Source: design/animal-body-reimagining-2026-09-24.md, the Body paragraphs of
CH-A2 (chorister), RS-D2 (ripple-snail), CG-D2 (capgnawer) and SP-AB2 (seedporter).
Sizes: each species section's lead paragraph and design/organism-large-animals-2026-09-23.md.
Rough blocks for judging silhouette, posture and size; nothing here is a finished body.

Every animal faces +x. Renders use the panel's oblique projection (organism_lineup: each
object sheared so depth y lifts it by y/2, then viewed face-on) over the 0.125 m checker
(animal_bodies.checker). Strips reuse scripts/blender/blockout_strip.py's animal mode
unchanged: this script stands in an `animal_bodies` module whose ROWS lists these codes,
runs blockout_strip.py, and stitches its five 512 px views into strip-<code>.png.

Choices where the prose is open (also in the run's README):
  CH-A2  standing on all six feet for the static views (the prose's rest folds the knees);
         leg pairs at x = +0.30, +0.02, -0.50 m (windows 0.28 and 0.52 m); plates stepped
         0.60 / 0.55 / 0.50 m high; jaws spread 22 deg each side (34 in the stride); tail
         level at 0.46 m. Motion: tripod stride, jaws parted, bladder swollen.
  RS-D2  case 0.19 long, 0.125 high incl. foot (3:2), foot 0.19 wide (1:1 footprint);
         static views with the lobes extended (the silhouette line), motion = crawling on
         a film patch with a cleared track behind.
  CG-D2  0.25 long, 0.19 high (4:3), 0.18 wide; belly plum; static = closed at rest,
         motion = feeding with the mantles opened against two glowcap caps.
  SP-AB2 body 0.5 long, 0.25 resting height (2:1), 0.18 wide; tail 0.6 m (unstated);
         static = climbing rest on a branch, skin gathered in flank folds, tail coiled
         round the branch, one hand holding a fruit (warm pulp) at the mouth;
         motion = descending glide, front and middle limbs spread with the membranes
         taut between them, hind feet reaching forward, tail trailing.
"""

import math
import os
import subprocess
import sys
import types

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import organism_lineup as L  # noqa: E402
import animal_bodies as A  # noqa: E402  (import-safe; adds check0/check1 etc. to L.PAL)

L.PAL.update({"dark": "#12093A", "film": "#1F4E8C", "track": "#2A2148"})
cyl, ell, box = L.cyl, L.ell, L.box
VOX = L.VOX


def rz(deg):
    return Matrix.Rotation(math.radians(deg), 4, "Z")


def rx(deg):
    return Matrix.Rotation(math.radians(deg), 4, "X")


def ry(deg):
    return Matrix.Rotation(math.radians(deg), 4, "Y")


def tube(pts, r0, r1, mat, seg=8):
    n = len(pts) - 1
    for k in range(n):
        cyl(pts[k], pts[k + 1], r0 + (r1 - r0) * k / n, r0 + (r1 - r0) * (k + 1) / n, mat=mat, seg=seg)
        if k:
            rr = r0 + (r1 - r0) * k / n
            ell(pts[k], (rr, rr, rr), mat)


def sheet(pts, mat, th=0.006):
    """A flat membrane through a polygon of 3D points (a thin slab, both faces)."""
    bm = bmesh.new()
    top = [bm.verts.new(Vector(p) + Vector((0, 0, th / 2))) for p in pts]
    bot = [bm.verts.new(Vector(p) - Vector((0, 0, th / 2))) for p in pts]
    bm.faces.new(top)
    bm.faces.new(list(reversed(bot)))
    n = len(pts)
    for k in range(n):
        bm.faces.new((top[k], top[(k + 1) % n], bot[(k + 1) % n], bot[k]))
    return L.mk("sheet", bm, mat)


# --- CH-A2 · six-footed split-jaw runner (1.4 x 0.35 x 0.6 m + 0.8 m tail) ------------

def ch_a2(x, y, pose="stand"):
    stride = pose == "stride"
    Lb, sh = 1.4, 0.6
    # torso: a plum under-mass and three stepped violet plates with two valleys between
    ell((x - 0.05, y, 0.38), (0.6, 0.15, 0.12), "plum")
    for cx, top, tilt in ((0.26, 0.60, -12), (-0.1, 0.55, -10), (-0.45, 0.50, -8)):
        ell((x + cx, y, top - 0.1), (0.16, 0.16, 0.1), "violet", rot=ry(tilt))
        ell((x + cx, y, top - 0.17), (0.17, 0.165, 0.08), "plum", rot=ry(tilt))
    # coarse ridges round the shoulders
    for k, (dx, dy) in enumerate(((0.3, 0.1), (0.34, -0.09), (0.22, 0.13), (0.2, -0.13), (0.36, 0.0))):
        box((x + dx, y + dy, 0.6 - 0.02 * (k % 2)), (0.05, 0.02, 0.03), "violet", rot=rz(30 * (k - 2)))
    # throat, split jaws with an open V, lilac inner faces
    cyl((x + 0.38, y, 0.44), (x + 0.5, y, 0.43), 0.06, 0.05, mat="plum", seg=10)
    spread = 34 if stride else 22
    for sy in (-1, 1):
        m = Matrix.Translation((x + 0.5, y + sy * 0.03, 0.42)) @ rz(sy * spread)
        box(m @ Vector((0.1, 0, 0)), (0.2, 0.03, 0.09), "plum", rot=rz(sy * spread))
        box(m @ Vector((0.1, -sy * 0.017, 0)), (0.18, 0.006, 0.07), "lilac", rot=rz(sy * spread))
        # cheek ridges with sense patches
        ell((x + 0.4, y + sy * 0.1, 0.52), (0.06, 0.03, 0.025), "violet")
        box((x + 0.41, y + sy * 0.11, 0.55), (0.04, 0.02, 0.012), "magenta")
    # call bladder in a dark opening under the throat
    ell((x + 0.43, y, 0.34), (0.06, 0.05, 0.05), "dark")
    b = 0.05 if stride else 0.035
    ell((x + 0.44, y, 0.33), (b, b * 0.9, b), "p2")
    # tail: stiff, level
    tube([(x - 0.6, y, 0.46), (x - 1.0, y, 0.47), (x - 1.45, y, 0.47)], 0.07, 0.02, "violet", seg=10)
    # six legs in three unequally spaced pairs; broad two-lobed feet
    pairs = (0.30, 0.02, -0.50)
    for pi, px in enumerate(pairs):
        for sy in (-1, 1):
            lifted = stride and ((pi % 2 == 0) == (sy > 0))  # tripods: FL, MR, RL / FR, ML, RR
            adv = 0.12 if lifted else (-0.06 if stride else 0.0)
            hip = Vector((x + px, y + sy * 0.12, 0.36))
            foot = Vector((x + px + adv, y + sy * 0.24, 0.1 if lifted else 0.0))
            knee = Vector((x + px + 0.08 + adv * 0.5, y + sy * 0.28, 0.28 if lifted else 0.22))
            cyl(hip, knee, 0.04, 0.032, mat="plum", seg=8)
            cyl(knee, foot + Vector((0, 0, 0.02)), 0.03, 0.025, mat="plum", seg=8)
            for lob in (-1, 1):
                ell(foot + Vector((0.035, lob * 0.022, 0.015)), (0.035, 0.018, 0.015), "plum")


# --- RS-D2 · open-front folded case (0.19 x 0.19 footprint, 3:2 profile) ---------------

def rs_d2(x, y, pose="feed"):
    crawl = pose == "crawl"
    if crawl:  # the film it grazes, and a cleared track behind the trailing rim
        box((x + 0.06, y, 0.001), (0.5, 0.26, 0.002), "film")
        box((x - 0.2, y, 0.0025), (0.3, 0.1, 0.002), "track")
    # one dark adhesive foot beneath the whole case
    ell((x, y, 0.012), (0.1, 0.095, 0.014), "dark")
    # the plum grit tube, front at +x
    cyl((x - 0.08, y, 0.063), (x + 0.07, y, 0.063), 0.052, 0.058, mat="plum", seg=16)
    ell((x - 0.08, y, 0.063), (0.02, 0.05, 0.05), "plum")
    # the rear opening narrows round the foot
    ell((x - 0.1, y, 0.03), (0.012, 0.03, 0.02), "dark")
    # front: a thick rolled rim round an oval opening
    for k in range(16):
        a0, a1 = 2 * math.pi * k / 16, 2 * math.pi * (k + 1) / 16
        p0 = (x + 0.072, y + 0.056 * math.cos(a0), 0.065 + 0.052 * math.sin(a0))
        p1 = (x + 0.072, y + 0.056 * math.cos(a1), 0.065 + 0.052 * math.sin(a1))
        cyl(p0, p1, 0.011, mat="plum", seg=6)
    ell((x + 0.071, y, 0.065), (0.004, 0.048, 0.044), "dark")
    # two violet folds across the top, one oblique seam between them
    ell((x + 0.02, y + 0.01, 0.11), (0.06, 0.05, 0.014), "violet", rot=rz(-25))
    ell((x - 0.045, y - 0.01, 0.105), (0.055, 0.05, 0.014), "violet", rot=rz(-25))
    box((x - 0.012, y, 0.118), (0.005, 0.1, 0.004), "dark", rot=rz(-25))
    # two lilac feeding lobes down from the opening, a V of water between them
    reach = 0.09 if crawl else 0.06
    for sy in (-1, 1):
        root = Vector((x + 0.075, y + sy * 0.018, 0.05))
        tip = Vector((x + 0.075 + reach, y + sy * 0.04, 0.006))
        cyl(root, tip, 0.016, 0.012, mat="lilac", seg=10)
        ell(tip, (0.018, 0.014, 0.006), "lilac")
        box((x + 0.08, y + sy * 0.022, 0.058), (0.012, 0.01, 0.01), "magenta")


# --- CG-D2 · folded spore mantle (0.25 x 0.19 high, 4:3) --------------------------------

def cg_d2(x, y, pose="rest"):
    feed = pose == "feed"
    if feed:  # two glowcap caps it braces against
        for sy in (-1, 1):
            L.glowcap_cap(x + 0.12, y + sy * 0.15, 0.0, 0.2, 0.05)
    # compact belly
    ell((x - 0.01, y, 0.07), (0.11, 0.075, 0.05), "plum")
    # two skin mantles from the flanks, folded toward each other over the back
    # rest: each mantle leans 18 deg inward so the two meet over the back round a cleft;
    # feeding: they swing 30 deg outward to brace against the caps
    lean = -30 if feed else 18
    for sy in (-1, 1):
        rot = rx(sy * lean)
        m = Matrix.Translation((x - 0.01, y + sy * 0.05, 0.07)) @ rot
        ell(m @ Vector((0, sy * 0.012, 0.055)), (0.12, 0.035, 0.075), "plum", rot=rot)
        for k, dx in enumerate((-0.05, 0.0, 0.05)):
            ell(m @ Vector((dx, sy * 0.042, 0.05)), (0.018, 0.01, 0.06), "violet", rot=rot)
        # rounded top edge with dense lilac fibres
        edge = [m @ Vector((dx, sy * 0.0, 0.128 - 0.0006 * (dx * 100) ** 2)) for dx in (-0.1, -0.05, 0.0, 0.05, 0.1)]
        tube(edge, 0.009, 0.009, "lilac", seg=8)
    # rasping lip between two thick palps, a gap between them, magenta on their inner faces
    lip_x = 0.125 if feed else 0.11
    ell((x + lip_x, y, 0.06), (0.018, 0.02, 0.012), "lilac")
    for sy in (-1, 1):
        cyl((x + 0.08, y + sy * 0.03, 0.07), (x + 0.13, y + sy * 0.032, 0.05), 0.016, 0.013, mat="violet", seg=8)
        box((x + 0.12, y + sy * 0.019, 0.056), (0.018, 0.004, 0.012), "magenta")
    # six short gripping legs, three pairs with two windows per side
    for px in (0.06, -0.01, -0.08):
        for sy in (-1, 1):
            cyl((x + px, y + sy * 0.05, 0.05), (x + px + 0.01, y + sy * 0.085, 0.0), 0.011, 0.009, mat="plum", seg=6)
            ell((x + px + 0.012, y + sy * 0.088, 0.006), (0.014, 0.01, 0.006), "dark")


# --- SP-AB2 · six-limbed grove glider (0.5 x 0.25 m body, tail 0.6 m) -------------------

def _sp_head(x, y, z):
    ell((x + 0.2, y, z + 0.02), (0.07, 0.065, 0.06), "violet")
    for sy in (-1, 1):
        ell((x + 0.22, y + sy * 0.05, z - 0.005), (0.04, 0.03, 0.035), "plum")   # cheek pouch
        box((x + 0.23, y + sy * 0.04, z + 0.055), (0.025, 0.018, 0.01), "magenta")
    box((x + 0.265, y, z - 0.01), (0.012, 0.03, 0.014), "lilac")  # mouth cleft


def sp_ab2(x, y, pose="perch"):
    if pose == "glide":
        z = 0.75
        ell((x, y, z), (0.19, 0.075, 0.055), "violet")
        ell((x, y, z - 0.025), (0.17, 0.06, 0.035), "plum")
        _sp_head(x, y, z - 0.01)
        for sy in (-1, 1):
            sh_f = Vector((x + 0.12, y + sy * 0.05, z))
            hand = Vector((x + 0.22, y + sy * 0.34, z + 0.01))
            sh_m = Vector((x - 0.04, y + sy * 0.05, z))
            mfoot = Vector((x - 0.06, y + sy * 0.36, z - 0.01))
            tube([sh_f, sh_f.lerp(hand, 0.5) + Vector((0, 0, 0.02)), hand], 0.014, 0.01, "plum", seg=6)
            tube([sh_m, sh_m.lerp(mfoot, 0.5) + Vector((0, 0, 0.02)), mfoot], 0.014, 0.01, "plum", seg=6)
            # the membrane between the front and middle limbs, taut
            sheet([sh_f + Vector((0, 0, 0.004)), hand, mfoot, sh_m + Vector((0, 0, 0.004))], "violet")
            tube([hand, mfoot], 0.007, 0.007, "lilac", seg=6)                     # lilac edge
            tube([sh_f, sh_m], 0.014, 0.014, "plum", seg=6)                        # attachment band
            # hind feet reach forward toward the next grip, open air behind the membrane
            hip = Vector((x - 0.15, y + sy * 0.05, z - 0.02))
            tube([hip, Vector((x - 0.13, y + sy * 0.12, z - 0.1)), Vector((x - 0.02, y + sy * 0.13, z - 0.14))],
                 0.013, 0.009, "plum", seg=6)
        tube([(x - 0.19, y, z), (x - 0.4, y, z - 0.02), (x - 0.6, y, z + 0.03), (x - 0.72, y, z + 0.1)],
             0.022, 0.008, "violet", seg=8)
        return
    # perch: climbing rest on a branch, skin gathered in flank folds, tail coiled round it
    bz = 0.32
    cyl((x - 0.55, y, bz), (x + 0.4, y, bz), 0.035, mat="bark", seg=12)
    for sx in (-0.5, 0.35):
        cyl((x + sx, y, 0.0), (x + sx, y, bz), 0.03, mat="bark", seg=10)
    z = bz + 0.035 + 0.1
    ell((x, y, z), (0.18, 0.075, 0.1), "violet", rot=ry(-8))
    ell((x, y, z - 0.04), (0.16, 0.065, 0.06), "plum", rot=ry(-8))
    _sp_head(x, y, z + 0.06)
    for sy in (-1, 1):
        # heavy flank folds: the gathered membrane
        ell((x + 0.02, y + sy * 0.075, z - 0.02), (0.12, 0.02, 0.045), "violet")
        tube([(x + 0.1, y + sy * 0.09, z - 0.05), (x - 0.06, y + sy * 0.095, z - 0.06)], 0.008, 0.008, "lilac", seg=6)
        # middle pair: long, folded, gripping the branch
        tube([(x - 0.02, y + sy * 0.06, z - 0.03), (x + 0.02, y + sy * 0.1, z - 0.1),
              (x - 0.03, y + sy * 0.05, bz + 0.03)], 0.014, 0.01, "plum", seg=6)
        # rear pair: short, gripping
        tube([(x - 0.14, y + sy * 0.05, z - 0.05), (x - 0.17, y + sy * 0.07, bz + 0.03)], 0.013, 0.01, "plum", seg=6)
    # front pair: one hand on the branch, one holding a fruit at the mouth
    tube([(x + 0.12, y - 0.05, z), (x + 0.2, y - 0.08, z - 0.08), (x + 0.18, y - 0.04, bz + 0.03)], 0.013, 0.01, "plum", seg=6)
    tube([(x + 0.12, y + 0.05, z), (x + 0.2, y + 0.09, z - 0.02), (x + 0.28, y + 0.03, z + 0.04)], 0.013, 0.01, "plum", seg=6)
    ell((x + 0.3, y + 0.02, z + 0.045), (0.028, 0.028, 0.028), "lilac")
    ell((x + 0.312, y + 0.02 - 0.012, z + 0.05), (0.012, 0.012, 0.012), "warm")
    # tail: leaves the rear and coils round the branch
    pts = [(x - 0.17, y, z - 0.02), (x - 0.3, y, z - 0.06)]
    for k in range(18):
        a = math.radians(90 + 40 * k)
        pts.append((x - 0.33 - 0.006 * k, y + math.sin(a) * 0.06, bz + math.cos(a) * 0.06))
    tube(pts, 0.022, 0.008, "violet", seg=6)


SPECIES = [
    ("chorister", "CH-A2", "six-footed split-jaw runner", ch_a2, "stride", 2.6, "11 x 3 x 5 v + tail 6.5"),
    ("ripple-snail", "RS-D2", "open-front folded case", rs_d2, "crawl", 0.42, "1.5 x 1.5 x 1 v"),
    ("capgnawer", "CG-D2", "folded spore mantle", cg_d2, "feed", 0.55, "2 x 1.5 x 1.5 v"),
    ("seedporter", "SP-AB2", "six-limbed grove glider", sp_ab2, "glide", 1.35, "4 x 1.5 x 2 v + tail 5"),
]


def place(fn, x, y, yaw=0.0, roll=0.0, pivot_z=0.0, **kw):
    """Build fn at the origin and move it to (x, y), turned yaw degrees about z (and, for
    a top-down look at a glide, rolled about x round height pivot_z)."""
    before = set(bpy.data.objects)
    fn(0.0, 0.0, **kw)
    m = Matrix.Translation((x, y, 0)) @ rz(yaw)
    if roll:
        m = m @ Matrix.Translation((0, 0, pivot_z)) @ rx(roll) @ Matrix.Translation((0, 0, -pivot_z))
    for ob in bpy.data.objects:
        if ob not in before and ob.type == "MESH":
            ob.data.transform(m)


def label(body, x, y, size, mat="label"):
    """A caption lying in front of the floor at depth y (below the row in the panel view)."""
    return L.label(body, x, y, 0.0, size, mat)


GROUPS = []


def group(name):
    L.COL = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(L.COL)
    GROUPS.append((name, L.COL))
    return L.COL


def build_rows(X0=0.0):
    for i, (key, code, title, fn, motion, sp, dims) in enumerate(SPECIES):
        group(f"row-{key}")
        X = X0 + i * 40.0
        views = [("side", 0.0, None, 0.0), ("yaw 45 toward", -45.0, None, 0.0), ("yaw 45 away", 45.0, None, 0.0),
                 (motion, 0.0, motion, 0.0)]
        if key == "seedporter":  # the glide again, rolled 70 deg so its back faces the viewer
            views.append(("glide, seen from above", 0.0, "glide", 70.0))
        for k, (name, yaw, pose, roll) in enumerate(views):
            x = X + k * sp * 1.25
            place(fn, x, 0.0, yaw, roll, 0.75, **({"pose": pose} if pose else {}))
            label(name, x, -sp * 0.56, sp * 0.05, "sublabel")
        xe = X + (len(views) - 1) * sp * 1.25
        A.checker(X - sp * 0.7, xe + sp * 0.7, -sp * 0.5, sp * 0.5)
        label(f"{code} {title} ({dims})", (X + xe) / 2, -sp * 0.7, sp * 0.065)


def build_true_scale(X=200.0):
    group("true-scale")
    x = X
    for key, code, title, fn, motion, sp, dims in SPECIES:
        w = {"chorister": 2.4, "ripple-snail": 0.45, "capgnawer": 0.5, "seedporter": 1.2}[key]
        place(fn, x + w / 2 + (0.45 if key == "chorister" else 0.0), 0.0)
        label(code, x + w / 2, -0.66, 0.07)
        x += w + 0.3
    A.fg(x + 0.45, 0.0)
    label("frondgrazer", x + 0.45, -0.66, 0.07, "sublabel")
    x += 1.2
    L.human(x, 0.0)
    label("1.75 m", x, -0.66, 0.07, "sublabel")
    A.checker(X - 0.4, x + 0.6, -0.6, 0.6)


def frame(name, col, width_px=2000):
    xs, zs = [], []
    for ob in col.objects:
        if ob.type == "MESH":
            for v in ob.data.vertices:
                p = ob.location + v.co
                xs.append(p.x)
                zs.append(p.z + 0.5 * p.y)
        elif ob.type == "FONT":
            xs.append(ob.location.x)
            zs.append(ob.location.z + 0.5 * ob.location.y)
    pad = 0.04 * (max(xs) - min(xs))
    x0, x1, z0, z1 = min(xs) - pad, max(xs) + pad, min(zs) - pad, max(zs) + pad
    ppm = width_px / (x1 - x0)
    w, h = int((x1 - x0) * ppm), int((z1 - z0) * ppm)
    cam = bpy.data.cameras.new(name)
    cam.type = "ORTHO"
    cam.ortho_scale = max(x1 - x0, z1 - z0)
    cam.clip_end = 500
    ob = bpy.data.objects.new(name, cam)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = ((x0 + x1) / 2, -100, (z0 + z1) / 2)
    ob.rotation_euler = (math.radians(90), 0, 0)
    return ob, w, h


# === v2 (Wrysk's blockout review, 2026-09-24) =========================================
#
# CH, RS and CG are rebuilt from the chosen Qwen images (art/gen/runs/2026-09-24-reimagining-
# concepts/<code>-2401-b0.png): the image sets the shape, the prose fills the rest. SP-AB2
# keeps its shape. Every animal, plus FG, LT and BW-A, gets two colourway proposals: a main
# hue family, a shadow, a pale secondary, a marking colour and glow accents, with the
# marking made of real geometry (bands, saddles, spots, stripes, rings).

COLOURWAYS = {
    # code: {cw: (name, {role: hex}, marking)}
    "CH": {1: ("violet stalker", dict(main="#6E5CD6", shadow="#2E2470", sec="#D6CCF6", mark="#1C1450",
                                      glow="#42C5F8", glow2="#FF2AFC"), "chevrons"),
           2: ("rust stalker", dict(main="#D8662A", shadow="#6A2410", sec="#F4E4CC", mark="#3A1408",
                                    glow="#42C5F8", glow2="#FF2AFC"), "rosettes")},
    "RS": {1: ("azure dome", dict(main="#4A7CF0", shadow="#141A40", sec="#A8C8FF", mark="#2A50C0",
                                  glow="#FF5AD8"), "rings"),
           2: ("jade dome", dict(main="#3AC89A", shadow="#0E3A30", sec="#D8FFF0", mark="#0E7A5A",
                                 glow="#FF5AD8"), "rays")},
    "CG": {1: ("rose-mauve", dict(main="#9A5A8C", shadow="#4A2446", sec="#F0C8E4", mark="#E6A6D8",
                                  glow="#FF4AC0"), "spots"),
           2: ("lichen", dict(main="#C8B040", shadow="#5A4A10", sec="#F8ECB0", mark="#6A5410",
                              glow="#FF4AC0"), "saddle")},
    "SP": {1: ("moss", dict(main="#5A9A3A", shadow="#23461E", sec="#D8F0A8", mark="#162E12",
                            glow="#FF2AFC"), "stripe"),
           2: ("orchid", dict(main="#A8429E", shadow="#4A1646", sec="#FFD0F4", mark="#F8A0E8",
                              glow="#5FF0FF"), "spots")},
    "FG": {1: ("coral", dict(main="#FF8A6E", shadow="#A22A4A", sec="#FFE0CC", mark="#6A1A30",
                             glow="#FF2AFC"), "saddles"),
           2: ("periwinkle", dict(main="#6A7AF0", shadow="#2E2A8A", sec="#E0E4FF", mark="#FFD860",
                                  glow="#5FF0FF"), "spots")},
    "LT": {1: ("gold", dict(main="#FFD860", shadow="#B06A18", sec="#FFF4C8", mark="#5A3208",
                            glow="#FF2AFC"), "bands"),
           2: ("teal", dict(main="#4ACAB8", shadow="#0E5A5A", sec="#D8FFF6", mark="#0A2A2A",
                            glow="#FF8A3A"), "stripe")},
    "BW": {1: ("turquoise", dict(main="#1E9AA8", shadow="#0A4A5A", sec="#A8F4F0", mark="#E8FFFF",
                                 glow="#FF2AFC"), "rings"),
           2: ("coral-pink", dict(main="#E0507A", shadow="#7A1A3A", sec="#FFD0DC", mark="#5FF0FF",
                                  glow="#5FF0FF"), "stripes")},
}
MARKING_TEXT = {
    "chevrons": "three dark chevron bands across the back and a dark tail tip; pale head and belly (countershading)",
    "rosettes": "dark rosette spots along both flanks and the tail; pale head and belly (countershading)",
    "rings": "concentric growth-ring bands (CH/RS) or glowing rings (BW) around the body",
    "rays": "radial stripes running down the dome from its crown",
    "spots": "scattered spots over the back, mantle or shield",
    "saddle": "one dark saddle band across the middle of the mantle",
    "stripe": "one dorsal stripe from head to tail; pale underside",
    "saddles": "dark saddle patches on both shield lobes",
    "bands": "alternating dark bands, one per body segment",
    "stripes": "vertical stripes round the bell",
}
CUR = {}


def use(code, cw):
    """Select a colourway: registers its materials and returns the marking name."""
    name, hexes, marking = COLOURWAYS[code][cw]
    CUR.clear()
    for role, hx in hexes.items():
        key = f"{code}{cw}-{role}"
        L.PAL[key] = hx
        CUR[role] = key
    CUR["_mark"] = marking
    return marking


def c(role):
    return CUR[role]


# --- CH-A2 v2: crouched stalker (image CH-A2-2401-b0), six legs, tapered tail ----------

def ch_v2(x, y, pose="stand"):
    stride = pose == "stride"
    mk = CUR["_mark"]
    # lean torso: one continuous mass sloping from the high chest to the lower hips
    def torso(t):  # t 0 chest .. 1 hips -> centre x, z and radii
        return (0.28 - 0.74 * t, 0.53 - 0.12 * t, (0.15 - 0.02 * t, 0.14 - 0.025 * t, 0.125 - 0.03 * t))
    # one long ellipsoid from chest to hips, a chest mass, one pale belly under both
    ell((x - 0.09, y, 0.47), (0.42, 0.125, 0.11), c("main"), rot=ry(-9))
    ell((x + 0.2, y, 0.52), (0.17, 0.14, 0.125), c("main"), rot=ry(-9))
    ell((x - 0.05, y, 0.43), (0.42, 0.115, 0.075), c("sec"), rot=ry(-9))
    if mk == "chevrons":  # three bands hugging the back and flanks
        for t in (0.15, 0.45, 0.75):
            bx, bz, r = torso(t)
            ell((x + bx, y, bz + 0.012), (0.032, r[1] * 1.035, r[2] * 1.02), c("mark"), rot=ry(-30))
    else:  # rosette spots along the flanks, flush with the body
        for t, dz in ((0.05, 0.02), (0.22, -0.01), (0.38, 0.03), (0.55, 0.0), (0.72, 0.03), (0.88, 0.0)):
            bx, bz, r = torso(t)
            for sy in (-1, 1):
                ell((x + bx, y + sy * r[1] * 0.93, bz + dz), (0.03, 0.014, 0.024), c("mark"))
    # neck and pale head with two swept lobes, magenta eye patch, cyan jaw
    cyl((x + 0.32, y, 0.53), (x + 0.45, y, 0.5), 0.06, 0.05, mat=c("main"), seg=10)
    ell((x + 0.52, y, 0.5), (0.1, 0.075, 0.075), c("sec"), rot=ry(10))
    for sy in (-1, 1):
        ell((x + 0.47, y + sy * 0.045, 0.565), (0.13, 0.022, 0.04), c("sec"), rot=ry(18))   # swept lobe
        ell((x + 0.46, y + sy * 0.046, 0.575), (0.09, 0.023, 0.012), c("mark"), rot=ry(18))  # lobe stripe
        ell((x + 0.55, y + sy * 0.066, 0.5), (0.04, 0.012, 0.032), c("glow2"))            # eye patch
    jaw = 0.035 if stride else 0.028
    ell((x + 0.58, y, 0.44), (0.06, 0.05, jaw), c("glow"))
    # tapered tail, a little below level, dark tip on the chevron colourway
    tail = [(x - 0.52, y, 0.41), (x - 0.75, y, 0.38), (x - 1.0, y, 0.34), (x - 1.32, y, 0.3)]
    tube(tail[:3], 0.07, 0.03, c("main"), seg=10)
    tube(tail[2:], 0.03, 0.006, c("mark") if mk == "chevrons" else c("main"), seg=10)
    if mk == "rosettes":
        for tx in (-0.65, -0.82):
            for sy in (-1, 1):
                ell((x + tx, y + sy * 0.05, 0.39), (0.025, 0.012, 0.02), c("mark"))
    # six long slender legs: front pair reaching forward, middle under the chest, hind crouched
    legs = [((0.28, 0.46), (0.36, 0.26), (0.58, 0.0)),
            ((0.02, 0.42), (-0.06, 0.32), (0.04, 0.0)),
            ((-0.4, 0.38), (-0.24, 0.4), (-0.5, 0.12), (-0.42, 0.0))]
    for li, leg in enumerate(legs):
        for sy in (-1, 1):
            lifted = stride and ((li % 2 == 0) == (sy > 0))
            pts = []
            for k, (lx, lz) in enumerate(leg):
                last = k == len(leg) - 1
                dx = (0.14 if lifted else -0.06) if (stride and last) else 0.0
                dz = 0.08 if (lifted and last) else 0.0
                spread = 0.1 + 0.08 * min(k, 1)
                pts.append((x + lx + dx, y + sy * spread, lz + dz))
            tube(pts, 0.024, 0.016, c("shadow"), seg=8)
            fx, fy, fz = pts[-1]
            for toe in (-1, 1):
                ell((fx + 0.03, fy + toe * 0.012, fz + 0.008), (0.03, 0.01, 0.008), c("shadow"))


# --- RS-D2 v2: smooth dome over a foot skirt (image RS-D2-2401-b0) ---------------------

def rs_v2(x, y, pose="feed"):
    crawl = pose == "crawl"
    mk = CUR["_mark"]
    if crawl:
        box((x - 0.24, y, 0.0015), (0.3, 0.1, 0.003), "track")
    ell((x, y, 0.004), (0.12, 0.1, 0.006), c("shadow"))                    # the foot skirt
    ell((x - 0.01, y, 0.03), (0.095, 0.085, 0.095), c("main"))              # the dome
    for k in range(24):                                                     # rolled rim
        a0, a1 = 2 * math.pi * k / 24, 2 * math.pi * (k + 1) / 24
        cyl((x - 0.01 + 0.098 * math.cos(a0), y + 0.088 * math.sin(a0), 0.032),
            (x - 0.01 + 0.098 * math.cos(a1), y + 0.088 * math.sin(a1), 0.032), 0.01, mat=c("sec"), seg=6)
    if mk == "rings":
        for zf in (0.45, 0.72):
            rr = math.sqrt(1 - zf ** 2)
            z = 0.03 + 0.095 * zf
            for k in range(24):
                a0, a1 = 2 * math.pi * k / 24, 2 * math.pi * (k + 1) / 24
                cyl((x - 0.01 + 0.096 * rr * math.cos(a0), y + 0.086 * rr * math.sin(a0), z),
                    (x - 0.01 + 0.096 * rr * math.cos(a1), y + 0.086 * rr * math.sin(a1), z), 0.006,
                    mat=c("mark"), seg=6)
    else:  # rays down the dome
        for k in range(8):
            a = 2 * math.pi * k / 8 + 0.2
            pts = []
            for t in (0.15, 0.4, 0.65, 0.9):
                el = math.pi / 2 * (1 - t)
                pts.append((x - 0.01 + 0.097 * math.cos(el) * math.cos(a), y + 0.087 * math.cos(el) * math.sin(a),
                            0.03 + 0.097 * math.sin(el)))
            tube(pts, 0.007, 0.007, c("mark"), seg=6)
    # the dark sleeve under the front edge, and two pale lobes reaching forward
    cyl((x + 0.05, y, 0.03), (x + 0.1, y, 0.026), 0.03, 0.028, mat=c("shadow"), seg=14)
    reach = 0.12 if crawl else 0.09
    for sy in (-1, 1):
        root = Vector((x + 0.1, y + sy * 0.013, 0.028))
        tip = Vector((x + 0.1 + reach, y + sy * 0.03, 0.008))
        cyl(root, tip, 0.012, 0.011, mat=c("sec"), seg=10)
        ell(tip, (0.018, 0.013, 0.009), c("glow"))


# --- CG-D2 v2: squat body under a fringed mantle (image CG-D2-2401-b0) -----------------

def cg_v2(x, y, pose="rest"):
    feed = pose == "feed"
    mk = CUR["_mark"]
    if feed:
        for sy in (-1, 1):
            L.glowcap_cap(x + 0.15, y + sy * 0.15, 0.0, 0.2, 0.05)
    ell((x - 0.005, y, 0.072), (0.1, 0.085, 0.062), c("shadow"))            # round belly
    # the broad mantle; feeding lifts its front edge
    tilt = ry(-14) if feed else Matrix.Identity(4)
    piv = Matrix.Translation((x - 0.1, y, 0.1)) @ tilt @ Matrix.Translation((-(x - 0.1), -y, -0.1))
    ell(piv @ Vector((x, y, 0.105)), (0.13, 0.11, 0.075), c("main"), rot=tilt)
    for k in range(40):  # the fringe: short pale fibres round the mantle edge
        a = 2 * math.pi * k / 40
        p = Vector((x + 0.128 * math.cos(a), y + 0.108 * math.sin(a), 0.1))
        q = p + Vector((0.016 * math.cos(a), 0.016 * math.sin(a), -0.012))
        cyl(piv @ p, piv @ q, 0.005, 0.0015, mat=c("sec"), seg=5)
    if mk == "spots":
        for sx, sy2, r in ((0.02, 0.03, 0.02), (-0.05, -0.04, 0.018), (-0.03, 0.06, 0.014), (0.06, -0.05, 0.014),
                           (-0.08, 0.01, 0.016), (0.05, 0.06, 0.012)):
            zt = 0.105 + 0.075 * math.sqrt(max(0.0, 1 - (sx / 0.13) ** 2 - (sy2 / 0.11) ** 2))
            ell(piv @ Vector((x + sx, y + sy2, zt - 0.002)), (r, r, 0.006), c("mark"), rot=tilt)
    else:  # one dark saddle band across the mantle's middle
        for k in range(13):
            t = -1 + 2 * k / 12
            sy2 = 0.108 * t
            zt = 0.105 + 0.075 * math.sqrt(max(0.0, 1 - t ** 2)) - 0.003
            ell(piv @ Vector((x - 0.01, y + sy2, zt)), (0.028, 0.012, 0.008), c("mark"), rot=tilt)
    # the face low under the mantle's front: pink eyes, pink cheek marks, a pale lip
    ell((x + 0.1, y, 0.075), (0.05, 0.065, 0.045), c("shadow"))
    for sy in (-1, 1):
        ell((x + 0.143, y + sy * 0.03, 0.084), (0.01, 0.017, 0.013), c("glow"))
        ell((x + 0.142, y + sy * 0.036, 0.058), (0.008, 0.012, 0.007), c("glow"))
    ell((x + 0.15 if not feed else x + 0.16, y, 0.066), (0.01, 0.028, 0.007), c("sec"))
    # six short stout legs with toes
    for px in (0.06, -0.01, -0.08):
        for sy in (-1, 1):
            cyl((x + px, y + sy * 0.06, 0.05), (x + px + 0.008, y + sy * 0.1, 0.008), 0.02, 0.016, mat=c("shadow"), seg=8)
            for toe in (-1, 0, 1):
                ell((x + px + 0.02, y + sy * 0.1 + toe * 0.008, 0.005), (0.012, 0.005, 0.005), c("shadow"))


# --- SP-AB2 v2: the same shape, colourways and markings ---------------------------------

def sp_v2(x, y, pose="perch"):
    mk = CUR["_mark"]

    def head(z):
        ell((x + 0.2, y, z + 0.02), (0.07, 0.065, 0.06), c("main"))
        for sy in (-1, 1):
            ell((x + 0.22, y + sy * 0.05, z - 0.005), (0.04, 0.03, 0.035), c("shadow"))
            box((x + 0.23, y + sy * 0.04, z + 0.055), (0.025, 0.018, 0.01), c("glow"))
        box((x + 0.265, y, z - 0.01), (0.012, 0.03, 0.014), c("sec"))

    def back_marks(z, glide):
        if mk == "stripe":
            tube([(x + 0.18, y, z + 0.075), (x + 0.05, y, z + (0.055 if glide else 0.1)),
                  (x - 0.15, y, z + (0.045 if glide else 0.08))], 0.012, 0.01, c("mark"), seg=6)
        else:
            for sx, sy, zz in ((0.08, 0.03, 0.05), (-0.02, -0.035, 0.055), (-0.1, 0.02, 0.045), (0.02, 0.04, 0.06)):
                ell((x + sx, y + sy, z + (zz if glide else zz + 0.04)), (0.018, 0.018, 0.01), c("mark"))

    if pose == "glide":
        z = 0.75
        ell((x, y, z), (0.19, 0.075, 0.055), c("main"))
        ell((x, y, z - 0.025), (0.17, 0.06, 0.035), c("sec"))   # pale underside
        head(z - 0.01)
        back_marks(z, True)
        for sy in (-1, 1):
            sh_f = Vector((x + 0.12, y + sy * 0.05, z))
            hand = Vector((x + 0.22, y + sy * 0.34, z + 0.01))
            sh_m = Vector((x - 0.04, y + sy * 0.05, z))
            mfoot = Vector((x - 0.06, y + sy * 0.36, z - 0.01))
            tube([sh_f, sh_f.lerp(hand, 0.5) + Vector((0, 0, 0.02)), hand], 0.014, 0.01, c("shadow"), seg=6)
            tube([sh_m, sh_m.lerp(mfoot, 0.5) + Vector((0, 0, 0.02)), mfoot], 0.014, 0.01, c("shadow"), seg=6)
            sheet([sh_f + Vector((0, 0, 0.004)), hand, mfoot, sh_m + Vector((0, 0, 0.004))], c("main"))
            if mk == "spots":
                for f in (0.35, 0.65):
                    p = sh_f.lerp(hand, f).lerp(sh_m.lerp(mfoot, f), 0.5) + Vector((0, 0, 0.005))
                    ell(p, (0.03, 0.03, 0.004), c("mark"))
            else:
                sheet([sh_f.lerp(hand, 0.55) + Vector((0, 0, 0.005)), hand + Vector((0, 0, 0.005)),
                       mfoot + Vector((0, 0, 0.005)), sh_m.lerp(mfoot, 0.55) + Vector((0, 0, 0.005))], c("mark"), 0.004)
            tube([hand, mfoot], 0.007, 0.007, c("sec"), seg=6)
            tube([sh_f, sh_m], 0.014, 0.014, c("shadow"), seg=6)
            hip = Vector((x - 0.15, y + sy * 0.05, z - 0.02))
            tube([hip, Vector((x - 0.13, y + sy * 0.12, z - 0.1)), Vector((x - 0.02, y + sy * 0.13, z - 0.14))],
                 0.013, 0.009, c("shadow"), seg=6)
        tube([(x - 0.19, y, z), (x - 0.4, y, z - 0.02), (x - 0.6, y, z + 0.03), (x - 0.72, y, z + 0.1)],
             0.022, 0.008, c("main"), seg=8)
        return
    bz = 0.32
    cyl((x - 0.55, y, bz), (x + 0.4, y, bz), 0.035, mat="bark", seg=12)
    for sx in (-0.5, 0.35):
        cyl((x + sx, y, 0.0), (x + sx, y, bz), 0.03, mat="bark", seg=10)
    z = bz + 0.035 + 0.1
    ell((x, y, z), (0.18, 0.075, 0.1), c("main"), rot=ry(-8))
    ell((x, y, z - 0.045), (0.16, 0.065, 0.055), c("sec"), rot=ry(-8))   # pale belly
    head(z + 0.06)
    back_marks(z, False)
    for sy in (-1, 1):
        ell((x + 0.02, y + sy * 0.075, z - 0.02), (0.12, 0.02, 0.045), c("main"))    # flank fold
        tube([(x + 0.1, y + sy * 0.09, z - 0.05), (x - 0.06, y + sy * 0.095, z - 0.06)], 0.008, 0.008, c("sec"), seg=6)
        tube([(x - 0.02, y + sy * 0.06, z - 0.03), (x + 0.02, y + sy * 0.1, z - 0.1),
              (x - 0.03, y + sy * 0.05, bz + 0.03)], 0.014, 0.01, c("shadow"), seg=6)
        tube([(x - 0.14, y + sy * 0.05, z - 0.05), (x - 0.17, y + sy * 0.07, bz + 0.03)], 0.013, 0.01, c("shadow"), seg=6)
    tube([(x + 0.12, y - 0.05, z), (x + 0.2, y - 0.08, z - 0.08), (x + 0.18, y - 0.04, bz + 0.03)], 0.013, 0.01, c("shadow"), seg=6)
    tube([(x + 0.12, y + 0.05, z), (x + 0.2, y + 0.09, z - 0.02), (x + 0.28, y + 0.03, z + 0.04)], 0.013, 0.01, c("shadow"), seg=6)
    ell((x + 0.3, y + 0.02, z + 0.045), (0.028, 0.028, 0.028), "lilac")
    ell((x + 0.312, y + 0.02 - 0.012, z + 0.05), (0.012, 0.012, 0.012), "warm")
    pts = [(x - 0.17, y, z - 0.02), (x - 0.3, y, z - 0.06)]
    for k in range(18):
        a = math.radians(90 + 40 * k)
        pts.append((x - 0.33 - 0.006 * k, y + math.sin(a) * 0.06, bz + math.cos(a) * 0.06))
    tube(pts, 0.022, 0.008, c("main"), seg=6)


# --- FG, LT, BW-A: the lineup builders, recoloured, with marking geometry ---------------

def _recolour(before, mapping):
    for ob in bpy.data.objects:
        if ob not in before and ob.type == "MESH" and ob.active_material:
            new = mapping.get(ob.active_material.name)
            if new:
                ob.data.materials[0] = L.M(new)


def fg_v2(x, y, pose=None):
    Lg, Wg, Hg = L.LADDER_ANIMALS["frondgrazer"]
    before = set(bpy.data.objects)
    L.frondgrazer(x, y, Lg, Wg, Hg, overlays=False)
    _recolour(before, {"plum": c("shadow"), "violet": c("main"), "lilac": c("sec"), "magenta": c("glow")})
    ell((x, y, 0.45 * Hg), (0.46 * Lg, 0.44 * Wg, 0.12 * Hg), c("sec"))  # pale underside
    if CUR["_mark"] == "saddles":
        for sx in (0.26, -0.24):
            ell((x + sx * Lg, y, 0.96 * Hg), (0.13 * Lg, 0.36 * Wg, 0.035 * Hg), c("mark"))
    else:
        for sx, sy in ((0.3, 0.2), (0.2, -0.25), (-0.15, 0.3), (-0.3, -0.1), (0.05, 0.05), (-0.05, -0.32)):
            zt = 0.68 * Hg + 0.3 * Hg * math.sqrt(max(0.0, 1 - (sy / 0.46) ** 2)) - 0.004
            ell((x + sx * Lg, y + sy * Wg, zt), (0.035, 0.035, 0.012), c("mark"))


def lt_v2(x, y, pose=None):
    Ll, Wl, Hl = L.LADDER_ANIMALS["littershredder"]
    before = set(bpy.data.objects)
    L.littershredder(x, y, Ll, Wl, Hl)
    _recolour(before, {"p0": c("main"), "lilac": c("sec"), "magenta": c("glow")})
    segs = [ob for ob in bpy.data.objects if ob not in before and ob.active_material
            and ob.active_material.name == c("main")]
    if CUR["_mark"] == "bands":
        for ob in segs[1::2]:
            ob.data.materials[0] = L.M(c("mark"))
    else:
        cyl((x + 0.42 * Ll, y, 0.98 * Hl), (x - 0.42 * Ll, y, 0.82 * Hl), 0.012, 0.008, mat=c("mark"), seg=6)
    for k in range(5):  # pale underside strip on each segment
        f = 1.0 - 0.1 * k
        ell((x + (0.38 - 0.19 * k) * Ll, y, 0.2 * Hl * f), (0.11 * Ll, 0.4 * Wl * f, 0.18 * Hl * f), c("sec"))


def bw_v2(x, y, pose=None, z=0.3):
    before = set(bpy.data.objects)
    A.bw_a(x, y, z)
    _recolour(before, {"plum": c("main"), "lilac": c("sec"), "magenta": c("glow")})
    if CUR["_mark"] == "rings":
        for f in (0.35, 0.7):
            r = 0.04 + (0.018 - 0.04) * f + 0.003
            cyl((x, y, z + 0.09 * f - 0.004), (x, y, z + 0.09 * f + 0.004), r, r, mat=c("mark"), seg=16)
    else:
        for k in range(8):
            a = 2 * math.pi * k / 8
            cyl((x + 0.042 * math.cos(a), y + 0.042 * math.sin(a), z + 0.004),
                (x + 0.02 * math.cos(a), y + 0.02 * math.sin(a), z + 0.088), 0.005, mat=c("mark"), seg=5)
    ell((x, y, z + 0.093), (0.02, 0.02, 0.008), c("shadow"))  # dark crown cap


V2 = [  # code, key, title, builder, motion pose, spacing, dims
    ("CH", "chorister", "CH-A2 crouched six-legged stalker", ch_v2, "stride", 2.6, "11 x 3 x 5 v + tail 6.5"),
    ("RS", "ripple-snail", "RS-D2 dome case on a foot skirt", rs_v2, "crawl", 0.5, "1.5 x 1.5 x 1 v"),
    ("CG", "capgnawer", "CG-D2 fringed-mantle little dude", cg_v2, "feed", 0.6, "2 x 1.75 x 1.5 v"),
    ("SP", "seedporter", "SP-AB2 six-limbed grove glider", sp_v2, "glide", 1.35, "4 x 1.5 x 2 v + tail 5"),
    ("FG", "frondgrazer", "frondgrazer", fg_v2, None, 1.1, "6 x 3 x 3 v"),
    ("LT", "littershredder", "littershredder", lt_v2, None, 0.6, "3 x 1 x 1 v"),
    ("BW", "bellwing", "bellwing BW-A", bw_v2, None, 0.6, "hovering"),
]
V2_BY = {v[0]: v for v in V2}
STRIP_CODES = {"CH": "CH-A2", "RS": "RS-D2", "CG": "CG-D2", "SP": "SP-AB2"}


def build_v2(out):
    L.PAL.update({"check0": "#6E6C78", "check1": "#7E7C88"})  # mid-grey floor
    # rows for the rebuilt three, colourway 1
    for i, code in enumerate(("CH", "RS", "CG")):
        _, key, title, fn, motion, sp, dims = V2_BY[code]
        use(code, 1)
        group(f"row-{key}-v2")
        X = i * 40.0
        views = [("side", 0.0, None), ("yaw 45 toward", -45.0, None), ("yaw 45 away", 45.0, None), (motion, 0.0, motion)]
        for k, (name, yaw, pose) in enumerate(views):
            xx = X + k * sp * 1.25
            place(fn, xx, 0.0, yaw, **({"pose": pose} if pose else {}))
            label(name, xx, -sp * 0.56, sp * 0.05, "sublabel")
        xe = X + 3 * sp * 1.25
        A.checker(X - sp * 0.7, xe + sp * 0.7, -sp * 0.5, sp * 0.5)
        label(f"{title} ({dims}), colourway 1", (X + xe) / 2, -sp * 0.7, sp * 0.065)
    # true scale, colourway 1
    group("true-scale-v2")
    X, x = 200.0, 200.0
    for code in ("CH", "RS", "CG", "SP", "FG", "LT", "BW"):
        _, key, title, fn, *_ = V2_BY[code]
        use(code, 1)
        w = {"CH": 2.4, "RS": 0.4, "CG": 0.45, "SP": 1.2, "FG": 1.0, "LT": 0.5, "BW": 0.4}[code]
        place(fn, x + w / 2 + (0.45 if code == "CH" else 0.0), 0.0)
        label(key, x + w / 2, -0.66, 0.06)
        x += w + 0.3
    L.human(x + 0.2, 0.0)
    label("1.75 m", x + 0.2, -0.66, 0.06, "sublabel")
    A.checker(X - 0.4, x + 0.8, -0.6, 0.6)
    # colourway cells: one group per animal and colourway, side view
    cells = []
    for i, code in enumerate(("CH", "SP", "FG", "RS", "CG", "LT", "BW")):
        _, key, title, fn, motion, sp, dims = V2_BY[code]
        for cw in (1, 2):
            use(code, cw)
            col = group(f"cw-{code}-{cw}")
            X = 400.0 + i * 30.0 + cw * 12.0
            place(fn, X, 0.0)
            A.checker(X - sp * 0.6, X + sp * 0.6, -sp * 0.35, sp * 0.35)
            cells.append((code, cw, col))
    return cells


def stitch_colourways(out, cells_dir, name="colourways.png"):
    """Stitch the colourway cells with labels (system python + PIL)."""
    script = r'''
import json, os, sys
from PIL import Image, ImageDraw
d, out, meta = sys.argv[1], sys.argv[2], json.loads(sys.argv[3])
cw, ch = 520, 330
im = Image.new("RGB", (cw * 7 + 20, ch * 2 + 110), (0x14, 0x12, 0x1C))
dr = ImageDraw.Draw(im)
dr.text((10, 8), "Colourway proposals, side view (each cell scaled to fit; see true-scale-v2.png for size). Row 1: colourway 1, row 2: colourway 2.", fill=(230, 221, 248))
for i, m in enumerate(meta):
    for r, cwn in enumerate((1, 2)):
        cell = Image.open(os.path.join(d, f"cw-{m['code']}-{cwn}.png")).convert("RGB")
        cell.thumbnail((cw - 10, ch - 40))
        x, y = 10 + i * cw, 30 + r * (ch + 40)
        im.paste(cell, (x, y))
        info = m["cw"][str(cwn)]
        dr.text((x, y + cell.height + 4), f"{m['code']} {m['key']} - {cwn}: {info['name']}", fill=(230, 221, 248))
        dr.text((x, y + cell.height + 18), "  ".join(info["hex"]) + f"  marks: {info['mark']}", fill=(157, 144, 196))
im.save(out)
'''
    meta = []
    for code in ("CH", "SP", "FG", "RS", "CG", "LT", "BW"):
        meta.append({"code": code, "key": V2_BY[code][1],
                     "cw": {str(k): {"name": v[0], "hex": list(v[1].values()), "mark": v[2]}
                            for k, v in COLOURWAYS[code].items()}})
    import json
    subprocess.run(["python3", "-c", script, cells_dir, os.path.join(out, name), json.dumps(meta)], check=True)


def main_v2(out):
    os.makedirs(out, exist_ok=True)
    sc = L.setup_scene()
    sc.display.shading.show_specular_highlight = False
    cells = build_v2(out)
    cams = []
    for name, col in GROUPS:
        if name.startswith("cw-"):
            cams.append((name, *frame(f"cam-{name}", col, 900)))
        else:
            cams.append((name, *frame(f"cam-{name}", col, 3000 if name.startswith("true") else 2000)))
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(out, "animal_bodies_r2_v2.blend"))
    shear = Matrix.Identity(4)
    shear[2][1] = 0.5
    root = bpy.data.objects.new("oblique", None)
    sc.collection.objects.link(root)
    for ob in list(sc.objects):
        if ob.type in {"MESH", "FONT"}:
            ob.parent = root
            ob.matrix_parent_inverse = shear
    cells_dir = os.path.join(out, "colourway-cells")
    os.makedirs(cells_dir, exist_ok=True)
    for name, cam, w, h in cams:
        path = os.path.join(cells_dir if name.startswith("cw-") else out, f"{name}.png")
        L.render(cam, path, w, h)
    stitch_colourways(out, cells_dir)
    views = os.path.join(out, "strip-views-v2")
    for code, scode in STRIP_CODES.items():
        subprocess.run([bpy.app.binary_path, "-b", "-P", os.path.abspath(__file__), "--", "--strip", scode, views,
                        "--v2"], check=True, stdout=subprocess.DEVNULL)
        stitch_strip(views, scode, os.path.join(out, f"strip-{scode}-v2.png"))
    print("done v2", flush=True)


def stitch_strip(views, code, path):
    import numpy as np
    names = ["side", "front", "back", "q3front", "q3back"]
    imgs = [bpy.data.images.load(os.path.join(views, f"{code}-{v}.png")) for v in names]
    W, H = imgs[0].size
    strip = bpy.data.images.new(f"strip-{code}", W * len(imgs), H, alpha=True)
    buf = np.zeros((H, W * len(imgs), 4), dtype=np.float32)
    for k, im in enumerate(imgs):
        buf[:, k * W:(k + 1) * W] = np.array(im.pixels[:], dtype=np.float32).reshape(H, W, 4)
    strip.pixels[:] = buf.ravel()
    strip.filepath_raw = path
    strip.file_format = "PNG"
    strip.save()


# === v3 chorister: one lofted body, muscular limbs (2026-09-24, second review) =========
#
# The v2 chorister still read as an insect. v3 skins ONE continuous tapered body from the
# neck through a deep chest and narrow hips into the tail (no seams, no bands), lofts every
# leg as a single limb with a thick upper segment sunk into the body and a slimmer lower
# leg, sets the middle pair close behind the front pair, and gives it a head about a fifth
# of the body length. Markings: countershading (dark back, pale belly) and soft rosettes.


def _catmull(pts, n):
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
    """One closed smooth mesh through stations (x, y, z, ry, rz). zone(i_frac, angle_sin)
    picks the material index per face; mats are material keys."""
    S = _catmull(stations, nring)
    bm = bmesh.new()
    rings = []
    for i, (px, py, pz, ry_, rz_) in enumerate(S):
        a = Vector(S[max(i - 1, 0)][:3])
        b = Vector(S[min(i + 1, len(S) - 1)][:3])
        t = (b - a).normalized()
        ref = Vector((0, 0, 1)) if abs(t.z) < 0.9 else Vector((1, 0, 0))
        side = t.cross(ref).normalized()
        up = side.cross(t).normalized()
        if up.z < 0:
            up, side = -up, -side
        c0 = Vector((px, py, pz))
        rings.append([bm.verts.new(c0 + side * ry_ * math.cos(2 * math.pi * k / nseg)
                                   + up * rz_ * math.sin(2 * math.pi * k / nseg)) for k in range(nseg)])
    for i in range(len(rings) - 1):
        for k in range(nseg):
            f = bm.faces.new((rings[i][k], rings[i][(k + 1) % nseg], rings[i + 1][(k + 1) % nseg], rings[i + 1][k]))
            f.material_index = zone(i / (len(rings) - 1), math.sin(2 * math.pi * (k + 0.5) / nseg))
            f.smooth = True
    for ring, rev in ((rings[0], True), (rings[-1], False)):
        cen = bm.verts.new(sum((v.co for v in ring), Vector()) / len(ring))
        for k in range(nseg):
            tri = (cen, ring[(k + 1) % nseg], ring[k]) if rev else (cen, ring[k], ring[(k + 1) % nseg])
            f = bm.faces.new(tri)
            f.material_index = zone(0.0 if rev else 1.0, 0.0)
            f.smooth = True
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


def ch_v3(x, y, pose="stand"):
    stride = pose == "stride"
    # spine: neck -> deep chest -> waist -> narrow hips -> tail tip; (x, y, z, half-width, half-depth)
    spine = [(0.5, 0, 0.53, 0.06, 0.07), (0.38, 0, 0.53, 0.1, 0.12), (0.2, 0, 0.5, 0.14, 0.17),
             (0.0, 0, 0.46, 0.12, 0.13), (-0.2, 0, 0.42, 0.1, 0.1), (-0.4, 0, 0.38, 0.105, 0.1),
             (-0.58, 0, 0.36, 0.075, 0.07), (-0.85, 0, 0.33, 0.045, 0.042), (-1.1, 0, 0.31, 0.022, 0.02),
             (-1.3, 0, 0.3, 0.004, 0.004)]
    spine = [(x + a, y + b, c_, d, e) for a, b, c_, d, e in spine]

    def body_zone(f, s):  # 0 main flank, 1 dark back, 2 pale belly
        return 1 if s > 0.5 else (2 if s < -0.3 else 0)
    loft(spine, [c("main"), c("shadow"), c("sec")], body_zone, nring=90, nseg=24)
    # soft rosettes on the flanks and back: a dark ring with a main-coloured heart
    S = _catmull(spine, 40)
    for idx, ang in ((6, 0.9), (9, 0.35), (12, 1.3), (15, 0.6), (18, 1.05), (21, 0.4), (24, 0.95), (27, 0.55),
                     (8, 1.6), (17, 1.55)):
        px, py, pz, ry_, rz_ = S[idx]
        for sy in (-1, 1):
            p = Vector((px, py + sy * ry_ * math.cos(ang) * 0.97, pz + rz_ * math.sin(ang) * 0.97))
            n = Vector((0, sy * math.cos(ang), math.sin(ang)))
            rot = Vector((0, 0, 1)).rotation_difference(n).to_matrix().to_4x4()
            rr = 0.022 if ry_ > 0.08 else 0.015
            ell(p, (rr, rr, 0.006), c("mark"), rot=rot)
            ell(p + n * 0.003, (rr * 0.5, rr * 0.5, 0.005), c("main"), rot=rot)
    # head, about a fifth of the body length: pale skull, two lobes swept back flat along it,
    # a darker face with a large magenta eye patch, and the cyan jaw
    ell((x + 0.58, y, 0.535), (0.14, 0.085, 0.085), c("sec"), rot=ry(8))
    ell((x + 0.6, y, 0.47), (0.12, 0.075, 0.055), c("main"), rot=ry(8))
    for sy in (-1, 1):
        ell((x + 0.5, y + sy * 0.05, 0.6), (0.17, 0.035, 0.032), c("sec"), rot=ry(10) @ rx(sy * 18))
        ell((x + 0.63, y + sy * 0.07, 0.5), (0.055, 0.016, 0.04), c("glow2"))
    jaw = 0.04 if stride else 0.032
    ell((x + 0.69, y, 0.44), (0.07, 0.055, jaw), c("glow"))

    def leg_zone(f, s):  # thigh / upper arm in main, lower leg in shadow
        return 0 if f < 0.38 else 1
    legs = [  # front: reaching forward; middle: close behind it; hind: digitigrade, crouched
        [(0.26, 0.07, 0.48, 0.075), (0.3, 0.13, 0.28, 0.04), (0.5, 0.15, 0.06, 0.026), (0.6, 0.15, 0.012, 0.02)],
        [(0.1, 0.07, 0.46, 0.07), (0.06, 0.14, 0.26, 0.038), (0.16, 0.16, 0.06, 0.025), (0.22, 0.16, 0.012, 0.02)],
        [(-0.38, 0.06, 0.38, 0.085), (-0.22, 0.13, 0.24, 0.045), (-0.44, 0.15, 0.1, 0.028),
         (-0.4, 0.15, 0.012, 0.02)],
    ]
    for li, leg in enumerate(legs):
        for sy in (-1, 1):
            lifted = stride and ((li % 2 == 0) == (sy > 0))
            pts = []
            for k, (lx, ly, lz, r) in enumerate(leg):
                last2 = k >= len(leg) - 2
                dx = (0.14 if lifted else -0.05) if (stride and last2) else 0.0
                dz = 0.07 if (lifted and last2) else 0.0
                pts.append((x + lx + dx, y + sy * ly, lz + dz, r, r))
            loft(pts, [c("main"), c("shadow")], leg_zone, nring=36, nseg=14)
            fx, fy, fz = pts[-1][:3]
            ell((fx + 0.03, fy, max(fz, 0.012)), (0.05, 0.03, 0.012), c("shadow"))


def main_v3(out):
    import shutil
    os.makedirs(out, exist_ok=True)
    sc = L.setup_scene()
    sc.display.shading.show_specular_highlight = False
    L.PAL.update({"check0": "#6E6C78", "check1": "#7E7C88"})
    use("CH", 1)
    CUR["_mark"] = "rosettes"
    group("row-chorister-v3")
    sp = 2.6
    views = [("side", 0.0, None), ("yaw 45 toward", -45.0, None), ("yaw 45 away", 45.0, None), ("stride", 0.0, "stride")]
    for k, (name, yaw, pose) in enumerate(views):
        xx = k * sp * 1.25
        place(ch_v3, xx, 0.0, yaw, **({"pose": pose} if pose else {}))
        label(name, xx, -sp * 0.56, sp * 0.05, "sublabel")
    xe = 3 * sp * 1.25
    A.checker(-sp * 0.7, xe + sp * 0.7, -sp * 0.5, sp * 0.5)
    label("CH-A2 v3: one lofted body, muscular limbs (11 x 3 x 5 v + tail 6.5), colourway 1", xe / 2, -sp * 0.7, sp * 0.065)
    for cw in (1, 2):
        use("CH", cw)
        CUR["_mark"] = "rosettes"
        group(f"cw-CH-{cw}")
        X = 400.0 + cw * 12.0
        place(ch_v3, X, 0.0)
        A.checker(X - sp * 0.6, X + sp * 0.6, -sp * 0.35, sp * 0.35)
    group("compare")  # three-quarter view like the image: turned 25 deg toward the viewer
    use("CH", 1)
    CUR["_mark"] = "rosettes"
    place(ch_v3, 600.0, 0.0, -25.0)
    A.checker(600.0 - 1.6, 600.0 + 1.3, -0.8, 0.8)
    cams = []
    for name, col in GROUPS:
        cams.append((name, *frame(f"cam-{name}", col, 900 if name.startswith(("cw-", "compare")) else 2000)))
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(out, "animal_bodies_r2_v3.blend"))
    shear = Matrix.Identity(4)
    shear[2][1] = 0.5
    root = bpy.data.objects.new("oblique", None)
    sc.collection.objects.link(root)
    for ob in list(sc.objects):
        if ob.type in {"MESH", "FONT"}:
            ob.parent = root
            ob.matrix_parent_inverse = shear
    cells_old = os.path.join(out, "colourway-cells")
    cells = os.path.join(out, "colourway-cells-v3")
    os.makedirs(cells, exist_ok=True)
    for f in os.listdir(cells_old):
        if not f.startswith("cw-CH-"):
            shutil.copy(os.path.join(cells_old, f), os.path.join(cells, f))
    for name, cam, w, h in cams:
        if name.startswith("cw-"):
            path = os.path.join(cells, f"{name}.png")
        elif name == "compare":
            path = os.path.join(cells, "compare-ch.png")
        else:
            path = os.path.join(out, f"{name}.png")
        L.render(cam, path, w, h)
    stitch_colourways(out, cells, "colourways-v3.png")
    views_dir = os.path.join(out, "strip-views-v3")
    subprocess.run([bpy.app.binary_path, "-b", "-P", os.path.abspath(__file__), "--", "--strip", "CH-A2", views_dir,
                    "--v3"], check=True, stdout=subprocess.DEVNULL)
    stitch_strip(views_dir, "CH-A2", os.path.join(out, "strip-CH-A2-v3.png"))
    # the image lives beside the run (art/gen/runs is not in every worktree): out's repo first
    rel = os.path.join("art", "gen", "runs", "2026-09-24-reimagining-concepts", "CH-A2-2401-b0.png")
    img = next(p for p in (os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(out))), rel),
                           os.path.join(REPO_ROOT, rel)) if os.path.exists(p))
    script = r'''
import sys
from PIL import Image, ImageDraw
img, render, q3, out = sys.argv[1:5]
a = Image.open(img).convert("RGB"); b = Image.open(render).convert("RGB"); c = Image.open(q3).convert("RGB")
H = 560
a = a.resize((int(a.width * H / a.height), H)); b = b.resize((int(b.width * H / b.height), H)); c = c.resize((H, H))
im = Image.new("RGB", (a.width + b.width + c.width + 40, H + 40), (20, 18, 28))
im.paste(a, (10, 30)); im.paste(b, (a.width + 20, 30)); im.paste(c, (a.width + b.width + 30, 30))
d = ImageDraw.Draw(im)
d.text((10, 8), "Qwen image CH-A2-2401-b0 (the target)", fill=(230, 221, 248))
d.text((a.width + 20, 8), "v3 blockout, colourway 1, panel oblique, turned 25 deg toward the viewer", fill=(230, 221, 248))
d.text((a.width + b.width + 30, 8), "v3 blockout, 3/4 front (method-B strip view)", fill=(230, 221, 248))
im.save(out)
'''
    subprocess.run(["python3", "-c", script, img, os.path.join(cells, "compare-ch.png"),
                    os.path.join(views_dir, "CH-A2-q3front.png"), os.path.join(out, "chorister-vs-image.png")], check=True)
    print("done v3", flush=True)


def strip_mode(code, out):
    """Stand in an animal_bodies module with this round's codes and run blockout_strip."""
    import runpy
    pose = None
    if code.endswith("-glide"):
        code0, pose = code[:-6], "glide"
    else:
        code0 = code
    if "--v3" in sys.argv:  # the v3 chorister, colourway 1
        use("CH", 1)
        CUR["_mark"] = "rosettes"
        fn = ch_v3
    elif "--v2" in sys.argv:  # colourway 1 of the v2 builders
        short = {v: k for k, v in STRIP_CODES.items()}[code0]
        use(short, 1)
        fn = V2_BY[short][3]
    else:
        fn = {c: f for _, c, _, f, *_ in SPECIES}[code0]
    mod = types.ModuleType("animal_bodies")
    mod.ROWS = [("r2", "round 2", 1.0, "grazer",
                 [(code, code, lambda x, y: fn(x, y, pose=pose) if pose else fn(x, y))])]
    sys.modules["animal_bodies"] = mod
    sys.argv = [sys.argv[0], "--", "animal", code, out]
    runpy.run_path(os.path.join(HERE, "blockout_strip.py"), run_name="__main__")


def main():
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if args and args[0] == "--strip":
        strip_mode(args[1], args[2])
        return
    if "--v2" in args:
        main_v2([a for a in args if a != "--v2"][0])
        return
    if "--v3" in args:
        main_v3([a for a in args if a != "--v3"][0])
        return
    out = args[0] if args else "/tmp/animal-bodies-r2"
    os.makedirs(out, exist_ok=True)
    sc = L.setup_scene()
    sc.display.shading.show_specular_highlight = False
    build_rows()
    build_true_scale()
    cams = [(name, *frame(f"cam-{name}", col, 2000 if name != "true-scale" else 3000)) for name, col in GROUPS]
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(out, "animal_bodies_r2.blend"))
    shear = Matrix.Identity(4)
    shear[2][1] = 0.5
    root = bpy.data.objects.new("oblique", None)
    sc.collection.objects.link(root)
    for ob in list(sc.objects):
        if ob.type in {"MESH", "FONT"}:
            ob.parent = root
            ob.matrix_parent_inverse = shear
    for name, cam, w, h in cams:
        L.render(cam, os.path.join(out, f"{name}.png"), w, h)
    # method-B strips: blockout_strip.py's five views, stitched side by side
    views = os.path.join(out, "strip-views")
    for code in [c for _, c, *_ in SPECIES] + ["SP-AB2-glide"]:
        subprocess.run([bpy.app.binary_path, "-b", "-P", os.path.abspath(__file__), "--", "--strip", code, views],
                       check=True, stdout=subprocess.DEVNULL)
        names = ["side", "front", "back", "q3front", "q3back"]
        imgs = [bpy.data.images.load(os.path.join(views, f"{code}-{v}.png")) for v in names]
        W, H = imgs[0].size
        strip = bpy.data.images.new(f"strip-{code}", W * len(imgs), H, alpha=True)
        import numpy as np
        buf = np.zeros((H, W * len(imgs), 4), dtype=np.float32)
        for k, im in enumerate(imgs):
            px = np.array(im.pixels[:], dtype=np.float32).reshape(H, W, 4)
            buf[:, k * W:(k + 1) * W] = px
        strip.pixels[:] = buf.ravel()
        strip.filepath_raw = os.path.join(out, f"strip-{code}.png")
        strip.file_format = "PNG"
        strip.save()
    print("done", flush=True)


main()
