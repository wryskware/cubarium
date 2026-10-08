"""Render the five chosen founder bins at true relative size, headless.

    blender -b --python-exit-code 1 -P scripts/blender/loftstrider_bake_sheet.py -- --out DIR

Uses the bake's bin builder and source.json, with the art direction and Wrysk's
calf-l38 pick in design/handoffs/foliage-variants-and-calf-round1-2026-10-07.md.
"""

import json
import os
import sys

import bpy
from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import loftstrider_calf_sheet as S


def main():
    os.makedirs(os.path.join(S.OUT, "tiles"), exist_ok=True)
    source = os.path.join(S.B.REPO, "assets", "voxel-models", "source.json")
    with open(source) as fh:
        f = next(f for f in json.load(fh)["founders"] if f["name"] == "loftstrider")
    sc = S.L.setup_scene()
    sc.display.shading.show_specular_highlight = False
    sc.world.color = S.L.lin("#F4F2F8")
    for vox in (0.125, 0.25):
        _, _, adult = S.B.build_founder_bin(f, S.B.BINS - 1, vox)
        frames = {v: S.frame(adult, *angles) for v, angles in S.VIEWS.items()}
        for view, angles in S.VIEWS.items():
            sheets = {kind: Image.new("RGB", (S.W * S.B.BINS, S.H + 40), "#f4f2f8")
                      for kind in ("blender", "voxels")}
            for n in range(S.B.BINS):
                length, _, objs = S.B.build_founder_bin(f, n, vox)
                # Mechanism check: plain linear factors, with both endpoints intact.
                p = S.A.ls_blend(S.A.LS_CALVES_R2["calf-l38"], n / (S.B.BINS - 1))
                for k, start in S.A.LS_CALVES_R2["calf-l38"].items():
                    assert abs(p[k] - (start + (S.A.LS_ADULT[k] - start) * n / (S.B.BINS - 1))) < 1e-12
                raw = S.B.loftstrider_cells(objs, vox)
                assert all(n == 6 for n in S.B.loftstrider_sectors(raw, vox).values())
                target, extent = frames[view]
                for kind in sheets:
                    if kind == "voxels":
                        S.B.reset_scene()
                        S.A.voxel_cubes("loftstrider", raw, vox)
                    path = os.path.join(S.OUT, "tiles", f"bin{n}-{round(vox*1000)}-{view}-{kind}.png")
                    S.L.render(S.camera(target, extent * 1.12, *angles), path, S.W, S.H)
                    sheets[kind].paste(Image.open(path).convert("RGB"), (n * S.W, 40))
                    ImageDraw.Draw(sheets[kind]).text((n * S.W + 12, 12),
                        f"bin {n}  length key {length:.3f} m", fill="#241e32")
            for kind, sheet in sheets.items():
                path = os.path.join(S.OUT, f"loftstrider-{kind}-{round(vox*1000)}-{view}.png")
                sheet.save(path)
                print(path, flush=True)


if __name__ == "__main__":
    main()
