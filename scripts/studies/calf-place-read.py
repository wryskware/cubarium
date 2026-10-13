#!/usr/bin/env python3
"""CALF-PLACE's read (design/handoffs/calf-place-2026-10-13.md): per world and in total, from
every census.csv under ROOT (a batch's out directory, or several):

  calves placed          ls_calves at the run's first row (the scenario's calves; a founded
                         world has no calf of its own at minute 0)
  calf-h alive           the calves standing, summed over the minute rows
  calf-h far / orphan    ... of them beyond `young.excursion_m` of a living mother, or with
                         their mother gone (ls_calves_far, ls_calves_orphan)
  mean calf-mother m     the calves' mean ground distance to a living mother, time-weighted
  excursions, bleats     YOUNG-ROAM's excursions begun and distress calls (session counts)
  standoffs              the Veilhounds' hunts given up (hound_give_ups)
  calves wounded/killed  by lineage (calf_wounds_*/calf_kills_*_loftstrider)
  adults killed          kills_*_loftstrider less the calf kills, by lineage
  hounds wounded/killed  wounding hits on Veilhounds and their kills, by lineage
  pack kills             the Veilhounds' kills by prey (kills_chorister_*)

Counters are read as last row minus first, so a run loaded from a checkpoint counts only its
own span. Lineage labels say veilhound for the pack predator. With --balance, each ROOT also
gets scripts/studies/balance-model.py's census rates line.

  scripts/studies/calf-place-read.py runs/lab/<job-a>/batch-task/batch runs/lab/<job-b>/batch-task/batch
"""
import argparse
import csv
import importlib.util
from pathlib import Path

PACK = 'chorister'  # the Veilhound's code identifier in census column names


def label(name):
    return 'veilhound' if name == PACK else name


def num(row, key):
    v = row.get(key, '')
    try:
        return float(v) if v not in ('', None) else 0.0
    except ValueError:
        return 0.0


def read(path):
    rows = list(csv.DictReader(open(path)))
    if len(rows) < 2 or 'ls_calves' not in rows[0]:
        return None
    first, last = rows[0], rows[-1]
    diff = lambda k: num(last, k) - num(first, k)
    lineages = [k[len('calf_kills_'):-len('_loftstrider')] for k in rows[0] if k.startswith('calf_kills_')]
    ticks = rows[1:]
    calf_min = sum(num(r, 'ls_calves') for r in ticks)
    with_mother = sum(num(r, 'ls_calves') - num(r, 'ls_calves_orphan') for r in ticks)
    dist = sum(num(r, 'ls_calf_dist_m') * (num(r, 'ls_calves') - num(r, 'ls_calves_orphan')) for r in ticks)
    out = dict(
        minutes=num(last, 'sim_min') - num(first, 'sim_min'),
        placed=num(first, 'ls_calves'),
        calves_end=num(last, 'ls_calves'),
        adults_end=num(last, 'founders_loftstrider') - num(last, 'ls_calves'),
        calf_h=calf_min / 60.0,
        far_h=sum(num(r, 'ls_calves_far') for r in ticks) / 60.0,
        orphan_h=sum(num(r, 'ls_calves_orphan') for r in ticks) / 60.0,
        dist_sum=dist,
        with_mother_min=with_mother,
        excursions=diff('ls_excursions'),
        bleats=diff('ls_bleats'),
        standoffs=diff('hound_give_ups'),
        tests=diff('hound_tests'),
        kicks=diff('kicks_landed_loftstrider'),
        hounds_end=num(last, f'founders_{PACK}'),
    )
    for f in lineages:
        out[f'calf_wounded:{f}'] = diff(f'calf_wounds_{f}_loftstrider')
        out[f'calf_killed:{f}'] = diff(f'calf_kills_{f}_loftstrider')
        out[f'adult_killed:{f}'] = diff(f'kills_{f}_loftstrider') - out[f'calf_killed:{f}']
        out[f'hound_wounded:{f}'] = diff(f'wounds_{f}_{PACK}')
        out[f'hound_killed:{f}'] = diff(f'kills_{f}_{PACK}')
        out[f'pack_kills:{f}'] = diff(f'kills_{PACK}_{f}')
    return out


def by(d, prefix):
    parts = [f'{label(k.split(":", 1)[1])} {v:g}' for k, v in sorted(d.items()) if k.startswith(prefix + ':') and v]
    return ', '.join(parts) or '0'


def report(name, d):
    mean = d['dist_sum'] / d['with_mother_min'] if d['with_mother_min'] else float('nan')
    total = lambda p: sum(v for k, v in d.items() if k.startswith(p + ':'))
    print(f'{name}: {d["minutes"]:g} min; calves placed {d["placed"]:g}, alive at end {d["calves_end"]:g}; '
          f'calf-h {d["calf_h"]:.2f}, beyond excursion distance {d["far_h"]:.2f} h, orphaned {d["orphan_h"]:.2f} h; '
          f'mean calf-mother {mean:.1f} m; excursions {d["excursions"]:g}, bleats {d["bleats"]:g}')
    print(f'    standoffs {d["standoffs"]:g} (tests {d["tests"]:g}); calves wounded {total("calf_wounded"):g} '
          f'[{by(d, "calf_wounded")}], killed {total("calf_killed"):g} [{by(d, "calf_killed")}]; '
          f'adults killed {total("adult_killed"):g} [{by(d, "adult_killed")}]')
    print(f'    veilhounds wounded {total("hound_wounded"):g} [{by(d, "hound_wounded")}], '
          f'killed {total("hound_killed"):g} [{by(d, "hound_killed")}]; kicks landed {d["kicks"]:g}; '
          f'pack kills [{by(d, "pack_kills")}]; at end: loftstrider adults {d["adults_end"]:g}, veilhounds {d["hounds_end"]:g}')


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('roots', nargs='+', type=Path)
    p.add_argument('--balance', action='store_true', help="also print balance-model.py's census rates per root")
    o = p.parse_args()
    total, worlds = {}, 0
    for root in o.roots:
        for path in sorted(root.rglob('census.csv')):
            d = read(path)
            if d is None:
                print(f'{path}: no CALF-PLACE columns or fewer than two rows')
                continue
            worlds += 1
            report(str(path.parent.relative_to(root)) if path.parent != root else str(path), d)
            for k, v in d.items():
                total[k] = total.get(k, 0.0) + v
    if worlds:
        report(f'TOTAL over {worlds} worlds', total)
    if o.balance:
        spec = importlib.util.spec_from_file_location('balance_model', Path(__file__).with_name('balance-model.py'))
        bm = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(bm)
        for root in o.roots:
            c = bm.census_rates(root)
            print('balance-model census', root, ' '.join(f'{k}={v:.3f}' for k, v in c.items()))


if __name__ == '__main__':
    main()
