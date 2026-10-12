#!/usr/bin/env python3
"""The dot_gap_fill ladder for the dotted negative plates (Wrysk, 2026-10-11): 4x crops of
the selected tab, the main button and the main button pressed, and a 1x strip of the
selected tab, one column per strength, from gallery-<variant>.png pictures made by
tests/neon_shots.gd at 1920x1080 with --dots new, gap25, gap40, gap55, gap70 and solid
(tests/dot_variants.gd):

    python3 game/tests/gap_fill_ladder_sheet.py DIR

Writes DIR/gap-fill-ladder-4x.png and DIR/gap-fill-ladder-1x.png, labelled as
dot_picks_sheet.py labels its sheet.
"""
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from dot_picks_sheet import NAMES, sheet  # noqa: E402
from dot_states_sheet import ROWS as STATE_ROWS  # noqa: E402

COLUMNS = [("new", "0 % (picks)"), ("gap25", "25 %"), ("gap40", "40 %"),
           ("gap55", "55 %"), ("gap70", "70 %"), ("solid", "solid")]
LADDER = [3, 4, 5]  # selected tab, main button, main button pressed in STATE_ROWS


def main() -> None:
    d = Path(sys.argv[1])
    shots = {v: Image.open(d / f"gallery-{v}.png").convert("RGB") for v, _ in COLUMNS}
    rows = [STATE_ROWS[i] for i in LADDER]
    names = [NAMES[i] for i in LADDER]
    for scale, picked, picked_names in [(4, rows, names), (1, rows[:1], names[:1])]:
        img, size = sheet(shots, COLUMNS, picked, picked_names, scale)
        out = d / f"gap-fill-ladder-{scale}x.png"
        img.save(out)
        print(out, img.size, "label px", size)


if __name__ == "__main__":
    main()
