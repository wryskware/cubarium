#!/usr/bin/env python3
"""Build a labelled contact sheet from the run.json files in a round directory.

One cell per candidate: the candidate itself, its `@8px` in-scene crop when one exists,
and a caption of id / lane / arm / model / seed. Plates have no in-scene crop (a plate is
the scene), so `--scale-bars` draws the 8 px and 4 px per-voxel rules on each cell
instead.

    contact_sheet.py DIR --out sheet.png [--scale-bars] [--cols 4] [--cell 420]

Each candidate is `<stem>.png` beside `<stem>.run.json` (kit section 6 schema).
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

from PIL import Image, ImageDraw, ImageFont

FONT_PATHS = [
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
]


def font(size: int) -> ImageFont.ImageFont:
    for p in FONT_PATHS:
        if pathlib.Path(p).exists():
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()


def fit(img: Image.Image, w: int, h: int) -> Image.Image:
    s = min(w / img.width, h / img.height)
    # integer nearest when shrinking by a whole factor, so pixel art stays pixel art
    return img.resize(
        (max(1, int(img.width * s)), max(1, int(img.height * s))), Image.Resampling.NEAREST
    )


def scale_bars(d: ImageDraw.ImageDraw, x: int, y: int, f: ImageFont.ImageFont) -> None:
    for i, (px, col) in enumerate(((8, (90, 230, 255)), (4, (255, 150, 60)))):
        yy = y + i * 14
        for k in range(10):
            d.rectangle(
                [x + k * px, yy, x + (k + 1) * px - 1, yy + 6],
                fill=col if k % 2 == 0 else (25, 25, 40),
            )
        d.text((x + 10 * px + 8, yy - 2), f"10 voxels @ {px} px/voxel", font=f, fill=(190, 190, 205))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("dir")
    ap.add_argument("--out", required=True)
    ap.add_argument("--cols", type=int, default=3)
    ap.add_argument("--cell", type=int, default=440)
    ap.add_argument("--scale-bars", action="store_true")
    ap.add_argument("--title", default=None)
    a = ap.parse_args()

    root = pathlib.Path(a.dir)
    runs = sorted(root.glob("*.run.json"))
    if not runs:
        print(f"no *.run.json in {root}", file=sys.stderr)
        return 2

    metas = [json.loads(p.read_text()) for p in runs]
    fs, fl = font(15), font(12)
    bars_h = 34 if a.scale_bars else 0
    cap_h = 54
    cell_w = a.cell
    first = root / metas[0].get("file", "")
    aspect = 1.0
    if first.exists():
        w0, h0 = Image.open(first).size
        aspect = h0 / w0
    img_h = min(int(a.cell * 0.62), int((a.cell - 16) * aspect) + 8)
    has_second = any(m.get("in_scene") for m in metas)
    scene_h = (int(a.cell * 0.40) if not a.scale_bars else img_h) if has_second else 0
    cell_h = img_h + scene_h + bars_h + cap_h
    cols = a.cols
    rows = (len(runs) + cols - 1) // cols
    head = 44
    sheet = Image.new("RGB", (cols * cell_w, head + rows * cell_h), (14, 14, 22))
    d = ImageDraw.Draw(sheet)
    d.text(
        (12, 14),
        a.title or f"{root.name} — {len(runs)} candidates",
        font=font(20),
        fill=(230, 230, 240),
    )

    for n, rj in enumerate(runs):
        meta = json.loads(rj.read_text())
        cx = (n % cols) * cell_w
        cy = head + (n // cols) * cell_h
        cand = root / meta.get("file", rj.name.replace(".run.json", ".png"))
        if cand.exists():
            im = fit(Image.open(cand).convert("RGB"), cell_w - 16, img_h - 8)
            sheet.paste(im, (cx + 8 + (cell_w - 16 - im.width) // 2, cy + 4))
        y = cy + img_h
        if scene_h:
            scenes = [p for p in meta.get("in_scene", []) if "@8px" in p]
            if scenes and (root / scenes[0]).exists():
                src = Image.open(root / scenes[0]).convert("RGB")
                if a.scale_bars:
                    # a plate at 8 px per voxel is judged 1:1, so crop rather than shrink
                    bw, bh = min(cell_w - 16, src.width), min(scene_h - 8, src.height)
                    im = src.crop(
                        (
                            (src.width - bw) // 2,
                            (src.height - bh) // 2,
                            (src.width - bw) // 2 + bw,
                            (src.height - bh) // 2 + bh,
                        )
                    )
                else:
                    im = fit(src, cell_w - 16, scene_h - 8)
                sheet.paste(im, (cx + 8 + (cell_w - 16 - im.width) // 2, y + 4))
            y += scene_h
        if a.scale_bars:
            scale_bars(d, cx + 12, y + 6, fl)
            y += 34
        sid = rj.name.replace(".run.json", "")
        d.text(
            (cx + 10, y + 2),
            f"{sid}  {meta.get('subject','?')}  arm {meta.get('arm','-')}  "
            f"{meta.get('provider','?')}/{meta.get('model','?')}",
            font=fs,
            fill=(225, 225, 235),
        )
        d.text(
            (cx + 10, y + 20),
            f"seed {meta.get('seed','?')}  {('x'.join(str(v) for v in meta.get('size', [])))}  "
            f"style {meta.get('style_block','?')}  by {meta.get('by','?')}",
            font=fl,
            fill=(160, 160, 180),
        )
        d.text(
            (cx + 10, y + 36),
            (meta.get("largest_failure", "") or "")[: int(cell_w / 6.0)],
            font=fl,
            fill=(235, 150, 120),
        )
        d.rectangle([cx, cy, cx + cell_w - 1, cy + cell_h - 1], outline=(46, 46, 62))

    sheet.save(a.out)
    print(f"{a.out} | {sheet.width}x{sheet.height} | {len(runs)} cells")
    return 0


if __name__ == "__main__":
    sys.exit(main())
