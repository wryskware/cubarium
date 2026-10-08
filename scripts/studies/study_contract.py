"""Shared, stdlib-only experiment identity and founding-roster contract."""

import csv
import hashlib
import math
from pathlib import Path
import tomllib


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024**2), b''):
            digest.update(block)
    return digest.hexdigest()


def validate_required(required):
    animals, plants = required.get('animals'), required.get('plants')
    references = required.get('N_ref', {})
    if not isinstance(animals, list) or not isinstance(plants, list) or not animals + plants:
        raise ValueError('required needs animals and plants lists (at least one required name)')
    for names in (animals, plants):
        if any(not isinstance(name, str) or not name for name in names) or len(set(names)) != len(names):
            raise ValueError('required names must be unique nonempty strings')
    if any(name not in references for name in animals):
        raise ValueError('every required animal needs N_ref (founding bodies + eggs)')
    if any(not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0
           for value in references.values()):
        raise ValueError('N_ref must be positive and finite')
    return required


def read_required(path):
    with Path(path).open('rb') as stream:
        document = tomllib.load(stream)
    return validate_required(document.get('required', document))


def founding_required(start):
    # Checkpoint layout is RUN/run/ckpt/mNNNNNN. Search its own run first,
    # then ancestors for older standalone start libraries.
    start = Path(start)
    candidates = [start / '../../census.csv', start / '../../../census.csv']
    candidates.extend(parent / 'census.csv' for parent in start.parents)
    for path in candidates:
        if not path.is_file():
            continue
        with path.open(newline='') as stream:
            row = next(csv.DictReader(stream), None)
        if not row or float(row.get('sim_min', -1)) != 0:
            raise ValueError(f'{path}: founding roster must be the minute-0 census')
        stocks = {key: float(value) for key, value in row.items()
                  if key.startswith(('founders_', 'eggs_', 'stands_'))}
        if any(not math.isfinite(value) or value < 0 for value in stocks.values()):
            raise ValueError(f'{path}: invalid founding counts')
        references = {key.removeprefix('founders_'): value + stocks.get('eggs_' + key.removeprefix('founders_'), 0)
                      for key, value in stocks.items() if key.startswith('founders_')}
        for key, value in stocks.items():
            if key.startswith('eggs_'):
                references.setdefault(key.removeprefix('eggs_'), value)
        required = dict(animals=sorted(name for name, count in references.items() if count > 0),
                        plants=sorted(key.removeprefix('stands_') for key, value in stocks.items()
                                      if key.startswith('stands_') and value > 0),
                        N_ref={name: count for name, count in references.items() if count > 0},
                        source=str(path.resolve()), source_sha256=sha256(path))
        return validate_required(required)
    raise ValueError(f'{start}: no founding census; supply --required FILE')


def load_census(start, minute):
    """Recover an old loaded run's omitted boundary without estimating counters."""
    start = Path(start)
    candidates = [start / '../../census.csv', start / '../../../census.csv']
    candidates.extend(parent / 'census.csv' for parent in start.parents)
    for path in candidates:
        if path.is_file():
            with path.open(newline='') as stream:
                for row in csv.DictReader(stream):
                    if float(row.get('sim_min', -1)) == minute:
                        return row
    return None
