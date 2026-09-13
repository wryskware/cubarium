#!/usr/bin/env python3
"""The glasscane wind-room candidate as a source edit (2026-09-13), recorded and checked.

Writes `candidate/plant_glasscane_trunk.svg` and `candidate/plant_glasscane_crown.svg`
beside this script from the production parts in `art/parts/`, removing exactly:

* trunk: every painted pixel of rows 0 and 15 (8 pixels: the `oMMo` joint row and the
  `oBCo` row, the two rows no drawn strip owns once the family opts into the shifted
  strips — see `art_present::trunk_strip`);
* crown: every painted pixel of row 15 (4 pixels of trunk pattern under the bulbs,
  which the loader's derived cap would otherwise keep once the trunk stops painting it).

Nothing else changes: not a bulb, not a joint, not the base, not a colour. The script
never writes into `art/parts/`; production integration is a separate, reviewed step.
Re-running it is a no-op check (`--check` compares the candidate files and exits 1 on
drift). It also prints the exact pixel differences as JSON.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PARTS = REPO / 'art' / 'parts'
OUT = HERE / 'candidate'
assert OUT.resolve() != PARTS.resolve() and PARTS not in OUT.parents

TRUNK_ROWS = (0, 15)
CROWN_ROWS = (15,)


def read(path: Path):
    text = path.read_text()
    w = int(re.search(r'width="(\d+)"', text).group(1))
    h = int(re.search(r'height="(\d+)"', text).group(1))
    px: dict[tuple[int, int], tuple[str, str | None]] = {}
    for fill, opacity, d in re.findall(
        r'<path fill="(#[0-9A-Fa-f]{6})"(?: fill-opacity="([0-9.]+)")? d="([^"]+)"/>', text
    ):
        for x, y in re.findall(r'M(\d+) (\d+)h1v1h-1z', d):
            px[(int(x), int(y))] = (fill, opacity or None)
    return w, h, px


def svg_of(w: int, h: int, px: dict) -> str:
    by: dict = {}
    for y in range(h):
        for x in range(w):
            if (x, y) in px:
                by.setdefault(px[(x, y)], []).append(f'M{x} {y}h1v1h-1z')

    def attrs(c):
        return f'fill="{c[0]}" fill-opacity="{c[1]}"' if c[1] else f'fill="{c[0]}"'

    paths = ''.join(f'\n  <path {attrs(c)} d="{"".join(d)}"/>' for c, d in by.items())
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" '
        f'shape-rendering="crispEdges">{paths}\n</svg>\n'
    )


def candidate(name: str, rows: tuple[int, ...]):
    w, h, px = read(PARTS / f'{name}.svg')
    assert (w, h) in ((4, 16), (15, 16)), (name, w, h)
    removed = sorted((x, y, c[0], c[1]) for (x, y), c in px.items() if y in rows)
    kept = {p: c for p, c in px.items() if p[1] not in rows}
    if name == 'plant_glasscane_trunk':
        # Rows 1–14 stay the 4-periodic pattern.
        for (x, y), c in kept.items():
            assert kept.get((x, y + 4), c) == c and kept.get((x, y - 4), c) == c, (x, y)
    return svg_of(w, h, kept), removed, len(px), len(kept)


def main() -> int:
    check = '--check' in sys.argv[1:]
    report = {}
    drift = False
    for name, rows in (('plant_glasscane_trunk', TRUNK_ROWS), ('plant_glasscane_crown', CROWN_ROWS)):
        text, removed, before, after = candidate(name, rows)
        target = OUT / f'{name}.svg'
        if check:
            if not target.exists() or target.read_text() != text:
                drift = True
        else:
            OUT.mkdir(exist_ok=True)
            target.write_text(text)
        report[name] = {
            'rows_cleared': list(rows),
            'painted_before': before,
            'painted_after': after,
            'removed': [{'x': x, 'y': y, 'fill': f, 'opacity': o} for x, y, f, o in removed],
        }
    assert len(report['plant_glasscane_trunk']['removed']) == 8
    assert len(report['plant_glasscane_crown']['removed']) == 4
    print(json.dumps(report, indent=1))
    if drift:
        print('candidate SVGs differ from the recorded transform', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
