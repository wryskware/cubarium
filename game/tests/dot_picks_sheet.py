#!/usr/bin/env python3
"""Before/after 4x crops of every Gridlite state in the neon gallery: Gridlite as the font
drew its own unlit dots (tests/dot_variants.gd variant A) beside the game's default (the
drawn field with Wrysk's picks, 2026-10-11), from gallery-A.png and gallery-new.png made
by tests/neon_shots.gd at 1920x1080:

    python3 game/tests/dot_picks_sheet.py DIR

Writes DIR/picks-4x.png. Column titles sit in a header band and row names in a left
gutter, in a large font (LABEL_SHARE of the sheet's height, at least LABEL_MIN px), never
over the pictures.
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).parent))
from dot_states_sheet import ROWS as STATE_ROWS, SCALE  # noqa: E402

FONT = "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"
LABEL_SHARE = 0.03
LABEL_MIN = 32
COLUMNS = [("A", "today (old font dots)"), ("new", "your picks")]
# short row names for the gutter, in dot_states_sheet.ROWS order
NAMES = [
    ["title"],
    ["chrome label", "(tab)"],
    ["second button", "keycap + plate"],
    ["selected tab", "(negative)"],
    ["main button", "(negative)"],
    ["main button", "pressed"],
    ["chip, faint"],
    ["chip, selected"],
    ["main button", "disabled"],
    ["second button", "disabled"],
]


def main() -> None:
    d = Path(sys.argv[1])
    shots = {v: Image.open(d / f"gallery-{v}.png").convert("RGB") for v, _ in COLUMNS}
    gap = 24
    crops_h = sum(h * SCALE + gap for *_, h in STATE_ROWS)
    # the label size depends on the sheet's height, which depends on the header band
    size = LABEL_MIN
    for _ in range(4):
        head = int(size * 2.2)
        height = head + crops_h + gap
        size = max(LABEL_MIN, int(round(height * LABEL_SHARE)))
    font = ImageFont.truetype(FONT, size)
    probe = ImageDraw.Draw(Image.new("RGB", (1, 1)))
    gutter = gap * 2 + max(int(probe.textlength(line, font=font)) for n in NAMES for line in n)
    col_w = max(w for *_, w, _ in STATE_ROWS) * SCALE
    width = gutter + len(COLUMNS) * (col_w + gap)
    sheet = Image.new("RGB", (width, height), (8, 6, 16))
    draw = ImageDraw.Draw(sheet)
    draw.rectangle((0, 0, width, head - 1), fill=(30, 24, 48))
    for i, (_, title) in enumerate(COLUMNS):
        x = gutter + i * (col_w + gap)
        draw.text((x + col_w / 2, head / 2), title, font=font, fill=(255, 230, 120), anchor="mm")
    y = head + gap // 2
    for (label, x, cy, w, h), names in zip(STATE_ROWS, NAMES):
        rh = h * SCALE
        lines = len(names)
        for j, line in enumerate(names):
            ty = y + rh / 2 + (j - (lines - 1) / 2) * size * 1.15
            draw.text((gap, ty), line, font=font, fill=(255, 255, 255), anchor="lm")
        for i, (v, _) in enumerate(COLUMNS):
            crop = shots[v].crop((x, cy, x + w, cy + h)).resize((w * SCALE, rh), Image.NEAREST)
            sheet.paste(crop, (gutter + i * (col_w + gap), y))
        y += rh + gap
    out = d / "picks-4x.png"
    sheet.save(out)
    print(out, sheet.size, "label px", size)


if __name__ == "__main__":
    main()
