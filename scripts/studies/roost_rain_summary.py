#!/usr/bin/env python3
"""Summarise roost_rain_study runs: OUT/<arm>-seed-<n>/summary.json -> OUT/table.md.

Usage: roost_rain_summary.py OUT
"""
import json
import sys
from pathlib import Path

PLACES = ["vaulttree", "ledge_top", "other_crown", "vine_face", "ground", "other"]


def main():
    out = Path(sys.argv[1])
    rows = {}
    for f in sorted(out.glob("*-seed-*/summary.json")):
        s = json.loads(f.read_text())
        rows.setdefault(s["arm"], []).append(s)
    lines = ["# ROOSTS-RAIN before/after", ""]
    lines.append("before = arm with `glider.roost_budget_m=0` and `flight.rain_ground_m_per_s=1` (knobs off).")
    lines.append("")
    lines.append("| arm | seed | night samples: " + " / ".join(PLACES) + " | asleep: vaulttree / ledge_top | "
                 "bellwing airborne rain / dry | bellwing spend per body-s rain / dry | rain min | deaths sp / bw |")
    lines.append("|---|---|---|---|---|---|---|---|")
    for arm in ("before", "after"):
        for s in rows.get(arm, []):
            n = s["seedporter_night_samples"]
            tot = sum(n.values()) or 1
            share = " / ".join(f"{100 * n[p] / tot:.0f}%" for p in PLACES)
            sl = s["seedporter_night_asleep_samples"]
            lines.append(
                f"| {arm} | {s['seed']} | {share} (n={tot}) | {sl['vaulttree']} / {sl['ledge_top']} | "
                f"{s['bellwing_airborne_rain']:.3f} / {s['bellwing_airborne_dry']:.3f} | "
                f"{s['bellwing_spend_per_body_s_rain']:.3e} / {s['bellwing_spend_per_body_s_dry']:.3e} | "
                f"{s['rain_minutes']:.1f} | {s['deaths']['seedporter']} / {s['deaths']['bellwing']} |")
    for arm in ("before", "after"):
        ss = rows.get(arm, [])
        if not ss:
            continue
        night = {p: sum(s["seedporter_night_samples"][p] for s in ss) for p in PLACES}
        tot = sum(night.values()) or 1
        def mean(k):
            v = [s[k] for s in ss if s[k] == s[k]]
            return sum(v) / len(v) if v else float("nan")
        lines.append(
            f"| **{arm} all** | {len(ss)} seeds | " + " / ".join(f"{100 * night[p] / tot:.0f}%" for p in PLACES)
            + f" | | {mean('bellwing_airborne_rain'):.3f} / {mean('bellwing_airborne_dry'):.3f} | "
            f"{mean('bellwing_spend_per_body_s_rain'):.3e} / {mean('bellwing_spend_per_body_s_dry'):.3e} | | "
            f"{sum(s['deaths']['seedporter'] for s in ss)} / {sum(s['deaths']['bellwing'] for s in ss)} |")
    (out / "table.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
