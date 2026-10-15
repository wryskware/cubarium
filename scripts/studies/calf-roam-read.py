#!/usr/bin/env python3
"""CALF-ROAM's read (design/handoffs/calf-roam-2026-10-15.md): per arm, from every census.csv
under ROOT... (job dirs or batch dirs; a run's arm is the directory under `batch/`), the
step-2 numbers as totals and per world-hour, with loftstrider kills per world-year beside
topic 8's targets, then one line per world (its own numbers and the den-to-bed distance).

Counters are last row minus first (a run loaded from a checkpoint counts only its own span);
calf-hours are the minute rows summed. Columns: CALF-PLACE's (ls_calves*, calf_*_*, kills_*,
wounds_*, hound_*) and CALF-ROAM's (ls_calf_max_m, ls_calves_unseen, ls_exc_end_*,
ls_asleep*). The Veilhound is `chorister` in column names only.

  far       calf-h whose living mother is beyond `young.excursion_m` (40 m on every arm)
  unseen    calf-h whose living mother is beyond its day sight (`senses.sight_m`)
  max       the farthest any calf got from its living mother in the run, metres
  den-bed   grown loftstriders asleep: time-weighted mean, and nearest, ground distance to a
            Veilhound den, metres

The world-year is `world.climate.dry_season_every_days` x `world.day.length_min` of CONFIG
(default config/desktop/terrarium-wide.toml: 5 x 120 min = 10 h).

  python3 -I scripts/studies/calf-roam-read.py runs/lab/*-calf-roam-arm-*
"""
import argparse
import csv
import math
import tomllib
from pathlib import Path

PACK = 'chorister'  # the Veilhound's code identifier in census column names
ENDS = ('alarmed', 'mother_called', 'lost_sight', 'arrived', 'timed_out')
# Topic 8 (design/behaviour-ai-2026-10-11.md §0 row 8), per world.
TARGET = 'about 1 loftstrider killed / world-year, mostly strayed calves; a standoff every 1-2 h; ' \
         'a hound kicked dead once in a few years'


def num(row, key):
    v = row.get(key, '')
    try:
        return float(v) if v not in ('', None) else math.nan
    except ValueError:
        return math.nan


def z(x):
    return 0.0 if math.isnan(x) else x


def arm_of(path):
    parts = path.parts
    if 'batch' in parts:
        i = len(parts) - 1 - parts[::-1].index('batch')
        if i + 1 < len(parts):
            return parts[i + 1]
    return path.parent.parent.name


def read(path):
    rows = list(csv.DictReader(open(path)))
    if len(rows) < 2 or 'ls_calf_max_m' not in rows[0]:
        return None
    first, last = rows[0], rows[-1]
    diff = lambda k: z(num(last, k)) - z(num(first, k))
    ticks = rows[1:]
    lineages = [k[len('calf_kills_'):-len('_loftstrider')] for k in rows[0] if k.startswith('calf_kills_')]
    calf_kills = sum(diff(f'calf_kills_{f}_loftstrider') for f in lineages)
    ls_kills = sum(diff(f'kills_{f}_loftstrider') for f in lineages)
    asleep = [(z(num(r, 'ls_asleep')), num(r, 'ls_asleep_den_m'), num(r, 'ls_asleep_den_min_m')) for r in ticks]
    asleep = [(n, m, lo) for n, m, lo in asleep if n > 0 and not math.isnan(m)]
    maxes = [num(r, 'ls_calf_max_m') for r in ticks]
    d = dict(
        hours=(z(num(last, 'sim_min')) - z(num(first, 'sim_min'))) / 60.0,
        placed=z(num(first, 'ls_calves')),
        calf_h=sum(z(num(r, 'ls_calves')) for r in ticks) / 60.0,
        far_h=sum(z(num(r, 'ls_calves_far')) for r in ticks) / 60.0,
        unseen_h=sum(z(num(r, 'ls_calves_unseen')) for r in ticks) / 60.0,
        orphan_h=sum(z(num(r, 'ls_calves_orphan')) for r in ticks) / 60.0,
        max_m=max((m for m in maxes if not math.isnan(m)), default=math.nan),
        excursions=diff('ls_excursions'),
        bleats=diff('ls_bleats'),
        standoffs=diff('hound_give_ups'),
        calf_wounded=sum(diff(f'calf_wounds_{f}_loftstrider') for f in lineages),
        calf_killed=calf_kills,
        calf_killed_pack=diff(f'calf_kills_{PACK}_loftstrider'),
        adult_wounded=sum(diff(f'wounds_{f}_loftstrider') for f in lineages) - sum(diff(f'calf_wounds_{f}_loftstrider') for f in lineages),
        adult_killed=ls_kills - calf_kills,
        ls_killed_pack=diff(f'kills_{PACK}_loftstrider'),
        hound_wounded=sum(diff(f'wounds_{f}_{PACK}') for f in lineages),
        hound_kicked_dead=diff(f'kills_loftstrider_{PACK}'),
        kicks=diff('kicks_landed_loftstrider'),
        den_sleep_min=sum(n for n, _, _ in asleep),
        den_sum=sum(n * m for n, m, _ in asleep),
        den_min=min((lo for _, _, lo in asleep), default=math.nan),
    )
    for e in ENDS:
        d[f'end_{e}'] = diff(f'ls_exc_end_{e}')
    for f in lineages:
        d[f'pack:{f}'] = diff(f'kills_{PACK}_{f}')
    return d


def label(f):
    return 'veilhound' if f == PACK else f


def report(name, d, year_h, worlds):
    h = d['hours'] or math.nan
    per_h = lambda k: d[k] / h
    per_y = lambda k: d[k] / h * year_h
    ends = ' '.join(f'{e} {d["end_" + e]:g}' for e in ENDS)
    pack = ', '.join(f'{label(k[5:])} {v:g}' for k, v in sorted(d.items()) if k.startswith('pack:') and v) or '0'
    ls_killed = d['calf_killed'] + d['adult_killed']
    print(f'{name}: {worlds} world(s), {d["hours"]:.1f} world-h; calves placed {d["placed"]:g}, calf-h {d["calf_h"]:.1f}')
    print(f'    LS killed {ls_killed:g} (calves {d["calf_killed"]:g}, adults {d["adult_killed"]:g}; by the pack {d["ls_killed_pack"]:g}) '
          f'= {ls_killed / h * year_h:.2f} / world-year [target ~1, mostly calves]; calves killed {per_y("calf_killed"):.2f} / world-year')
    print(f'    per world-h: standoffs {per_h("standoffs"):.2f} [target 0.5-1], excursions {per_h("excursions"):.2f}, '
          f'bleats {per_h("bleats"):.2f}, calves wounded {per_h("calf_wounded"):.3f}, adults wounded {per_h("adult_wounded"):.3f}')
    print(f'    hounds wounded {d["hound_wounded"]:g}, kicked dead {d["hound_kicked_dead"]:g} = {per_y("hound_kicked_dead"):.2f} / world-year '
          f'[target ~0.2-0.5]; kicks landed {d["kicks"]:g}')
    print(f'    strays: far (>40 m) {d["far_h"]:.2f} calf-h, unseen {d["unseen_h"]:.2f} calf-h '
          f'({d["unseen_h"] / d["calf_h"] if d["calf_h"] else math.nan:.1%} of calf-h), orphaned {d["orphan_h"]:.2f}, '
          f'max {d["max_m"]:.1f} m; per calf-h excursions {d["excursions"] / d["calf_h"] if d["calf_h"] else math.nan:.2f}')
    print(f'    excursions ended: {ends}; pack kills by prey [{pack}]')


def world_line(name, d):
    mean = d['den_sum'] / d['den_sleep_min'] if d['den_sleep_min'] else math.nan
    print(f'  {name}: {d["hours"]:.1f} h; calves {d["placed"]:g}; LS killed {d["calf_killed"] + d["adult_killed"]:g} '
          f'(calves {d["calf_killed"]:g}); standoffs {d["standoffs"]:g}; hounds kicked dead {d["hound_kicked_dead"]:g}; '
          f'far {d["far_h"]:.2f} calf-h, unseen {d["unseen_h"]:.2f}, max {d["max_m"]:.1f} m; '
          f'den-bed mean {mean:.1f} m, nearest {d["den_min"]:.1f} m')


def year_hours(config):
    c = tomllib.loads(Path(config).read_text())
    days = c['world']['climate']['dry_season_every_days']
    return days * c['world']['day']['length_min'] / 60.0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('roots', nargs='+', type=Path)
    p.add_argument('--config', default='config/desktop/terrarium-wide.toml')
    p.add_argument('--worlds', action='store_true', help='also print one line per world')
    o = p.parse_args()
    year_h = year_hours(o.config)
    print(f'world-year {year_h:g} h (from {o.config}); topic 8: {TARGET}')
    arms = {}
    for root in o.roots:
        for path in sorted(root.rglob('census.csv')):
            if 'inputs' in path.parts:
                continue
            d = read(path)
            if d is None:
                print(f'{path}: no CALF-ROAM columns or fewer than two rows')
                continue
            arms.setdefault(arm_of(path), []).append((path, d))
    for arm, runs in arms.items():
        total = {}
        for _, d in runs:
            for k, v in d.items():
                if k == 'max_m':
                    total[k] = max(z(total.get(k, 0.0)), z(v))
                elif k == 'den_min':
                    total[k] = min(total.get(k, math.inf), v if not math.isnan(v) else math.inf)
                else:
                    total[k] = total.get(k, 0.0) + v
        report(f'ARM {arm}', total, year_h, len(runs))
        if o.worlds:
            for path, d in runs:
                world_line(str(path.parent.name), d)


if __name__ == '__main__':
    main()
