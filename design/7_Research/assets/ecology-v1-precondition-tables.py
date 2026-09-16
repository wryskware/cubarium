#!/usr/bin/env python3
"""Workstream S's tables, read straight off the retained rows.

    python3 design/7_Research/assets/ecology-v1-precondition-tables.py \
        runs/ecology-v1-precondition

Nothing here recomputes ecology: every number is a field of `field.jsonl` or of
`compare.jsonl` as the campaign wrote it, aggregated over the six training seeds.
"""

import json
import statistics as st
import sys
from pathlib import Path

root = Path(sys.argv[1] if len(sys.argv) > 1 else "runs/ecology-v1-precondition")
field = [json.loads(l) for l in open(root / "field" / "field.jsonl")]
compare = [json.loads(l) for l in open(root / "compare" / "compare.jsonl")]

CONFIGS = ["baseline", "fast-leaf"]
AGES = sorted({r["age"] for r in compare})
FORMS = ["lantern", "sail", "mossback", "skimmer", "(4)"]


def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else float("nan")


def reading_at(run, tick):
    return next(r for r in run["readings"] if r["tick"] == tick)


# ------------------------------------------------------------------ the operator

print("## 1. How settled the plant-only field is at each age (6,000-tick window)\n")
print("| config | age | ΣP | ΔΣP/ΣP | per-cell P | cells moving | ΔΣW/ΣW | ΔΣQ/ΣQ | "
      "ΔΣN/ΣN | income | crossings in window | cumulative |")
print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
for c in CONFIGS:
    runs = [r for r in field if r["candidate"] == c]
    for age in AGES:
        rs = [reading_at(r, age) for r in runs]
        print(
            f"| {c} | {age:,} | {mean([x['foliage']['total'] for x in rs]):.1f} "
            f"| {mean([x['foliage']['relative_total'] for x in rs]):+.4f} "
            f"| {mean([x['foliage']['cell_change_rate'] for x in rs]):.4f} "
            f"| {mean([x['foliage']['cells_moving'] for x in rs]):.0f} "
            f"| {mean([x['wood']['relative_total'] for x in rs]):+.4f} "
            f"| {mean([x['plant_reserve']['relative_total'] for x in rs]):+.4f} "
            f"| {mean([x['nutrient']['relative_total'] for x in rs]):+.4f} "
            f"| {mean([x['income'] for x in rs]):.3f} "
            f"| {mean([x['crossings_in_window'] for x in rs]):.1f} "
            f"| {mean([x['crossings_total'] for x in rs]):.1f} |"
        )

print("\n## 2. The opening a founding at each age meets (watched cells)\n")
print("| config | age | ΣP | median | p10 | p90 | CV | alive cells | already depleted |")
print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
for c in CONFIGS:
    runs = [r for r in field if r["candidate"] == c]
    for age in AGES:
        rs = [reading_at(r, age) for r in runs]
        print(
            f"| {c} | {age:,} | {mean([x['foliage']['total'] for x in rs]):.1f} "
            f"| {mean([x['foliage_median'] for x in rs]):.4f} "
            f"| {mean([x['foliage_p10'] for x in rs]):.4f} "
            f"| {mean([x['foliage_p90'] for x in rs]):.4f} "
            f"| {mean([x['foliage_cv'] for x in rs]):.3f} "
            f"| {mean([x['alive_cells'] for x in rs]):.0f} "
            f"| {mean([x['depleted_now'] for x in rs]):.1f} |"
        )

print("\n## 3. Withdrawal in the plant-only run (must be exactly zero)\n")
worst = max(abs(x["withdrawal"]) for r in field for x in r["readings"])
resid = max(abs(r["max_identity_residual"]) for r in field)
print(f"- largest exact consumer withdrawal in any window of any plant-only run: `{worst}`")
print(f"- worst per-cell plant identity residual at the horizon: `{resid:.2e}` (acceptance 1e-9)")
print(f"- crossings at tick 180,000, per config: "
      f"baseline {sum(reading_at(r, 180000)['crossings_total'] for r in field if r['candidate']=='baseline')}, "
      f"fast-leaf {sum(reading_at(r, 180000)['crossings_total'] for r in field if r['candidate']=='fast-leaf')}")

# ------------------------------------------------------------------ the arms

print("\n## 4. The comparison arms\n")
print("| config | age | opening ΣP | crossings | with withdrawal | without | ever visited | "
      "founders that bred (of 24) | broods | final pop | forms | "
      "late foliage / opening | late alive cells |")
print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")

summary = {}
for c in CONFIGS:
    for age in AGES:
        rows = [r for r in compare if r["candidate"] == c and r["age"] == age]
        assert len(rows) == 6, (c, age, len(rows))
        ms = [r["evaluation"]["metrics"] for r in rows]
        mv = [r["evaluation"]["movement"] for r in rows]
        pb = [m["plant_budget"] for m in mv]
        fb = [m["founder_broods"] for m in mv]
        # founder survival: lineages alive is the founders' own, so read the census instead
        founders = mean([sum(b["founders_by_form"]) for b in fb])
        bred = mean([sum(b["parents_by_form"]) for b in fb])
        broods = mean([sum(b["broods_by_form"]) for b in fb])
        firsts = [
            min((t - age for t in b["first_brood_tick_by_form"] if t is not None), default=None)
            for b in fb
        ]
        retention = mean([
            (m["late"]["foliage_mean"] / max(m["opening_foliage"], 1e-12)) if m["late"] else 0.0
            for m in ms
        ])
        alive = mean([m["late"]["alive_cells_final"] if m["late"] else 0 for m in ms])
        row = dict(
            opening=mean([m["opening_foliage"] for m in ms]),
            crossings=mean([p["total_crossings"] for p in pb]),
            withw=sum(p["with_withdrawal"] for p in pb),
            without=sum(p["without_withdrawal"] for p in pb),
            visited=sum(p["ever_visited"] for p in pb),
            founders=founders,
            bred=bred,
            first=mean(firsts),
            broods=broods,
            pop=mean([m["final_population"] for m in ms]),
            forms=mean([m["final_forms_present"] for m in ms]),
            retention=retention,
            alive=alive,
            lineages=mean([m["founder_lineages_alive"] for m in ms]),
            starv=mean([m["deaths_starvation"] for m in ms]),
            prey_births=mean([m["prey_births"] for m in ms]),
            final_prod=mean([m["final_producer"] for m in ms]),
            mean_prod=mean([m["mean_producer"] for m in ms]),
            min_prod=mean([m["min_producer"] for m in ms]),
            skimmer=mean([m["census"]["skimmer_alive_final"] for m in mv]),
            evenness=mean([m["mean_form_evenness"] for m in ms]),
        )
        summary[(c, age)] = row
        print(
            f"| {c} | {age:,} | {row['opening']:.1f} | {row['crossings']:.1f} | {row['withw']} "
            f"| {row['without']} | {row['visited']} | {row['bred']:.1f} "
            f"| {row['broods']:.1f} | {row['pop']:.1f} | {row['forms']:.1f} "
            f"| {row['retention']:.2f} | {row['alive']:.0f} |"
        )

print("\n## 5. Founder outcomes and variety at the horizon\n")
print("| config | age | founder lineages alive | prey births | starvation deaths | "
      "skimmers alive | form evenness | ΣP final | ΣP mean | ΣP min |")
print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
for c in CONFIGS:
    for age in AGES:
        r = summary[(c, age)]
        print(
            f"| {c} | {age:,} | {r['lineages']:.1f} | {r['prey_births']:.0f} | {r['starv']:.0f} "
            f"| {r['skimmer']:.1f} | {r['evenness']:.3f} | {r['final_prod']:.1f} "
            f"| {r['mean_prod']:.1f} | {r['min_prod']:.1f} |"
        )

print("\n## 6. The first simulated hour after founding (mean over six seeds)\n")
print("| config | age | ΣP at 0 | 6k | 18k | 36k | 72k | ΣP min in the hour | at tick | "
      "withdrawal over the hour | plant net |")
print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
for c in CONFIGS:
    for age in AGES:
        rows = [r for r in compare if r["candidate"] == c and r["age"] == age]
        series = [r["evaluation"]["opening"] for r in rows]

        def at(t):
            return mean([next(p["foliage"] for p in s if p["ticks_since_founding"] == t)
                         for s in series])

        mins = [min(s, key=lambda p: p["foliage"]) for s in series]
        last = [s[-1] for s in series]
        print(
            f"| {c} | {age:,} | {at(0):.1f} | {at(6000):.1f} | {at(18000):.1f} | {at(36000):.1f} "
            f"| {at(72000):.1f} | {mean([m['foliage'] for m in mins]):.1f} "
            f"| {mean([m['ticks_since_founding'] for m in mins]):.0f} "
            f"| {mean([p['withdrawal'] for p in last]):.2f} "
            f"| {mean([p['foliage_in'] - p['foliage_out'] for p in last]):+.1f} |"
        )

print("\n## 7. Per-seed crossings, so the spread is visible\n")
print("| config | age | " + " | ".join(str(s) for s in sorted({r['seed'] for r in compare})) + " | total |")
print("| --- | ---: |" + " ---: |" * 7)
for c in CONFIGS:
    for age in AGES:
        rows = sorted([r for r in compare if r["candidate"] == c and r["age"] == age],
                      key=lambda r: r["seed"])
        v = [r["evaluation"]["movement"]["plant_budget"]["total_crossings"] for r in rows]
        print(f"| {c} | {age:,} | " + " | ".join(str(x) for x in v) + f" | {sum(v)} |")
