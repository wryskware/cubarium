#!/usr/bin/env python3
"""The depletion count on **one** reference, so the arms can be compared at all.

    python3 design/7_Research/assets/ecology-v1-precondition-common-reference.py \
        runs/ecology-v1-precondition

The crossing counter reads each cell against its own foliage at the moment recording opened,
which is the founding. That is the right reference *inside* one arm and the wrong one
*between* arms: a preconditioned arm opens 3.6x greener, so "lost three quarters of its
opening" is a different quantity there. This re-reads every arm's terminal per-cell foliage
against the same reference — the §11 seeding, taken from the age-0 arm of the same
(config, seed) — and reports the same counts on that common footing.
"""

import json
import statistics as st
import sys
from pathlib import Path

root = Path(sys.argv[1] if len(sys.argv) > 1 else "runs/ecology-v1-precondition")
compare = [json.loads(l) for l in open(root / "compare" / "compare.jsonl")]
by = {(r["candidate"], r["seed"], r["age"]): r for r in compare}
AGES = sorted({r["age"] for r in compare})
SEEDS = sorted({r["seed"] for r in compare})

print("| config | age | cells below 1/4 of the §11 seeding | below 1/2 | "
      "median P_final / P_§11 | ΣP final | ΣP § 11 |")
print("| --- | ---: | ---: | ---: | ---: | ---: | ---: |")
for c in ["baseline", "fast-leaf"]:
    for age in AGES:
        q, h, ratios, sp, s11 = [], [], [], [], []
        for s in SEEDS:
            ref = {x["cell"]: x["p_open"]
                   for x in by[(c, s, 0)]["evaluation"]["movement"]["plant_budget"]["cells"]}
            cells = by[(c, s, age)]["evaluation"]["movement"]["plant_budget"]["cells"]
            watched = [(x["cell"], x["p_final"]) for x in cells if ref.get(x["cell"], 0.0) > 1e-9]
            q.append(sum(1 for cid, pf in watched if pf < 0.25 * ref[cid]))
            h.append(sum(1 for cid, pf in watched if pf < 0.5 * ref[cid]))
            ratios.append(st.median(pf / ref[cid] for cid, pf in watched))
            sp.append(sum(x["p_final"] for x in cells))
            s11.append(sum(ref.values()))
        print(f"| {c} | {age:,} | {st.mean(q):.1f} | {st.mean(h):.1f} | {st.mean(ratios):.2f} "
              f"| {st.mean(sp):.1f} | {st.mean(s11):.1f} |")
