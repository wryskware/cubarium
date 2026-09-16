#!/usr/bin/env python3
"""Founder survival and first broods **by kind**, per arm.

    python3 design/7_Research/assets/ecology-v1-precondition-by-kind.py runs/ecology-v1-precondition

The form order is the pack's creature order (`crates/cubarium-core/src/config.rs`): lantern 0
the grazer, sail 1 the glider, mossback 2 the burrower, skimmer 3. First-brood ticks are
reported relative to the founding, not to world creation, so the arms are on one axis.
"""

import json
import statistics as st
import sys
from pathlib import Path

F = ["lantern", "sail", "mossback", "skimmer"]
root = Path(sys.argv[1] if len(sys.argv) > 1 else "runs/ecology-v1-precondition")
rows = [json.loads(l) for l in open(root / "compare" / "compare.jsonl")]

print("| config | age | founders by form | completed broods by form | "
      "first brood, ticks after founding (seeds with one) | founders that bred, of that form |")
print("| --- | ---: | --- | --- | --- | --- |")
for c in ["baseline", "fast-leaf"]:
    for age in sorted({r["age"] for r in rows}):
        mine = [r["evaluation"]["movement"] for r in rows
                if r["candidate"] == c and r["age"] == age]
        fb = [m["founder_broods"] for m in mine]
        fo = [st.mean(b["founders_by_form"][i] for b in fb) for i in range(4)]
        br = [st.mean(b["broods_by_form"][i] for b in fb) for i in range(4)]
        pa = [st.mean(b["parents_by_form"][i] for b in fb) for i in range(4)]
        first = []
        for i in range(4):
            v = [b["first_brood_tick_by_form"][i] - age for b in fb
                 if b["first_brood_tick_by_form"][i] is not None]
            first.append(f"{F[i]} {st.mean(v):.0f} ({len(v)}/6)" if v else f"{F[i]} none")
        print(f"| {c} | {age:,} | " + ", ".join(f"{F[i]} {fo[i]:.0f}" for i in range(4))
              + " | " + ", ".join(f"{F[i]} {br[i]:.1f}" for i in range(4))
              + " | " + ", ".join(first)
              + " | " + ", ".join(f"{F[i]} {pa[i]:.1f}" for i in range(4)) + " |")
