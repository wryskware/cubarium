#!/usr/bin/env python3
"""Analyze an E2 batch: scripts/e2-analyze.py runs/<batch>

Reads every `runs/<batch>/<row>/telemetry.jsonl` and writes `runs/<batch>/summary.csv`
and `runs/<batch>/summary.md`, one line per row (a row is one axis combination at one
seed). Standard library only; a 6-hour run at the experiment cadence is 4,320 samples,
so everything below is plain Python over lists of floats. No metric is combined into
a score.

Conventions
-----------

* Sample time. Telemetry carries `tick`; simulated seconds are `tick * 0.05`
  (the host's fixed 20 Hz tick, crates/cubarium/README.md "Clock").
* Sample interval. The median difference between consecutive sample times, falling
  back to the row config's `capacity.telemetry_seconds`, then to 5 s.
* Burn-in. BURN_IN_SECONDS = 600 (the contract's 10-minute burn-in). The analysis
  window is every sample at simulated time >= 600 s. Every metric below is computed
  on that window except `extinct_at` and `residual_max`, which scan the whole run.
* Empty cells. A metric that is undefined for a run (no window samples, a zero
  denominator, a constant series where a correlation would divide by zero) is left
  empty rather than reported as 0.

Metrics (design/experiments-e2-harness.md "Analyzer")
-----------------------------------------------------

pop_min, pop_max, pop_final
    min / max / last of `population` over the window.
pop_cv
    population std / population mean over the window; std is the population
    (divide-by-N) standard deviation. Empty when the mean is 0.
extinct_at
    Simulated seconds of the first sample in the whole run whose `population` is 0;
    empty if the world never empties.
time_at_cap
    Fraction of window samples with `population >= 0.95 * capacity.max_organisms`,
    where `max_organisms` comes from the row's config.toml (default 512).
births_per_hour, deaths_per_hour
    The telemetry `births` and `deaths_*` fields are per-sample counters (reset when
    each sample is emitted), so summing them over the window reproduces the
    cumulative counter's increase across the window. deaths = starvation + age +
    collapse. The divisor is the window duration, n_window_samples * interval / 3600,
    because each sample's counter covers the interval that ended at that sample.
producer_mean
    Mean of `producer` (that is, of Sigma P) over the window, in material units.
producer_min_frac
    min(Sigma P) over the window divided by CELL_COUNT * P_max, with CELL_COUNT =
    1280 (cubarium_surface: five faces x 16 x 16 cells) and P_max from the row's
    config.toml `producer.max` (default 2.0).
detritus_mean_frac
    Mean over window samples of detritus / (nutrient + producer + detritus); samples
    whose field total is 0 are skipped.
occupied_cells_mean
    Mean of `occupied_cells` over the window.
face_sync
    The five `population_by_face` series are detrended by subtracting a 30-minute
    centered moving average (window = round(1800 / interval) samples, forced odd,
    truncated at the ends so the detrended series keeps full length). face_sync is
    the mean over the ten face pairs of the zero-lag Pearson correlation of the
    detrended series. Pairs whose detrended series is constant (zero variance) are
    skipped; empty if no pair is defined.
face_lag_max
    On the same detrended series: for every face pair, the Pearson correlation of
    x[t] against y[t + k] is evaluated for every lag k in [-L, L] (each lag uses the
    overlapping samples only, with the means and standard deviations of those
    overlapping slices), L = min(round(1800 / interval), n_window // 2) clamped at 0.
    The second term only binds on runs too short to hold a 30-minute lag; it keeps
    every lag's overlap at least half the window rather than letting a handful of
    samples produce a spurious |r| near 1.
    The pair's lag is the k with the largest |correlation|; face_lag_max is the
    largest |k| over all pairs, in samples. Multiply by the interval for seconds.
crash_cycles
    Walk the window's population series tracking a running local maximum (value and
    time). A sample that exceeds the running maximum replaces it. Otherwise, if the
    population is below 50 % of the running maximum and the sample is within 1800
    simulated seconds of the sample that set that maximum, count one crash and reset
    the running maximum to the current sample. A maximum that goes stale without
    being crashed is not reset (the contract does not say to).
residual_max
    max |mass_residual| over the whole run, burn-in included: it is a correctness
    check on the world, not an ecological measurement.

summary.md additionally embeds, per row, a 32-character ASCII sparkline of
`population` and of Sigma P over the whole run (burn-in included, so the raw
trajectory is visible as the E2 protocol asks). The series is split into 32
contiguous buckets, each bucket is averaged, and the bucket means are mapped
linearly from the lowest to the highest bucket mean onto the eight block characters.
The min/max printed beside a sparkline are the raw series extremes, which the
bucket averaging can smooth away.
"""

from __future__ import annotations

import csv
import json
import math
import sys
import tomllib
from pathlib import Path

DT = 0.05  # simulated seconds per tick (fixed 20 Hz host clock)
BURN_IN_SECONDS = 600.0
MOVING_AVERAGE_SECONDS = 1800.0
LAG_LIMIT_SECONDS = 1800.0
CRASH_WINDOW_SECONDS = 1800.0
CRASH_FRACTION = 0.5
CAP_FRACTION = 0.95
CELL_COUNT = 1280  # five faces x 16 x 16 field cells
DEFAULT_MAX_ORGANISMS = 512
DEFAULT_PRODUCER_MAX = 2.0
SPARK_WIDTH = 32
BLOCKS = "▁▂▃▄▅▆▇█"

METRICS = [
    "pop_min",
    "pop_max",
    "pop_final",
    "pop_cv",
    "extinct_at",
    "time_at_cap",
    "births_per_hour",
    "deaths_per_hour",
    "producer_mean",
    "producer_min_frac",
    "detritus_mean_frac",
    "occupied_cells_mean",
    "face_sync",
    "face_lag_max",
    "crash_cycles",
    "residual_max",
]


# --------------------------------------------------------------------------- io


def flatten(table, prefix=""):
    out = {}
    for key, value in table.items():
        dotted = f"{prefix}{key}"
        if isinstance(value, dict):
            out.update(flatten(value, dotted + "."))
        else:
            out[dotted] = value
    return out


def read_config(path: Path) -> dict:
    if not path.is_file():
        return {}
    with path.open("rb") as fh:
        return flatten(tomllib.load(fh))


def read_samples(path: Path) -> list:
    samples = []
    with path.open() as fh:
        for line in fh:
            line = line.strip()
            if line:
                samples.append(json.loads(line))
    samples.sort(key=lambda s: s["tick"])
    return samples


# ------------------------------------------------------------------ statistics


def mean(xs):
    return sum(xs) / len(xs) if xs else None


def stdev(xs):
    if not xs:
        return None
    m = sum(xs) / len(xs)
    return math.sqrt(sum((x - m) ** 2 for x in xs) / len(xs))


def median(xs):
    if not xs:
        return None
    ordered = sorted(xs)
    mid = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[mid]
    return 0.5 * (ordered[mid - 1] + ordered[mid])


def pearson(xs, ys):
    """Pearson correlation of two equal-length sequences, or None if degenerate."""
    n = len(xs)
    if n < 2:
        return None
    mx = sum(xs) / n
    my = sum(ys) / n
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    sxx = sum((x - mx) ** 2 for x in xs)
    syy = sum((y - my) ** 2 for y in ys)
    if sxx <= 0.0 or syy <= 0.0:
        return None
    return sxy / math.sqrt(sxx * syy)


def centered_moving_average(xs, window):
    """Centered moving average with the window truncated at the ends."""
    n = len(xs)
    if window <= 1 or n == 0:
        return list(xs)
    half = window // 2
    prefix = [0.0]
    for x in xs:
        prefix.append(prefix[-1] + x)
    out = []
    for i in range(n):
        lo = max(0, i - half)
        hi = min(n, i + half + 1)
        out.append((prefix[hi] - prefix[lo]) / (hi - lo))
    return out


def detrend(xs, window):
    trend = centered_moving_average(xs, window)
    return [x - t for x, t in zip(xs, trend)]


# ------------------------------------------------------------------ sparklines


def sparkline(xs, width=SPARK_WIDTH):
    if not xs:
        return ""
    n = len(xs)
    buckets = []
    for i in range(width):
        lo = i * n // width
        hi = max(lo + 1, (i + 1) * n // width)
        hi = min(hi, n)
        if lo >= n:
            break
        buckets.append(sum(xs[lo:hi]) / (hi - lo))
    lo = min(buckets)
    hi = max(buckets)
    if hi - lo <= 0:
        return BLOCKS[0] * len(buckets)
    return "".join(BLOCKS[min(7, int((b - lo) / (hi - lo) * 8))] for b in buckets)


# --------------------------------------------------------------------- metrics


def analyze_row(row_dir: Path) -> dict | None:
    telemetry = row_dir / "telemetry.jsonl"
    manifest_path = row_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.is_file() else {}
    config = read_config(row_dir / "config.toml")

    if not telemetry.is_file():
        return None
    samples = read_samples(telemetry)
    if not samples:
        return None

    times = [s["tick"] * DT for s in samples]
    diffs = [b - a for a, b in zip(times, times[1:])]
    interval = median(diffs) or float(
        config.get("capacity.telemetry_seconds", 5.0)
    )
    if not interval or interval <= 0:
        interval = 5.0

    max_organisms = float(config.get("capacity.max_organisms", DEFAULT_MAX_ORGANISMS))
    producer_max = float(config.get("producer.max", DEFAULT_PRODUCER_MAX))

    result = {
        "row": row_dir.name,
        "seed": manifest.get("seed", config.get("seed")),
        "exit_code": manifest.get("exit_code"),
        "wall_seconds": manifest.get("wall_seconds"),
        "build_id": manifest.get("build_id"),
        "axes": manifest.get("axes", {}),
        "samples": len(samples),
        "sim_seconds": times[-1],
        "interval": interval,
        "spark_population": sparkline([float(s["population"]) for s in samples]),
        "spark_producer": sparkline([float(s["producer"]) for s in samples]),
    }
    for name in METRICS:
        result[name] = None

    # residual_max and extinct_at scan the whole run, burn-in included.
    result["residual_max"] = max(abs(float(s["mass_residual"])) for s in samples)
    for sample, t in zip(samples, times):
        if sample["population"] == 0:
            result["extinct_at"] = t
            break

    window = [(s, t) for s, t in zip(samples, times) if t >= BURN_IN_SECONDS]
    result["window_samples"] = len(window)
    if not window:
        return result

    ws = [s for s, _ in window]
    wt = [t for _, t in window]
    pop = [float(s["population"]) for s in ws]

    result["pop_min"] = min(pop)
    result["pop_max"] = max(pop)
    result["pop_final"] = pop[-1]
    pop_mean = mean(pop)
    result["pop_cv"] = (stdev(pop) / pop_mean) if pop_mean else None

    if max_organisms > 0:
        threshold = CAP_FRACTION * max_organisms
        result["time_at_cap"] = sum(1 for p in pop if p >= threshold) / len(pop)

    hours = len(ws) * interval / 3600.0
    if hours > 0:
        births = sum(int(s["births"]) for s in ws)
        deaths = sum(
            int(s["deaths_starvation"]) + int(s["deaths_age"]) + int(s["deaths_collapse"])
            for s in ws
        )
        result["births_per_hour"] = births / hours
        result["deaths_per_hour"] = deaths / hours

    producer = [float(s["producer"]) for s in ws]
    result["producer_mean"] = mean(producer)
    if producer_max > 0:
        result["producer_min_frac"] = min(producer) / (CELL_COUNT * producer_max)

    fracs = []
    for s in ws:
        total = float(s["nutrient"]) + float(s["producer"]) + float(s["detritus"])
        if total > 0:
            fracs.append(float(s["detritus"]) / total)
    result["detritus_mean_frac"] = mean(fracs)

    result["occupied_cells_mean"] = mean([float(s["occupied_cells"]) for s in ws])

    # Per-face synchrony on detrended series.
    ma_window = max(1, int(round(MOVING_AVERAGE_SECONDS / interval)))
    if ma_window % 2 == 0:
        ma_window += 1
    faces = [
        detrend([float(s["population_by_face"][f]) for s in ws], ma_window)
        for f in range(5)
    ]
    pairs = [(a, b) for a in range(5) for b in range(a + 1, 5)]

    zero_lag = [pearson(faces[a], faces[b]) for a, b in pairs]
    defined = [r for r in zero_lag if r is not None]
    result["face_sync"] = mean(defined) if defined else None

    n = len(ws)
    lag_limit = max(0, min(int(round(LAG_LIMIT_SECONDS / interval)), n // 2))
    best_abs_lag = None
    for a, b in pairs:
        x, y = faces[a], faces[b]
        best_r, best_lag = None, None
        for k in range(-lag_limit, lag_limit + 1):
            # correlate x[t] with y[t + k]
            if k >= 0:
                xs, ys = x[: n - k], y[k:]
            else:
                xs, ys = x[-k:], y[: n + k]
            r = pearson(xs, ys)
            if r is None:
                continue
            if best_r is None or abs(r) > abs(best_r):
                best_r, best_lag = r, k
        if best_lag is not None:
            best_abs_lag = max(best_abs_lag or 0, abs(best_lag))
    result["face_lag_max"] = best_abs_lag

    # Crash cycles.
    crashes = 0
    running_max = pop[0]
    running_max_time = wt[0]
    for p, t in zip(pop[1:], wt[1:]):
        if p > running_max:
            running_max = p
            running_max_time = t
        elif (
            running_max > 0
            and p < CRASH_FRACTION * running_max
            and t - running_max_time <= CRASH_WINDOW_SECONDS
        ):
            crashes += 1
            running_max = p
            running_max_time = t
    result["crash_cycles"] = crashes

    return result


# ----------------------------------------------------------------- formatting


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


def write_csv(path: Path, rows, axis_keys):
    header = (
        ["row", "seed"]
        + axis_keys
        + ["samples", "window_samples", "sim_seconds", "interval"]
        + METRICS
        + ["exit_code", "wall_seconds", "build_id"]
    )
    with path.open("w", newline="") as fh:
        writer = csv.writer(fh)
        writer.writerow(header)
        for r in rows:
            line = [r["row"], fmt(r["seed"])]
            line += [fmt(r["axes"].get(k)) for k in axis_keys]
            line += [
                fmt(r["samples"]),
                fmt(r.get("window_samples")),
                fmt(r["sim_seconds"]),
                fmt(r["interval"]),
            ]
            line += [fmt(r[m], 6) for m in METRICS]
            line += [fmt(r["exit_code"]), fmt(r["wall_seconds"]), r.get("build_id") or ""]
            writer.writerow(line)


def md_table(header, rows):
    widths = [len(h) for h in header]
    for row in rows:
        for i, cell in enumerate(row):
            widths[i] = max(widths[i], len(cell))
    out = ["| " + " | ".join(h.ljust(widths[i]) for i, h in enumerate(header)) + " |"]
    out.append("| " + " | ".join("-" * widths[i] for i in range(len(header))) + " |")
    for row in rows:
        out.append("| " + " | ".join(row[i].ljust(widths[i]) for i in range(len(header))) + " |")
    return out


def write_md(path: Path, batch_dir: Path, rows, axis_keys, meta):
    lines = [f"# E2 batch `{batch_dir.name}`", ""]
    if meta:
        lines.append(f"- matrix: `{meta.get('matrix', '')}`")
        lines.append(f"- simulated seconds per row: {fmt(meta.get('seconds'))}")
        lines.append(f"- seeds: {meta.get('seeds')}")
        if meta.get("wall_seconds") is not None:
            lines.append(f"- batch wall seconds: {fmt(meta['wall_seconds'])}")
    build_ids = sorted({r["build_id"] for r in rows if r.get("build_id")})
    if build_ids:
        lines.append(f"- build id: {', '.join(build_ids)}")
    lines.append(f"- rows analyzed: {len(rows)}")
    failed = [r["row"] for r in rows if r.get("exit_code") not in (0, None)]
    if failed:
        lines.append(f"- **rows with a nonzero exit code: {', '.join(failed)}**")
    lines += [
        "",
        f"Burn-in {int(BURN_IN_SECONDS)} s; every metric uses the post-burn-in window except",
        "`extinct_at` and `residual_max`, which scan the whole run. Metric definitions are in",
        "the header of `scripts/e2-analyze.py`. No metric is combined into a score.",
        "",
        "## Metrics",
        "",
    ]

    header = ["row", "seed"] + axis_keys + METRICS
    body = []
    for r in rows:
        cells = [r["row"], fmt(r["seed"])]
        cells += [fmt(r["axes"].get(k)) for k in axis_keys]
        cells += [fmt(r[m]) for m in METRICS]
        body.append(cells)
    lines += md_table(header, body)

    lines += [
        "",
        "## Raw trajectories",
        "",
        f"{SPARK_WIDTH}-character sparklines over the whole run (burn-in included). The",
        "series is downsampled into 32 bucket means and the blocks span the lowest to the",
        "highest bucket mean; the min and max columns are the raw series extremes.",
        "",
    ]
    spark_header = ["row", "series", "min", "max", "trajectory"]
    spark_body = []
    for r in rows:
        spark_body.append(
            [
                r["row"],
                "population",
                fmt(r.get("pop_series_min")),
                fmt(r.get("pop_series_max")),
                "`" + r["spark_population"] + "`",
            ]
        )
        spark_body.append(
            [
                "",
                "Σ P",
                fmt(r.get("prod_series_min")),
                fmt(r.get("prod_series_max")),
                "`" + r["spark_producer"] + "`",
            ]
        )
    lines += md_table(spark_header, spark_body)
    lines.append("")
    path.write_text("\n".join(lines))


def main(argv):
    if len(argv) != 2:
        print("usage: e2-analyze.py runs/<batch>", file=sys.stderr)
        return 2
    batch_dir = Path(argv[1])
    if not batch_dir.is_dir():
        print(f"e2-analyze: no such batch directory: {batch_dir}", file=sys.stderr)
        return 1

    meta = {}
    batch_json = batch_dir / "batch.json"
    if batch_json.is_file():
        meta = json.loads(batch_json.read_text())

    row_names = meta.get("rows") or sorted(
        p.name for p in batch_dir.iterdir() if (p / "telemetry.jsonl").is_file()
    )

    rows = []
    for name in row_names:
        row_dir = batch_dir / name
        analyzed = analyze_row(row_dir)
        if analyzed is None:
            print(f"e2-analyze: skipping {name} (no telemetry)", file=sys.stderr)
            continue
        # Series extremes for the sparkline labels.
        samples = read_samples(row_dir / "telemetry.jsonl")
        analyzed["pop_series_min"] = min(float(s["population"]) for s in samples)
        analyzed["pop_series_max"] = max(float(s["population"]) for s in samples)
        analyzed["prod_series_min"] = min(float(s["producer"]) for s in samples)
        analyzed["prod_series_max"] = max(float(s["producer"]) for s in samples)
        rows.append(analyzed)

    if not rows:
        print(f"e2-analyze: no rows with telemetry under {batch_dir}", file=sys.stderr)
        return 1

    axis_keys = list(meta.get("axes", {}))
    if not axis_keys:
        seen = []
        for r in rows:
            for k in r["axes"]:
                if k not in seen:
                    seen.append(k)
        axis_keys = seen

    write_csv(batch_dir / "summary.csv", rows, axis_keys)
    write_md(batch_dir / "summary.md", batch_dir, rows, axis_keys, meta)
    print(f"e2-analyze: {len(rows)} rows -> {batch_dir / 'summary.csv'}, {batch_dir / 'summary.md'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
