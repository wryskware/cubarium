#!/usr/bin/env python3
"""BALANCE rate model (design/7_Research/balance-2026-10-12.md): per world-year predictions
from per-encounter harness shares and live encounter rates.

  kills a year      = YEAR_H * sum_k E_k * P(kill | k)
  standoffs an hour = sum_k E_k * P(gave up | k)          (grouped types)
  hound deaths a yr = YEAR_H * sum_k E_k * P(hound death | k)

Per-encounter shares come from balance-reduce.py summaries (`--summary`, any number) by arm:
`--type family=family-3`. For the strayed-calf type the share is per excursion: calf kills
over excursions in that arm's rows (`strays=strays-3--gtc2p5`).

Encounter rates (an hour) are given (`--rate family=0.5`) or read from a wide run's census
(`--census DIR`: every census.csv under it): the pack's give-ups and tests an hour while
both lineages live, and loftstrider kills an hour, for comparison.
"""
import argparse
import csv
from pathlib import Path

YEAR_H = 10.0


def load(summaries):
    rows = {}
    for s in summaries:
        with open(s) as f:
            for r in csv.DictReader(f, delimiter='\t'):
                rows[r['arm']] = r
    return rows


def census_rates(root):
    hours = tests = give_ups = kills = kicks = 0.0
    hd = 0.0
    for path in sorted(Path(root).rglob('census.csv')):
        rows = list(csv.DictReader(open(path)))
        if len(rows) < 2:
            continue
        live = [r for r in rows if float(r.get('founders_loftstrider', 0) or 0) > 0 and float(r.get('founders_chorister', 0) or 0) > 0]
        if len(live) >= 2:
            hours += (float(live[-1]['sim_min']) - float(live[0]['sim_min'])) / 60.0
        get = lambda k: float(rows[-1].get(k, 0) or 0) - float(rows[0].get(k, 0) or 0)
        tests += get('hound_tests')
        give_ups += get('hound_give_ups')
        kills += get('kills_chorister_loftstrider')
        kicks += get('kicks_landed_loftstrider')
        hd += get('kills_loftstrider_chorister')
    h = max(hours, 1e-9)
    return {'exposure_h': hours, 'tests_per_h': tests / h, 'give_ups_per_h': give_ups / h,
            'ls_kills_per_h': kills / h, 'kicks_per_h': kicks / h, 'hound_kick_deaths_per_h': hd / h}


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--summary', action='append', default=[])
    p.add_argument('--type', action='append', default=[], help='TYPE=ARM')
    p.add_argument('--rate', action='append', default=[], help='TYPE=encounters per hour')
    p.add_argument('--census')
    o = p.parse_args()
    rows = load(o.summary)
    types = dict(t.split('=', 1) for t in o.type)
    rates = {k: float(v) for k, v in (r.split('=', 1) for r in o.rate)}
    if o.census:
        c = census_rates(o.census)
        print('census', ' '.join(f'{k}={v:.3f}' for k, v in c.items()))
    adults = calves = deaths = standoffs = 0.0
    print('type\tarm\tE/h\tP(adult)\tP(calf)\tP(gave up)\tP(hound death)\tadults/yr\tcalves/yr\thound deaths/yr')
    for k, arm in types.items():
        r = rows[arm]
        n = float(r['n'])
        e = rates.get(k, 0.0)
        if k == 'strays':
            exc = max(float(r['excursions']), 1e-9)
            pa, pc, pg, ph = 0.0, float(r['calf_killed']) * n / exc, 0.0, float(r['hound_deaths']) / exc
        else:
            pa, pc, pg, ph = float(r['adult_killed']), float(r['calf_killed']), float(r['gave_up']), float(r['hound_deaths']) / n
        a, c, d = YEAR_H * e * pa, YEAR_H * e * pc, YEAR_H * e * ph
        adults, calves, deaths, standoffs = adults + a, calves + c, deaths + d, standoffs + e * pg
        print(f'{k}\t{arm}\t{e:.3f}\t{pa:.3f}\t{pc:.3f}\t{pg:.3f}\t{ph:.4f}\t{a:.2f}\t{c:.2f}\t{d:.3f}')
    print(f'TOTAL a year: loftstriders {adults + calves:.2f} (adults {adults:.2f}, calves {calves:.2f}); '
          f'standoffs {standoffs:.2f}/h; hound deaths {deaths:.3f}')


if __name__ == '__main__':
    main()
