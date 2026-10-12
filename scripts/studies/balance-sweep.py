#!/usr/bin/env python3
"""BALANCE: write an engagement sweep config from a base harness config.

Each (value, base arm) pair becomes one arm named `<arm>--<tag><value>` whose `sets` carry the
base arm's sets plus `<key>=<value>`. Several knobs may be swept in one config, one at a time
(the others at their defaults).

Usage: balance-sweep.py --base config/engagement.toml --arms family-3,adults-4 \
    --knob dmg:fauna.founders.loftstrider.kick.damage:0.09,0.18,0.36 [--knob ...] \
    [--episodes N] --out runs/specs/kick.toml
"""
import argparse
from pathlib import Path
import tomllib


def fmt(v):
    if isinstance(v, str):
        return '"' + v.replace('"', '\\"') + '"'
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, list):
        return '[' + ', '.join(fmt(x) for x in v) + ']'
    return repr(v)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--base', type=Path, required=True)
    p.add_argument('--arms', required=True, help='comma-separated base arm names')
    p.add_argument('--knob', action='append', default=[], help='tag:key:v1,v2,...')
    p.add_argument('--set', action='append', default=[], help='key=value added to every arm')
    p.add_argument('--episodes', type=int)
    p.add_argument('--out', type=Path, required=True)
    o = p.parse_args()
    base = tomllib.loads(o.base.read_text())
    arms = {a['name']: a for a in base['arms']}
    wanted = o.arms.split(',')
    out = []
    for knob in o.knob:
        tag, key, values = knob.split(':')
        for v in values.split(','):
            for name in wanted:
                a = arms[name]
                vt = v.replace('.', 'p').replace('-', 'm')
                out.append({'name': f'{name}--{tag}{vt}', 'composition': a['composition'],
                            'pack_size': a['pack_size'], 'sets': list(a.get('sets', [])) + o.set + [f'{key}={v}']})
    if not o.knob:
        for name in wanted:
            a = arms[name]
            out.append({'name': name, 'composition': a['composition'], 'pack_size': a['pack_size'],
                        'sets': list(a.get('sets', [])) + o.set})
    lines = [f'# BALANCE sweep from {o.base}; knobs {o.knob}; sets {o.set}']
    for k, v in base.items():
        if k == 'arms':
            continue
        if k == 'episodes' and o.episodes:
            v = o.episodes
        lines.append(f'{k} = {fmt(v)}')
    for a in out:
        lines += ['', '[[arms]]', f'name = {fmt(a["name"])}', f'composition = {fmt(a["composition"])}',
                  f'pack_size = {a["pack_size"]}']
        if a['sets']:
            lines.append(f'sets = {fmt(a["sets"])}')
    o.out.write_text('\n'.join(lines) + '\n')
    print(o.out, len(out), 'arms')


if __name__ == '__main__':
    main()
