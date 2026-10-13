#!/usr/bin/env python3
"""How much of a live-training score is the variant? (LJ-SIGNAL, stdlib only)

    pair_signal.py RUN_DIR [RUN_DIR...] [--lineage lanternjaw] [--gens A:B]

Reads a `voxel-live-train` run's generations.jsonl. Within one generation a pair p is
often measured in several worlds (placement wraps round the pairs). Each measurement is
the within-world difference of its two signs' raw kin-weighted scores,
d(p, w) = mean s(p+ in w) - mean s(p- in w). The same variants in different worlds share
their variant effect and nothing else, so a one-way random-effects ANOVA of d over pairs
(repeats = worlds, pooled over generations) splits a pair difference's variance into:

- the variant's share (ICC): how much of one measured difference is the perturbation;
- the rest: body and place luck over the window.

Also printed: the correlation of a pair's first two measurements (a second ICC estimate),
the mean of d (a sign or position bias), the between-world share of the raw score
variance (what within-world z-scoring removes), and placement holes (pairs with one or no
sign placed; per_body scores a missing sign 0).

At sigma 0 every variant is the centre, so the ICC must come out ~0: that run is the
frozen-brain noise floor (design/behaviour-ai-2026-10-11.md section 10.3).
"""

import argparse
import json
import math
import statistics as st
from collections import defaultdict
from pathlib import Path


def records(run, lineage, gens):
    for line in (Path(run) / "generations.jsonl").read_text().splitlines():
        if not line.strip():
            continue
        r = json.loads(line)
        if gens and not (gens[0] <= r["generation"] < gens[1]):
            continue
        lu = next(l for l in r["lineages"] if l["lineage"].lower() == lineage)
        worlds = defaultdict(list)
        for b in r["body_scores"]:
            if b["founder"].lower() == lineage:
                worlds[b["world"]].append((b["candidate"], b["kin"]["combined"]))
        yield r["generation"], len(lu["candidate_scores"]) // 2, worlds


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("runs", nargs="+")
    ap.add_argument("--lineage", default="lanternjaw")
    ap.add_argument("--gens", help="A:B, generations A <= g < B")
    a = ap.parse_args(argv)
    gens = tuple(int(x) for x in a.gens.split(":")) if a.gens else None
    for run in a.runs:
        ssb = ssw = 0.0
        dfb = dfw = 0
        k0s, alld, firsts = [], [], []
        between = within = 0.0
        n_gen = bodies = both = one = none = 0
        for g, pairs, worlds in records(run, a.lineage, gens):
            n_gen += 1
            scores = [s for w in worlds.values() for _, s in w]
            bodies += len(scores)
            gm = st.fmean(scores)
            for w in worlds.values():
                m = st.fmean([s for _, s in w])
                between += len(w) * (m - gm) ** 2
                within += sum((s - m) ** 2 for _, s in w)
            placed = defaultdict(int)
            d = defaultdict(list)
            for w in worlds.values():
                by = defaultdict(list)
                for c, s in w:
                    by[c].append(s)
                    placed[c] += 1
                for p in range(pairs):
                    if 2 * p in by and 2 * p + 1 in by:
                        d[p].append(st.fmean(by[2 * p]) - st.fmean(by[2 * p + 1]))
            for p in range(pairs):
                k = (placed[2 * p] > 0) + (placed[2 * p + 1] > 0)
                both += k == 2
                one += k == 1
                none += k == 0
            alld += [x for v in d.values() for x in v]
            rep = {p: v for p, v in d.items() if len(v) >= 2}
            firsts += [(v[0], v[1]) for v in rep.values()]
            if len(rep) < 2:
                continue
            allv = [x for v in rep.values() for x in v]
            mu, n = st.fmean(allv), len(allv)
            ssb += sum(len(v) * (st.fmean(v) - mu) ** 2 for v in rep.values())
            dfb += len(rep) - 1
            ssw += sum(sum((x - st.fmean(v)) ** 2 for x in v) for v in rep.values())
            dfw += sum(len(v) - 1 for v in rep.values())
            k0s.append((n - sum(len(v) ** 2 for v in rep.values()) / n) / (len(rep) - 1))
        print(f"== {run}")
        print(f"generations {n_gen}, bodies/gen {bodies / max(1, n_gen):.1f}; pairs both signs {both}, one sign {one}, none {none}")
        if dfb == 0 or dfw == 0:
            print("no repeated pairs")
            continue
        msb, msw, k0 = ssb / dfb, ssw / dfw, st.fmean(k0s)
        f = msb / msw
        # 95 % interval on F (log-normal approximation of the F ratio's sampling spread).
        spread = 1.96 * math.sqrt(2 / dfb + 2 / dfw)
        def icc(fr):
            s2 = (fr - 1) / k0
            return s2 / (1 + s2)
        x = [p[0] for p in firsts]
        y = [p[1] for p in firsts]
        mx, my = st.fmean(x), st.fmean(y)
        r = sum((u - mx) * (v - my) for u, v in zip(x, y)) / (len(x) * st.pstdev(x) * st.pstdev(y))
        rse = 1 / math.sqrt(max(1, len(x) - 3))
        print(f"pair differences: MS between pairs {msb:.4g} (df {dfb}), within (repeats) {msw:.4g} (df {dfw}), "
              f"repeats/pair k0 {k0:.2f}, F {f:.3f}")
        print(f"variant share of one difference (ICC) {icc(f):.3f}, 95% [{icc(f * math.exp(-spread)):.3f}, "
              f"{icc(f * math.exp(spread)):.3f}]; first-two-repeat r {r:.3f} +- {1.96 * rse:.3f} over {len(x)} pairs")
        print(f"mean d {st.fmean(alld):.3f} +- {1.96 * st.pstdev(alld) / math.sqrt(len(alld)):.3f} over {len(alld)}; "
              f"sd d {st.pstdev(alld):.3f}; raw score variance between worlds {between / bodies:.3f}, within {within / bodies:.3f}")


if __name__ == "__main__":
    main()
