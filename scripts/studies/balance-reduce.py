#!/usr/bin/env python3
"""BALANCE: one row per arm from OUT/<arm>.jsonl (engagement rows): outcome shares, median
time, kicks landed, hound deaths and wounds, give-ups, calf chases and excursions."""
import json
from pathlib import Path
import statistics
import sys

root = Path(sys.argv[1])
cols = ['arm', 'n', 'adult_killed', 'calf_killed', 'gave_up', 'timeout', 'median_s', 'mean_s',
        'kicks', 'hound_deaths', 'hound_wounds', 'hound_tests', 'give_ups', 'calf_chases',
        'adult_chases', 'excursions']
print('\t'.join(cols))
for f in sorted(root.glob('*.jsonl')):
    rows = [json.loads(l) for l in f.read_text().splitlines() if l.startswith('{')]
    if not rows:
        continue
    n = len(rows)
    share = lambda o: f"{sum(r['outcome'] == o for r in rows) / n:.3f}"
    total = lambda k: sum(r.get(k, 0) for r in rows)
    print('\t'.join(map(str, [f.stem, n, share('adult_killed'), share('calf_killed'), share('pack_gave_up'),
                              share('timeout'), f"{statistics.median(r['time_s'] for r in rows):.1f}",
                              f"{statistics.mean(r['time_s'] for r in rows):.1f}", total('kicks'),
                              total('hound_deaths'), f"{total('hound_wounds'):.2f}", total('hound_tests'),
                              total('give_ups'), total('calf_chases'), total('adult_chases'),
                              sum(sum(r.get('excursions', [0])) for r in rows)])))
