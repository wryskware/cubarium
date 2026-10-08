#!/usr/bin/env python3
"""Brain-pilot readout of a `voxel-live-train` run's generations.jsonl (stdlib only).

    noise.py RUN_DIR [--curiosity-out FILE]

Per lineage and generation:
- bodies per window (and newborns), candidates placed, pairs with both signs placed;
- the paired score differences d_p = s(p+) - s(p-) of the pooled within-world z scores
  (the trainer's own candidate_scores): sd, mean |d|. At sigma 0 every variant is the
  centre, so this is placement noise; at sigma > 0 compare it with the sigma-0 run;
- sd(novelty) and sd(E), E = delta + given (the energy term), both within-world sds over
  the scored bodies, averaged over worlds and windows (the 2026-10-02 calibration method);
- Spearman rank correlation between delta_body_normalized and
  delta_stomach_aware_normalized over body-windows (and the mean of within-window ones).

With --curiosity-out, writes the recalibrated curiosity TOML from
`weight * sd(N) = share * sd(E)`, shares frondgrazer 0.5, littershredder 0.3
(design/training-plan-2026-10-08.md section 2a).
"""

import argparse
import json
import math
import statistics
import sys
from collections import defaultdict
from pathlib import Path

NAMES = {"browser": "frondgrazer", "blind": "littershredder"}
SHARES = {"frondgrazer": 0.5, "littershredder": 0.3}


def lineage(name):
    n = str(name).lower()
    return NAMES.get(n, n)


def ranks(xs):
    order = sorted(range(len(xs)), key=lambda i: xs[i])
    r = [0.0] * len(xs)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and xs[order[j + 1]] == xs[order[i]]:
            j += 1
        for k in range(i, j + 1):
            r[order[k]] = (i + j) / 2.0
        i = j + 1
    return r


def spearman(a, b):
    if len(a) < 3:
        return math.nan
    ra, rb = ranks(a), ranks(b)
    ma, mb = statistics.fmean(ra), statistics.fmean(rb)
    num = sum((x - ma) * (y - mb) for x, y in zip(ra, rb))
    den = math.sqrt(sum((x - ma) ** 2 for x in ra) * sum((y - mb) ** 2 for y in rb))
    return num / den if den > 0 else math.nan


def psd(xs):
    return statistics.pstdev(xs) if len(xs) >= 2 else math.nan


def fmt(x):
    return "nan" if x is None or (isinstance(x, float) and math.isnan(x)) else f"{x:.4g}"


def mean_finite(xs):
    xs = [x for x in xs if not math.isnan(x)]
    return statistics.fmean(xs) if xs else math.nan


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run", type=Path)
    ap.add_argument("--curiosity-out", type=Path)
    args = ap.parse_args(argv)
    records = [json.loads(l) for l in (args.run / "generations.jsonl").read_text().splitlines() if l.strip()]
    if not records:
        sys.exit("no generations logged")

    sd_e = defaultdict(list)
    sd_n = defaultdict(list)
    all_body = defaultdict(lambda: ([], []))
    within_rho = defaultdict(list)
    all_d = defaultdict(list)
    print("gen\tlineage\tbodies\tnewborn\tbodies_per_world\tplaced\tpairs_both\tpairs\tsd_d\tmean_abs_d\tmean_raw")
    for rec in records:
        g = rec["generation"]
        by_world = defaultdict(lambda: defaultdict(list))
        per_cand = defaultdict(lambda: defaultdict(int))
        for b in rec["body_scores"]:
            lin = lineage(b["founder"])
            by_world[lin][b["world"]].append(b)
            per_cand[lin][b["candidate"]] += 1
        for lu in rec["lineages"]:
            lin = lineage(lu["lineage"])
            cs = lu["candidate_scores"]
            pairs = len(cs) // 2
            both = [p for p in range(pairs) if per_cand[lin][2 * p] > 0 and per_cand[lin][2 * p + 1] > 0]
            d = [cs[2 * p] - cs[2 * p + 1] for p in both]
            all_d[lin].extend(d)
            worlds = by_world[lin]
            bpw = lu["bodies"] / max(1, len({b["world"] for b in rec["body_scores"]}))
            print("\t".join(map(str, [g, lin, lu["bodies"], lu["newborn_bodies"], fmt(bpw), lu["candidates_placed"],
                                      len(both), pairs, fmt(psd(d)), fmt(statistics.fmean(map(abs, d)) if d else math.nan),
                                      fmt(lu["mean_raw_score"])])))
            for bodies in worlds.values():
                e = [b["score"]["delta_normalized"] + b["score"]["given_normalized"] for b in bodies]
                n = [b["score"].get("novelty", 0.0) for b in bodies]
                if len(bodies) >= 2:
                    sd_e[lin].append(psd(e))
                    sd_n[lin].append(psd(n))
                x = [b["delta_body_normalized"] for b in bodies]
                y = [b["delta_stomach_aware_normalized"] for b in bodies]
                all_body[lin][0].extend(x)
                all_body[lin][1].extend(y)
                within_rho[lin].append(spearman(x, y))

    print()
    print("lineage\tpaired_d_sd_all\tn_pairs\tsdE\tsdN\tshare\tweight\trho_pooled\trho_within_mean\tbody_windows")
    weights = {}
    for lin in sorted(set(sd_e) | set(all_d)):
        se, sn = mean_finite(sd_e[lin]), mean_finite(sd_n[lin])
        share = SHARES.get(lin)
        w = share * se / sn if share is not None and sn and not math.isnan(sn) and sn > 0 and not math.isnan(se) else math.nan
        weights[lin] = (w, se, sn, share)
        x, y = all_body[lin]
        print("\t".join(map(str, [lin, fmt(psd(all_d[lin])), len(all_d[lin]), fmt(se), fmt(sn), share, fmt(w),
                                  fmt(spearman(x, y)), fmt(mean_finite(within_rho[lin])), len(x)])))

    if args.curiosity_out:
        lines = [
            "# Brain curiosity recalibrated 2026-10-08 (brain pilot, part 1): weight x sd(N) = share x sd(E),",
            "# E = delta + given, both within-world sds over the scored bodies, averaged over the windows of",
            f"# {args.run} (train-flat-nopred, C1 centres, sigma 0, curiosity weight 0).",
            "# Shares: frondgrazer 0.5, littershredder 0.3 (design/training-plan-2026-10-08.md 2a).",
            "",
        ]
        for lin in ("frondgrazer", "littershredder"):
            w, se, sn, share = weights.get(lin, (math.nan, math.nan, math.nan, SHARES[lin]))
            if math.isnan(w):
                sys.exit(f"cannot calibrate {lin}: sdE {se} sdN {sn}")
            lines += [f"#   {lin}: sdE {se:.4g} sdN {sn:.4g} share {share} -> {w:.4g}",
                      f"[{lin}]", f"weight = {w:.6g}", "memory_min = 30.0", "cell_bl = 1.0", ""]
        lines += ["[lanternjaw]", "weight = 0.0", "memory_min = 30.0", "cell_bl = 1.0", ""]
        args.curiosity_out.write_text("\n".join(lines))
        print(f"\nwrote {args.curiosity_out}")


if __name__ == "__main__":
    main()
