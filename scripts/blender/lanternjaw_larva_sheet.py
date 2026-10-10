"""Lanternjaw larva candidates: render the tiles, then compose the contact sheet
(`design/handoffs/lanternjaw-larva-candidates-2026-10-09.md`).

    blender -b -P scripts/blender/lanternjaw_larva_sheet.py -- [--out DIR]

DIR defaults to `art/gen/runs/lanternjaw-larva-2026-10-09/` (gitignored); the sheet is
`DIR/sheet.png`, composed by `lanternjaw_larva_compose.py` (system Python, for PIL), and
`DIR/meta.json` records each candidate's cells, voxel span and built length.

Candidates only: nothing here is baked. Per candidate (`animal_models.lanternjaw_larva`):

- the larva voxelized at 0.125 m (`bake_voxel_models.voxelize_animal`, `floor=None` so the
  parts that bite into the host keep their cells) on an adult frondgrazer's back (the
  founder's adult body, voxelized the same way), three-quarter and side; its belly row on
  the row above the host's back, its bite cells standing in for the host's top cells; one
  camera scale for every on-host tile;
- the larva alone, voxels, enlarged (one scale for all four), and its Blender model;
- the smallest adult lanternjaw bin (`bake_voxel_models.build_founder_bin`, bin 0), the
  body it is drawn as after drop-off, at the on-host tiles' scale.

Cubes are coloured as `animal_models.voxel_cubes` colours them (the slots' ramps, four
height bands, each model banded on its own height as the renderer bands it).
"""

import json
import os
import subprocess
import sys

from mathutils import Matrix

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import organism_lineup as L  # noqa: E402
import animal_models as A  # noqa: E402
import bake_voxel_models as B  # noqa: E402

ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = (ARGS[ARGS.index("--out") + 1] if "--out" in ARGS
       else os.path.join(A.REPO, "art", "gen", "runs", "lanternjaw-larva-2026-10-09"))

VOX = 0.125
VIEWS = {"q": (-45.0, 30.0), "side": (-90.0, 0.0), "front": (0.0, 20.0)}
W, H = 520, 360          # on-host and adult-bin tiles
WL, HL = 360, 360        # the larva alone
# where the larva's cell (0, 0, 0) goes on the host: over the middle of its back, on the row
# above its top (found from the host's cells)
AT_I, AT_J = 0, 0
# `--round 2`: the symmetric tick and grub (`animal_models.LJ_LARVA_R2`), each drawn shifted
# across by `LJ_LARVA_SHIFT` so it is centred on the host's spine
ROUND2 = "--round" in ARGS and ARGS[ARGS.index("--round") + 1] == "2"
KINDS = A.LJ_LARVA_R2 if ROUND2 else A.LJ_LARVA_KINDS


def founder(name):
    # read here, not through `B.SOURCE`: the bake reads its own `--out` from the same argv
    src = json.load(open(os.path.join(A.REPO, "assets", "voxel-models", "source.json")), parse_constant=float)
    return next(f for f in src["founders"] if f["name"] == name)


def host_cells():
    B.reset_scene()
    fg = founder("frondgrazer")
    objs = A.build("frondgrazer", 0.5 * VOX, 0.5 * VOX, fg["adult_length_m"], fg["adult_width_m"],
                   fg["adult_height_m"])
    return B.voxelize_animal(objs, VOX)


def larva(kind):
    """(objects, cells) of the `kind` larva, built at its true size on the anchor cell."""
    B.reset_scene()
    objs = A.build_larva(kind, 0.5 * VOX, 0.5 * VOX)
    return objs, B.voxelize_animal(objs, VOX, None)


def cubes_at(name, cells, di, dj, dk):
    objs = A.voxel_cubes(name, cells, VOX)
    m = Matrix.Translation((di * VOX, dj * VOX, dk * VOX))
    for ob in objs:
        ob.data.transform(m)
        ob.data.update()
    return objs


def camera(objs, view, aspect, scale=None):
    cam = A.ortho_camera(objs, *VIEWS[view], aspect)
    if scale is not None:
        cam.data.ortho_scale = scale
    return cam


def length_m(objs):
    xs = [v[0] for ob in objs for v in ob.bound_box]
    return max(xs) - min(xs)


def main():
    tiles = os.path.join(OUT, "tiles")
    os.makedirs(tiles, exist_ok=True)
    sc = L.setup_scene()
    sc.display.shading.show_specular_highlight = False
    sc.world.color = L.lin("#F4F2F8")
    sc.display.shading.object_outline_color = (0.05, 0.03, 0.08)

    host = host_cells()
    meta = {"host": {"cells": len(host), "span": A.span(host)}}
    built = {}
    for kind in KINDS:
        objs, cells = larva(kind)
        built[kind] = cells
        bite = sorted(k for k in cells if k[2] < 0)
        meta[kind] = {"cells": len(cells), "span": A.span(cells),
                      "built_length_m": round(length_m(objs), 3),
                      "voxel_length_m": int(A.span(cells).split(" x ")[0]) * VOX,
                      "bite_cells": len(bite)}
        print(f"{kind}: {meta[kind]}", flush=True)

    # the host's top under each larva: its belly row goes on the row above the highest host
    # cell under its footprint
    top = {}
    for (i, j, k) in host:
        top[(i, j)] = max(top.get((i, j), -1), k)
    scales = {view: 0.0 for view in VIEWS}
    placed = {}
    for kind, cells in built.items():
        sj = A.LJ_LARVA_SHIFT.get(kind, 0.0)
        # the host rows under a cell: its own, and the one below it when drawn half across
        rows = (0,) if sj == 0 else (0, -1)
        under = [top[(i + AT_I, j + AT_J + r)] for (i, j, k) in cells for r in rows
                 if (i + AT_I, j + AT_J + r) in top and k >= 0]
        dk = max(under) + 1
        moved = {(i + AT_I, j + AT_J, k + dk) for (i, j, k) in cells} if sj == 0 else set()
        placed[kind] = (dk, {key: s for key, s in host.items() if key not in moved})
        meta[kind]["host_cells_bitten"] = len(host) - len(placed[kind][1])
        # the common on-host scale: the widest framing over the four
        B.reset_scene()
        objs = A.voxel_cubes("frondgrazer", placed[kind][1], VOX) + cubes_at(
            "lanternjaw-larva", cells, AT_I, AT_J + sj, dk)
        for view in scales:
            cam = camera(objs, view, W / H)
            scales[view] = max(scales[view], cam.data.ortho_scale)

    # the larva-alone scale, common to the four
    lscale = {"vox": 0.0, "model": 0.0}
    for kind, cells in built.items():
        B.reset_scene()
        lscale["vox"] = max(lscale["vox"], camera(A.voxel_cubes("lanternjaw-larva", cells, VOX), "q", WL / HL).data.ortho_scale)
        objs, _ = larva(kind)
        lscale["model"] = max(lscale["model"], camera(objs, "q", WL / HL).data.ortho_scale)

    for kind, cells in built.items():
        dk, rest = placed[kind]
        for view in VIEWS:
            B.reset_scene()
            objs = A.voxel_cubes("frondgrazer", rest, VOX) + cubes_at(
                "lanternjaw-larva", cells, AT_I, AT_J + A.LJ_LARVA_SHIFT.get(kind, 0.0), dk)
            L.render(camera(objs, view, W / H, scales[view]), os.path.join(tiles, f"{kind}-host-{view}.png"), W, H)
        B.reset_scene()
        L.render(camera(A.voxel_cubes("lanternjaw-larva", cells, VOX), "q", WL / HL, lscale["vox"]),
                 os.path.join(tiles, f"{kind}-vox.png"), WL, HL)
        objs, _ = larva(kind)
        L.render(camera(objs, "q", WL / HL, lscale["model"]), os.path.join(tiles, f"{kind}-model.png"), WL, HL)

    # the smallest adult bin, at the on-host three-quarter scale
    B.reset_scene()
    length, width, objs = B.build_founder_bin(founder("lanternjaw"), 0, VOX)
    adult = B.voxelize_animal(objs, VOX)
    B.reset_scene()
    L.render(camera(A.voxel_cubes("lanternjaw", adult, VOX), "q", W / H, scales["q"]),
             os.path.join(tiles, "adult-bin0-q.png"), W, H)
    meta["adult_bin0"] = {"length_m": round(length, 3), "cells": len(adult), "span": A.span(adult)}
    meta["scales"] = {"host": scales, "larva": lscale}
    meta["kinds"] = list(KINDS)
    print(f"adult bin 0: {meta['adult_bin0']}", flush=True)

    with open(os.path.join(OUT, "meta.json"), "w") as fh:
        json.dump(meta, fh, indent=1)
    subprocess.run(["python3", os.path.join(HERE, "lanternjaw_larva_compose.py"), OUT], check=True)


main()
