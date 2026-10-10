#!/usr/bin/env python3
"""Reduce ROOT/ARM/RUN/drift.json into drift.csv and drift.txt.

Each run has equal weight. SD is the sample standard deviation (undefined
for fewer than two observations). Null ratios remain missing observations.
Offtake is the whole-world leaf_eaten / leaf_produced ratio, repeated on each
lineage row. Kills are summed over prey for that predator, per world-hour;
the original JSON retains prey pairs and body masses for size filtering.
"""

import argparse
import csv
import json
import math
from pathlib import Path
import statistics
from collections import defaultdict


METRICS = ('rho', 'rho_E', 'offtake', 'kills_per_world_hour')
FIELDS = ['arm', 'lineage', 'runs'] + [
    f'{metric}_{suffix}' for metric in METRICS for suffix in ('mean', 'sd', 'n')
]


def number(value, name, nullable=False):
    if value is None and nullable:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f'{name}: expected a finite number' + (' or null' if nullable else ''))
    return value


def read_run(path):
    with path.open() as stream:
        data = json.load(stream)
    start = number(data['from_minute'], 'from_minute')
    stop = number(data['to_minute'], 'to_minute')
    hours = number(data['hours'], 'hours')
    if start < 0 or stop <= start or not math.isclose(hours, (stop - start) / 60):
        raise ValueError('window boundaries and hours disagree')
    if data['clipped'] is not False:
        raise ValueError('clipped windows cannot be aggregated')
    world = data['world']
    offtake = number(world['leaf_offtake'], 'leaf_offtake', nullable=True)
    if offtake is not None and offtake < 0:
        raise ValueError('leaf_offtake must be nonnegative')
    kills = defaultdict(float)
    for pair in world['hunts']:
        rate = number(pair['kills_per_hour'], 'kills_per_hour')
        if rate < 0:
            raise ValueError('kills_per_hour must be nonnegative')
        kills[pair['predator']] += rate
    rows = {}
    for entry in data['lineages']:
        name = entry['lineage']
        if not isinstance(name, str) or not name or name in rows:
            raise ValueError('lineages must have unique nonempty names')
        rows[name] = dict(rho=number(entry['rho'], 'rho', nullable=True),
                          rho_E=number(entry['rho_E'], 'rho_E', nullable=True),
                          offtake=offtake, kills_per_world_hour=kills[name])
    if not rows:
        raise ValueError('no lineage readouts')
    if set(kills) - set(rows):
        raise ValueError('hunt predator has no lineage readout')
    return (start, stop), rows


def aggregate(root, filename='drift.json'):
    """Aggregate per-run observations; reject mixed windows or lineage sets."""
    grouped = defaultdict(list)
    window = None
    lineages = None
    seen = set()
    paths = sorted(root.rglob(filename))
    if not paths:
        raise ValueError(f'no {filename} files under {root}')
    for path in paths:
        relative = path.relative_to(root)
        if len(relative.parts) != 3:
            raise ValueError(f'{path}: expected ROOT/ARM/RUN/{filename}')
        arm, run = relative.parts[:2]
        if (arm, run) in seen:
            raise ValueError(f'{path}: duplicate run')
        seen.add((arm, run))
        try:
            current_window, rows = read_run(path)
            if window is None:
                window, lineages = current_window, set(rows)
            if current_window != window:
                raise ValueError('window differs between runs')
            if set(rows) != lineages:
                raise ValueError('lineage set differs between runs')
        except (ValueError, KeyError, TypeError) as error:
            raise ValueError(f'{path}: {error}') from error
        for lineage, row in rows.items():
            grouped[arm, lineage].append(row)
    results = []
    for (arm, lineage), observations in sorted(grouped.items()):
        result = dict(arm=arm, lineage=lineage, runs=len(observations))
        for metric in METRICS:
            values = [row[metric] for row in observations if row[metric] is not None]
            result[f'{metric}_mean'] = statistics.mean(values) if values else None
            result[f'{metric}_sd'] = statistics.stdev(values) if len(values) > 1 else None
            result[f'{metric}_n'] = len(values)
        results.append(result)
    return results


def display(value):
    return 'NA' if value is None else f'{value:.6g}'


def text_table(rows):
    lines = ['arm lineage runs | rho mean(sd) | rho_E mean(sd) | world offtake mean(sd) | predator kills/world-hour mean(sd)']
    for row in rows:
        values = [f"{display(row[f'{metric}_mean'])}({display(row[f'{metric}_sd'])})"
                  for metric in METRICS]
        lines.append(f"{row['arm']} {row['lineage']} {row['runs']} | " + ' | '.join(values))
    lines.append('SD: sample; NA: undefined. Offtake repeats the world ratio; kills sum over prey. See CSV metric n for non-null runs.')
    return '\n'.join(lines) + '\n'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--filename', default='drift.json', help='JSON basename in each run directory')
    parser.add_argument('--csv', type=Path, help='default: ROOT/drift.csv')
    parser.add_argument('--text', type=Path, help='default: ROOT/drift.txt')
    args = parser.parse_args(argv)
    if Path(args.filename).name != args.filename:
        parser.error('--filename must be a basename')
    try:
        rows = aggregate(args.root, args.filename)
        with (args.csv or args.root / 'drift.csv').open('w', newline='') as stream:
            writer = csv.DictWriter(stream, fieldnames=FIELDS)
            writer.writeheader()
            writer.writerows(rows)
        report = text_table(rows)
        (args.text or args.root / 'drift.txt').write_text(report)
        print(report, end='')
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))


if __name__ == '__main__':
    main()
