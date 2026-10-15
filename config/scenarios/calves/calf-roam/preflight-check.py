#!/usr/bin/env python3
"""CALF-ROAM effect-check preflight (memory: effect-check preflight): the two quick runs
(base and the strongest arm, c, on one fresh wide world with late calves) must differ in the
knobs (their resolved params) and in what the knobs move (the calves' census). Prints the
evidence; exits 0 when they differ, 1 when not, 2 when a run is missing.

  preflight-check.py BASE_JOB_DIR C_JOB_DIR
"""
import csv
import sys
from pathlib import Path

KNOBS = ('lost_s', 'excursion_per_h')
COLS = ('ls_calves', 'ls_calf_max_m', 'ls_calves_unseen', 'ls_excursions', 'ls_bleats',
        'ls_exc_end_lost_sight', 'ls_exc_end_arrived', 'ls_exc_end_timed_out')


def load(job):
    run = Path(job) / 'run'
    params = (run / 'params.toml').read_text().splitlines()
    rows = list(csv.DictReader(open(run / 'stdout.log')))
    return params, rows


def summary(rows):
    out = {}
    for c in COLS:
        vals = [float(r[c]) for r in rows if r.get(c) not in (None, '')]
        out[c] = (max(vals) if c == 'ls_calf_max_m' else sum(vals) if c == 'ls_calves_unseen' else vals[-1] if vals else 0.0) if vals else 0.0
    return out


def main():
    try:
        (pb, rb), (pc, rc) = load(sys.argv[1]), load(sys.argv[2])
    except (OSError, IndexError, KeyError) as e:
        print(f'preflight: missing output ({e})')
        sys.exit(2)
    sb, sc = summary(rb), summary(rc)
    print(f'preflight minutes: base {len(rb)} rows, c {len(rc)} rows')
    # The resolved params differ only in the arm's knobs (the loftstrider's young table).
    changed = [(a, b) for a, b in zip(pb, pc) if a != b]
    for a, b in changed:
        print(f'params: base `{a}`  c `{b}`')
    for c in COLS:
        print(f'  {c}: base {sb[c]:g}  c {sc[c]:g}')
    knobs_differ = len(pb) == len(pc) and bool(changed) and all(a.split('=')[0].strip() in KNOBS for a, _ in changed)
    census_differs = any(sb[c] != sc[c] for c in COLS if c != 'ls_calves')
    print(f'knobs differ: {knobs_differ}; census differs: {census_differs}')
    sys.exit(0 if knobs_differ and census_differs else 1)


if __name__ == '__main__':
    main()
