#!/usr/bin/env python3
"""Side-by-side 4x crops of every Gridlite state in the neon gallery, one column per
chrome-dots variant (tests/dot_variants.gd), from gallery-<variant>.png pictures made by
tests/neon_shots.gd at 1920x1080:

    python3 game/tests/dot_states_sheet.py DIR [VARIANT ...]

Reads DIR/gallery-<variant>.png and writes DIR/states-4x.png. Default columns:
A B new gap1 fill bright. The crops are the gallery's controls at UI scale 1.0.
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw

SCALE = 4
WIDTH = 220  # px of each crop before scaling
# (row label, crop x, y, w, h) in the 1920x1080 gallery picture
ROWS = [
    ("title (TitleLabel, BACK 90 RECT 120, shadow)", 302, 668, WIDTH, 115),
    ("chrome label (unselected tab, BACK 135 RECT 71)", 524, 100, WIDTH, 52),
    ("second button plate (BACK 320 RECT 220)", 306, 308, WIDTH, 62),
    ("selected tab, negative (BACK 700 RECT 700)", 312, 96, WIDTH, 60),
    ("main button, negative (BACK 700 RECT 700)", 306, 204, WIDTH, 62),
    ("main button pressed (wght 900, BACK 240 RECT 240)", 974, 204, WIDTH, 62),
    ("chip faint (dot plate style box, Pixelify)", 936, 460, WIDTH, 42),
    ("chip bright / selected", 1157, 460, WIDTH, 42),
    ("main button disabled (40 %)", 1308, 204, WIDTH, 62),
    ("second button disabled (40 %)", 1224, 308, WIDTH, 62),
]
NAMES = {
    "A": "A  Gridlite today (font's own dots)",
    "B": "B  trial: lit dots + uniform field",
    "new": "new  BACK/RECT field (default)",
    "gap1": "alt: dot_min_gap 1 (dotted plates)",
    "fill": "alt: dot_fill_spacing 1",
    "bright": "alt: chrome dot_light 270 %",
}


def main() -> None:
    d = Path(sys.argv[1])
    variants = sys.argv[2:] or ["A", "B", "new", "gap1", "fill", "bright"]
    shots = {v: Image.open(d / f"gallery-{v}.png").convert("RGB") for v in variants}
    gut, head, label_h = 16, 28, 22
    col_w = WIDTH * SCALE
    height = head + sum(label_h + h * SCALE + gut for _, _, _, _, h in ROWS)
    sheet = Image.new("RGB", (gut + len(variants) * (col_w + gut), height), (0, 0, 0))
    draw = ImageDraw.Draw(sheet)
    for i, v in enumerate(variants):
        draw.text((gut + i * (col_w + gut), 8), NAMES.get(v, v), fill=(255, 210, 90))
    y = head
    for label, x, cy, w, h in ROWS:
        draw.text((gut, y + 4), label, fill=(200, 200, 200))
        y += label_h
        for i, v in enumerate(variants):
            crop = shots[v].crop((x, cy, x + w, cy + h)).resize((w * SCALE, h * SCALE), Image.NEAREST)
            sheet.paste(crop, (gut + i * (col_w + gut), y))
        y += h * SCALE + gut
    out = d / "states-4x.png"
    sheet.save(out)
    print(out, sheet.size)


if __name__ == "__main__":
    main()
