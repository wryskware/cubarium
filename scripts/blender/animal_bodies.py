"""Animal body candidates: one row per species, every candidate at true size.

Rough blocks for judging silhouette, posture and size, per
design/animal-body-candidates-2026-09-23.md (codes LS-A, LJ-C, ... match). Each row
stands on a 0.125 m voxel checker and ends with a frondgrazer for reference; big rows
add a 1.75 m person. Renders: one 3/4 orthographic image per row, a true-scale
overview of every candidate on one floor, and the .blend.

    blender -b -P scripts/blender/animal_bodies.py -- OUT_DIR
    python3 scripts/blender/stitch_matrix.py OUT_DIR    (matrix.png)
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import organism_lineup as L  # noqa: E402
from organism_lineup import VOX, box, cyl, ell, ring_dirs, strap  # noqa: E402

OUT = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "/tmp/animal-bodies"
L.PAL.update({"sac": "#9CC3F0", "membrane": "#5B4A9A", "case": "#6A5E80",
              "check0": "#241C40", "check1": "#2E2552"})
TILT, YAW = 30.0, 30.0


def tube(pts, r0, r1, mat, seg=8):
    n = len(pts) - 1
    for k in range(n):
        ra = r0 + (r1 - r0) * k / n
        rb = r0 + (r1 - r0) * (k + 1) / n
        cyl(pts[k], pts[k + 1], ra, rb, mat=mat, seg=seg)


def rot(axis, deg):
    return Matrix.Rotation(math.radians(deg), 4, axis)


# --- settled / close ---------------------------------------------------------------

def fg(x, y):
    L.frondgrazer(x, y, 0.75, 0.375, 0.375, overlays=False)


def lt(x, y):
    L.littershredder(x, y, 0.375, 0.125, 0.125)


def bw_a(x, y, z=0.5):
    cyl((x, y, z), (x, y, z + 0.09), 0.04, 0.018, mat="plum", seg=12)
    cyl((x, y, z - 0.005), (x, y, z + 0.005), 0.042, mat="magenta", seg=12)
    cyl((x, y, z), (x + 0.02, y, z - 0.035), 0.005, mat="lilac", seg=4)
    for sx, sweep in ((0.02, 0.35), (-0.02, -0.35)):
        for sy in (-1, 1):
            p0 = Vector((x + sx, y + sy * 0.02, z + 0.07))
            strap(p0, p0 + Vector((sweep * 0.3, sy * 0.16, 0.01)), 0.02, "lilac")


def bw_b(x, y, z=0.5):
    """Fan-tail: one fan wing over a long counterweight tail."""
    ell((x, y, z), (0.045, 0.03, 0.035), "plum")
    ell((x, y, z - 0.03), (0.02, 0.02, 0.012), "magenta")
    for k in range(7):
        a = math.radians(-60 + 20 * k)
        strap((x, y, z + 0.02), (x - 0.04, y + math.sin(a) * 0.19, z + 0.02 + math.cos(a) * 0.12), 0.018, "lilac")
    tube([(x - 0.04, y, z), (x - 0.14, y, z - 0.02), (x - 0.24, y, z - 0.01)], 0.01, 0.004, "plum", seg=6)
    ell((x - 0.25, y, z - 0.01), (0.02, 0.02, 0.02), "p2")


def bw_c(x, y, z=0.5):
    """Spinner: a disc body carried by one helical vane."""
    cyl((x, y, z - 0.012), (x, y, z + 0.012), 0.05, mat="plum", seg=16)
    cyl((x, y, z - 0.016), (x, y, z - 0.008), 0.053, mat="magenta", seg=16)
    pts = []
    for k in range(13):
        a = math.radians(30 * k)
        rr = 0.06 + 0.12 * k / 12
        pts.append((x + math.cos(a) * rr, y + math.sin(a) * rr, z + 0.02 + 0.05 * k / 12))
    for p0, p1 in zip(pts, pts[1:]):
        strap(p0, p1, 0.03, "lilac")
    cyl((x, y, z - 0.012), (x, y, z - 0.05), 0.004, mat="lilac", seg=4)


# --- loftstrider ----------------------------------------------------------------------

def ls_a(x, y):
    n = len(L.OVL)
    L.loftstrider(x, y, 1.0, True)
    for ob in L.OVL[n:]:
        bpy.data.objects.remove(ob, do_unlink=True)
    del L.OVL[n:]


def ls_b(x, y):
    """Tethered float: body slung under a gas sac among the lobes, tether legs to the ground."""
    ell((x, y, 2.95), (0.62, 0.5, 0.45), "sac")
    ell((x, y, 2.4), (0.34, 0.26, 0.17), "plum")
    for dx, dy in ring_dirs(10):
        strap((x + dx * 0.2, y + dy * 0.15, 2.5), (x + dx * 0.34, y + dy * 0.28, 2.7), 0.025, "lilac")
    for dx, dy in ring_dirs(4, 0.4):
        box((x + dx * 0.3, y + dy * 0.22, 2.38), (0.06, 0.06, 0.04), "magenta")
    for dx, dy in ring_dirs(6, 0.2):
        hip = (x + dx * 0.25, y + dy * 0.2, 2.3)
        knee = (x + dx * 0.75, y + dy * 0.6, 2.05)
        foot = (x + dx * 1.0, y + dy * 0.8, 0.0)
        cyl(hip, knee, 0.02, mat="plum", seg=6)
        cyl(knee, foot, 0.016, mat="plum", seg=6)
        ell((foot[0], foot[1], 0.01), (0.05, 0.05, 0.012), "detritus")


def ls_c(x, y):
    """Arch walker: the body is a hoop, legs at each end, mouth at the apex."""
    pts = []
    for k in range(17):
        t = math.pi * k / 16
        pts.append((x - 0.85 * math.cos(t), y, 0.3 + 3.0 * math.sin(t)))
    half = pts[:9]
    tube(half, 0.11, 0.16, "plum", seg=10)
    tube(list(reversed(pts[8:])), 0.11, 0.16, "plum", seg=10)
    for k in (3, 5, 11, 13):
        p = Vector(pts[k])
        cyl(p - Vector((0, 0, 0.03)), p + Vector((0, 0, 0.03)), 0.175, mat="violet", seg=10)
    for ex in (-0.85, 0.85):
        for dx, dy in ring_dirs(4, 0.8):
            cyl((x + ex, y, 0.32), (x + ex + dx * 0.25, y + dy * 0.25, 0.18), 0.03, mat="plum", seg=6)
            cyl((x + ex + dx * 0.25, y + dy * 0.25, 0.18), (x + ex + dx * 0.32, y + dy * 0.32, 0.0), 0.025, mat="plum", seg=6)
    top = pts[8]
    cyl((top[0], y, top[2] + 0.1), (top[0], y, top[2] + 0.16), 0.13, mat="magenta", seg=12)
    for dx, dy in ring_dirs(6):
        strap((top[0], y, top[2] + 0.16), (top[0] + dx * 0.14, y + dy * 0.14, top[2] + 0.34), 0.03, "lilac")


def ls_d(x, y):
    """Tongue caster: heavy quadruped, ballistic tongue fired into the canopy."""
    ell((x, y, 1.0), (0.75, 0.36, 0.36), "plum")
    ell((x - 0.15, y, 1.28), (0.45, 0.25, 0.14), "violet")
    for hx in (0.45, -0.45):
        for sy in (-1, 1):
            cyl((x + hx, y + sy * 0.22, 0.85), (x + hx, y + sy * 0.26, 0.0), 0.07, 0.06, mat="plum", seg=8)
    for sx in (0.15, -0.35):
        ell((x + sx, y, 1.4), (0.1, 0.1, 0.04), "magenta")
    ell((x + 0.78, y, 1.05), (0.18, 0.2, 0.2), "plum")
    tube([(x + 0.85, y, 1.2), (x + 0.92, y, 2.2), (x + 0.95, y, 3.3)], 0.02, 0.01, "lilac", seg=6)
    ell((x + 0.95, y, 3.36), (0.05, 0.05, 0.07), "lilac")


# --- lanternjaw ------------------------------------------------------------------------

def lj_a(x, y):
    L.lanternjaw(x, y)


def lj_b(x, y):
    """Trapdoor: a flat disc in a shallow pit, a mouth that opens upward, the lure above."""
    for dx, dy in ring_dirs(14):
        ell((x + dx * 0.5, y + dy * 0.45, 0.02), (0.12, 0.12, 0.05), "detritus")
    ell((x, y, 0.05), (0.42, 0.36, 0.06), "plum")
    lid = Matrix.Translation((x + 0.3, y, 0.1)) @ rot("Y", 55) @ Matrix.Translation((-0.28, 0, 0))
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=16, v_segments=8, radius=1.0)
    bmesh.ops.transform(bm, matrix=lid @ Matrix.Diagonal((0.3, 0.32, 0.03, 1)), verts=bm.verts)
    L.mk("lid", bm, "violet")
    for k in range(7):
        yy = y - 0.24 + 0.08 * k
        cyl((x + 0.3, yy, 0.1), (x + 0.33, yy, 0.18), 0.018, 0.003, mat="lilac", seg=4)
        cyl((x + 0.08, yy, 0.43), (x + 0.12, yy, 0.36), 0.018, 0.003, mat="lilac", seg=4)
    tube([(x - 0.15, y, 0.1), (x - 0.1, y, 0.3), (x + 0.05, y, 0.42)], 0.012, 0.007, "plum", seg=6)
    ell((x + 0.08, y, 0.43), (0.035, 0.035, 0.035), "p2")


def lj_c(x, y):
    """Tail-lure mantis: tall and thin, blades raised, the lure on a tail curled over its back."""
    ell((x - 0.18, y, 0.32), (0.2, 0.07, 0.07), "detritus", rot=rot("Y", 12))
    cyl((x, y, 0.33), (x + 0.07, y, 0.6), 0.04, 0.03, mat="plum", seg=8)
    ell((x + 0.09, y, 0.64), (0.05, 0.05, 0.045), "plum")
    for sy in (-1, 1):
        box((x + 0.12, y + sy * 0.03, 0.66), (0.02, 0.02, 0.015), "magenta")
        cyl((x + 0.06, y + sy * 0.05, 0.54), (x + 0.2, y + sy * 0.06, 0.64), 0.018, mat="plum", seg=6)
        strap((x + 0.2, y + sy * 0.06, 0.64), (x + 0.24, y + sy * 0.06, 0.4), 0.03, "lilac")
    for hx in (0.0, -0.12, -0.25):
        for sy in (-1, 1):
            knee = (x + hx, y + sy * 0.2, 0.42)
            cyl((x + hx, y + sy * 0.04, 0.33), knee, 0.012, mat="plum", seg=5)
            cyl(knee, (x + hx - 0.04, y + sy * 0.28, 0.0), 0.01, mat="plum", seg=5)
    tube([(x - 0.37, y, 0.33), (x - 0.45, y, 0.55), (x - 0.3, y, 0.78), (x - 0.02, y, 0.86), (x + 0.22, y, 0.8)],
         0.03, 0.01, "detritus", seg=6)
    cyl((x + 0.22, y, 0.8), (x + 0.3, y, 0.72), 0.005, mat="plum", seg=4)
    ell((x + 0.31, y, 0.69), (0.035, 0.035, 0.035), "p2")


def lj_d(x, y):
    """Coil serpent: legless, lying in an S, lure on a hanging forked tongue."""
    pts = []
    for k in range(25):
        t = k / 24
        pts.append((x - 0.55 + 1.0 * t, y + 0.28 * math.sin(t * 2.6 * math.pi), 0.07 if t < 0.85 else 0.07 + (t - 0.85) * 1.1))
    tube(list(reversed(pts)), 0.075, 0.02, "detritus", seg=8)
    for k in range(2, 24, 3):
        p = pts[k]
        ell((p[0], p[1], p[2] + 0.05), (0.03, 0.03, 0.015), "violet")
    head = Vector(pts[-1])
    ell(head + Vector((0.07, 0, 0.02)), (0.12, 0.08, 0.06), "plum")
    for sy in (-1, 1):
        box(head + Vector((0.12, sy * 0.05, 0.06)), (0.03, 0.02, 0.015), "magenta")
        cyl(head + Vector((0.18, 0, 0)), head + Vector((0.28, sy * 0.04, -0.12)), 0.005, mat="lilac", seg=4)
    ell(head + Vector((0.3, 0, -0.15)), (0.035, 0.035, 0.035), "p2")


# --- chorister --------------------------------------------------------------------------

def ch_a(x, y):
    L.chorister(x, y)


def ch_b(x, y):
    """Relay centipede-hound: six legs, dorsal vents that light nose to tail."""
    for k in range(7):
        sx = 0.7 - 0.23 * k
        ell((x + sx, y, 0.42), (0.14, 0.16, 0.11), "violet" if k % 2 else "plum")
        ell((x + sx, y, 0.53), (0.04, 0.05, 0.02 + 0.012 * (6 - k)), "p2")
    for k in (1, 3, 5):
        sx = 0.7 - 0.23 * k
        for sy in (-1, 1):
            knee = (x + sx + 0.05, y + sy * 0.35, 0.5)
            cyl((x + sx, y + sy * 0.12, 0.4), knee, 0.03, mat="plum", seg=6)
            cyl(knee, (x + sx + 0.12, y + sy * 0.42, 0.0), 0.025, mat="plum", seg=6)
    ell((x + 0.9, y, 0.38), (0.14, 0.12, 0.08), "plum")
    for sy in (-1, 1):
        strap((x + 0.95, y + sy * 0.06, 0.35), (x + 1.08, y + sy * 0.03, 0.3), 0.035, "lilac")
        box((x + 0.93, y + sy * 0.07, 0.45), (0.04, 0.03, 0.02), "magenta")
    tube([(x - 0.7, y, 0.42), (x - 1.0, y, 0.45), (x - 1.25, y, 0.5)], 0.06, 0.015, "violet", seg=6)


def ch_c(x, y):
    """Fork-crest strider: a biped with a tuning-fork crest that glows when it calls."""
    ell((x, y, 0.78), (0.4, 0.15, 0.16), "violet")
    tube([(x - 0.35, y, 0.8), (x - 0.8, y, 0.78), (x - 1.25, y, 0.72)], 0.08, 0.015, "violet", seg=8)
    for sy in (-1, 1):
        hip, knee = (x - 0.05, y + sy * 0.13, 0.72), (x + 0.14, y + sy * 0.15, 0.45)
        ankle, foot = (x - 0.1, y + sy * 0.15, 0.15), (x + 0.04, y + sy * 0.15, 0.0)
        cyl(hip, knee, 0.05, 0.035, mat="plum", seg=6)
        cyl(knee, ankle, 0.03, mat="plum", seg=6)
        cyl(ankle, foot, 0.025, mat="plum", seg=6)
        cyl((x + 0.3, y + sy * 0.1, 0.7), (x + 0.4, y + sy * 0.12, 0.6), 0.015, mat="plum", seg=5)
        strap((x + 0.52, y + sy * 0.06, 1.0), (x + 0.46, y + sy * 0.13, 1.45), 0.05, "plum")
        strap((x + 0.545, y + sy * 0.06, 1.0), (x + 0.485, y + sy * 0.13, 1.45), 0.012, "p2")
    tube([(x + 0.33, y, 0.82), (x + 0.45, y, 0.92), (x + 0.52, y, 1.02)], 0.08, 0.06, "violet", seg=8)
    ell((x + 0.55, y, 0.98), (0.09, 0.08, 0.06), "plum")
    box((x + 0.62, y, 0.95), (0.05, 0.1, 0.03), "magenta")


def ch_d(x, y):
    """Radial star: five legs round a disc, no front or back, a pulsing dorsal lens."""
    cyl((x, y, 0.4), (x, y, 0.52), 0.36, 0.3, mat="violet", seg=20)
    ell((x, y, 0.54), (0.13, 0.13, 0.06), "p2")
    for dx, dy in ring_dirs(5, 0.3):
        hip = (x + dx * 0.3, y + dy * 0.3, 0.45)
        knee = (x + dx * 0.6, y + dy * 0.6, 0.62)
        foot = (x + dx * 0.82, y + dy * 0.82, 0.0)
        cyl(hip, knee, 0.045, 0.035, mat="plum", seg=6)
        cyl(knee, foot, 0.03, mat="plum", seg=6)
        box((x + dx * 0.33, y + dy * 0.33, 0.52), (0.05, 0.05, 0.02), "magenta")


# --- capgnawer -----------------------------------------------------------------------

def cg_a(x, y):
    """Puffball nibbler: round, spore-catching bristles, rasp disc underneath."""
    ell((x, y, 0.11), (0.12, 0.1, 0.09), "violet")
    for hx in (0.06, -0.06):
        for sy in (-1, 1):
            cyl((x + hx, y + sy * 0.05, 0.06), (x + hx, y + sy * 0.08, 0.0), 0.012, mat="plum", seg=5)
    for dx, dy in ring_dirs(9, 0.2):
        strap((x + dx * 0.05, y + dy * 0.04, 0.17), (x + dx * 0.09, y + dy * 0.08, 0.24), 0.008, "lilac")
    ell((x + 0.1, y, 0.06), (0.025, 0.04, 0.03), "lilac")
    for sy in (-1, 1):
        box((x + 0.1, y + sy * 0.04, 0.13), (0.015, 0.015, 0.012), "magenta")


def cg_b(x, y):
    """Log limpet: clamps to bark; its shell mimics a glowcap cap. A real cap beside it."""
    d = 0.19
    cyl((x - 0.3, y, d / 2), (x + 0.3, y, d / 2), d / 2, mat="bark", seg=12)
    ell((x - 0.05, y, d + 0.02), (0.13, 0.1, 0.045), "violet")
    ell((x - 0.05, y, d + 0.005), (0.12, 0.09, 0.012), "p2")
    for hx in (0.03, -0.13):
        for sy in (-1, 1):
            box((x + hx, y + sy * 0.09, d - 0.01), (0.02, 0.03, 0.02), "plum")
    L.glowcap_cap(x + 0.18, y + 0.02, d, 0.2, 0.0625)


def cg_c(x, y):
    """Probe snout: spindly legs over the log, a long tube snout into splits and gills."""
    ell((x, y, 0.17), (0.07, 0.05, 0.05), "violet")
    for hx in (0.04, -0.04):
        for sy in (-1, 1):
            knee = (x + hx, y + sy * 0.09, 0.2)
            cyl((x + hx, y + sy * 0.03, 0.16), knee, 0.006, mat="plum", seg=5)
            cyl(knee, (x + hx * 1.8, y + sy * 0.12, 0.0), 0.005, mat="plum", seg=5)
    tube([(x + 0.07, y, 0.18), (x + 0.15, y, 0.14), (x + 0.2, y, 0.04)], 0.012, 0.005, "lilac", seg=6)
    for sy in (-1, 1):
        box((x + 0.05, y + sy * 0.035, 0.2), (0.015, 0.012, 0.012), "magenta")
    ell((x - 0.08, y, 0.19), (0.03, 0.03, 0.02), "plum")


def cg_d(x, y, z=0.35):
    """Glow moth: a flyer drawn to the caps' light, carrying spores."""
    ell((x, y, z), (0.06, 0.022, 0.022), "plum")
    for sy in (-1, 1):
        for sx, big in ((0.02, 1.0), (-0.04, 0.7)):
            c = (x + sx, y + sy * (0.08 * big + 0.02), z + 0.015)
            ell(c, (0.05 * big, 0.08 * big, 0.004), "lilac", rot=rot("X", sy * 12))
        cyl((x + 0.05, y + sy * 0.01, z + 0.01), (x + 0.1, y + sy * 0.05, z + 0.06), 0.003, mat="lilac", seg=4)
    for k in range(6):
        ell((x - 0.02 - 0.03 * k, y + 0.01 * (k % 3 - 1), z - 0.03 - 0.02 * k), (0.006, 0.006, 0.006), "p2")


# --- ripple snail ----------------------------------------------------------------------

def film(x, y):
    box((x, y, 0.003), (0.36, 0.3, 0.006), "film")


def rs_a(x, y):
    film(x, y)
    ell((x + 0.02, y, 0.015), (0.1, 0.045, 0.012), "lilac")
    ell((x - 0.01, y, 0.1), (0.085, 0.035, 0.085), "violet")
    ell((x - 0.01, y + 0.012, 0.1), (0.055, 0.03, 0.055), "plum")
    ell((x - 0.01, y + 0.02, 0.1), (0.025, 0.025, 0.025), "lilac")
    for sy in (-1, 1):
        cyl((x + 0.09, y + sy * 0.02, 0.02), (x + 0.12, y + sy * 0.03, 0.06), 0.004, mat="lilac", seg=4)
        ell((x + 0.12, y + sy * 0.03, 0.062), (0.008, 0.008, 0.008), "magenta")


def rs_b(x, y):
    film(x, y)
    ell((x, y, 0.006), (0.09, 0.09, 0.065), "violet")
    for dx, dy in ring_dirs(8):
        strap((x, y, 0.07), (x + dx * 0.088, y + dy * 0.088, 0.012), 0.01, "lilac")
    for dx, dy in ring_dirs(14, 0.1):
        cyl((x + dx * 0.09, y + dy * 0.09, 0.008), (x + dx * 0.12, y + dy * 0.12, 0.008), 0.003, mat="lilac", seg=4)


def rs_c(x, y):
    film(x, y)
    ell((x, y, 0.014), (0.12, 0.08, 0.012), "teal")
    for k, (dx, dy) in enumerate(ring_dirs(12)):
        ell((x + dx * 0.11, y + dy * 0.072, 0.012 + (0.01 if k % 2 else -0.004)), (0.025, 0.02, 0.006), "teal")
    for sy in (-1, 1):
        ell((x + 0.07, y + sy * 0.025, 0.028), (0.01, 0.01, 0.008), "magenta")


def rs_d(x, y):
    film(x, y)
    cyl((x - 0.1, y, 0.035), (x + 0.06, y, 0.035), 0.03, 0.036, mat="case", seg=8)
    for k in range(5):
        xx = x - 0.08 + 0.03 * k
        cyl((xx, y, 0.035), (xx + 0.008, y, 0.035), 0.04, mat="detritus", seg=8)
    ell((x + 0.08, y, 0.03), (0.03, 0.024, 0.022), "lilac")
    for k in range(3):
        for sy in (-1, 1):
            cyl((x + 0.06 + 0.012 * k, y + sy * 0.015, 0.02), (x + 0.07 + 0.012 * k, y + sy * 0.04, 0.0), 0.003, mat="plum", seg=4)
    for sy in (-1, 1):
        strap((x + 0.1, y + sy * 0.01, 0.025), (x + 0.12, y + sy * 0.02, 0.008), 0.008, "lilac")


# --- seedporter --------------------------------------------------------------------------

def sp_a(x, y):
    """Coil-tail climber on a trunk: tail coiled round it, cheek pouches of fruit."""
    tx = x + 0.12
    cyl((tx, y, 0), (tx, y, 1.3), 0.08, mat="bark", seg=12)
    ell((x, y, 0.72), (0.075, 0.07, 0.2), "violet")
    ell((x + 0.01, y, 0.96), (0.07, 0.06, 0.07), "violet")
    for sy in (-1, 1):
        box((x - 0.04, y + sy * 0.03, 0.99), (0.02, 0.02, 0.015), "magenta")
        ell((x - 0.02, y + sy * 0.05, 0.92), (0.025, 0.02, 0.022), "warm")
        for zz, zk in ((0.85, 0.9), (0.58, 0.52)):
            cyl((x, y + sy * 0.05, zz), (tx - 0.07, y + sy * 0.07, zk), 0.015, mat="plum", seg=5)
    pts = []
    for k in range(25):
        a = math.radians(-90 + 30 * k)
        pts.append((tx + math.cos(a) * 0.11, y + math.sin(a) * 0.11, 0.55 - 0.018 * k))
    tube([(x, y, 0.55)] + pts, 0.02, 0.008, "violet", seg=6)


def sp_b(x, y, z=0.7):
    """Four-sail glider, mid-glide: membranes between the front four limbs."""
    ell((x, y, z), (0.2, 0.06, 0.05), "violet")
    ell((x + 0.22, y, z + 0.01), (0.05, 0.045, 0.04), "violet")
    for sy in (-1, 1):
        box((x + 0.25, y + sy * 0.03, z + 0.035), (0.02, 0.015, 0.012), "magenta")
        for sx, span in ((0.1, 0.34), (-0.06, 0.3)):
            box((x + sx, y + sy * (span / 2 + 0.04), z), (0.13, span, 0.006), "membrane", rot=rot("X", sy * 8))
            cyl((x + sx + 0.06, y + sy * 0.04, z), (x + sx + 0.07, y + sy * (span + 0.05), z + 0.02), 0.008, mat="plum", seg=5)
        cyl((x - 0.16, y + sy * 0.04, z - 0.02), (x - 0.24, y + sy * 0.1, z - 0.06), 0.01, mat="plum", seg=5)
    tube([(x - 0.2, y, z), (x - 0.32, y, z + 0.02), (x - 0.42, y, z + 0.01)], 0.02, 0.006, "violet", seg=6)


def sp_c(x, y):
    """Basket crab: carries fruit and seeds in a woven dorsal basket."""
    ell((x, y, 0.16), (0.2, 0.15, 0.07), "violet")
    for k in range(4):
        hx = 0.12 - 0.08 * k
        for sy in (-1, 1):
            knee = (x + hx, y + sy * 0.25, 0.24)
            cyl((x + hx, y + sy * 0.12, 0.15), knee, 0.014, mat="plum", seg=5)
            cyl(knee, (x + hx - 0.02, y + sy * 0.32, 0.0), 0.012, mat="plum", seg=5)
    for sy in (-1, 1):
        cyl((x + 0.17, y + sy * 0.08, 0.15), (x + 0.28, y + sy * 0.1, 0.12), 0.018, mat="plum", seg=5)
        ell((x + 0.3, y + sy * 0.1, 0.12), (0.035, 0.025, 0.02), "lilac")
        box((x + 0.19, y + sy * 0.05, 0.21), (0.02, 0.02, 0.015), "magenta")
    for dx, dy in ring_dirs(12):
        cyl((x - 0.02 + dx * 0.12, y + dy * 0.1, 0.2), (x - 0.02 + dx * 0.14, y + dy * 0.12, 0.34), 0.008, mat="case", seg=4)
    cyl((x - 0.02, y, 0.2), (x - 0.02, y, 0.22), 0.12, mat="case", seg=12)
    for k, (dx, dy) in enumerate(ring_dirs(3, 0.5)):
        ell((x - 0.02 + dx * 0.05, y + dy * 0.05, 0.27), (0.035, 0.035, 0.045), "warm")


def sp_d(x, y):
    """Tri-arm brachiator hanging from a limb: three long hooked arms round a small body."""
    cyl((x - 0.5, y, 1.25), (x + 0.5, y, 1.25), 0.04, mat="bark", seg=10)
    ell((x, y, 0.85), (0.08, 0.08, 0.08), "violet")
    box((x + 0.07, y, 0.87), (0.02, 0.05, 0.02), "magenta")
    arms = [((0.08, 0, 1.23), (0.0, 0, 1.28)), ((0.38, 0.05, 0.62), (0.44, 0.05, 0.7)), ((-0.36, -0.05, 0.6), (-0.42, -0.05, 0.68))]
    for (ex, ey, ez), (hx, hy, hz) in arms:
        mid = (x + ex * 0.5, y + ey * 0.5, (0.85 + ez) / 2 + 0.05)
        cyl((x, y, 0.85), mid, 0.022, mat="plum", seg=6)
        cyl(mid, (x + ex, y + ey, ez), 0.016, mat="plum", seg=6)
        strap((x + ex, y + ey, ez), (x + hx, y + hy, hz), 0.02, "lilac")


# --- rows -------------------------------------------------------------------------

ROWS = [
    ("settled", "Settled / close: frondgrazer, littershredder, bellwing (A/B/C)", 1.0, "grazer", [
        ("FG", "frondgrazer: saddle shield", fg), ("LT", "littershredder: plough segment", lt),
        ("BW-A", "bellwing: bell + paired vanes", bw_a), ("BW-B", "bellwing: fan-tail", bw_b),
        ("BW-C", "bellwing: spinner", bw_c)]),
    ("loftstrider", "Loftstrider: canopy browser, reach 3.5 m", 2.8, "human", [
        ("LS-A", "stilt column", ls_a), ("LS-B", "tethered float", ls_b),
        ("LS-C", "arch walker", ls_c), ("LS-D", "tongue caster", ls_d)]),
    ("lanternjaw", "Lanternjaw: ambush mesopredator, ~1.0 x 0.45 m", 1.6, "grazer", [
        ("LJ-A", "brow lure stalker", lj_a), ("LJ-B", "trapdoor", lj_b),
        ("LJ-C", "tail-lure mantis", lj_c), ("LJ-D", "coil serpent", lj_d)]),
    ("chorister", "Chorister: pack hunter, 1.4 m + tail", 2.8, "human", [
        ("CH-A", "split-jaw runner", ch_a), ("CH-B", "relay centipede-hound", ch_b),
        ("CH-C", "fork-crest strider", ch_c), ("CH-D", "radial star", ch_d)]),
    ("capgnawer", "Capgnawer: fungivore of glowcap caps, ~0.25 m", 0.62, "grazer", [
        ("CG-A", "puffball nibbler", cg_a), ("CG-B", "log limpet (cap beside it)", cg_b),
        ("CG-C", "probe snout", cg_c), ("CG-D", "glow moth", cg_d)]),
    ("ripple-snail", "Ripple snail: film grazer on wet rock, ~0.19 m", 0.45, "grazer", [
        ("RS-A", "flat spiral", rs_a), ("RS-B", "ribbed dome", rs_b),
        ("RS-C", "ripple ray", rs_c), ("RS-D", "case bearer", rs_d)]),
    ("seedporter", "Seedporter: fruit eater and seed carrier, ~0.5 m", 1.1, "grazer", [
        ("SP-A", "coil-tail climber", sp_a), ("SP-B", "four-sail glider", sp_b),
        ("SP-C", "basket crab", sp_c), ("SP-D", "tri-arm brachiator", sp_d)]),
]


def checker(x0, x1, y0, y1):
    for parity, mat in ((0, "check0"), (1, "check1")):
        bm = bmesh.new()
        for i in range(math.floor(x0 / VOX), math.ceil(x1 / VOX)):
            for j in range(math.floor(y0 / VOX), math.ceil(y1 / VOX)):
                if (i + j) % 2 != parity:
                    continue
                vs = [bm.verts.new((a * VOX, b * VOX, -0.001)) for a, b in ((i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1))]
                bm.faces.new(vs)
        L.mk("floor", bm, mat, overlay=True)


def text(body, x, y, z, size, mat="label"):
    ob = L.label(body, x, y, z, size, mat)
    ob.rotation_euler = (math.radians(90 - TILT), 0, math.radians(YAW))
    return ob


def build_row(key, title, spacing, ref, cands, y0, true_scale=False):
    """Candidates along x at y0; returns the objects built (for framing)."""
    L.COL = bpy.data.collections.new(key)
    bpy.context.scene.collection.children.link(L.COL)
    start = set(bpy.data.objects)
    size = max(0.03, spacing * 0.045)
    x = 0.0
    for code, name, fn in cands:
        L.ORG.clear()
        fn(x, y0)
        text(f"{code}  {name}", x, y0 - spacing * 0.42, 0.0, size)
        x += spacing
    L.ORG.clear()
    fg(x - spacing * 0.2, y0)
    text("frondgrazer (ref)", x - spacing * 0.2, y0 - spacing * 0.42, 0.0, size, "sublabel")
    if ref == "human":
        L.human(x + 0.9, y0)
        text("1.75 m", x + 0.9, y0 - spacing * 0.42, 0.0, size, "sublabel")
        x += 1.2
    checker(-spacing * 0.6, x + spacing * 0.2, y0 - spacing * 0.55, y0 + spacing * 0.5)
    text(title, -spacing * 0.6 + len(title) * size * 0.36, y0 - spacing * 1.15, 0.0, size * 1.4)
    return [ob for ob in bpy.data.objects if ob not in start]


def frame_camera(name, objs, w, h):
    cam = bpy.data.cameras.new(name)
    cam.type = "ORTHO"
    cam.clip_end = 500
    ob = bpy.data.objects.new(name, cam)
    bpy.context.scene.collection.objects.link(ob)
    ob.rotation_euler = (math.radians(90 - TILT), 0, math.radians(YAW))
    R = ob.rotation_euler.to_matrix()
    right, up, fwd = R.col[0], R.col[1], -R.col[2]
    pts = []
    for o in objs:
        if o.type == "MESH":
            vs = o.data.vertices
            pts += [vs[i].co for i in range(0, len(vs), max(1, len(vs) // 40))]
        elif o.type == "FONT":
            pts.append(o.location)
    xs = [p.dot(right) for p in pts]
    ys = [p.dot(up) for p in pts]
    cx, cy = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
    span_x, span_y = max(xs) - min(xs), max(ys) - min(ys) + 0.2
    cam.ortho_scale = max(span_x, span_y * w / h) * 1.06
    ob.location = right * cx + up * cy + fwd * (min(p.dot(fwd) for p in pts) - 50)
    return ob


def main():
    os.makedirs(OUT, exist_ok=True)
    sc = L.setup_scene()
    sc.display.shading.show_shadows = True
    rows = []
    for r, (key, title, spacing, ref, cands) in enumerate(ROWS):
        rows.append((key, build_row(key, title, spacing, ref, cands, y0=r * 80.0)))
    # true-scale overview: every candidate on one floor
    allc = [c for row in ROWS for c in row[4]]
    L.COL = bpy.data.collections.new("true-scale")
    sc.collection.children.link(L.COL)
    start = set(bpy.data.objects)
    x, y0 = 0.0, -150.0
    for code, name, fn in allc:
        L.ORG.clear()
        fn(x, y0)
        text(code, x, y0 - 1.0, 0.0, 0.14)
        x += {"LS": 2.4, "CH": 2.5, "LJ": 1.5, "SP": 0.9}.get(code[:2], 0.6)
    L.human(x + 0.6, y0)
    checker(-1.0, x + 1.2, y0 - 1.2, y0 + 1.2)
    overview = [ob for ob in bpy.data.objects if ob not in start]

    cams = [(key, frame_camera(f"row-{key}", objs, 3000, 1100)) for key, objs in rows]
    tcam = frame_camera("true-scale", overview, 3840, 1100)
    sc.camera = cams[1][1]
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT, "animal_bodies.blend"))
    for key, cam in cams:
        L.render(cam, os.path.join(OUT, f"row-{key}.png"), 3000, 1100)
    L.render(tcam, os.path.join(OUT, "true-scale.png"), 3840, 1100)


if __name__ == "__main__":
    main()
