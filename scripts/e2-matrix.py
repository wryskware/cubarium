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

    [pairs.contrast]                   # keys that move together as ONE axis
    keys   = ["habitat.light_noise_gain", "habitat.moisture_noise_gain"]
    values = [[0.1, 0.2], [0.3, 0.4]]  # one inner list per level, in `keys` order

    [abbrev]                           # optional: dotted key (or pair name) -> prefix
    producer.growth = "g"

`[base]`, `[axes]` and `[abbrev]` are flattened, so `producer.growth = ...` and
`"producer.growth" = ...` mean the same thing. `[pairs]` is not flattened: each
`[pairs.<name>]` sub-table is one factorial axis whose levels are the value tuples,
and a level sets every one of its `keys` at once. Axis and pair order in the file is
the order of the fields in a row name.

Row names concatenate one field per axis in declaration order, then one per pair in
declaration order, then the seed, e.g. `g0.008_d0.05_v1.5_wtrue_s1` or
`k0.0_a0.15_r4.0_v0.75_c0_s1`. An axis field is its prefix followed by the value; a
pair field is its prefix followed by the *level index*, since a tuple of values does
not spell a readable token. The prefix comes from `[abbrev]` (keyed by the dotted
key, or by the pair name), else from the built-in table below, else from the initials
of the last dotted segment — or, for a pair, its first letter. Prefixes are checked
for uniqueness across axes and pairs together.

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
    "nutrient.initial": "n",
    "organism.speed_max": "v",
    "weather.moving": "w",
    "habitat.noise_wavelength": "p",
}

# Reserved for the seed field appended to every row name.
SEED_ABBREV = "s"

SAFE = re.compile(r"[^A-Za-z0-9._+-]")

# Recovering the order axes were WRITTEN in. tomllib nests `producer.growth = ...`
# under a `producer` table, so flattening a mixed `[axes]` block returns every
# `organism.*` key together regardless of where those lines actually appear. Row
# names must follow the file, so the declared order is read back from the text.
_TABLE_RE = re.compile(r"^\s*\[\s*([^\]]+?)\s*\]\s*(?:#.*)?$")
_KEY_RE = re.compile(
    r"^\s*((?:\"[^\"]*\"|'[^']*'|[A-Za-z0-9_-]+)"
    r"(?:\s*\.\s*(?:\"[^\"]*\"|'[^']*'|[A-Za-z0-9_-]+))*)\s*="
)
_SEGMENT_RE = re.compile(r"\"[^\"]*\"|'[^']*'|[A-Za-z0-9_-]+")


def declared_keys(path: Path, table: str):
    """The dotted keys of one top-level table, in the order the file writes them."""
    order = []
    inside = False
    for line in path.read_text().splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        header = _TABLE_RE.match(line)
        if header:
            inside = header.group(1).strip() == table
            continue
        if not inside:
            continue
        key = _KEY_RE.match(line)
        if key:
            segments = [
                seg.strip("\"'") for seg in _SEGMENT_RE.findall(key.group(1))
            ]
            order.append(".".join(segments))
    return order


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


def abbrev_for(key: str, explicit: dict, used: set, is_pair: bool = False) -> str:
    """A short, unique prefix for one axis key or pair name."""
    if key in explicit:
        candidate = str(explicit[key])
    elif key in KNOWN_ABBREV:
        candidate = KNOWN_ABBREV[key]
    elif is_pair:
        # A pair is named, not dotted: its first letter is the natural token.
        candidate = key[:1] or "x"
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

    unknown = set(raw) - {"seconds", "seeds", "base", "axes", "abbrev", "pairs"}
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

    # Restore the written order; fall back to the flattened order if the text scan
    # and the parsed table disagree, so a surprising file degrades rather than
    # silently renaming rows.
    written = declared_keys(path, "axes")
    if len(written) == len(axes) and set(written) == set(axes):
        axes = {key: axes[key] for key in written}

    for key, values in axes.items():
        if not isinstance(values, list) or not values:
            raise SystemExit(f"e2-matrix: axis `{key}` must be a non-empty list")

    # `[pairs]` is deliberately NOT flattened: each sub-table is one axis.
    pairs = raw.get("pairs", {})
    if not isinstance(pairs, dict):
        raise SystemExit("e2-matrix: `[pairs]` must be a table of named pairs")
    for name, spec in pairs.items():
        if not isinstance(spec, dict) or set(spec) != {"keys", "values"}:
            raise SystemExit(
                f"e2-matrix: `[pairs.{name}]` needs exactly `keys` and `values`"
            )
        keys, levels = spec["keys"], spec["values"]
        if not isinstance(keys, list) or not keys or not all(isinstance(k, str) for k in keys):
            raise SystemExit(f"e2-matrix: `pairs.{name}.keys` must be a non-empty list of strings")
        if not isinstance(levels, list) or not levels:
            raise SystemExit(f"e2-matrix: `pairs.{name}.values` must be a non-empty list of levels")
        for i, level in enumerate(levels):
            if not isinstance(level, list) or len(level) != len(keys):
                raise SystemExit(
                    f"e2-matrix: `pairs.{name}.values[{i}]` must list one value per key "
                    f"({len(keys)} expected, got {level!r})"
                )
        overlap = set(keys) & set(axes)
        if overlap:
            raise SystemExit(
                f"e2-matrix: `pairs.{name}` and `[axes]` both set {sorted(overlap)}"
            )

    return seconds, seeds, base, axes, explicit, pairs


def expand(matrix_path: Path, out_dir: Path, batch: str):
    seconds, seeds, base, axes, explicit, pairs = load_matrix(matrix_path)

    axis_keys = list(axes)
    pair_names = list(pairs)

    # Axes first, then pairs, so the uniqueness suffixes stay stable as pairs are added.
    used: set = set()
    prefixes = {key: abbrev_for(key, explicit, used) for key in axis_keys}
    prefixes.update(
        {name: abbrev_for(name, explicit, used, is_pair=True) for name in pair_names}
    )

    rows = []
    axis_levels = [axes[key] for key in axis_keys]
    # A pair contributes its level *indices*; the values are looked up per level.
    pair_levels = [range(len(pairs[name]["values"])) for name in pair_names]
    for combo in product(*axis_levels, *pair_levels):
        values = dict(zip(axis_keys, combo[: len(axis_keys)]))
        indices = dict(zip(pair_names, combo[len(axis_keys) :]))
        for seed in seeds:
            fields = [f"{prefixes[k]}{name_value(values[k])}" for k in axis_keys]
            fields += [f"{prefixes[n]}{indices[n]}" for n in pair_names]
            fields.append(f"{SEED_ABBREV}{name_value(seed)}")
            name = SAFE.sub("-", "_".join(fields))
            rows.append((name, values, indices, seed))

    names = [r[0] for r in rows]
    if len(set(names)) != len(names):
        raise SystemExit("e2-matrix: row names are not unique; give the axes distinct [abbrev] prefixes")

    out_dir.mkdir(parents=True, exist_ok=True)
    for name, values, indices, seed in rows:
        row_dir = out_dir / name
        row_dir.mkdir(parents=True, exist_ok=True)

        # One level of a pair sets every key of that pair.
        pair_values = {}
        pair_levels_used = {}
        for pair_name in pair_names:
            level = pairs[pair_name]["values"][indices[pair_name]]
            pair_values.update(dict(zip(pairs[pair_name]["keys"], level)))
            pair_levels_used[pair_name] = {
                "index": indices[pair_name],
                "keys": list(pairs[pair_name]["keys"]),
                "values": list(level),
            }

        merged = dict(base)
        merged.update(values)
        merged.update(pair_values)
        merged["seed"] = seed

        lines = [
            f"# generated by scripts/e2-matrix.py from {matrix_path}",
            f"# batch {batch}, row {name}",
            "",
        ]
        # `seed` first, then base keys, then the axis keys, then the paired keys.
        varied = set(values) | set(pair_values)
        ordered = ["seed"]
        ordered += [k for k in sorted(base) if k not in varied and k != "seed"]
        ordered += [k for k in axis_keys if k != "seed"]
        ordered += [k for n in pair_names for k in pairs[n]["keys"] if k != "seed"]
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
        if pair_levels_used:
            manifest["pairs"] = pair_levels_used
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
    if pair_names:
        batch_meta["pairs"] = {n: pairs[n] for n in pair_names}
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
