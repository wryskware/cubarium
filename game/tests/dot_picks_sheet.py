#!/usr/bin/env python3
"""Before/after 4x crops of every Gridlite state in the neon gallery: Gridlite as the font
drew its own unlit dots (tests/dot_variants.gd variant A) beside the game's default (the
drawn field with Wrysk's picks, 2026-10-11), from gallery-A.png and gallery-new.png made
by tests/neon_shots.gd at 1920x1080:

    python3 game/tests/dot_picks_sheet.py DIR

Writes DIR/picks-4x.png. Column titles sit in a header band and row names in a left
gutter, in a large font (LABEL_SHARE of the sheet's height, or of half its width when
wider, at least LABEL_MIN px), never over the pictures.
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


def sheet(shots: dict, columns: list, rows: list, names: list, scale: int = SCALE) -> tuple:
    """Crops `rows` ((label, x, y, w, h) in each picture) out of `shots` (column key: the
    picture), `scale`x nearest-neighbour, one column per (key, title) in `columns`, with
    the titles in a header band and each row's `names` lines in a left gutter."""
    gap = 24
    crops_h = sum(h * scale + gap for *_, h in rows)
    probe = ImageDraw.Draw(Image.new("RGB", (1, 1)))

    def layout(size: int):
        font = ImageFont.truetype(FONT, size)
        head = int(size * 2.2)
        height = head + crops_h + gap
        gutter = gap * 2 + max(int(probe.textlength(line, font=font)) for n in names for line in n)
        col_w = max(max(w for *_, w, _ in rows) * scale,
                    max(int(probe.textlength(t, font=font)) for _, t in columns) + gap)
        width = gutter + len(columns) * (col_w + gap)
        return font, head, height, gutter, col_w, width

    # the label size follows the sheet's size (LABEL_SHARE of its height, or half that of
    # its width for a wide sheet), which depends on the labels: settle it in a few rounds
    size = LABEL_MIN
    for _ in range(6):
        *_, height, _, _, width = layout(size)
        size = max(LABEL_MIN, int(round(max(height, width / 2) * LABEL_SHARE)))
    font, head, height, gutter, col_w, width = layout(size)
    out = Image.new("RGB", (width, height), (8, 6, 16))
    draw = ImageDraw.Draw(out)
    draw.rectangle((0, 0, width, head - 1), fill=(30, 24, 48))
    for i, (_, title) in enumerate(columns):
        x = gutter + i * (col_w + gap)
        draw.text((x + col_w / 2, head / 2), title, font=font, fill=(255, 230, 120), anchor="mm")
    y = head + gap // 2
    for (label, x, cy, w, h), lines in zip(rows, names):
        rh = h * scale
        for j, line in enumerate(lines):
            ty = y + rh / 2 + (j - (len(lines) - 1) / 2) * size * 1.15
            draw.text((gap, ty), line, font=font, fill=(255, 255, 255), anchor="lm")
        for i, (v, _) in enumerate(columns):
            crop = shots[v].crop((x, cy, x + w, cy + h)).resize((w * scale, rh), Image.NEAREST)
            out.paste(crop, (gutter + i * (col_w + gap) + (col_w - w * scale) // 2, y))
        y += rh + gap
    return out, size


def main() -> None:
    d = Path(sys.argv[1])
    shots = {v: Image.open(d / f"gallery-{v}.png").convert("RGB") for v, _ in COLUMNS}
    img, size = sheet(shots, COLUMNS, STATE_ROWS, NAMES)
    out = d / "picks-4x.png"
    img.save(out)
    print(out, img.size, "label px", size)


if __name__ == "__main__":
    main()
