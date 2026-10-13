#!/usr/bin/env python3
"""What do a lanternjaw's twins differ in? (LJ-TWIN, stdlib only)

    twin_signal.py DIR [DIR...]      # lj_twin's out dirs (pairs.jsonl)

Every row of pairs.jsonl is one matched pair: one live start run twice, identical but
for the variant sign (`all`: every lanternjaw on +eps_p / -eps_p; `focal`: one; `null`:
the same candidate in both copies, the determinism floor). Per (sigma, mode):

- divergence: when the twins first differ, in seconds into the window: the variant
  bodies' proposals (the top's choice before the hold), their held intents, their poses,
  and any animal's pose ("world"); `never` counts the pairs with no difference at all.
  `prop_pre` is the share of the variant bodies' decision samples, taken while the world
  was still identical (same inputs in both copies), whose proposal differs.
- behaviour: per variant body, the total-variation distance between the twins' held-intent
  shares (time-weighted, 0..1) and |switches(+) - switches(-)|.
- outcome: d = score(+) - score(-), the window's energy score (as training ranks it), for
  the variant bodies (focal) and for the other lanternjaws (focal mode); all-sign takes the
  mean over the lanternjaws per pair.
- consistency: a one-way random-effects ANOVA of d over the variant pairs p, replicates =
  starts: ICC = the share of one d that is the variant (what per_body training needs),
  with a 95 % interval (log-normal F approximation, as pair_signal.py).
- meals: the share of variant body-windows that swallowed anything (> 0.05 body
  references) and a meal (> 0.5), and of pairs whose twins differ in having a meal.
- window length: sd(d) and ICC from the per-minute scores at 2, 5 and the full window.
"""

import json
import math
import statistics as st
import sys
from collections import defaultdict
from pathlib import Path


def icc(groups):
    """groups: {p: [d over replicates]} -> (icc, lo, hi, k0, n_groups)."""
    rep = {p: v for p, v in groups.items() if len(v) >= 2}
    if len(rep) < 2:
        return None
    allv = [x for v in rep.values() for x in v]
    mu, n = st.fmean(allv), len(allv)
    ssb = sum(len(v) * (st.fmean(v) - mu) ** 2 for v in rep.values())
    ssw = sum(sum((x - st.fmean(v)) ** 2 for x in v) for v in rep.values())
    dfb, dfw = len(rep) - 1, sum(len(v) - 1 for v in rep.values())
    if dfw == 0 or ssw == 0:
        return None
    k0 = (n - sum(len(v) ** 2 for v in rep.values()) / n) / dfb
    f = (ssb / dfb) / (ssw / dfw)
    spread = 1.96 * math.sqrt(2 / dfb + 2 / dfw)

    def to(fr):
        s2 = (fr - 1) / k0
        return s2 / (1 + s2)

    return to(f), to(f * math.exp(-spread)), to(f * math.exp(spread)), k0, len(rep)


def q(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(p * len(xs)))] if xs else float("nan")


def fmt_icc(r):
    return "n/a" if r is None else f"{r[0]:+.3f} [{r[1]:+.3f},{r[2]:+.3f}] ({r[4]} pairs x {r[3]:.1f})"


def tv(a, b):
    return 0.5 * sum(abs(x - y) for x, y in zip(a, b))


def main(dirs):
    rows = []
    for d in dirs:
        for line in (Path(d) / "pairs.jsonl").read_text().splitlines():
            if line.strip():
                rows.append(json.loads(line))
    by = defaultdict(list)
    for r in rows:
        by[(r["mode"], r["sigma"])].append(r)
    for (mode, sigma), rs in sorted(by.items(), key=lambda kv: (kv[0][0] != "null", kv[0][1], kv[0][0])):
        var = [b for r in rs for b in r["bodies"] if b["on_variant"]]
        oth = [b for r in rs for b in r["bodies"] if not b["on_variant"]]
        print(f"== {mode} sigma {sigma}: {len(rs)} pairs, {len(var)} variant body-windows, starts {len({r['start'] for r in rs})}")
        if mode == "null":
            same = sum(1 for r in rs if r["first_world_diff_s"] is None and all(b["score"][0] == b["score"][1] for b in r["bodies"]) and r["max_score_gap_other_lineages"] == 0)
            print(f"  bit-identical twins {same}/{len(rs)}; max |d| {max(abs(b['score'][0] - b['score'][1]) for b in var):.3g}")
            continue
        never = lambda k, xs: sum(1 for x in xs if x[k] is None)
        for k in ("first_proposal_diff_s", "first_held_diff_s", "first_pose_diff_s"):
            got = [b[k] for b in var if b[k] is not None]
            print(f"  {k[6:-7]:>13}: never {never(k, var)}/{len(var)}; seconds q25 {q(got, .25):.1f} median {q(got, .5):.1f} q75 {q(got, .75):.1f}")
        w = [r["first_world_diff_s"] for r in rs if r["first_world_diff_s"] is not None]
        print(f"          world: never {sum(1 for r in rs if r['first_world_diff_s'] is None)}/{len(rs)}; seconds q25 {q(w, .25):.1f} median {q(w, .5):.1f} q75 {q(w, .75):.1f}")
        pre = [sum(b["proposal_diff_pre"][0] for b in var), sum(b["proposal_diff_pre"][1] for b in var)]
        print(f"  prop_pre: {pre[0]}/{pre[1]} samples ({pre[0] / max(1, pre[1]):.3f}) of identical-input samples propose differently")
        tvs = [tv(*b["shares"]) for b in var]
        sw = [abs(b["switches"][0] - b["switches"][1]) for b in var]
        msw = st.fmean([x for b in var for x in b["switches"]])
        mean_shares = [st.fmean([(b["shares"][0][i] + b["shares"][1][i]) / 2 for b in var]) for i in range(len(var[0]["shares"][0]))]
        print(f"  intent shares TV(+,-): mean {st.fmean(tvs):.3f} median {q(tvs, .5):.3f} q90 {q(tvs, .9):.3f}; switches mean {msw:.1f} |diff| mean {st.fmean(sw):.2f}")
        print(f"  mean held shares (ambush patrol feed give_way flee rest ...): {' '.join(f'{s:.3f}' for s in mean_shares)}")
        # Is the behaviour change the variant's (the same direction on every start) or chaos?
        names = ["ambush", "patrol", "feed", "rest"]
        idx = {"ambush": 0, "patrol": 1, "feed": 2, "rest": 5}
        parts = []
        for nm in names + ["switches"]:
            g = defaultdict(list)
            for r in rs:
                vb = [b for b in r["bodies"] if b["on_variant"]]
                if nm == "switches":
                    g[r["pair"]].append(st.fmean(b["switches"][0] - b["switches"][1] for b in vb))
                else:
                    g[r["pair"]].append(st.fmean(b["shares"][0][idx[nm]] - b["shares"][1][idx[nm]] for b in vb))
            parts.append(f"{nm} {fmt_icc(icc(g)).split(' (')[0]}")
        print(f"  behaviour ICC over starts (+ minus -): {'; '.join(parts)}")
        dv = [b["score"][0] - b["score"][1] for b in var]
        print(f"  d variant bodies: sd {st.pstdev(dv):.3f} mean |d| {st.fmean(abs(x) for x in dv):.3f} share d==0 {sum(1 for x in dv if x == 0) / len(dv):.3f}")
        if oth:
            do = [b["score"][0] - b["score"][1] for b in oth]
            print(f"  d other lanternjaws: sd {st.pstdev(do):.3f} share d==0 {sum(1 for x in do if x == 0) / len(do):.3f}")
        # consistency over starts
        groups = defaultdict(list)
        for r in rs:
            ds = [b["score"][0] - b["score"][1] for b in r["bodies"] if b["on_variant"]]
            groups[r["pair"]].append(st.fmean(ds))
        flat = [x for v in groups.values() for x in v]
        print(f"  consistency ICC over starts: {fmt_icc(icc(groups))}; sd d(pair) {st.pstdev(flat):.3f}, mean d {st.fmean(flat):+.3f}")
        # meals
        any_ = sum(1 for b in var for s in b["swallowed"] if s > 0.05) / (2 * len(var))
        meal = sum(1 for b in var for s in b["swallowed"] if s > 0.5) / (2 * len(var))
        flip = [b for b in var if (b["swallowed"][0] > 0.5) != (b["swallowed"][1] > 0.5)]
        ssd = sum(x * x for x in dv)
        ssf = sum((b["score"][0] - b["score"][1]) ** 2 for b in flip)
        print(f"  meals: swallowed>0.05 {any_:.3f}, >0.5 {meal:.3f} of variant body-windows; twins differ in a meal {len(flip)}/{len(var)}, carrying {ssf / max(ssd, 1e-300):.3f} of sum d^2")
        # window length
        # The per-minute scores; a body-window whose last minute disagrees with its window
        # score (lj_twin before it counted births' `given`) is left out and counted.
        ok = lambda b: all(len(b[k][j]) and abs(b[k][j][-1] - b["score"][j]) < 0.05 for k in ("score_by_min",) for j in (0, 1))
        bad = sum(1 for b in var if not ok(b))
        if bad:
            print(f"  per-minute: {bad}/{len(var)} variant body-windows left out (last minute != window score)")
        mins = len(var[0]["score_by_min"][0])
        for t in sorted({min(2, mins), min(5, mins), mins}):
            if t < 1:
                continue
            for label, key in (("energy", "score_by_min"), ("stomach-aware", "stomach_score_by_min")):
                g = defaultdict(list)
                for r in rs:
                    ds = [b[key][0][t - 1] - b[key][1][t - 1] for b in r["bodies"] if b["on_variant"] and ok(b) and len(b[key][0]) >= t and len(b[key][1]) >= t]
                    if ds:
                        g[r["pair"]].append(st.fmean(ds))
                fl = [x for v in g.values() for x in v]
                print(f"  first {t:>2} min {label:>13}: sd d {st.pstdev(fl):.3f}, ICC {fmt_icc(icc(g))}")


if __name__ == "__main__":
    main(sys.argv[1:])
