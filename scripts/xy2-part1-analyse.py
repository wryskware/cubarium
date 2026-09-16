#!/usr/bin/env python3
"""Workstream XY2, Part 1: X's training table and held-out table for a seed, and the
registered replication rule applied to them.

Reads only retained artefacts: each arm's `generations.jsonl` and `holdout.json`. The rule is
the one committed in the pre-registration and is applied here, not chosen here.
"""
import json
import math
import sys
from pathlib import Path

RUNS = Path(sys.argv[1] if len(sys.argv) > 1 else "/home/wrysk/wryskware/cubarium/runs")


def sign_test(diffs):
    """Exact two-sided sign test. Zero differences are ties: dropped, n is what remains."""
    pos = sum(1 for d in diffs if d > 0)
    neg = sum(1 for d in diffs if d < 0)
    tie = sum(1 for d in diffs if d == 0)
    n = pos + neg
    if n == 0:
        return pos, neg, tie, 1.0
    k = min(pos, neg)
    tail = sum(math.comb(n, i) for i in range(k + 1)) / 2 ** n
    return pos, neg, tie, min(1.0, 2 * tail)


def favours(diffs):
    """The registered definition: p < 0.05 and strictly more than half the non-zero
    differences positive."""
    pos, neg, tie, p = sign_test(diffs)
    return (p < 0.05) and (pos > neg), (pos, neg, tie, p)


def generations(run):
    out = []
    for line in (RUNS / run / "generations.jsonl").read_text().splitlines():
        if not line.strip():
            continue
        d = json.loads(line)
        cand = [j for j in d["jobs"] if j["candidate"] != "center"]
        e = [j["episode"] for j in cand]
        out.append(
            dict(
                generation=d["generation"],
                center=d["center_score"],
                best=max(d["candidate_scores"]),
                mean=sum(d["candidate_scores"]) / len(d["candidate_scores"]),
                opening=sum(x["ticks_in_opening"] / x["ticks"] for x in e) / len(e),
                intake=sum(x["intake_producer"] / x["ticks"] for x in e) / len(e),
                grad=d.get("gradient_norm"),
                episodes=len(e),
            )
        )
    out.sort(key=lambda r: r["generation"])
    return out


def selected(run):
    """X's selection rule: highest recorded centre score, earliest generation on ties,
    training results only."""
    gs = generations(run)
    best = max(g["center"] for g in gs)
    return min(g["generation"] for g in gs if g["center"] == best), best


def holdout(run):
    h = json.load(open(RUNS / run / "holdout.json"))
    rows = []
    for e in h["episodes"]:
        rows.append(
            dict(
                layout=e["layout"],
                ticks=e["ticks"],
                intake_p=e["intake_producer"],
                opening=e["ticks_in_opening"] / e["ticks"],
                intake_rate=e["intake_producer"] / e["ticks"],
            )
        )
    return h, rows


def table(seed, one, two):
    a, b = generations(one), generations(two)
    assert len(a) == len(b) == 16, (len(a), len(b))
    print(f"\n### Seed {seed}: the training table\n")
    print("| gen | centre 1 | centre 2 | best 1 | best 2 | mean 1 | mean 2 | opening 1 | "
          "opening 2 | intake/tick 1 | intake/tick 2 |")
    print("| --- " * 11 + "|")
    for x, y in zip(a, b):
        print(f"| {x['generation']} | {x['center']:.0f} | {y['center']:.0f} | {x['best']:.0f} | "
              f"{y['best']:.0f} | {x['mean']:.0f} | {y['mean']:.0f} | {x['opening']:.4f} | "
              f"{y['opening']:.4f} | {x['intake']:.2e} | {y['intake']:.2e} |")

    print(f"\n### Seed {seed}: paired by generation, exact two-sided sign tests\n")
    print("| quantity | `cub-act-2` higher | p | read by the rule |")
    print("| --- | --- | --- | --- |")
    results = {}
    for name, key, read in [
        ("mean population score", "mean", "**yes**"),
        ("mean producer intake per lived tick", "intake", "**yes**"),
        ("mean opening residence fraction", "opening", "no"),
        ("best candidate", "best", "no"),
        ("centre score", "center", "no"),
    ]:
        diffs = [y[key] - x[key] for x, y in zip(a, b)]
        ok, (pos, neg, tie, p) = favours(diffs)
        results[key] = (ok, pos, neg, tie, p)
        print(f"| {name} | {pos} / {pos + neg + tie} | {p:.5f} | {read} |")

    print(f"\nmean over the run  score {sum(g['mean'] for g in a)/16:.0f} -> "
          f"{sum(g['mean'] for g in b)/16:.0f}   opening "
          f"{sum(g['opening'] for g in a)/16:.4f} -> {sum(g['opening'] for g in b)/16:.4f}"
          f"   intake {sum(g['intake'] for g in a)/16:.2e} -> {sum(g['intake'] for g in b)/16:.2e}")
    return results


def holdout_table(seed, one, two, gen_one, gen_two):
    h1, r1 = holdout(one)
    h2, r2 = holdout(two)
    print(f"\n### Seed {seed}: held out (control gen {gen_one}, `cub-act-2` gen {gen_two})\n")
    print("| layout | control ticks | `cub-act-2` ticks | d | intake P control | `cub-act-2` |")
    print("| --- | --- | --- | --- | --- | --- |")
    for x, y in zip(r1, r2):
        print(f"| {x['layout']} | {x['ticks']} | {y['ticks']} | {y['ticks'] - x['ticks']:+d} | "
              f"{x['intake_p']:.3f} | {y['intake_p']:.3f} |")
    m1 = min(x["ticks"] for x in r1)
    m2 = min(y["ticks"] for y in r2)
    higher = sum(1 for x, y in zip(r1, r2) if y["ticks"] > x["ticks"])
    print(f"\nminimum {m1} -> {m2}   mean {sum(x['ticks'] for x in r1)/8:.0f} -> "
          f"{sum(y['ticks'] for y in r2)/8:.0f}   higher on {higher} of 8")
    print(f"mean opening residence {sum(x['opening'] for x in r1)/8:.4f} -> "
          f"{sum(y['opening'] for y in r2)/8:.4f}   mean intake/tick "
          f"{sum(x['intake_rate'] for x in r1)/8:.2e} -> {sum(y['intake_rate'] for y in r2)/8:.2e}")
    print(f"digests  control {h1['policy_digest']}  cub-act-2 {h2['policy_digest']}")
    return m1, m2


def verdict(seed, res, m1, m2):
    score_ok = res["mean"][0]
    intake_ok = res["intake"][0]
    held_ok = m2 > m1
    if score_ok and intake_ok and held_ok:
        v = "REPLICATED"
    elif not (score_ok and intake_ok):
        v = "NOT REPLICATED"
    else:
        v = "MIXED"
    print(f"\n**Seed {seed} verdict by the registered rule: {v}** "
          f"(score favours={score_ok}, intake favours={intake_ok}, held-out min {m1} -> {m2}, "
          f"higher={held_ok})")
    return v


if __name__ == "__main__":
    for seed, one, two in [
        (20260915, "es-eco-v1-fastleaf", "es-eco-v1-fastleaf-act2"),
        (20260916, "es-eco-v1-fastleaf-s2", "es-eco-v1-fastleaf-act2-s2"),
    ]:
        if not (RUNS / two / "generations.jsonl").exists():
            print(f"\n(seed {seed}: {two} not present)")
            continue
        g1, s1 = selected(one)
        g2, s2 = selected(two)
        print(f"\n## Seed {seed}   selected: control gen {g1} ({s1:.0f}), "
              f"cub-act-2 gen {g2} ({s2:.0f})")
        res = table(seed, one, two)
        if (RUNS / two / "holdout.json").exists() and (RUNS / one / "holdout.json").exists():
            m1, m2 = holdout_table(seed, one, two, g1, g2)
            verdict(seed, res, m1, m2)
        else:
            print("(held-out evaluations not yet run)")
