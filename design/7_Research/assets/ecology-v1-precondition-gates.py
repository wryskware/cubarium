#!/usr/bin/env python3
"""The declared plausibility gates, re-evaluated on each arm.

    python3 design/7_Research/assets/ecology-v1-precondition-gates.py runs/ecology-v1-precondition

The gates are `calibrate::gates_for`'s, applied unchanged: the late-window foliage floor is
half the **opening** foliage, so an arm that opens 3.6x greener is asked for 3.6x more standing
crop by the same rule. Reported here because that is a consequence of adopting a preconditioned
opening, not a property of any one run.
"""

import json
import sys
from pathlib import Path

FOLIAGE_FLOOR = 0.5
ALIVE_CELL_FLOOR = 0.8

root = Path(sys.argv[1] if len(sys.argv) > 1 else "runs/ecology-v1-precondition")
compare = [json.loads(l) for l in open(root / "compare" / "compare.jsonl")]
AGES = sorted({r["age"] for r in compare})

print("| config | age | min retention over 6 seeds | mean | vegetated | stands intact | "
      "guilds intact | turning over | persists | skimmers alive (per seed) |")
print("| --- | ---: | ---: | ---: | :-: | :-: | :-: | :-: | :-: | --- |")
for c in ["baseline", "fast-leaf"]:
    for age in AGES:
        rows = sorted([r for r in compare if r["candidate"] == c and r["age"] == age],
                      key=lambda r: r["seed"])
        ret, veg, stands, guilds, turn, persists, skim = [], True, True, True, True, True, []
        for r in rows:
            m = r["evaluation"]["metrics"]
            late = m["late"]
            opening = max(m["opening_foliage"], 1e-12)
            ret.append(late["foliage_mean"] / opening if late else 0.0)
            if not late or ret[-1] < FOLIAGE_FLOOR:
                veg = False
            if not late or late["alive_cells_final"] < ALIVE_CELL_FLOOR * max(m["opening_alive_cells"], 1):
                stands = False
            if not late or late["guild_final"][0] == 0 or late["guild_final"][1] == 0:
                guilds = False
            if not late or late["prey_births"] == 0 or late["prey_deaths"] == 0:
                turn = False
            if m["final_population"] == 0:
                persists = False
            skim.append(r["evaluation"]["movement"]["census"]["skimmer_alive_final"])
        tick = lambda b: "yes" if b else "**no**"
        print(f"| {c} | {age:,} | {min(ret):.2f} | {sum(ret)/len(ret):.2f} | {tick(veg)} "
              f"| {tick(stands)} | {tick(guilds)} | {tick(turn)} | {tick(persists)} "
              f"| {','.join(str(x) for x in skim)} |")
