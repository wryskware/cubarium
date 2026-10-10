#!/usr/bin/env python3
"""Compare bake libraries bytewise; describe CVM changes by step/variant/state/cell."""
import argparse
from pathlib import Path
import struct


def cvm_cells(path):
    data = path.read_bytes()
    assert data[:4] == b'CVM2', path
    at = 5

    def take(fmt):
        nonlocal at
        values = struct.unpack_from(fmt, data, at)
        at += struct.calcsize(fmt)
        return values

    def name():
        nonlocal at
        size, = take('<B')
        value = data[at:at + size].decode()
        at += size
        return value

    palette = [name() for _ in range(take('<B')[0])]
    states = []
    for _ in range(take('<B')[0]):
        states.append((name(), take('<d')[0]))
    cells = {}
    for step in range(take('<H')[0]):
        _, _, variants = take('<ddB')
        for variant in range(variants):
            for state in range(len(states)):
                for _ in range(take('<I')[0]):
                    x, y, z, mat, tag = take('<bbbBB')
                    key = (step, variant, states[state][0], x, y, z)
                    assert key not in cells, (path, key)
                    cells[key] = (palette[mat], tag)
    assert at == len(data), path
    return cells


def rig_cells(path):
    data = path.read_bytes()
    assert data[:4] in (b'CRG2', b'CRG3'), path
    at = 4

    def take(fmt):
        nonlocal at
        values = struct.unpack_from(fmt, data, at)
        at += struct.calcsize(fmt)
        return values

    def name():
        nonlocal at
        size, = take('<B')
        value = data[at:at + size].decode()
        at += size
        return value

    bones = []
    for _ in range(take('<B')[0]):
        bones.append(name())
        take('<B')  # parent
    order, = take('<B')
    if order >= 2:
        at += len(bones)
    frames = []
    for _ in range(take('<B')[0]):
        name()
        _, _, count = take('<BfH')
        frames.append(count)
    cells = {}
    for step in range(take('<B')[0]):
        take('<df')  # length, scale
        at += len(frames) * (8 if data[:4] == b'CRG3' else 4)
        at += len(bones) * 12  # heads
        at += sum(frames) * len(bones) * 28  # quaternion, displacement
        for _ in range(take('<I')[0]):
            x, y, z, bone = take('<bbbB')
            key = (step, x, y, z)
            assert key not in cells, (path, key)
            cells[key] = bones[bone]
    assert at == len(data), path
    return cells


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('left', type=Path)
    parser.add_argument('right', type=Path)
    parser.add_argument('--cells', action='store_true', help='print every changed cell')
    args = parser.parse_args()
    paths = sorted({p.relative_to(root) for root in (args.left, args.right)
                    for p in root.rglob('*') if p.is_file()
                    and (p.suffix in ('.cvm', '.rig') or p.name == 'manifest.json')})
    changed = 0
    for path in paths:
        left, right = args.left / path, args.right / path
        if left.exists() and right.exists() and left.read_bytes() == right.read_bytes():
            continue
        changed += 1
        if not left.exists() or not right.exists():
            print(f'{path}: missing on one side')
        elif path.suffix in ('.cvm', '.rig'):
            read = cvm_cells if path.suffix == '.cvm' else rig_cells
            a, b = read(left), read(right)
            keys = sorted(k for k in a.keys() | b.keys() if a.get(k) != b.get(k))
            print(f'{path}: {len(keys)} cells differ '
                  f'({len(b.keys() - a.keys())} added, {len(a.keys() - b.keys())} removed, '
                  f'{sum(k in a and k in b for k in keys)} changed)')
            if not keys:
                print('  header/animation/other non-cell bytes differ')
            if args.cells:
                for key in keys:
                    print(f'  {key}: {a.get(key)} -> {b.get(key)}')
        else:
            print(f'{path}: bytes differ')
    print(f'{len(paths)} files compared; {changed} differ')
    return bool(changed)


if __name__ == '__main__':
    raise SystemExit(main())
