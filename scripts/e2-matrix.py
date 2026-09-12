#!/usr/bin/env python3
"""Expand an E2 matrix file into one run directory per row.

Usage:
    e2-matrix.py seconds <matrix.toml>
    e2-matrix.py expand  <matrix.toml> <out-dir> [--batch NAME]

`seconds` prints the matrix's simulated run length (used by scripts/e2-run.sh).
`expand` creates `<out-dir>/<row>/config.toml` and `<out-dir>/<row>/manifest.json`
for every row of the full factorial, writes `<out-dir>/batch.json`, and prints one
row name per line on stdout.

Matrix file format (TOML)
-------------------------

    seconds = 21600                    # simulated seconds per run (required)
    seeds   = [1, 2, 3, 4]             # config.seed values (default [1])

    [base]                             # dotted keys merged into every row config
    capacity.telemetry_seconds = 5.0

    [axes]                             # dotted key -> list of values, full factorial
    producer.growth      = [0.004, 0.008, 0.016]
    nutrient.diffusion   = [0.02, 0.05, 0.2]
    organism.speed_max   = [0.75, 1.5]
    weather.moving       = [false, true]

    [abbrev]                           # optional: dotted key -> row-name prefix
    producer.growth = "g"

`[base]`, `[axes]` and `[abbrev]` are flattened, so `producer.growth = ...` and
`"producer.growth" = ...` mean the same thing. Axis order in the file is the order
of the fields in a row name.

Row names concatenate one field per axis in declaration order plus the seed, e.g.
`g0.008_d0.05_v1.5_wtrue_s1`. The prefix letter comes from `[abbrev]`, else from
the built-in table below, else from the initials of the last dotted segment.

Row configs are written as flat TOML dotted keys (`producer.growth = 0.008`), which
is what `cubarium run --config` expects; missing fields take the built-in
`WorldConfig` defaults. Values are emitted with the same TOML type they were read
with, so write `5.0` (not `5`) for a field the world parses as a float.

Standard library only (`tomllib` to read; TOML is written by hand).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from itertools import product
from pathlib import Path

# Short row-name prefixes for the axes named in design/experiments-e2-harness.md.
KNOWN_ABBREV = {
    "producer.growth": "g",
    "nutrient.diffusion": "d",
    "organism.speed_max": "v",
    "weather.moving": "w",
    "habitat.noise_wavelength": "p",
}

# Reserved for the seed field appended to every row name.
SEED_ABBREV = "s"

SAFE = re.compile(r"[^A-Za-z0-9._+-]")


def flatten(table, prefix=""):
    """Flatten nested tables into dotted keys; lists and scalars are leaves."""
    out = {}
    for key, value in table.items():
        dotted = f"{prefix}{key}"
        if isinstance(value, dict):
            out.update(flatten(value, dotted + "."))
        else:
            out[dotted] = value
    return out


def toml_value(value) -> str:
    """Render a scalar or list as TOML, preserving int/float/bool types."""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        return repr(value)
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, list):
        return "[" + ", ".join(toml_value(v) for v in value) + "]"
    raise SystemExit(f"e2-matrix: cannot write {value!r} as TOML")


def name_value(value) -> str:
    """The row-name spelling of an axis value: 0.008, 1.5, true, 3."""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, float):
        return repr(value)
    if isinstance(value, list):
        return "-".join(name_value(v) for v in value)
    return str(value)


def abbrev_for(key: str, explicit: dict, used: set) -> str:
    """A short, unique prefix for one axis key."""
    if key in explicit:
        candidate = str(explicit[key])
    elif key in KNOWN_ABBREV:
        candidate = KNOWN_ABBREV[key]
    else:
        # Initials of the last dotted segment's words: organism.sense_radius -> sr.
        last = key.rsplit(".", 1)[-1]
        candidate = "".join(word[0] for word in last.split("_") if word) or "x"
    base = candidate
    n = 2
    while candidate in used or candidate == SEED_ABBREV:
        candidate = f"{base}{n}"
        n += 1
    used.add(candidate)
    return candidate


def load_matrix(path: Path):
    with path.open("rb") as fh:
        raw = tomllib.load(fh)

    unknown = set(raw) - {"seconds", "seeds", "base", "axes", "abbrev"}
    if unknown:
        raise SystemExit(f"e2-matrix: unknown top-level key(s) in {path}: {sorted(unknown)}")
    if "seconds" not in raw:
        raise SystemExit(f"e2-matrix: {path} has no `seconds`")

    seconds = raw["seconds"]
    if not isinstance(seconds, (int, float)) or isinstance(seconds, bool) or seconds <= 0:
        raise SystemExit(f"e2-matrix: `seconds` must be a positive number, got {seconds!r}")

    seeds = raw.get("seeds", [1])
    if not isinstance(seeds, list) or not seeds:
        raise SystemExit("e2-matrix: `seeds` must be a non-empty list")

    base = flatten(raw.get("base", {}))
    axes = flatten(raw.get("axes", {}))
    explicit = flatten(raw.get("abbrev", {}))

    for key, values in axes.items():
        if not isinstance(values, list) or not values:
            raise SystemExit(f"e2-matrix: axis `{key}` must be a non-empty list")

    return seconds, seeds, base, axes, explicit


def expand(matrix_path: Path, out_dir: Path, batch: str):
    seconds, seeds, base, axes, explicit = load_matrix(matrix_path)

    used: set = set()
    prefixes = {key: abbrev_for(key, explicit, used) for key in axes}

    axis_keys = list(axes)
    rows = []
    for combo in product(*(axes[key] for key in axis_keys)):
        values = dict(zip(axis_keys, combo))
        for seed in seeds:
            fields = [f"{prefixes[k]}{name_value(values[k])}" for k in axis_keys]
            fields.append(f"{SEED_ABBREV}{name_value(seed)}")
            name = SAFE.sub("-", "_".join(fields))
            rows.append((name, values, seed))

    names = [r[0] for r in rows]
    if len(set(names)) != len(names):
        raise SystemExit("e2-matrix: row names are not unique; give the axes distinct [abbrev] prefixes")

    out_dir.mkdir(parents=True, exist_ok=True)
    for name, values, seed in rows:
        row_dir = out_dir / name
        row_dir.mkdir(parents=True, exist_ok=True)

        merged = dict(base)
        merged.update(values)
        merged["seed"] = seed

        lines = [
            f"# generated by scripts/e2-matrix.py from {matrix_path}",
            f"# batch {batch}, row {name}",
            "",
        ]
        # `seed` first, then base keys, then the axis keys this row varies.
        ordered = ["seed"]
        ordered += [k for k in sorted(base) if k not in values and k != "seed"]
        ordered += [k for k in axis_keys if k != "seed"]
        for key in ordered:
            lines.append(f"{key} = {toml_value(merged[key])}")
        (row_dir / "config.toml").write_text("\n".join(lines) + "\n")

        manifest = {
            "batch": batch,
            "row": name,
            "axes": values,
            "seed": seed,
            "seconds": seconds,
            "config": "config.toml",
            "telemetry": "telemetry.jsonl",
            "build_id": None,
            "wall_seconds": None,
            "exit_code": None,
        }
        (row_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

    batch_meta = {
        "batch": batch,
        "matrix": str(matrix_path),
        "seconds": seconds,
        "seeds": seeds,
        "axes": {k: axes[k] for k in axis_keys},
        "abbrev": prefixes,
        "base": base,
        "rows": names,
    }
    (out_dir / "batch.json").write_text(json.dumps(batch_meta, indent=2) + "\n")

    for name in names:
        print(name)


def main(argv=None):
    parser = argparse.ArgumentParser(description="Expand an E2 matrix into run directories.")
    sub = parser.add_subparsers(dest="command", required=True)

    p_seconds = sub.add_parser("seconds", help="print the matrix's simulated seconds")
    p_seconds.add_argument("matrix", type=Path)

    p_expand = sub.add_parser("expand", help="write row configs and print row names")
    p_expand.add_argument("matrix", type=Path)
    p_expand.add_argument("out_dir", type=Path)
    p_expand.add_argument("--batch", default=None)

    args = parser.parse_args(argv)

    if args.command == "seconds":
        seconds, *_ = load_matrix(args.matrix)
        print(repr(float(seconds)) if isinstance(seconds, float) else seconds)
        return 0

    batch = args.batch or args.out_dir.name
    expand(args.matrix, args.out_dir, batch)
    return 0


if __name__ == "__main__":
    sys.exit(main())
