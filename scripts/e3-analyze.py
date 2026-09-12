#!/usr/bin/env python3
"""Analyze an E3 batch: scripts/e3-analyze.py runs/<batch>

Reads every run's life-event log and writes `runs/<batch>/summary-e3.csv` and
`summary-e3.md`, one line per row. Standard library only.

These are the reproductive-opportunity measures E3 asks for
(design/experiments.md "E3 — actual evolutionary opportunity"): parent age at
birth, time to first reproduction, births per organism and reproductive skew,
ancestry depth over simulated time, founder-lineage survival, and the lifespan
distribution by cause. Intervals and distributions are reported; nothing is
combined into a score, and nothing here estimates an effective population size.

Input
-----

`runs/<batch>/<row>/state/events.jsonl`, falling back to `<row>/events.jsonl`
(design/m2-world-spec.md "Observer" -> "Life events"; the host writes it when
`capacity.event_log` is true). One JSON object per line:

    {"kind":"birth","tick","id","parent","parent_age_ticks","parent_births",
     "genome","origin"}
    {"kind":"death","tick","id","age_ticks","cause","births","genome"}

`id` and `parent` are `slot:generation` strings. Slots are reused, so the full
string is the identity and the bare slot number is not. A trailing partial line
(the batch is still running) is skipped with a warning rather than failing.

Time
----

TICKS_PER_SECOND = 20 (the host's fixed 20 Hz tick, crates/cubarium/README.md
"Clock"). Durations are reported in simulated minutes unless a column says
otherwise. `run_end_tick` is the row's last telemetry tick when telemetry.jsonl
is present, else the manifest's `seconds` x 20, else the last event tick; it is
what "still alive at the end" is measured against.

Founders
--------

Founders are placed at world creation and emit no birth event, so they are
exactly the ids that never appear as a birth `id`. (The narrower reading --
"any parent id that never appears as a child" -- is the same set restricted to
founders that reproduced; using the wider set gives identical depths, because a
founder that never reproduced has no children, and a truer lineage count.) A
founder that neither reproduced nor died inside the run appears nowhere in the
log at all, so `founder_lineages_seen` is a lower bound on the founders that
existed, and is normally below `founders.count`.

Metrics
-------

parent_age_{p10,median,p90}_min
    The `parent_age_ticks` of every birth event, in simulated minutes. This is
    the age of the parent when that child was born, so a parent that reproduces
    repeatedly contributes once per child -- it is a distribution over births,
    not over parents.

ttfr_{n,p10,median,p90}_min, ttfr_founders_excluded, ttfr_check_mismatch
    Time to first reproduction, per organism that reproduced: the tick of its
    first child's birth minus the tick of its own birth. Founders have no birth
    event and are excluded; `ttfr_founders_excluded` counts the reproducing
    organisms dropped for that reason. `ttfr_check_mismatch` is an integrity
    check, not a measurement: for a non-founder parent the first child's own
    `parent_age_ticks` (the record where `parent_births == 1`) must equal the
    same interval, and this counts the parents where it does not. It should be 0.

births_per_dead_mean, births_per_dead_n, gini_births
    The `births` field of every death event -- the completed reproductive output
    of organisms whose life is over, zeros included. Organisms still alive at the
    end of the run are excluded, because their count is not final. gini_births is
    the Gini coefficient of that distribution (0 = every dead organism produced
    the same number of offspring, 1 = one organism produced all of them),
    computed as G = 2*sum(i * x_i)/(n * sum(x)) - (n+1)/n over the ascending sort
    with i = 1..n. An all-zero distribution is perfectly equal, so G = 0.

depth_max, depth_mean_overall, depth_mean_by_window, depth_max_by_window
    Ancestry depth is reconstructed from the birth events' parent links: a
    founder has depth 0 and a child has its parent's depth plus one. The two
    `by_window` columns are semicolon-separated, one entry per DEPTH_WINDOW_HOURS
    = 2 simulated hours from tick 0, giving the mean and the max depth of the
    organisms *born* in that window (empty for a window with no births). Depth is
    a property of an organism's ancestry, so a window with few births can move
    the mean sharply; summary-e3.md prints the per-window counts beside it.

founder_lineages_seen, founder_lineages_alive_end, lineage_last_alive_{median,max}_h
    Every organism is assigned to a lineage by walking parent links up to its
    founder; the lineage is labelled by that founder's slot. An organism is alive
    from its birth until its death event, or until `run_end_tick` if it has none.
    A lineage's last-alive tick is the maximum of those over its members, and a
    lineage is alive at the end when that equals `run_end_tick`. The median and
    max are over lineages, in simulated hours.

lifespan_<cause>_{n,p10,median,p90}_min
    The `age_ticks` of death events, split by `cause`, in simulated minutes. Only
    completed lives appear, so a run that ends with many long-lived organisms
    still alive under-reports long lifespans.

Percentiles use linear interpolation between order statistics (the usual
"inclusive" definition: rank = p * (n - 1) over the ascending sort), so p10 and
p90 of a 2-element sample are interpolations, not order statistics.

summary-e3.md adds two 32-character sparklines per row over
SPARK_WINDOW_MINUTES = 10 simulated minutes: births per window, and the mean
ancestry depth of the organisms born in each window (a window with no births
contributes 0 to the depth sparkline, so a gap reads as a dip).
"""

from __future__ import annotations

import csv
import json
import sys
from pathlib import Path

TICKS_PER_SECOND = 20
TICKS_PER_MINUTE = TICKS_PER_SECOND * 60
TICKS_PER_HOUR = TICKS_PER_MINUTE * 60
DEPTH_WINDOW_HOURS = 2
SPARK_WINDOW_MINUTES = 10
SPARK_WIDTH = 32
BLOCKS = "▁▂▃▄▅▆▇█"

# Death causes in the order crates/cubarium-core/src/organism.rs declares them;
# anything else the log carries is appended in sorted order.
KNOWN_CAUSES = ["starvation", "age", "collapse"]


# --------------------------------------------------------------------------- io


def find_events(row_dir: Path):
    for candidate in (row_dir / "state" / "events.jsonl", row_dir / "events.jsonl"):
        if candidate.is_file():
            return candidate
    return None


def read_events(path: Path):
    """Birth and death records, oldest first. A truncated final line is dropped."""
    births, deaths = [], []
    with path.open() as fh:
        lines = fh.readlines()
    for number, line in enumerate(lines, 1):
        line = line.strip()
        if not line:
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            if number == len(lines):
                print(
                    f"e3-analyze: {path}: ignoring a partial final line "
                    f"(the run is probably still writing)",
                    file=sys.stderr,
                )
                continue
            raise
        if record.get("kind") == "birth":
            births.append(record)
        elif record.get("kind") == "death":
            deaths.append(record)
    births.sort(key=lambda r: r["tick"])
    deaths.sort(key=lambda r: r["tick"])
    return births, deaths


def last_telemetry_tick(row_dir: Path):
    path = row_dir / "telemetry.jsonl"
    if not path.is_file():
        return None
    last = None
    with path.open() as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                last = json.loads(line)["tick"]
            except (json.JSONDecodeError, KeyError):
                continue
    return last


# ------------------------------------------------------------------ statistics


def mean(xs):
    return sum(xs) / len(xs) if xs else None


def percentile(xs, p):
    """Linear interpolation between order statistics; p in [0, 1]."""
    if not xs:
        return None
    ordered = sorted(xs)
    if len(ordered) == 1:
        return float(ordered[0])
    rank = p * (len(ordered) - 1)
    low = int(rank)
    high = min(low + 1, len(ordered) - 1)
    frac = rank - low
    return ordered[low] * (1.0 - frac) + ordered[high] * frac


def median(xs):
    return percentile(xs, 0.5)


def gini(xs):
    """Gini coefficient over a non-negative sample, zeros included."""
    if not xs:
        return None
    ordered = sorted(xs)
    n = len(ordered)
    total = sum(ordered)
    if total <= 0:
        # Everyone produced nothing: perfectly equal.
        return 0.0
    weighted = sum((i + 1) * x for i, x in enumerate(ordered))
    return 2.0 * weighted / (n * total) - (n + 1) / n


def sparkline(values, width=SPARK_WIDTH):
    if not values:
        return ""
    n = len(values)
    buckets = []
    for i in range(width):
        lo = i * n // width
        hi = max(lo + 1, (i + 1) * n // width)
        hi = min(hi, n)
        if lo >= n:
            break
        buckets.append(sum(values[lo:hi]) / (hi - lo))
    lo, hi = min(buckets), max(buckets)
    if hi - lo <= 0:
        return BLOCKS[0] * len(buckets)
    return "".join(BLOCKS[min(7, int((b - lo) / (hi - lo) * 8))] for b in buckets)


def ticks_to_minutes(ticks):
    return None if ticks is None else ticks / TICKS_PER_MINUTE


# --------------------------------------------------------------------- lineages


def depths_of(parent_of):
    """Depth per organism: founders 0, a child one deeper than its parent."""
    depth = {}

    def resolve(node):
        chain = []
        while node not in depth:
            parent = parent_of.get(node)
            if parent is None:  # never born: a founder, and the chain's root
                depth[node] = 0
                break
            chain.append(node)
            node = parent
        base = depth[node]
        for node in reversed(chain):
            base += 1
            depth[node] = base

    for child in parent_of:
        resolve(child)
    return depth


def roots_of(parent_of, everyone):
    """The founder each organism descends from, by walking parent links up."""
    root = {}
    for start in everyone:
        chain = []
        node = start
        while node not in root:
            parent = parent_of.get(node)
            if parent is None:
                root[node] = node
                break
            chain.append(node)
            node = parent
        found = root[node]
        for node in chain:
            root[node] = found
    return root


# --------------------------------------------------------------------- analysis


def analyze_row(row_dir: Path):
    events_path = find_events(row_dir)
    if events_path is None:
        return None
    births, deaths = read_events(events_path)
    if not births and not deaths:
        return None

    manifest_path = row_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.is_file() else {}

    born_tick = {b["id"]: b["tick"] for b in births}
    parent_of = {b["id"]: b["parent"] for b in births}
    death_tick = {d["id"]: d["tick"] for d in deaths}

    everyone = set(born_tick) | set(parent_of.values()) | set(death_tick)
    founders = {i for i in everyone if i not in born_tick}

    # The run's end: what "still alive" is measured against.
    last_event = max(
        [b["tick"] for b in births] + [d["tick"] for d in deaths], default=0
    )
    run_end = last_telemetry_tick(row_dir)
    if run_end is None and manifest.get("seconds"):
        run_end = int(manifest["seconds"]) * TICKS_PER_SECOND
    if run_end is None or run_end < last_event:
        run_end = last_event

    result = {
        "row": row_dir.name,
        "seed": manifest.get("seed"),
        "exit_code": manifest.get("exit_code"),
        "build_id": manifest.get("build_id"),
        "events": len(births) + len(deaths),
        "births_total": len(births),
        "deaths_total": len(deaths),
        "organisms_born": len(born_tick),
        "founders_seen": len(founders),
        "run_end_tick": run_end,
        "run_end_hours": run_end / TICKS_PER_HOUR,
    }

    # --- parent age at birth (one entry per birth) ---------------------------
    parent_ages = [b["parent_age_ticks"] for b in births if "parent_age_ticks" in b]
    result["parent_age_p10_min"] = ticks_to_minutes(percentile(parent_ages, 0.10))
    result["parent_age_median_min"] = ticks_to_minutes(median(parent_ages))
    result["parent_age_p90_min"] = ticks_to_minutes(percentile(parent_ages, 0.90))

    # --- time to first reproduction -----------------------------------------
    first_child_tick = {}
    first_child_parent_age = {}
    for b in births:
        parent = b["parent"]
        if parent not in first_child_tick or b["tick"] < first_child_tick[parent]:
            first_child_tick[parent] = b["tick"]
        if b.get("parent_births") == 1:
            first_child_parent_age[parent] = b["parent_age_ticks"]

    ttfr = []
    excluded = 0
    mismatch = 0
    for parent, tick in first_child_tick.items():
        if parent not in born_tick:
            excluded += 1  # a founder: no birth event to measure from
            continue
        interval = tick - born_tick[parent]
        ttfr.append(interval)
        reported = first_child_parent_age.get(parent)
        if reported is not None and reported != interval:
            mismatch += 1
    result["ttfr_n"] = len(ttfr)
    result["ttfr_p10_min"] = ticks_to_minutes(percentile(ttfr, 0.10))
    result["ttfr_median_min"] = ticks_to_minutes(median(ttfr))
    result["ttfr_p90_min"] = ticks_to_minutes(percentile(ttfr, 0.90))
    result["ttfr_founders_excluded"] = excluded
    result["ttfr_check_mismatch"] = mismatch

    # --- births per dead organism, and skew ---------------------------------
    per_dead = [d.get("births", 0) for d in deaths]
    result["births_per_dead_n"] = len(per_dead)
    result["births_per_dead_mean"] = mean(per_dead)
    result["gini_births"] = gini(per_dead)

    # --- ancestry depth ------------------------------------------------------
    depth = depths_of(parent_of)
    for founder in founders:
        depth.setdefault(founder, 0)
    born_depths = [depth[i] for i in born_tick]
    result["depth_max"] = max(born_depths) if born_depths else None
    result["depth_mean_overall"] = mean(born_depths)

    window_ticks = DEPTH_WINDOW_HOURS * TICKS_PER_HOUR
    n_windows = max(1, -(-run_end // window_ticks))  # ceil
    by_window = [[] for _ in range(n_windows)]
    for organism, tick in born_tick.items():
        index = min(tick // window_ticks, n_windows - 1)
        by_window[index].append(depth[organism])
    result["depth_windows"] = [
        {
            "start_hours": i * DEPTH_WINDOW_HOURS,
            "births": len(values),
            "mean": mean(values),
            "max": max(values) if values else None,
        }
        for i, values in enumerate(by_window)
    ]
    result["depth_mean_by_window"] = ";".join(
        "" if w["mean"] is None else f"{w['mean']:.4g}" for w in result["depth_windows"]
    )
    result["depth_max_by_window"] = ";".join(
        "" if w["max"] is None else str(w["max"]) for w in result["depth_windows"]
    )

    # --- founder-lineage survival -------------------------------------------
    root = roots_of(parent_of, everyone)
    last_alive = {}
    for organism in everyone:
        until = death_tick.get(organism, run_end)
        lineage = root[organism]
        if until > last_alive.get(lineage, -1):
            last_alive[lineage] = until
    result["founder_lineages_seen"] = len(last_alive)
    result["founder_lineages_alive_end"] = sum(
        1 for t in last_alive.values() if t >= run_end
    )
    ends = list(last_alive.values())
    result["lineage_last_alive_median_h"] = (
        median(ends) / TICKS_PER_HOUR if ends else None
    )
    result["lineage_last_alive_max_h"] = max(ends) / TICKS_PER_HOUR if ends else None
    result["lineages"] = sorted(
        (
            {
                "founder": lineage,
                "slot": int(str(lineage).split(":")[0]) if ":" in str(lineage) else None,
                "last_alive_hours": tick / TICKS_PER_HOUR,
                "alive_at_end": tick >= run_end,
            }
            for lineage, tick in last_alive.items()
        ),
        key=lambda entry: (-entry["last_alive_hours"], entry["founder"]),
    )

    # --- lifespan by cause ---------------------------------------------------
    by_cause = {}
    for d in deaths:
        by_cause.setdefault(d.get("cause", "unknown"), []).append(d["age_ticks"])
    result["lifespans"] = {
        cause: {
            "n": len(ages),
            "p10_min": ticks_to_minutes(percentile(ages, 0.10)),
            "median_min": ticks_to_minutes(median(ages)),
            "p90_min": ticks_to_minutes(percentile(ages, 0.90)),
        }
        for cause, ages in by_cause.items()
    }

    # --- sparkline series ----------------------------------------------------
    spark_ticks = SPARK_WINDOW_MINUTES * TICKS_PER_MINUTE
    n_spark = max(1, -(-run_end // spark_ticks))  # ceil
    births_series = [0] * n_spark
    depth_sums = [0] * n_spark
    for organism, tick in born_tick.items():
        index = min(tick // spark_ticks, n_spark - 1)
        births_series[index] += 1
        depth_sums[index] += depth[organism]
    depth_series = [
        (depth_sums[i] / births_series[i]) if births_series[i] else 0.0
        for i in range(n_spark)
    ]
    result["spark_births"] = sparkline(births_series)
    result["spark_depth"] = sparkline(depth_series)
    result["spark_births_max"] = max(births_series)
    result["spark_depth_max"] = max(depth_series)

    return result


# ---------------------------------------------------------------------- output

SCALARS = [
    "births_total",
    "deaths_total",
    "organisms_born",
    "founders_seen",
    "parent_age_p10_min",
    "parent_age_median_min",
    "parent_age_p90_min",
    "ttfr_n",
    "ttfr_p10_min",
    "ttfr_median_min",
    "ttfr_p90_min",
    "ttfr_founders_excluded",
    "ttfr_check_mismatch",
    "births_per_dead_n",
    "births_per_dead_mean",
    "gini_births",
    "depth_max",
    "depth_mean_overall",
    "depth_mean_by_window",
    "depth_max_by_window",
    "founder_lineages_seen",
    "founder_lineages_alive_end",
    "lineage_last_alive_median_h",
    "lineage_last_alive_max_h",
    "run_end_hours",
    "events",
]

# The compact set shown in summary-e3.md's headline table.
HEADLINE = [
    "births_total",
    "deaths_total",
    "founders_seen",
    "parent_age_median_min",
    "ttfr_n",
    "ttfr_median_min",
    "births_per_dead_mean",
    "gini_births",
    "depth_max",
    "depth_mean_overall",
    "founder_lineages_seen",
    "founder_lineages_alive_end",
    "run_end_hours",
]


def fmt_full(value):
    if value is None:
        return ""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, float):
        if value == int(value) and abs(value) < 1e15:
            return str(int(value))
        return repr(value)
    return str(value)


def fmt(value, digits=4):
    if value is None:
        return ""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        if value == int(value) and abs(value) < 1e15:
            return str(int(value))
        return f"{value:.{digits}g}"
    return str(value)


def cause_order(rows):
    seen = set()
    for r in rows:
        seen.update(r["lifespans"])
    ordered = [c for c in KNOWN_CAUSES if c in seen]
    ordered += sorted(seen - set(ordered))
    return ordered


def md_table(header, body):
    widths = [len(h) for h in header]
    for row in body:
        for i, cell in enumerate(row):
            widths[i] = max(widths[i], len(cell))
    out = ["| " + " | ".join(h.ljust(widths[i]) for i, h in enumerate(header)) + " |"]
    out.append("| " + " | ".join("-" * widths[i] for i in range(len(header))) + " |")
    for row in body:
        out.append("| " + " | ".join(row[i].ljust(widths[i]) for i in range(len(header))) + " |")
    return out


def write_csv(path: Path, rows, causes):
    cause_columns = []
    for cause in causes:
        for field in ("n", "p10_min", "median_min", "p90_min"):
            cause_columns.append(f"lifespan_{cause}_{field}")
    header = ["row", "seed"] + SCALARS + cause_columns + ["exit_code", "build_id"]
    with path.open("w", newline="") as fh:
        writer = csv.writer(fh)
        writer.writerow(header)
        for r in rows:
            line = [r["row"], fmt_full(r["seed"])]
            line += [fmt_full(r.get(name)) for name in SCALARS]
            for cause in causes:
                entry = r["lifespans"].get(cause, {})
                line += [
                    fmt_full(entry.get("n")),
                    fmt_full(entry.get("p10_min")),
                    fmt_full(entry.get("median_min")),
                    fmt_full(entry.get("p90_min")),
                ]
            line += [fmt_full(r.get("exit_code")), r.get("build_id") or ""]
            writer.writerow(line)


def write_md(path: Path, batch_dir: Path, rows, causes, meta):
    lines = [f"# E3 life events — batch `{batch_dir.name}`", ""]
    if meta:
        lines.append(f"- matrix: `{meta.get('matrix', '')}`")
        lines.append(f"- simulated seconds per row: {fmt(meta.get('seconds'))}")
        lines.append(f"- seeds: {meta.get('seeds')}")
    builds = sorted({r["build_id"] for r in rows if r.get("build_id")})
    if builds:
        lines.append(f"- build id: {', '.join(builds)}")
    lines.append(f"- rows analyzed: {len(rows)}")
    unfinished = [r["row"] for r in rows if r.get("exit_code") is None]
    if unfinished:
        lines.append(
            f"- **still running (no exit code yet): {', '.join(unfinished)}** — these"
            " numbers are a snapshot of an incomplete log"
        )
    mismatches = [r["row"] for r in rows if r.get("ttfr_check_mismatch")]
    if mismatches:
        lines.append(
            f"- **integrity check failed on: {', '.join(mismatches)}** (a first child's"
            " `parent_age_ticks` disagrees with its parent's own birth tick)"
        )
    lines += [
        "",
        "Durations are simulated minutes unless the column says otherwise; ticks are"
        f" {TICKS_PER_SECOND}/s.",
        "Founders emit no birth event, so they are excluded from time to first",
        "reproduction and a founder that neither reproduced nor died is invisible to",
        "this log. Definitions are in the header of `scripts/e3-analyze.py`.",
        "",
        "## Overview",
        "",
    ]
    body = [[r["row"], fmt(r["seed"])] + [fmt(r.get(k)) for k in HEADLINE] for r in rows]
    lines += md_table(["row", "seed"] + HEADLINE, body)

    lines += [
        "",
        "## Distributions",
        "",
        "Parent age at birth is one entry per birth; time to first reproduction is one",
        "entry per non-founder organism that reproduced.",
        "",
    ]
    dist_header = [
        "row",
        "parent_age p10",
        "parent_age median",
        "parent_age p90",
        "ttfr n",
        "ttfr p10",
        "ttfr median",
        "ttfr p90",
        "founders excluded",
    ]
    dist_body = [
        [
            r["row"],
            fmt(r["parent_age_p10_min"]),
            fmt(r["parent_age_median_min"]),
            fmt(r["parent_age_p90_min"]),
            fmt(r["ttfr_n"]),
            fmt(r["ttfr_p10_min"]),
            fmt(r["ttfr_median_min"]),
            fmt(r["ttfr_p90_min"]),
            fmt(r["ttfr_founders_excluded"]),
        ]
        for r in rows
    ]
    lines += md_table(dist_header, dist_body)

    lines += [
        "",
        "## Lifespan by cause",
        "",
        "From the `age_ticks` of completed lives only; organisms alive at the end of the",
        "run are not represented, which truncates the long tail.",
        "",
    ]
    life_header = ["row", "cause", "n", "p10", "median", "p90"]
    life_body = []
    for r in rows:
        first = True
        for cause in causes:
            entry = r["lifespans"].get(cause)
            if not entry:
                continue
            life_body.append([
                r["row"] if first else "",
                cause,
                fmt(entry["n"]),
                fmt(entry["p10_min"]),
                fmt(entry["median_min"]),
                fmt(entry["p90_min"]),
            ])
            first = False
    lines += md_table(life_header, life_body)

    lines += [
        "",
        f"## Ancestry depth per {DEPTH_WINDOW_HOURS}-hour window",
        "",
        "Mean and max depth of the organisms *born* in each window, with the number of",
        "births that the mean is over.",
        "",
    ]
    depth_header = ["row", "window (h)", "births", "mean depth", "max depth"]
    depth_body = []
    for r in rows:
        first = True
        for window in r["depth_windows"]:
            depth_body.append([
                r["row"] if first else "",
                f"{window['start_hours']}-{window['start_hours'] + DEPTH_WINDOW_HOURS}",
                fmt(window["births"]),
                fmt(window["mean"]),
                fmt(window["max"]),
            ])
            first = False
    lines += md_table(depth_header, depth_body)

    lines += [
        "",
        "## Founder-lineage survival",
        "",
        "One line per founder lineage seen in the log, by the founder's slot: the last",
        "simulated hour at which any of its descendants was alive.",
        "",
    ]
    lin_header = ["row", "lineages seen", "alive at end", "last-alive median (h)", "last-alive max (h)"]
    lin_body = [
        [
            r["row"],
            fmt(r["founder_lineages_seen"]),
            fmt(r["founder_lineages_alive_end"]),
            fmt(r["lineage_last_alive_median_h"]),
            fmt(r["lineage_last_alive_max_h"]),
        ]
        for r in rows
    ]
    lines += md_table(lin_header, lin_body)
    for r in rows:
        dead = [entry for entry in r["lineages"] if not entry["alive_at_end"]]
        if dead:
            slots = ", ".join(
                f"{entry['slot']}@{entry['last_alive_hours']:.2f}h" for entry in dead[:24]
            )
            more = "" if len(dead) <= 24 else f", +{len(dead) - 24} more"
            lines.append("")
            lines.append(f"`{r['row']}` lineages that ended (slot@last-alive): {slots}{more}")

    lines += [
        "",
        "## Raw trajectories",
        "",
        f"{SPARK_WIDTH}-character sparklines over {SPARK_WINDOW_MINUTES}-minute windows,",
        "each scaled to its own min..max. A window with no births contributes 0 to the",
        "depth line.",
        "",
    ]
    spark_header = ["row", "series", "max", "trajectory"]
    spark_body = []
    for r in rows:
        spark_body.append([r["row"], "births", fmt(r["spark_births_max"]), "`" + r["spark_births"] + "`"])
        spark_body.append(["", "mean depth", fmt(r["spark_depth_max"]), "`" + r["spark_depth"] + "`"])
    lines += md_table(spark_header, spark_body)
    lines.append("")
    path.write_text("\n".join(lines))


def main(argv):
    if len(argv) != 2:
        print("usage: e3-analyze.py runs/<batch>", file=sys.stderr)
        return 2
    batch_dir = Path(argv[1])
    if not batch_dir.is_dir():
        print(f"e3-analyze: no such batch directory: {batch_dir}", file=sys.stderr)
        return 1

    meta = {}
    batch_json = batch_dir / "batch.json"
    if batch_json.is_file():
        meta = json.loads(batch_json.read_text())

    row_names = meta.get("rows") or sorted(
        p.name for p in batch_dir.iterdir() if p.is_dir() and find_events(p)
    )

    rows = []
    for name in row_names:
        analyzed = analyze_row(batch_dir / name)
        if analyzed is None:
            print(f"e3-analyze: skipping {name} (no life events)", file=sys.stderr)
            continue
        rows.append(analyzed)

    if not rows:
        print(
            f"e3-analyze: no row under {batch_dir} has an events.jsonl; the runs need "
            f"`capacity.event_log = true`",
            file=sys.stderr,
        )
        return 1

    causes = cause_order(rows)
    write_csv(batch_dir / "summary-e3.csv", rows, causes)
    write_md(batch_dir / "summary-e3.md", batch_dir, rows, causes, meta)
    print(
        f"e3-analyze: {len(rows)} rows -> {batch_dir / 'summary-e3.csv'}, "
        f"{batch_dir / 'summary-e3.md'}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
