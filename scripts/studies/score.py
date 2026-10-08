#!/usr/bin/env python3
"""How to run a paired check: launch, wait for OUT/DONE externally, then score.

Desktop: python3 scripts/studies/batch.py runs/check --bin /path/to/voxel_census --arm base=heuristic --arm 'trial=heuristic set=fauna.founding.arrival_stomach=0.15' --config config/desktop/terrarium-d72.toml --seeds 1-8 --hours 2 --threads 2 --max-parallel 4 --detach
Eidolon (run there): python3 scripts/studies/batch.py /tmp/check --bin /tmp/voxel_census --arm base= --arm trial=heuristic --starts '/tmp/starts/s*/run/ckpt/m000060' --hours 2 --threads 1 --max-parallel 16 --detach
After DONE: python3 scripts/studies/score.py OUT --baseline base

Components precede the composite. All fitness choices are provisional TOML
settings. Raw per-lineage readings are preserved in JSON and TSV; plain output
prints their detail too. Q needs search cycle books and is NA with contribution
zero. Other unavailable data are also NA, explicitly marked as a partial score.
Failed, paused, running or missing runs have invalid P and invalidate their
arm's aggregate and their seed's pair. They remain visible in every format.
Late means use the scoring window's second half, excluding the interval ending
at its midpoint. N0 and the required roster always use the run's first CSV row,
even with --from. Counter deltas use the observation at/before the lower bound.
Health condition/activity use the same late window; births/deaths use the full
window. Sample SD is used (zero for a single seed). Negative paired deltas win.
The current census refuses parameter overrides beside load=. The saved-start
example therefore compares shipped policies with heuristics; ecological
parameter arms currently require fresh --config starts.
"""

import argparse
import csv
import json
import math
from pathlib import Path
import statistics
import sys
import tomllib

from vitals_summary import average, count_share, flag, number, read_csv, weighted


COMPONENTS = ("L", "J", "Q", "A", "K", "V", "Z", "M", "G_b", "G_v", "H", "F", "S")
DEFAULT_CONFIG = Path(__file__).resolve().parents[2] / "config/training/fitness-v1.toml"


def clip(value):
    return min(1.0, max(0.0, value))


def h(value):
    return clip(1.0 - value)


def d(value, low, high):
    if low <= value <= high:
        return 0.0
    if value < low:
        return clip((low - value) / abs(low)) if low else 1.0
    return clip((value - high) / abs(high)) if high else 1.0


def band_penalty(counts, reference, k_hi):
    return average([d(n, reference, k_hi * reference) for n in counts])


def peak_penalty(counts, reference, k_peak):
    if not counts:
        return None
    ceiling = k_peak * reference
    return clip((max(counts) - ceiling) / ceiling) if ceiling else float(max(counts) > 0)


def reproductive_penalty(late_births, eligible, cfg):
    if late_births:
        return 0.0
    if eligible is None:
        return None
    return cfg["eligible_deficit"] if eligible else cfg["ineligible_deficit"]


def stats(values):
    values = [v for v in values if v is not None]
    if not values:
        return dict(median=None, min=None, max=None, n=0)
    return dict(median=statistics.median(values), min=min(values), max=max(values), n=len(values))


def component(raw=None, penalty=None, weight=0.0, **details):
    return dict(raw=raw, penalty=penalty, weight=weight,
                contribution=weight * penalty if penalty is not None else 0.0, **details)


def validate_config(cfg):
    for term in COMPONENTS:
        if cfg["weights"][term] < 0 or not math.isfinite(cfg["weights"][term]):
            raise ValueError(f"invalid weight: {term}")
    targets = cfg["targets"]
    for key in ("moving_share", "voxel_m", "meadow_m2", "grove_m2", "vaulttree_recruits", "fullness_p10"):
        if not math.isfinite(targets[key]) or targets[key] <= 0:
            raise ValueError(f"target {key} must be positive and finite")
    for lo, hi in (("predator_share_lo", "predator_share_hi"), ("hunts_per_hour_lo", "hunts_per_hour_hi")):
        if not 0 <= targets[lo] <= targets[hi] or not math.isfinite(targets[hi]):
            raise ValueError(f"invalid band: {lo}, {hi}")
    pop = cfg["population"]
    if not 1 <= pop["k_hi"] <= pop["k_peak"] or not math.isfinite(pop["k_peak"]):
        raise ValueError("require 1 <= k_hi <= k_peak, finite")
    if any(v < 0 or not math.isfinite(v) for v in pop["references"].values()):
        raise ValueError("population references must be finite and nonnegative")
    if any(not 0 <= v <= 1 for v in cfg["reproduction"].values()):
        raise ValueError("reproductive deficits must be in [0,1]")
    summary = cfg["summary"]
    if not 0 < summary["consistent_fraction"] <= 1 or summary["consistent_min_seeds"] < 1 or summary["sd_weight"] < 0:
        raise ValueError("invalid summary settings")


def born(row):
    # Loaded founders have reconstructed born_minute but origin=observed.
    return flag(row, "native") and row.get("origin", "").lower() == "born"


def within(value, start, end, exclusive=False):
    return value is not None and (start < value if exclusive else start <= value) and value <= end


def read_optional(path, required, warnings):
    if not path.exists():
        warnings.append(f"missing {path.name}")
        return []
    return read_csv(path, required)


def score_world(root, arm, seed, cfg, start=0.0, stop=math.inf, state=None):
    result = dict(arm=arm, seed=seed, state=state or "missing", P=None, components={}, health={}, plants={}, warnings=[])
    run = root / "run"
    markers = [name for name in ("FAILED", "PAUSED", "DONE") if (run / name).exists()]
    result["state"] = markers[0].lower() if markers else (state if state != "done" else "missing") or "missing"
    if state in {"failed", "paused"}:
        result["state"] = state
    if result["state"] != "done":
        result["warnings"].append(f"run is {result['state']}; P invalid")
        return result
    try:
        rows = read_csv(root / "census.csv", ["sim_min", "seed"])
        if not rows:
            raise ValueError("empty census")
        minutes = [number(row, "sim_min") for row in rows]
        if any(value is None for value in minutes) or any(a >= b for a, b in zip(minutes, minutes[1:])):
            raise ValueError("census minutes must be finite and strictly increasing")
        if any(number(row, "seed") != seed for row in rows):
            raise ValueError("census seed differs from batch seed")
        first, last = rows[0], rows[-1]
        begin, end = max(start, minutes[0]), min(stop, minutes[-1])
        if begin >= end:
            raise ValueError("scoring window needs two distinct observation times")
        window = [row for row in rows if within(number(row, "sim_min"), begin, end)]
        opening = next(row for row in reversed(rows) if number(row, "sim_min") <= begin)
        closing = next(row for row in reversed(rows) if number(row, "sim_min") <= end)
        midpoint = (begin + end) / 2
        late = [row for row in window if within(number(row, "sim_min"), midpoint, end)]
        if not late:
            raise ValueError("no census observations in second half")
        vitals = read_optional(root / "vitals/vitals.csv", ["minute", "lineage", "n"], result["warnings"])
        lives = read_optional(root / "vitals/life.csv", ["lineage", "native", "origin", "born_minute", "died_minute", "cause"], result["warnings"])
        lineages = sorted(key.removeprefix("founders_") for key in first if key.startswith("founders_"))
        species = sorted(key.removeprefix("stands_") for key in first if key.startswith("stands_"))
        if not lineages or not species:
            raise ValueError("missing per-lineage or per-species census counts")
        stock_keys = [f"{prefix}{name}" for prefix, names in (("founders_", lineages), ("eggs_", lineages), ("stands_", species)) for name in names]
        if any(number(row, key) is None or number(row, key) < 0 for row in rows for key in stock_keys):
            raise ValueError("missing or invalid stock count")
        required_animals = set(cfg["population"]["required_animals"]) | {name for name in lineages if number(first, f"founders_{name}") + number(first, f"eggs_{name}") > 0}
        required_plants = set(cfg["population"]["required_plants"]) | {name for name in species if number(first, f"stands_{name}") > 0}
        unknown = required_animals - set(lineages) | required_plants - set(species)
        if unknown:
            raise ValueError(f"required names absent from census: {', '.join(sorted(unknown))}")
        lost = [f"animal:{name}" for name in sorted(required_animals) if number(closing, f"founders_{name}") + number(closing, f"eggs_{name}") == 0]
        lost += [f"plant:{name}" for name in sorted(required_plants) if number(closing, f"stands_{name}") == 0]
        components = result["components"]
        weights, targets, pop = cfg["weights"], cfg["targets"], cfg["population"]

        def put(term, raw, penalty, **details):
            components[term] = component(raw, penalty, weights[term], **details)

        put("L", len(lost), float(len(lost)), lost=lost, required_animals=sorted(required_animals), required_plants=sorted(required_plants))
        active = [name for name in lineages if name in required_animals or any(number(row, f"founders_{name}") + number(row, f"eggs_{name}") > 0 for row in window)]
        per_j, per_a, per_k = {}, {}, {}
        for name in active:
            vr = [r for r in vitals if r["lineage"] == name and within(number(r, "minute"), begin, end)]
            vl = [r for r in vr if within(number(r, "minute"), midpoint, end)]
            intervals = [r for r in vl if number(r, "minute") > midpoint]
            lr = [r for r in lives if r["lineage"] == name]
            births = [r for r in lr if born(r) and within(number(r, "born_minute"), begin, end)]
            late_births = sum(within(number(r, "born_minute"), midpoint, end) for r in births)
            if not (root / "vitals/life.csv").exists():
                birth_keys = [key for key in (f"births_{name}", f"eggs_hatched_{name}") if key in first]
                lower = next(row for row in reversed(rows) if number(row, "sim_min") <= midpoint)
                late_births = sum(max(0, number(closing, key, 0) - number(lower, key, 0)) for key in birth_keys)
            observed_eligible = [number(r, "breeding_eligible_n") for r in vr if number(r, "breeding_eligible_n") is not None]
            eligible = any(value > 0 for value in observed_eligible) if observed_eligible else None
            jp = reproductive_penalty(late_births, eligible, cfg["reproduction"])
            slow = name in pop["slow"]
            per_j[name] = dict(late_births=late_births, eligible=eligible, penalty=jp, weight=0.0 if slow else 1.0, flag="lifecycle untested" if slow else "")
            reference = pop["references"].get(name, number(first, f"founders_{name}"))
            counts = [number(row, f"founders_{name}") for row in late]
            peaks = [number(row, f"founders_{name}") for row in window]
            per_a[name] = dict(N0=reference, raw=average(counts), penalty=band_penalty(counts, reference, pop["k_hi"]))
            per_k[name] = dict(N0=reference, raw=max(peaks), penalty=peak_penalty(peaks, reference, pop["k_peak"]))
            deaths = [r for r in lr if within(number(r, "died_minute"), begin, end, True)]
            causes = {r["cause"] for r in lr if r["cause"]} | {"starved", "drowned", "aged", "fell", "desiccated", "removed"}
            causes |= {f"killed_by_{hunter}" for hunter in lineages}
            death_counts = {cause: sum(r["cause"] == cause for r in deaths) for cause in sorted(causes)}
            if not (root / "vitals/life.csv").exists():
                death_counts = {cause: number(closing, f"{cause}_{name}") - number(opening, f"{cause}_{name}") for cause in ("starved", "drowned", "killed") if number(closing, f"{cause}_{name}") is not None}
            result["health"][name] = dict(alive_end=number(closing, f"founders_{name}"), eggs_end=number(closing, f"eggs_{name}"), births=len(births) if (root / "vitals/life.csv").exists() else None, deaths=death_counts, fullness_p10=weighted(vl, "fullness_p10"), fullness_p50=weighted(vl, "fullness_p50"), net_energy_per_body_hour=weighted(intervals, "net_energy_per_body_hour", "energy_body_hours"), moving_share=weighted(intervals, "moving_share", "awake_body_s"), stuck_share=count_share(vl, "stuck_n", "stuck_observed_n"), flag="lifecycle untested" if slow else "")
        j_values = [v["penalty"] for v in per_j.values() if v["weight"]]
        j = average(j_values) if j_values and all(v is not None for v in j_values) else (0.0 if not j_values else None)
        put("J", per_j, j)
        put("Q", None, None, note="needs targets.rs cycle books: inflow I and stock change delta S over a day")
        put("A", per_a, average([v["penalty"] for v in per_a.values()]))
        put("K", per_k, average([v["penalty"] for v in per_k.values()]))
        browser = targets["moving_lineage"]
        moving = result["health"].get(browser, {}).get("moving_share")
        put("V", moving, h(moving / targets["moving_share"]) if moving is not None else None)
        hunter = targets["predator_lineage"]
        shares = []
        for row in late:
            total = sum(number(row, f"founders_{name}") for name in lineages)
            shares.append(number(row, f"founders_{hunter}", 0) / total if total else 0.0)
        share = average(shares)
        free_first = number(first, f"founders_{hunter}", 0) - number(first, "riders_now", 0)
        put("Z", share, d(share, targets["predator_share_lo"], targets["predator_share_hi"]), note="weight zero: no initially free lanternjaw" if free_first <= 0 else "")
        if free_first <= 0:
            components["Z"].update(weight=0.0, contribution=0.0)
        for term, key, target in (("M", "meadow_patch_max", "meadow_m2"), ("G_b", "grove_patch_max", "grove_m2")):
            areas = [number(row, key) * targets["voxel_m"]**2 for row in late if number(row, key) is not None]
            area = statistics.median(areas) if areas else None
            put(term, area, h(area / targets[target]) if area is not None else None, unit="m2")
        recruits = number(closing, "recruits_vaulttree")
        recruits0 = number(opening, "recruits_vaulttree")
        recruits = recruits - recruits0 if recruits is not None and recruits0 is not None else None
        put("G_v", recruits, h(recruits / targets["vaulttree_recruits"]) if recruits is not None else None)
        kill_keys = [key for key in first if any(key.startswith(f"kills_{hunter}_") for hunter in targets["hunt_lineages"])]
        kills = sum(number(closing, key) - number(opening, key) for key in kill_keys) if kill_keys else None
        hunts = kills / ((end - begin) / 60) if kills is not None else None
        put("H", hunts, d(hunts, targets["hunts_per_hour_lo"], targets["hunts_per_hour_hi"]) if hunts is not None else None, unit="kills/world-hour")
        condition, fullness, stuck = {}, {}, {}
        for name in active:
            vl = [r for r in vitals if r["lineage"] == name and number(r, "n", 0) > 0 and within(number(r, "minute"), midpoint, end)]
            vals = [number(r, "fullness_p10") for r in vl if number(r, "fullness_p10") is not None]
            if vals:
                fullness[name] = average(vals)
                condition[name] = average([h(value / targets["fullness_p10"]) for value in vals])
            if vl and all(number(r, "stuck_n") is not None and number(r, "stuck_observed_n") is not None for r in vl):
                stuck[name] = count_share(vl, "stuck_n", "stuck_observed_n") or 0.0
        put("F", fullness, average(list(condition.values())), per_lineage_penalty=condition)
        put("S", stuck, average(list(stuck.values())))
        result["plants"] = {name: number(closing, f"stands_{name}") for name in species}
        result["window"] = dict(start=begin, end=end, midpoint=midpoint)
        result["P"] = sum(value["contribution"] for value in components.values())
        result["missing_components"] = [term for term in COMPONENTS if components[term]["penalty"] is None]
        result["partial"] = any(term != "Q" for term in result["missing_components"])
        if result["partial"]:
            result["warnings"].append("partial P: unavailable components contribute zero")
    except (OSError, ValueError, TypeError) as error:
        result.update(P=None, state="invalid")
        result["warnings"].append(str(error))
    return result


def arm_summary(worlds, cfg):
    values = [world["P"] for world in worlds]
    complete = all(value is not None for value in values)
    mean = average(values) if complete else None
    sd = statistics.stdev(values) if complete and len(values) > 1 else (0.0 if complete else None)
    components = {}
    for term in COMPONENTS:
        components[term] = stats([world["components"].get(term, {}).get("penalty") for world in worlds])
    health = {}
    for name in sorted({name for world in worlds for name in world["health"]}):
        rows = [world["health"][name] for world in worlds if name in world["health"]]
        metrics = ("alive_end", "eggs_end", "births", "fullness_p10", "fullness_p50", "net_energy_per_body_hour", "moving_share", "stuck_share")
        health[name] = {metric: stats([row[metric] for row in rows]) for metric in metrics}
        health[name]["deaths"] = {cause: stats([row["deaths"].get(cause, 0) for row in rows]) for cause in sorted({cause for row in rows for cause in row["deaths"]})}
        health[name]["flag"] = next((row["flag"] for row in rows if row["flag"]), "")
    plants = {name: stats([world["plants"].get(name) for world in worlds]) for name in sorted({name for world in worlds for name in world["plants"]})}
    return dict(mean_P=mean, sd_P=sd, rank=mean + cfg["summary"]["sd_weight"] * sd if complete else None, invalid_seeds=[world["seed"] for world in worlds if world["P"] is None], partial=any(world.get("partial", False) for world in worlds), components=components, health=health, plants=plants)


def pair_worlds(worlds, baseline, cfg):
    by_arm = {}
    for world in worlds:
        by_arm.setdefault(world["arm"], {})[world["seed"]] = world
    if baseline not in by_arm:
        raise ValueError(f"unknown baseline arm: {baseline}")
    result = {}
    for arm, candidates in by_arm.items():
        if arm == baseline:
            continue
        pairs = []
        for seed in sorted(set(candidates) | set(by_arm[baseline])):
            a, b = candidates.get(seed), by_arm[baseline].get(seed)
            valid = a is not None and b is not None and a["P"] is not None and b["P"] is not None
            deltas = {}
            for term in COMPONENTS:
                av = a["components"].get(term, {}).get("penalty") if a else None
                bv = b["components"].get(term, {}).get("penalty") if b else None
                deltas[term] = av - bv if valid and av is not None and bv is not None else None
            pairs.append(dict(seed=seed, delta_P=a["P"] - b["P"] if valid else None, components=deltas, valid=valid))
        consistent = []
        for term in COMPONENTS:
            deltas = [pair["components"][term] for pair in pairs]
            for sign, word in ((-1, "lower"), (1, "higher")):
                count = sum(delta is not None and delta * sign > 0 for delta in deltas)
                if len(pairs) >= cfg["summary"]["consistent_min_seeds"] and count / len(pairs) >= cfg["summary"]["consistent_fraction"]:
                    consistent.append(f"{term} penalty is {word} in {count}/{len(pairs)} paired seeds.")
        result[arm] = dict(baseline=baseline, pairs=pairs, wins=sum(pair["delta_P"] is not None and pair["delta_P"] < 0 for pair in pairs), n=len(pairs), invalid=sum(not pair["valid"] for pair in pairs), consistent=consistent)
    return result


def build_report(out, cfg, baseline=None, start=0.0, stop=math.inf):
    manifest = json.loads((out / "batch.json").read_text())
    states = {}
    if (out / "STATE.tsv").exists():
        with (out / "STATE.tsv").open(newline="") as stream:
            for row in csv.DictReader(stream, delimiter="\t"):
                states[(row["arm"], int(row["seed"]))] = row["state"]
    worlds = [score_world(out / arm / f"s{seed}", arm, seed, cfg, start, stop, states.get((arm, seed))) for arm in manifest["arms"] for seed in manifest["seeds"]]
    arms = {arm: arm_summary([world for world in worlds if world["arm"] == arm], cfg) for arm in manifest["arms"]}
    # Refuse misleading pairs if recorded inputs differ from the common starts.
    if baseline and manifest.get("runs"):
        inputs = {}
        for job in manifest["runs"]:
            common = [arg for arg in job["command"] if arg.startswith(("load=", "seed=", "config="))]
            if job["seed"] in inputs and inputs[job["seed"]] != common:
                raise ValueError(f"seed {job['seed']} has different starts across arms")
            inputs[job["seed"]] = common
    return dict(worlds=worlds, arms=arms, paired=pair_worlds(worlds, baseline, cfg) if baseline else {})


def display(value):
    if value is None:
        return "NA"
    if isinstance(value, (int, float)):
        return f"{value:.6g}"
    if isinstance(value, (dict, list)):
        return json.dumps(value, separators=(",", ":"))
    return str(value)


def records(report):
    for world in report["worlds"]:
        for term, item in world["components"].items():
            yield ("world", world["arm"], world["seed"], term, item["raw"], item["penalty"], item["weight"], item["contribution"])
        yield ("world", world["arm"], world["seed"], "P", world["P"], None, None, world["state"])
        for warning in world["warnings"]:
            yield ("warning", world["arm"], world["seed"], "note", warning, None, None, None)
    for arm, summary in report["arms"].items():
        for metric in ("mean_P", "sd_P", "rank"):
            yield ("arm", arm, "", metric, summary[metric], None, None, None)
        for term, item in summary["components"].items():
            yield ("component", arm, "", term, item["median"], item["min"], item["max"], item["n"])
        for name, health in summary["health"].items():
            for metric, item in health.items():
                if metric == "flag":
                    yield ("health", arm, "", f"{name}:flag", item, None, None, None)
                elif metric == "deaths":
                    for cause, values in item.items():
                        yield ("health", arm, "", f"{name}:deaths:{cause}", values["median"], values["min"], values["max"], values["n"])
                else:
                    yield ("health", arm, "", f"{name}:{metric}", item["median"], item["min"], item["max"], item["n"])
        for name, item in summary["plants"].items():
            yield ("plant", arm, "", name, item["median"], item["min"], item["max"], item["n"])
    for arm, paired in report["paired"].items():
        for pair in paired["pairs"]:
            yield ("pair", arm, pair["seed"], "delta_P", pair["delta_P"], None, None, paired["baseline"])
            for term, value in pair["components"].items():
                yield ("pair", arm, pair["seed"], term, value, None, None, paired["baseline"])
        yield ("pair", arm, "", "wins", paired["wins"], None, None, paired["n"])
        for line in paired["consistent"]:
            yield ("consistent", arm, "", "note", line, None, None, None)


def plain(report):
    print("Per world: component raw / penalty (details retained for each lineage)")
    for world in report["worlds"]:
        print(f"{world['arm']} s{world['seed']} {world['state']}")
        for term, item in world["components"].items():
            print(f"  {term}: {display(item['raw'])} / {display(item['penalty'])} weight={display(item['weight'])}")
            if item.get("lost"):
                print("    lost: " + ", ".join(item["lost"]))
        print(f"  P: {display(world['P'])}" + (" (partial)" if world.get("partial") else ""))
        for warning in world["warnings"]:
            print(f"  {warning}")
    print("Arms: mean P | sd | mean + sd weight | invalid seeds")
    for arm, summary in report["arms"].items():
        print(f"{arm}: {display(summary['mean_P'])} | {display(summary['sd_P'])} | {display(summary['rank'])} | {summary['invalid_seeds']}")
        print("  component: median [min,max] (observed/expected seeds)")
        expected = sum(world["arm"] == arm for world in report["worlds"])
        for term, item in summary["components"].items():
            print(f"  {term}: {display(item['median'])} [{display(item['min'])},{display(item['max'])}] ({item['n']}/{expected})")
    print("Paired: candidate minus baseline (penalties; negative is improvement)")
    for arm, paired in report["paired"].items():
        print(f"{arm} vs {paired['baseline']}: wins {paired['wins']}/{paired['n']}; invalid {paired['invalid']}")
        print("seed\tΔP\t" + "\t".join(COMPONENTS))
        for pair in paired["pairs"]:
            print("\t".join(map(display, [pair["seed"], pair["delta_P"], *pair["components"].values()])))
        for line in paired["consistent"]:
            print(line)
    print("Health: per-arm medians over seeds; condition/activity from second half")
    print("arm lineage alive eggs births deaths_by_cause p10 p50 net_energy/body_hour moving stuck")
    for arm, summary in report["arms"].items():
        for name, health in summary["health"].items():
            values = [health[key]["median"] for key in ("alive_end", "eggs_end", "births")]
            causes = {cause: data["median"] for cause, data in health["deaths"].items() if data["median"]}
            values += [causes] + [health[key]["median"] for key in ("fullness_p10", "fullness_p50", "net_energy_per_body_hour", "moving_share", "stuck_share")]
            print(" ".join([arm, name, *map(display, values)]) + (f" ({health['flag']})" if health["flag"] else ""))
        print(f"{arm} plants alive end: " + ", ".join(f"{name}={display(data['median'])}" for name, data in summary["plants"].items()))
    print("Q: NA (zero contribution); needs search day-cycle inflow and stock-change books.")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--baseline")
    parser.add_argument("--from", dest="start", type=float, default=0.0, metavar="MIN")
    parser.add_argument("--to", dest="stop", type=float, default=math.inf, metavar="MIN")
    formats = parser.add_mutually_exclusive_group()
    formats.add_argument("--tsv", action="store_true")
    formats.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    if not math.isfinite(args.start) or args.start < 0 or math.isnan(args.stop) or args.stop <= args.start:
        parser.error("require finite 0 <= --from < --to")
    try:
        with args.config.open("rb") as stream:
            cfg = tomllib.load(stream)
        validate_config(cfg)
        report = build_report(args.out, cfg, args.baseline, args.start, args.stop)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    if args.json:
        print(json.dumps(report, indent=2, allow_nan=False))
    elif args.tsv:
        writer = csv.writer(sys.stdout, delimiter="\t", lineterminator="\n")
        writer.writerow(("kind", "arm", "seed", "metric", "raw_or_median", "penalty_or_min", "weight_or_max", "contribution_or_n"))
        writer.writerows([[display(value) for value in row] for row in records(report)])
    else:
        plain(report)


if __name__ == "__main__":
    main()
