#!/usr/bin/env python3
"""Earn the spiretree its wind room (2026-09-13).

The checked-in SVGs stay the source of truth; this script records exactly how two of them
were changed from the art/plants/author_tall.py originals, and re-running it is refused
once they are.

Why: a column's bend budget is the smallest headroom of any painted texel in its parts at
their highest placement, against the 9 px stamp footprint. The spiretree cap was bound to
0.30 px by the dome's rightmost column — the dome was drawn one pixel right of the trunk
axis — and, behind that, every 4-wide trunk tile to 0.47 px by its own top row, which sits
7.5 px above the tile pivot where the footprint circle is only ±1.96 px wide.

What:
* crown rows 0–7 are redrawn centred on the pivot (widths 2, 6, 8, 10, 12, 12, 10, 8 — a
  pointed tip, the same 12 px dome, the same palette; the mint/cyan vein stays on the
  trunk's cyan column); rows 8–14 keep the trunk pattern; row 15 is left unpainted so the
  cap the loader derives is the dome alone;
* trunk rows 0 and 15 are left unpainted. Neither row is owned by any drawn strip once the
  presenter's strips own tile rows 1–4 (1–6 for the first segment), so the drawn column
  is unchanged pixel for pixel while the tile's measured headroom rises from 0.47 to
  about 2.5 px.
"""
from __future__ import annotations
import re
from pathlib import Path

PARTS = Path('/home/wrysk/wryskware/cubarium/art/parts')
O, G, C, T = '#18115D', '#7BEBC9', '#42C5F8', '#1F8A96'


def read(name: str):
    text = (PARTS / f'{name}.svg').read_text()
    w = int(re.search(r'width="(\d+)"', text).group(1))
    h = int(re.search(r'height="(\d+)"', text).group(1))
    px = {}
    for fill, d in re.findall(r'<path fill="(#[0-9A-Fa-f]{6})" d="([^"]+)"/>', text):
        for x, y in re.findall(r'M(\d+) (\d+)h1v1h-1z', d):
            px[(int(x), int(y))] = fill
    return w, h, px


def write(name: str, w: int, h: int, px: dict) -> None:
    by = {}
    for y in range(h):
        for x in range(w):
            if (x, y) in px:
                by.setdefault(px[(x, y)], []).append(f'M{x} {y}h1v1h-1z')
    paths = ''.join(f'\n  <path fill="{c}" d="{"".join(d)}"/>' for c, d in by.items())
    (PARTS / f'{name}.svg').write_text(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" '
        f'shape-rendering="crispEdges">{paths}\n</svg>\n')


# The new dome, in sprite columns (sprite x = tile column − 1; the pivot lies between tile
# columns 7 and 8, i.e. between sprite columns 6 and 7).
DOME = [
    '......OO.......',   # row 0: a two-pixel tip
    '....OGGGGO.....',   # row 1
    '...OGCGGCGO....',   # row 2
    '..OGCGGGGCGO...',   # row 3
    '.OTGGGGCGGGTO..',   # row 4
    '.OTTGGGGGGTTO..',   # row 5
    '..OTTTGGGTTO...',   # row 6
    '...OOTTTTOO....',   # row 7
]
PAL = {'O': O, 'G': G, 'C': C, 'T': T}


def main() -> None:
    w, h, crown = read('plant_spiretree_crown')
    assert (w, h) == (15, 16)
    assert (13, 4) in crown, 'the crown was already recentred'
    new = {p: c for p, c in crown.items() if 8 <= p[1] <= 14}
    for y, row in enumerate(DOME):
        for x, ch in enumerate(row):
            if ch != '.':
                new[(x, y)] = PAL[ch]
    # Symmetric outline about the pivot, row by row.
    for y in range(8):
        xs = [x for (x, yy) in new if yy == y]
        assert min(xs) + max(xs) == 13, (y, min(xs), max(xs))
    assert not any(p[1] == 15 for p in new)
    write('plant_spiretree_crown', w, h, new)

    w, h, trunk = read('plant_spiretree_trunk')
    assert (w, h) == (4, 16) and (0, 0) in trunk, 'the trunk end rows were already cleared'
    new = {p: c for p, c in trunk.items() if 1 <= p[1] <= 14}
    for (x, y), c in new.items():
        assert new.get((x, y + 4), c) == c and new.get((x, y - 4), c) == c, 'not 4-periodic'
    write('plant_spiretree_trunk', w, h, new)
    print('spiretree: crown dome recentred (tip 2 px, row 15 clear); trunk rows 0 and 15 cleared')


if __name__ == '__main__':
    main()
