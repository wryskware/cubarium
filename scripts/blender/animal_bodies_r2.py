"""Animal body blockouts, round 2: four reimagined options from Astra's prose.

    blender -b -P scripts/blender/animal_bodies_r2.py -- OUT_DIR           rows, true scale, .blend, strips
    blender -b -P scripts/blender/animal_bodies_r2.py -- --strip CODE OUT  (internal: one strip's views)

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


def strip_mode(code, out):
    """Stand in an animal_bodies module with this round's codes and run blockout_strip."""
    import runpy
    pose = None
    if code.endswith("-glide"):
        code0, pose = code[:-6], "glide"
    else:
        code0 = code
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
