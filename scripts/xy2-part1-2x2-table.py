#!/usr/bin/env python3
"""Workstream XY2, Part 1, Astra's addendum: the 2 x 2 of weights x adapter.

Each seed's two selected centres, each replayed on the same four training layouts under both
adapters. `t_min` is the minimum over the four layouts (the protocol's survival term); the
other three columns are means over the four layouts, the weighting X's replay used.
"""
import json
from pathlib import Path

OUT = Path("runs/ecology-v1-round5-followups/replay")
ARMS = [
    ("20260915", "cub-act-1 weights", "s1-act1"),
    ("20260915", "cub-act-2 weights", "s1-act2"),
    ("20260916", "cub-act-1 weights", "s2-act1"),
    ("20260916", "cub-act-2 weights", "s2-act2"),
]


def centre_rows(label):
    d = json.load(open(OUT / label / "replay.json"))
    out = {}
    for a in ("cub-act-1", "cub-act-2"):
        rs = [r for r in d["rows"] if r["candidate"] == "center" and r["adapter"] == a]
        assert len(rs) == 4, (label, a, len(rs))
        out[a] = dict(
            t_min=min(r["ticks"] for r in rs),
            on_food=sum(r["on_food_fraction"] for r in rs) / 4,
            dwell=sum(r["mean_dwell_bout"] for r in rs) / 4,
            intake=sum(r["intake_producer"] / r["ticks"] for r in rs) / 4,
            turn=sum(r["turn_active_fraction"] for r in rs) / 4,
        )
    return d, out


print("\n| seed | weights | replayed under | `t_min` | on-food fraction | mean dwell bout | "
      "producer intake / tick | turn active fraction |")
print("| --- | --- | --- | --- | --- | --- | --- | --- |")
store = {}
for seed, weights, label in ARMS:
    d, rows = centre_rows(label)
    store[(seed, weights)] = rows
    for a in ("cub-act-1", "cub-act-2"):
        r = rows[a]
        print(f"| {seed} | {weights} | `{a}` | {r['t_min']} | {r['on_food']:.4f} | "
              f"{r['dwell']:.1f} | {r['intake']:.2e} | {r['turn']:.4f} |")

print("\n#### The three contrasts the reading rule names\n")
print("| seed | column | adapter effect on `cub-act-1` weights | adapter effect on `cub-act-2` "
      "weights | weight effect under `cub-act-1` | weight effect under `cub-act-2` |")
print("| --- | --- | --- | --- | --- | --- |")
for seed in ("20260915", "20260916"):
    w1 = store[(seed, "cub-act-1 weights")]
    w2 = store[(seed, "cub-act-2 weights")]
    for col, fmt in [("t_min", "{:+.0f}"), ("on_food", "{:+.4f}"), ("dwell", "{:+.1f}"),
                     ("intake", "{:+.2e}")]:
        print(f"| {seed} | {col} | "
              f"{fmt.format(w1['cub-act-2'][col] - w1['cub-act-1'][col])} | "
              f"{fmt.format(w2['cub-act-2'][col] - w2['cub-act-1'][col])} | "
              f"{fmt.format(w2['cub-act-1'][col] - w1['cub-act-1'][col])} | "
              f"{fmt.format(w2['cub-act-2'][col] - w1['cub-act-2'][col])} |")
