#!/usr/bin/env python3
"""The founder outcomes seed by seed, paired against the status quo.

    python3 design/7_Research/assets/ecology-v1-precondition-paired.py runs/ecology-v1-precondition

Six seeds is a small sample, so the means in the main table are not enough on their own: the
question is whether every seed moves the same way or one seed carries the mean. Each arm is
compared with the age-0 arm of the *same* (config, seed), which is the same world with the
same founders and the same landscape.
"""

import json
import sys
from pathlib import Path

root = Path(sys.argv[1] if len(sys.argv) > 1 else "runs/ecology-v1-precondition")
compare = [json.loads(l) for l in open(root / "compare" / "compare.jsonl")]
by = {(r["candidate"], r["seed"], r["age"]): r for r in compare}
AGES = sorted({r["age"] for r in compare})
SEEDS = sorted({r["seed"] for r in compare})

MEASURES = [
    ("founder lineages alive", lambda e: e["metrics"]["founder_lineages_alive"], +1),
    ("forms present at horizon", lambda e: e["metrics"]["final_forms_present"], +1),
    ("mean form evenness", lambda e: e["metrics"]["mean_form_evenness"], +1),
    ("final population", lambda e: e["metrics"]["final_population"], +1),
    ("completed broods", lambda e: sum(e["movement"]["founder_broods"]["broods_by_form"]), +1),
    ("crossings (own reference)", lambda e: e["movement"]["plant_budget"]["total_crossings"], -1),
]

for c in ["baseline", "fast-leaf"]:
    print(f"\n### {c}\n")
    print("| measure | age | " + " | ".join(str(s) for s in SEEDS) +
          " | seeds better than age 0 | seeds worse |")
    print("| --- | ---: |" + " ---: |" * (len(SEEDS) + 2))
    for name, get, sign in MEASURES:
        for age in AGES[1:]:
            vals, better, worse = [], 0, 0
            for s in SEEDS:
                a = get(by[(c, s, 0)]["evaluation"])
                b = get(by[(c, s, age)]["evaluation"])
                vals.append(b)
                if sign * (b - a) > 1e-9:
                    better += 1
                elif sign * (b - a) < -1e-9:
                    worse += 1
            base = [get(by[(c, s, 0)]["evaluation"]) for s in SEEDS]
            shown = " | ".join(f"{v:.3g}" for v in vals)
            if age == AGES[1]:
                print(f"| {name} — age 0 | 0 | " + " | ".join(f"{v:.3g}" for v in base) + " | — | — |")
            print(f"| {name} | {age:,} | {shown} | {better} | {worse} |")
