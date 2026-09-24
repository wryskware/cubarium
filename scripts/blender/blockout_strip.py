"""Render blockout views for character-sheet references (flat, plain bg, ortho).

    blender -b -P scripts/blender/blockout_strip.py -- animal CODE OUT_DIR
        views side (head right), front, back, q3front, q3back  (animal_bodies.py ROWS code)
    blender -b -P scripts/blender/blockout_strip.py -- plant SPECIES OUT_DIR
        turnaround: side, front, back, q3, top (adult, ladder sizes)
        growth:     seedling, juvenile, adult side + adult top, adult's scale throughout
Writes OUT_DIR/<name>-<view>.png at 512x512. Does not modify the source scripts.
"""
import math, os, random, sys
import bpy
from mathutils import Vector
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import organism_lineup as L

kind, code, out = sys.argv[sys.argv.index("--") + 1:][:3]
os.makedirs(out, exist_ok=True)
sc = L.setup_scene()
sh = sc.display.shading
sh.light = "FLAT"; sh.show_cavity = False; sh.show_shadows = False
sc.world.color = L.lin("#1E1B26")
L.COL = sc.collection
TILT = 8.0


def clear():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    L.ORG.clear()


def bounds():
    pts = [o.matrix_world @ v.co for o in bpy.data.objects if o.type == "MESH" for v in o.data.vertices]
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    return lo, hi


def shoot(path, c, span, az, tilt=TILT):
    cam = bpy.data.cameras.new("c"); cam.type = "ORTHO"; cam.ortho_scale = span; cam.clip_end = 200
    ob = bpy.data.objects.new("c", cam); sc.collection.objects.link(ob)
    a, t = math.radians(az), math.radians(tilt)
    d = Vector((math.cos(a) * math.cos(t), math.sin(a) * math.cos(t), math.sin(t)))
    ob.location = c + d * 50
    ob.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
    L.render(ob, path, 512, 512)
    bpy.data.objects.remove(ob, do_unlink=True)


if kind == "animal":
    import animal_bodies as A
    fn = {c: f for row in A.ROWS for c, _, f in row[4]}[code]
    fn(0.0, 0.0)
    lo, hi = bounds(); c = (lo + hi) / 2; span = max(hi - lo) * 1.3
    for name, az in [("side", -90), ("front", 0), ("back", 180), ("q3front", -45), ("q3back", -135)]:
        shoot(os.path.join(out, f"{code}-{name}.png"), c, span, az)
else:
    def build(wf):
        clear()
        _, h, r, layers = L.plant_size("ladder", code, wf)
        i = L.stage_of(code, wf)[0]
        getattr(L, code)(0.0, 0.0, h, r, layers, i, random.Random(f"{code}:{wf}:ladder"))
    build(1.0)
    lo, hi = bounds(); c = (lo + hi) / 2; span = max(hi - lo) * 1.2
    for name, az, t in [("side", -90, TILT), ("front", 0, TILT), ("back", 180, TILT), ("q3", -45, TILT), ("top", -90, 89.9)]:
        shoot(os.path.join(out, f"{code}-turn-{name}.png"), c, span, az, t)
    for name, wf in [("seedling", 0.1), ("juvenile", 0.35), ("adult", 1.0)]:
        build(wf)
        shoot(os.path.join(out, f"{code}-grow-{name}.png"), c, span, -90)
    shoot(os.path.join(out, f"{code}-grow-top.png"), c, span, -90, 89.9)
