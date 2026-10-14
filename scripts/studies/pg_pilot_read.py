#!/usr/bin/env python3
"""PG-PILOT reader: the policy-gradient run beside the ES all-sign twins run.

    scripts/studies/pg_pilot_read.py --pg <pg run dir> --es <es run dir> [--every N]

A run dir is the trainer's `--out` (it holds generations.jsonl and checks.jsonl).

Prints, per run:
- PG, per generation: decisions, bodies, mean return, the split-half cosine over worlds and
  over bodies beside its shuffled-advantage floor, the cosine with the previous generation,
  explained variance, entropy, KL, clip fraction, share sampled off the argmax, and the
  lanternjaw's live movement (moving_share, pace) -- the seedporter's "sit still" failure
  shows as movement collapsing;
- ES, per generation: the twins' mean d, |d|, zero share, the live movement; its split-half
  cosine comes from `step_noise` (the line to run is printed);
- both: every check (24 matched seeds): gain per body brain vs rules and paired, deaths,
  switches per body-minute brain vs rules, scene agreement brain/rules, the diet.
"""
import argparse
import json
import math
import os
import statistics


def lines(path):
    if not os.path.exists(path):
        return []
    with open(path) as f:
        return [json.loads(x) for x in f if x.strip()]


def readout(rec, name):
    for k, v in rec.get("readout", []):
        if k == name:
            return v
    return float("nan")


def mean_se(xs):
    xs = [x for x in xs if x is not None and not (isinstance(x, float) and math.isnan(x))]
    if not xs:
        return float("nan"), float("nan")
    m = statistics.fmean(xs)
    se = statistics.stdev(xs) / math.sqrt(len(xs)) if len(xs) > 1 else float("nan")
    return m, se


def pg_table(run, every):
    gens = lines(os.path.join(run, "generations.jsonl"))
    print(f"## PG {run}: {len(gens)} generations")
    print("gen  wall_s  dec    bodies ret      splitW  nullW   splitB  nullB   next    ev     H      kl      clip  off   mov   pace")
    keep = {k: [] for k in ("split_worlds", "split_worlds_null", "split_bodies", "split_bodies_null", "next")}
    for g in gens:
        l = next((x for x in g["lineages"] if x.get("pg")), None)
        if l is None:
            continue
        p = {k: (float("nan") if v is None else v) for k, v in l["pg"].items()}
        for k in keep:
            keep[k].append(p[k])
        if g["generation"] % every == 0 or g is gens[-1]:
            print(f"{g['generation']:<4} {g['wall_s']:6.1f}  {p['decisions']:<6} {p['bodies']:<6} {p['mean_return']:+.4f}  "
                  f"{p['split_worlds']:+.3f}  {p['split_worlds_null']:+.3f}  {p['split_bodies']:+.3f}  {p['split_bodies_null']:+.3f}  "
                  f"{p['next']:+.3f}  {p['explained_variance']:+.2f}  {p['entropy']:.3f}  {p['approx_kl']:.4f}  "
                  f"{p['clip_fraction']:.2f}  {p['off_argmax']:.2f}  {readout(g, 'lanternjaw.moving_share'):.2f}  "
                  f"{readout(g, 'lanternjaw.pace_bl_per_s'):.3f}")
    for k, v in keep.items():
        m, se = mean_se(v)
        print(f"mean {k}: {m:+.4f} +- {se:.4f} over {len(v)} generations")


def es_table(run, every):
    gens = lines(os.path.join(run, "generations.jsonl"))
    print(f"## ES {run}: {len(gens)} generations")
    print("gen  wall_s  pairs  d        |d|     zero  mov   pace")
    for g in gens:
        l = next((x for x in g["lineages"] if x.get("twins")), None)
        if l is None:
            continue
        t = {k: (float("nan") if v is None else v) for k, v in l["twins"].items()}
        if g["generation"] % every == 0 or g is gens[-1]:
            print(f"{g['generation']:<4} {g['wall_s']:6.1f}  {t['pairs_scored']:<5}  {t['mean_d']:+.4f}  {t['mean_abs_d']:.4f}  "
                  f"{t['share_d_zero']:.2f}  {readout(g, 'lanternjaw.moving_share'):.2f}  {readout(g, 'lanternjaw.pace_bl_per_s'):.3f}")
    print("split-half cosine (the same yardstick): "
          f"target/release/examples/step_noise run={run} "
          "start=/home/wrysk/wryskware/cubarium/runs/lab/1012-154001-brest-lj-clone-then-smok-fb68/run/clone/lanternjaw-two-layer.json")


def diet_text(d):
    d = sorted(d or [], key=lambda x: -x["eaten_share"])
    return ", ".join(f"{x['food']} e{x['eaten_share']:.2f} k{x['kill_share']:.2f}" for x in d[:5])


def checks(run, label):
    cs = lines(os.path.join(run, "checks.jsonl"))
    print(f"## {label} checks {run}: {len(cs)}")
    for c in cs:
        scenes = ", ".join(f"{s['intent']} {s['brain']:.2f}/{s['heuristic']:.2f}" for s in c.get("scenes", []))
        print(f"gen {c['generation']:<4} seeds {len(c['brain'])}: gain/body brain {c['gain_per_body_brain']:+.5f} "
              f"rules {c['gain_per_body_heuristic']:+.5f} paired {c['gain_per_body_brain_minus_heuristic']:+.5f}; "
              f"deaths {c['deaths_brain']}/{c['deaths_heuristic']}; switches/body-min "
              f"{c['holds_brain']['switches_per_body_min']:.2f}/{c['holds_heuristic']['switches_per_body_min']:.2f}; "
              f"scenes {scenes}")
        print(f"         diet brain [{diet_text(c.get('diet_brain'))}] rules [{diet_text(c.get('diet_heuristic'))}]")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pg")
    ap.add_argument("--es")
    ap.add_argument("--every", type=int, default=1, help="print every Nth generation")
    a = ap.parse_args()
    if a.pg:
        pg_table(a.pg, a.every)
        checks(a.pg, "PG")
    if a.es:
        es_table(a.es, a.every)
        checks(a.es, "ES")


if __name__ == "__main__":
    main()
