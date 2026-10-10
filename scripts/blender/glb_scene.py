"""Lay `voxel_export` .glb files out in rows in one .blend, labelled, ready to open.

    blender -b --factory-startup --python scripts/blender/glb_scene.py -- OUT.blend \
        --row "today:DIR/vaulttree-v?.glb" --row "scale:DIR/loftstrider.glb,DIR/seedporter.glb"

Each `--row` is `LABEL:GLOB[,GLOB...]`; rows run back from the front, models left to right
in name order, each with its file name as a label at its feet. The viewport shows the
cells' own colours (solid shading by vertex colour), so nothing needs setting up.
"""

import glob
import math
import os
import sys

import bpy

GAP_M = 1.0  # between models in a row, metres
ROW_GAP_M = 2.0  # between rows


def args():
    a = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out, rows = None, []
    it = iter(a)
    for x in it:
        if x == "--row":
            label, _, globs = next(it).partition(":")
            files = sorted({f for g in globs.split(",") for f in glob.glob(g)})
            rows.append((label, files))
        elif out is None:
            out = x
        else:
            raise SystemExit(f"unexpected argument {x}")
    if out is None or not rows:
        raise SystemExit("usage: glb_scene.py -- OUT.blend --row LABEL:GLOB[,GLOB...] ...")
    return out, rows


def bounds(objs):
    pts = [o.matrix_world @ v.co for o in objs if o.type == "MESH" for v in o.data.vertices]
    lo = [min(p[i] for p in pts) for i in range(3)]
    hi = [max(p[i] for p in pts) for i in range(3)]
    return lo, hi


def label(text, x, y, size=0.18):
    bpy.ops.object.text_add(location=(x, y, 0.01))
    t = bpy.context.object
    t.data.body = text
    t.data.size = size
    t.data.align_x = "CENTER"
    t.name = f"label {text}"
    return t


def main():
    out, rows = args()
    bpy.ops.wm.read_factory_settings(use_empty=True)
    y = 0.0
    for label_text, files in rows:
        x, depth = 0.0, 0.0
        for f in files:
            before = set(bpy.data.objects)
            bpy.ops.import_scene.gltf(filepath=f)
            objs = [o for o in bpy.data.objects if o not in before]
            lo, hi = bounds(objs)
            # Its left edge at x, its front edge (−y, toward the default front view) at y.
            dx, dy = x - lo[0], y - lo[1]
            for o in objs:
                if o.parent is None:
                    o.location.x += dx
                    o.location.y += dy
            label(os.path.splitext(os.path.basename(f))[0], x + 0.5 * (hi[0] - lo[0]), y - 0.3)
            x += (hi[0] - lo[0]) + GAP_M
            depth = max(depth, hi[1] - lo[1])
        label(label_text, -1.5, y - 0.3, size=0.3)
        y += depth + ROW_GAP_M
    # A sun so Material Preview and renders are lit; the solid viewport needs none.
    bpy.ops.object.light_add(type="SUN", rotation=(math.radians(50), 0, math.radians(30)))
    # Every 3D view shows the cells' colours, with room to see the whole layout.
    for screen in bpy.data.screens:
        for area in screen.areas:
            for space in area.spaces:
                if space.type == "VIEW_3D":
                    space.shading.type = "SOLID"
                    space.shading.color_type = "VERTEX"
                    space.clip_end = 1000.0
    bpy.ops.wm.save_as_mainfile(filepath=os.path.abspath(out))


if __name__ == "__main__":
    main()
