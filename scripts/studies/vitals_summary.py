#!/usr/bin/env python3
"""Reduce census vitals across runs, reporting median and min–max over seeds.

Bounds are inclusive sim minutes. Endpoint stocks use the last observation in
the window. Minute means (including each minute's p10) are population weighted;
activity and energy use their sampled body-time denominators when available.
Intervals ending exactly at --from are excluded from activity and energy means,
as those readings cover the preceding minute. Stuck share uses only eligible
bodies with a complete history window; its coverage share reports observed/n.
Deaths and lifetime outcomes select bodies dying within the window. Native
maturity selects the birth cohort in the window and ignores maturation after its
end; this is an observed fraction, with living juveniles still censored.
The birth cohort excludes bodies first seen as Observed after a load, even when
their age reconstructs a birth time. Known ages may still enter lifespan means.
"""

import argparse
import csv
import math
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def number(row, key, default=None):
    value = row.get(key, "")
    if value is None or value.strip() == "":
        return default
    result = float(value)
    if not math.isfinite(result):
        return default
    return result


def flag(row, key):
    return row.get(key, "").strip().lower() in {"true", "1", "yes"}


def read_csv(path, required):
    with path.open(newline="") as stream:
        reader = csv.DictReader(stream)
        missing = set(required) - set(reader.fieldnames or [])
        if missing:
            raise ValueError(f"{path}: missing columns: {', '.join(sorted(missing))}")
        return list(reader)


def weighted(rows, key, weight="n"):
    total = denominator = 0.0
    for row in rows:
        value = number(row, key)
        mass = number(row, weight, 0.0)
        if value is not None and mass > 0:
            total += value * mass
            denominator += mass
    return total / denominator if denominator else None


def count_share(rows, key, weight="n"):
    numerator = denominator = 0.0
    for row in rows:
        value = number(row, key)
        if value is not None:
            numerator += value
            denominator += number(row, weight, 0.0)
    return numerator / denominator if denominator else None


def average(values):
    return statistics.mean(values) if values else None


def in_window(value, start, end):
    return value is not None and start <= value <= end


def summarize_run(directory, start, stop):
    vitals = read_csv(directory / "vitals.csv", ["minute", "lineage", "n"])
    plants = read_csv(directory / "plants_vitals.csv", ["minute", "species", "stands"])
    lives = read_csv(directory / "life.csv", ["lineage", "native", "origin", "born_minute", "died_minute", "alive", "cause"])
    minutes = [number(row, "minute") for row in vitals + plants]
    minutes = [minute for minute in minutes if minute is not None]
    if not minutes:
        raise ValueError(f"{directory}: no vitals observations")
    end = min(max(minutes), stop)
    if start > end:
        raise ValueError(f"{directory}: no observations in requested window")
    animal_rows = defaultdict(list)
    plant_rows = defaultdict(list)
    life_rows = defaultdict(list)
    for row in vitals:
        if in_window(number(row, "minute"), start, end):
            animal_rows[row["lineage"]].append(row)
    for row in plants:
        if in_window(number(row, "minute"), start, end):
            plant_rows[row["species"]].append(row)
    for row in lives:
        life_rows[row["lineage"]].append(row)

    results = {}
    # Departure::ALL's names, including predator-specific killed_by names. The
    # census exports every founder lineage even when extinct, so zero-death
    # runs still expose all the current causes. Preserve new observed causes.
    causes = {"starved", "drowned", "aged", "removed", "desiccated", "fell"}
    causes.update(f"killed_by_{lineage}" for lineage in animal_rows)
    causes.update(row["cause"] for row in lives if row["cause"])
    for lineage, rows in sorted(animal_rows.items()):
        latest = max(rows, key=lambda row: number(row, "minute"))
        records = life_rows[lineage]
        deaths = [row for row in records if in_window(number(row, "died_minute"), start, end)]
        births = [row for row in records if flag(row, "native")
                  and row["origin"].lower() == "born"
                  and in_window(number(row, "born_minute"), start, end)]
        mature = [row for row in births if in_window(number(row, "matured_minute"), 0, end)]
        adults = [row for row in deaths if flag(row, "adult")
                  or (number(row, "matured_minute") is not None
                      and number(row, "matured_minute") <= number(row, "died_minute"))]
        present = [row for row in records
                   if number(row, "observed_minute", number(row, "born_minute", 0.0)) <= end
                   and (number(row, "born_minute") is None or number(row, "born_minute") <= end)
                   and (number(row, "died_minute") is None or number(row, "died_minute") >= start)]
        lifespan = [number(row, "died_minute") - number(row, "born_minute")
                    for row in deaths if number(row, "born_minute") is not None]
        interval_rows = [row for row in rows if number(row, "minute") > start]
        activity_weight = "awake_body_s" if "awake_body_s" in rows[0] else "n"
        energy_weight = "energy_body_hours" if "energy_body_hours" in rows[0] else "n"
        metrics = {
            "alive_at_end": number(latest, "n"),
            "births": len(births),
            "fullness_mean": weighted(rows, "fullness_mean"),
            "fullness_p10": weighted(rows, "fullness_p10"),
            "emergency_share": count_share(rows, "emergency_n"),
            "moving_share": weighted(interval_rows, "moving_share", activity_weight),
            "net_energy_per_body_hour": weighted(interval_rows, "net_energy_per_body_hour", energy_weight),
            "stuck_share": count_share(rows, "stuck_n", "stuck_observed_n"),
            "stuck_coverage_share": count_share(rows, "stuck_observed_n"),
            "generations_reached": max((number(row, "generation", 0.0) for row in present), default=0.0),
            "dead_lifespan_mean_min": average(lifespan),
            "offspring_per_dead_adult": average([number(row, "offspring") for row in adults if number(row, "offspring") is not None]),
            "native_maturity_share": len(mature) / len(births) if births else None,
        }
        metrics.update({f"deaths:{cause}": sum(row["cause"] == cause for row in deaths) for cause in sorted(causes)})
        results[("animal", lineage)] = metrics
    for species, rows in sorted(plant_rows.items()):
        latest = max(rows, key=lambda row: number(row, "minute"))
        results[("plant", species)] = {
            "stands_at_end": number(latest, "stands"),
            "vigour_mean_at_end": number(latest, "vigour_mean"),
            "vigour_p10_at_end": number(latest, "vigour_p10"),
            "vigour_p50_at_end": number(latest, "vigour_p50"),
        }
    if not results:
        raise ValueError(f"{directory}: no observations in requested window")
    return results


def aggregate(runs):
    keys = sorted({key for run in runs for key in run})
    all_causes = {metric for run in runs for key, metrics in run.items()
                  if key[0] == "animal" for metric in metrics if metric.startswith("deaths:")}
    rows = []
    for key in keys:
        metrics = {metric for run in runs for metric in run.get(key, {})}
        if key[0] == "animal":
            metrics.update(all_causes)
        for metric in sorted(metrics):
            values = []
            for run in runs:
                if key not in run:
                    continue
                value = run[key].get(metric, 0 if metric.startswith("deaths:") else None)
                if value is not None:
                    values.append(value)
            rows.append((*key, metric, statistics.median(values) if values else None,
                         min(values) if values else None, max(values) if values else None, len(values)))
    return rows


def display(value):
    return "NA" if value is None else f"{value:.6g}" if isinstance(value, (int, float)) else str(value)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runs", metavar="RUN_DIR", nargs="+", type=Path)
    parser.add_argument("--from", dest="start", type=float, default=0.0, metavar="MIN")
    parser.add_argument("--to", dest="stop", type=float, default=math.inf, metavar="MIN")
    parser.add_argument("--tsv", action="store_true", help="print machine-readable tab-separated rows")
    args = parser.parse_args(argv)
    if not math.isfinite(args.start) or args.start < 0 or math.isnan(args.stop) or args.stop < args.start:
        parser.error("require finite 0 <= --from <= --to")
    try:
        rows = aggregate([summarize_run(run, args.start, args.stop) for run in args.runs])
    except (OSError, ValueError) as exc:
        parser.error(str(exc))
    header = ("kind", "name", "metric", "median", "min", "max", "seeds")
    if args.tsv:
        writer = csv.writer(sys.stdout, delimiter="\t", lineterminator="\n")
        writer.writerow(header)
        writer.writerows([[display(value) for value in row] for row in rows])
    else:
        formatted = [list(header)] + [[display(value) for value in row] for row in rows]
        widths = [max(len(row[index]) for row in formatted) for index in range(len(header))]
        for row in formatted:
            print("  ".join(value.ljust(width) for value, width in zip(row, widths)).rstrip())


if __name__ == "__main__":
    main()
