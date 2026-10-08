#!/usr/bin/env python3
"""Contact sheets for the browsed-foliage candidates (round 1, 2026-10-07, package A).

    python3 scripts/blender/foliage_variants_sheets.py RUN_DIR

RUN_DIR is `foliage_variants.py`'s output after the in-scene strips were drawn:

    voxel_specimens RUN_DIR/inscene-plain-<species>.png --gpu --textures-only /nonexistent \
        --models RUN_DIR/models --candidates <species>:0,1,4,0,2,5,0,3,6
    (and inscene-textured-<species>.png without --textures-only)

The in-scene stands are in triplets (full, browsed k, stripped k) for k = 1, 2, 3, the
sheet's rows. Writes RUN_DIR/sheet-<species>.png: per candidate row, full | browsed |
stripped, each as the Blender model and its 0.125 m voxels; then the in-scene strips.
System python with Pillow (not Blender's).
"""

import json
import pathlib
import sys

from PIL import Image, ImageDraw, ImageFont

RUN = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else
                   "/home/wrysk/wryskware/cubarium/art/gen/runs/2026-10-07-foliage-variants")
BG = (20, 18, 26)
FG = (230, 221, 248)
SUB = (157, 144, 196)
SCALE = 0.5  # panel renders are 576 x 720


def font(size):
    for p in ("/usr/share/fonts/TTF/DejaVuSans.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"):
        try:
            return ImageFont.truetype(p, size)
        except OSError:
            pass
    return ImageFont.load_default()


def content_box(im, thresh=18):
    """The strip's bounding box of everything that differs from its row's sky colour at
    the left edge (the strip is sky over flat ground), plus a margin."""
    px = im.load()
    w, h = im.size
    top, left, right = h, w, 0
    ground = h
    for y in range(h):
        ref = px[2, y]
        hit = [x for x in range(0, w, 2) if sum(abs(a - b) for a, b in zip(px[x, y], ref)) > thresh]
        if hit:
            if len(hit) > w // 4:  # a ground row: every column differs from nothing, stop
                ground = min(ground, y)
                continue
            top = min(top, y)
            left, right = min(left, hit[0]), max(right, hit[-1])
    return max(0, left - 16), max(0, top - 16), min(w, right + 18), h, ground


def main():
    meta = json.loads((RUN / "candidates.json").read_text())
    f_title, f_head, f_lab = font(30), font(22), font(16)
    for sp, m in meta.items():
        v = {x["key"]: x for x in m["variants"]}
        p0 = Image.open(RUN / "panels" / f"{sp}-full-mesh.png")
        pw, ph = int(p0.width * SCALE), int(p0.height * SCALE)
        gap, lab_h, col_gap = 10, 44, 34
        cols = ["full", "browsed", "stripped"]
        sheet_w = 3 * (2 * pw + gap) + 2 * col_gap + 2 * 24
        rows_h = 3 * (ph + lab_h + gap)
        strips = []
        for kind in ("plain", "textured"):
            path = RUN / f"inscene-{kind}-{sp}.png"
            if path.exists():
                im = Image.open(path).convert("RGB")
                l, t, r, b, _ = content_box(im)
                strips.append((kind, path, im.crop((l, t, r, b))))
        strip_h = sum(s[2].height + 60 for s in strips)
        H = 110 + 40 + rows_h + 40 + strip_h + 20
        W = max(sheet_w, max((s[2].width + 48 for s in strips), default=0))
        out = Image.new("RGB", (W, H), BG)
        d = ImageDraw.Draw(out)
        ah = m["adult_step"]
        d.text((24, 18), f"{sp}: browsed-foliage candidates, round 1 (2026-10-07)", font=f_title, fill=FG)
        d.text((24, 60), f"adult step {ah['height_m']:.3f} m tall, radius {ah['radius_m']:.4f} m, variant 0's phase. "
               "Each pair: Blender model | its 0.125 m voxels (one camera per species). "
               "No candidate is preferred.", font=f_lab, fill=SUB)
        y = 110
        for c, name in enumerate(cols):
            x = 24 + c * (2 * pw + gap + col_gap)
            d.text((x, y), name, font=f_head, fill=FG)
        y += 40
        for k in (1, 2, 3):
            keys = ["full", f"browsed-{k}", f"stripped-{k}"]
            for c, key in enumerate(keys):
                x = 24 + c * (2 * pw + gap + col_gap)
                for j, kind in enumerate(("mesh", "vox")):
                    im = Image.open(RUN / "panels" / f"{sp}-{key}-{kind}.png").convert("RGB").resize((pw, ph))
                    out.paste(im, (x + j * (pw + gap), y))
                info = v[key]
                text = info["label"] if key != "full" else "full (accepted model)"
                tags = ", ".join(f"{t} {n}" for t, n in sorted(info["by_tag"].items()))
                d.text((x, y + ph + 4), text, font=f_lab, fill=FG)
                d.text((x, y + ph + 23), f"{info['cells']} cells: {tags}", font=f_lab, fill=SUB)
            y += ph + lab_h + gap
        y += 40
        for kind, path, im in strips:
            d.text((24, y), f"in scene, voxel_specimens --gpu ({'solid faces, the main draw' if kind == 'plain' else 'experimental face textures'}): "
                   "full | browsed k | stripped k for k = 1, 2, 3, left to right"
                   + ("; the orange bloom shows only when ripe" if sp == "bloomcrown" else ""),
                   font=f_lab, fill=FG)
            out.paste(im, (24, y + 26))
            third = im.width / 3
            for k in range(3):
                d.text((24 + int(k * third) + 20, y + 30), f"candidate {k + 1}", font=f_lab, fill=SUB)
            y += im.height + 60
        dst = RUN / f"sheet-{sp}.png"
        out.save(dst)
        print(dst)


main()
