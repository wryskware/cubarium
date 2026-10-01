"""Model-pass contact sheets (design/handoffs/voxel-model-pass-2026-10-01.md, evidence):
per plant, the artbook pick | the Blender builder (adult, the pick's view) | the baked
adult model at 0.125 m, side by side in one PNG.

    blender -b -P scripts/blender/plant_sheets.py -- OUT_DIR [--only NAME[,NAME]]

Reads the bake's own growth rule and builders (bake_voxel_models.py, executed without its
main()), the baked `assets/voxel-models/125/<name>.cvm` (adult step, variant 0), and
colours both renders with SHEET_COLOURS below, a mirror of `recipe()` in
crates/cubarium/src/voxel/colours.rs (keep the two in step). Headless; no windows.
"""

import json
import math
import os
import struct
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
ARGV = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = ARGV[0] if ARGV and not ARGV[0].startswith("--") else os.path.join(REPO, "runs", "model-pass-2026-10-01", "plants")
ONLY = set(filter(None, (ARGV[ARGV.index("--only") + 1] if "--only" in ARGV else "").split(",")))

# The bake, minus its main(): its growth rule, builders and voxelizer.
_bake = os.path.join(HERE, "bake_voxel_models.py")
_saved = sys.argv
sys.argv = [sys.argv[0]]
B = {"__name__": "bake", "__file__": _bake}
exec(compile(open(_bake).read().rsplit("\nmain()", 1)[0], _bake, "exec"), B)
sys.argv = _saved
L = B["L"]

# Mirror of colours.rs recipe(): per species, {material: (base, tip)} for every part the
# builder draws, plus the accent colours. Unlisted foliage uses "leaf", trunk "bark".
SHEET_COLOURS = {
    "vaulttree": {"bark": ("#5F36A9", "#4A2D8E"), "leaf": ("#014C4D", "#01696E"),
                  "plum": ("#7B46CA", "#6C3EB9"), "violet": ("#5F36A9", "#4A2D8E"),
                  "detritus": ("#01261F", "#01261F"), "p1": ("#014C4D", "#01696E"),
                  "p2": ("#50C5A9", "#96F9DC"), "teal": ("#00CEEC", "#0197B1")},  # drape: (bottom tip, top)
    "umbrellafrond": {"bark": ("#4D1378", "#7823B4"), "leaf": ("#00696F", "#029894"),
                      "violet": ("#4D1378", "#7823B4"), "plum": ("#51176D", "#7E2C9D"),
                      "p1": ("#00696F", "#029894"), "p2": ("#4DE0C4", "#DCFDEE")},
    "bloomcrown": {"bark": ("#2E035F", "#5C1EAF"), "leaf": ("#8B0251", "#FD48D1"),
                   "violet": ("#2E035F", "#5C1EAF"), "p0": ("#5D55C2", "#8582F3"),
                   "p2": ("#8B0251", "#FD48D1"), "p1": ("#FED9F2", "#FED9F2"), "ripe": "#FEAC42"},
    "lanternberry": {"bark": ("#2E0944", "#4E178C"), "leaf": ("#013793", "#18ADFC"),
                     "violet": ("#2E0944", "#4E178C"), "p1": ("#013793", "#18ADFC"), "ripe": "#FDA243"},
    "siphonreed": {"leaf": ("#014B74", "#23A6BC"), "p1": ("#014B74", "#23A6BC"),
                   "lilac": ("#57C5C9", "#A5EED3"), "p2": ("#EB2189", "#FD4FB6")},
    "springturf": {"leaf": ("#037B7E", "#48F5BE"), "p2": ("#037B7E", "#48F5BE"),
                   "p0": ("#023A47", "#037B7E"), "violet": ("#20054E", "#32166B")},
    "stonecushion": {"bark": ("#390542", "#561470"), "leaf": ("#7A4ED0", "#DABBFD"),
                     "lilac": ("#7A4ED0", "#DABBFD"), "p2": ("#01B8EC", "#01B8EC"),
                     "detritus": ("#390542", "#561470")},
    "velvetpad": {"leaf": ("#1B296C", "#3866DA"), "violet": ("#331D47", "#48295E"),
                  "plum": ("#5F3D8D", "#8F60B8"), "p1": ("#1B296C", "#3866DA"),
                  "p2": ("#4594F0", "#75F1F8"), "teal": ("#75F1F8", "#75F1F8")},
    "glowcap": {"bark": ("#554199", "#35256D"), "leaf": ("#4426A1", "#764BD7"),
                "detritus": ("#554199", "#35256D"), "violet": ("#4426A1", "#764BD7"),
                "p0": ("#291654", "#291654"), "lilac": ("#B078EF", "#B078EF"),
                "magenta": ("#F607FC", "#F607FC"), "p2": ("#2FB0E2", "#3FF9FC")},
}

# How each pick was drawn: (elevation deg, azimuth deg) of the view, and the pick file.
VIEWS = {
    "vaulttree": (4, 0, "vaulttree.png"),
    "umbrellafrond": (8, 0, "umbrellafrond.png"),
    "bloomcrown": (6, 0, "bloomcrown-pink-preferred.png"),
    "lanternberry": (4, 0, "lanternberry-blue.png"),
    "siphonreed": (6, 0, "siphonreed.png"),
    "springturf": (14, 0, "springturf.png"),
    "stonecushion": (24, 0, "stonecushion.png"),
    "velvetpad": (26, 0, "velvetpad-provisional.png"),
    "glowcap": (16, 0, "glowcap-on-vaulttree.png"),
}
PX = 900  # panel height


def lin(hexs):
    return L.lin(hexs)


def ramp(sp, mat, t, tag):
    c = SHEET_COLOURS.get(sp, {})
    if mat in c:
        a, b = c[mat]
    elif tag == "trunk":
        a, b = c.get("bark", ("#3A1A5A", "#5A2E7A"))
    elif mat == "warm":
        a = b = c.get("ripe", "#FF9B50")
    elif mat == "magenta":
        a = b = "#FF2AFC"
    else:
        a, b = c.get("leaf", ("#1E2798", "#42C5F8"))
    ca, cb = lin(a), lin(b)
    t = min(max(t, 0.0), 1.0)
    return tuple(ca[k] + (cb[k] - ca[k]) * t for k in range(3)) + (1.0,)


def paint(ob, sp, h, tag_of):
    """Vertex colours: the material's ramp by height in the model."""
    me = ob.data
    mat = ob.active_material.name
    attr = me.color_attributes.new("col", "FLOAT_COLOR", "POINT")
    for v in me.vertices:
        attr.data[v.index].color = ramp(sp, mat, v.co.z / max(h, 1e-6), tag_of(mat))
    me.color_attributes.active_color = attr


def setup():
    sc = L.setup_scene()
    sc.display.shading.color_type = "VERTEX"
    sc.render.film_transparent = False
    sc.world.color = L.lin("#26222E")
    return sc


def frame(objs, elev, azim, path):
    pts = [ob.matrix_world @ v.co for ob in objs for v in ob.data.vertices]
    lo = Vector(min(p[k] for p in pts) for k in range(3))
    hi = Vector(max(p[k] for p in pts) for k in range(3))
    c = (lo + hi) / 2
    e, a = math.radians(elev), math.radians(azim)
    view = Matrix.Rotation(a, 3, "Z") @ Vector((0, math.cos(e), -math.sin(e)))
    right = Matrix.Rotation(a, 3, "Z") @ Vector((1, 0, 0))
    up = right.cross(view).normalized()
    w = max(abs((p - c).dot(right)) for p in pts) * 2
    hh = max(abs((p - c).dot(up)) for p in pts) * 2
    aspect = 0.75
    cam = bpy.data.cameras.new("cam")
    cam.type = "ORTHO"
    cam.ortho_scale = max(w / aspect, hh) * 1.08
    cam.clip_end = 500
    ob = bpy.data.objects.new("cam", cam)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = c - view * 100
    ob.rotation_euler = (math.radians(90) - e, 0, a)
    sc = bpy.context.scene
    sc.camera = ob
    sc.render.resolution_x, sc.render.resolution_y = int(PX * aspect), PX
    sc.render.resolution_percentage = 100
    sc.render.filepath = path
    bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(ob, do_unlink=True)


def clear():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    for me in list(bpy.data.meshes):
        bpy.data.meshes.remove(me)
    L.ORG.clear()
    L.OVL.clear()


def read_cvm(path):
    b = open(path, "rb").read()
    assert b[:4] == b"CVM1"
    o = 4
    kind, nm = struct.unpack_from("<BB", b, o)
    o += 2
    names = []
    for _ in range(nm):
        n = b[o]
        names.append(b[o + 1:o + 1 + n].decode())
        o += 1 + n
    (ns,) = struct.unpack_from("<H", b, o)
    o += 2
    steps = []
    for _ in range(ns):
        h, r, nv = struct.unpack_from("<ddB", b, o)
        o += 17
        vs = []
        for _ in range(nv):
            (nc,) = struct.unpack_from("<I", b, o)
            o += 4
            cells = []
            for _ in range(nc):
                dx, dy, dz, m, tag = struct.unpack_from("<bbbBB", b, o)
                o += 5
                cells.append((dx, dy, dz, names[m], tag))
            vs.append(cells)
        steps.append((h, r, vs))
    return steps


def tag_kind(tag):
    return {0x00: "trunk", 0x30: "accent"}.get(tag & 0xF0, "drape" if tag & 0xF0 == 0x20 else "foliage")


def voxel_objs(sp, cells, vox):
    """Cubes per cell, coloured like colours.rs: the cell's band (4 bands by height)."""
    top = max(c[1] for c in cells)
    bm = bmesh.new()
    cols = []
    for dx, dy, dz, mat, tag in cells:
        band = round(min(max(dy, 0), top) / top * 3) if top > 0 else 3
        t = band / 3
        kind = tag_kind(tag)
        col = ramp(sp, mat, t, kind)
        # world (dx, dy up, dz away) -> Blender (x, y away, z up)
        m = Matrix.Translation(((dx + 0.5) * vox, (dz + 0.5) * vox, (dy + 0.5) * vox)) @ Matrix.Diagonal((vox, vox, vox, 1))
        n0 = len(bm.verts)
        bmesh.ops.create_cube(bm, size=1.0, matrix=m)
        cols.extend([col] * (len(bm.verts) - n0))
    me = bpy.data.meshes.new("vox")
    bm.to_mesh(me)
    bm.free()
    attr = me.color_attributes.new("col", "FLOAT_COLOR", "POINT")
    for k, col in enumerate(cols):
        attr.data[k].color = col
    me.color_attributes.active_color = attr
    ob = bpy.data.objects.new("vox", me)
    bpy.context.scene.collection.objects.link(ob)
    return [ob]


def stitch(sp, pick, builder, voxel, path):
    """pick | builder | voxel, each scaled to PX tall, labelled, via Blender's image API."""
    import numpy as np  # Blender bundles numpy
    panels = []
    for p in (pick, builder, voxel):
        im = bpy.data.images.load(p)
        w, h = im.size
        a = np.array(im.pixels[:], dtype=np.float32).reshape(h, w, 4)
        bg = np.array([0x26 / 255, 0x22 / 255, 0x2E / 255, 1.0], dtype=np.float32)
        a = a[..., :3] * a[..., 3:4] + bg[:3] * (1 - a[..., 3:4])
        # nearest resample to PX tall
        nh = PX
        nw = int(round(w * PX / h))
        ys = (np.arange(nh) * h / nh).astype(int)
        xs = (np.arange(nw) * w / nw).astype(int)
        panels.append(a[ys][:, xs])
        bpy.data.images.remove(im)
    gap = 12
    W = sum(p.shape[1] for p in panels) + gap * (len(panels) + 1)
    out = np.zeros((PX + 2 * gap, W, 3), dtype=np.float32) + np.array([0x14 / 255, 0x12 / 255, 0x1A / 255])
    x = gap
    for p in panels:
        out[gap:gap + PX, x:x + p.shape[1]] = p
        x += p.shape[1] + gap
    img = bpy.data.images.new(sp, W, PX + 2 * gap, alpha=False)
    flat = np.concatenate([out, np.ones(out.shape[:2] + (1,), np.float32)], axis=2)
    img.pixels[:] = flat.ravel()
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)


def main():
    os.makedirs(OUT, exist_ok=True)
    src = json.load(open(B["SOURCE"]), parse_constant=float)
    for sp in src["species"]:
        for st in sp["profile"]:
            st["wood_fraction_max"] = float(st["wood_fraction_max"])
    setup()
    tmp = os.path.join(OUT, "_parts")
    os.makedirs(tmp, exist_ok=True)
    for sp in src["species"]:
        name = sp["name"]
        if name not in VIEWS or (ONLY and name not in ONLY) or not hasattr(L, name):
            continue
        elev, azim, pick = VIEWS[name]
        prof = sp["profile"]
        si = len(prof) - 1
        h = B["dimension"](prof, sp["crown_height_m"], 1.0)
        r = B["dimension"](prof, sp["crown_radius_m"], 1.0)
        roles = dict(B["PLANT_ROLE"])
        roles.update(B["PLANT_ROLE_BY_SPECIES"].get(name, {}))
        role = lambda m: roles.get(m, "foliage")  # noqa: E731
        # builder
        clear()
        import random
        objs = B["build_plant"](name, h, r, si, prof[si], random.Random(f"{name}:0"), 0.125)
        for ob in list(objs):
            if role(ob.active_material.name) is None:  # scenery the world draws (rock, water)
                bpy.data.objects.remove(ob, do_unlink=True)
                objs.remove(ob)
                continue
            paint(ob, name, h, role)
        fb = os.path.join(tmp, f"{name}-builder.png")
        frame(objs, elev, azim, fb)
        # baked voxels
        clear()
        steps = read_cvm(os.path.join(REPO, "assets", "voxel-models", "125", f"{name}.cvm"))
        vobjs = voxel_objs(name, steps[-1][2][0], 0.125)
        fv = os.path.join(tmp, f"{name}-voxel.png")
        frame(vobjs, elev, azim, fv)
        stitch(name, os.path.join(REPO, "art", "gen", "artbook", pick), fb, fv, os.path.join(OUT, f"{name}.png"))
        print(f"sheet {name}: h {h:.3f} r {r:.3f}, {len(steps[-1][2][0])} cells", flush=True)


main()
