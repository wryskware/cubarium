#!/usr/bin/env python3
"""DEN-REST study runner and final table writer; one slot, four complete seeds.

Called by scripts/den-rest-batch.sh under one long timeout. Zero births/rests are
table values, never an abort condition. No checkpoints or retained binary copies.
"""

import argparse
import csv
import json
from pathlib import Path
import subprocess


def summarize(out):
    rows = []
    for seed in (41, 42, 43, 44):
        folder = out / f"seed-{seed}"
        summary_file = folder / "summary.json"
        summary = json.loads(summary_file.read_text()) if summary_file.exists() else {}
        row = {
            "seed": seed,
            "status": "complete" if summary.get("minutes") == 60 else "unfinished",
            "exit": (folder / "exit").read_text().strip() if (folder / "exit").exists() else "",
            "terrain_seed": summary.get("terrain_seed", ""),
            "accepted": summary.get("accepted", ""),
            "sim_minutes": summary.get("minutes", 0),
            "founding_loftstriders": summary.get("founding_loftstriders", ""),
            "loftstrider_births": summary.get("loftstrider_births", 0),
            "founder_calves": summary.get("founder_calves", 0),
            "observed_founder_calves_per_pair_world_day": summary.get("founder_calves_per_pair_per_world_day_observed", ""),
            "veilhounds_logged": summary.get("veilhounds_logged", 0),
            "rest_ticks": summary.get("rest_ticks", 0),
            "at_den_ticks": summary.get("at_den_ticks", 0),
            "roofed_ticks": summary.get("roofed_ticks", 0),
            "wall_seconds": summary.get("wall_seconds", ""),
        }
        clocks_file = folder / "birth-clocks.json"
        clocks = json.loads(clocks_file.read_text()) if clocks_file.exists() else {}
        for arm in ("before", "after"):
            row[f"{arm}_analytic_fed_pair_calves_per_world_day"] = (clocks.get(arm) or {}).get("fed_pair_calves_per_world_day", "")
        rows.append(row)
    with (out / "table.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    with (out / "veilhound-table.csv").open("w", newline="") as handle:
        columns = ("seed", "animal", "rest_ticks", "at_den_ticks", "roofed_ticks", "max_distance_m")
        writer = csv.DictWriter(handle, fieldnames=columns)
        writer.writeheader()
        for seed in (41, 42, 43, 44):
            source = out / f"seed-{seed}" / "rest-summary.csv"
            if source.exists():
                with source.open() as file:
                    for row in csv.DictReader(file):
                        writer.writerow({"seed": seed, **row})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("run", "summarize"))
    parser.add_argument("output", type=Path)
    parser.add_argument("--slot", type=int, default=1)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    if args.mode == "run":
        for seed in (41, 42, 43, 44):
            folder = out / f"seed-{seed}"
            folder.mkdir(parents=True, exist_ok=True)
            command = ["chrt", "-i", "0", "nice", "-n", "19", "scripts/run-study.py", "--slot", str(args.slot), "--",
                       "target/release/examples/den_rest_study", "config/desktop/terrarium.toml", str(seed), "60", str(folder),
                       "threads=1", "water_every=4", f"baseline={out / 'before.toml'}"]
            with (folder / "stdout.log").open("w") as stdout, (folder / "stderr.log").open("w") as stderr:
                result = subprocess.run(command, stdout=stdout, stderr=stderr, check=False)
            (folder / "exit").write_text(f"{result.returncode}\n")
    summarize(out)


if __name__ == "__main__":
    main()
