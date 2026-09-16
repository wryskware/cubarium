#!/usr/bin/env python3
"""Workstream Z's three openings, side by side, and the pre-registered reading rule.

    python3 design/7_Research/assets/ecology-v1-grazed-opening-tables.py \
        runs/ecology-v1-grazed-opening runs/ecology-v1-precondition

Three openings per configuration:

* **status quo** — workstream Z's own age-0 arms, which reproduce workstream S's retained
  age-0 rows hash for hash (§0 below checks it, and checks every shared measure too);
* **plant-only 48,000** — S's retained `compare.jsonl` rows, read as retained;
* **coupled-grazed 48,000 / 96,000 / 180,000** — Z's arms: an ordinary coupled burn-in, its
  population removed, the identical fresh roster founded into the field it grazed.

Both denominators are printed side by side wherever a ratio appears, and the terminal starved
cells are printed on the arm's own reference **and** on the common §11 reference, because the
own-reference count moves with the opening (S §4.2). The reading rule is the one committed in
`ecology-v1-grazed-opening-preregistration-2026-09-16.md` §6, before a row existed.
"""

import json
import statistics as st
import sys
from pathlib import Path

Z_ROOT = Path(sys.argv[1] if len(sys.argv) > 1 else "runs/ecology-v1-grazed-opening")
S_ROOT = Path(sys.argv[2] if len(sys.argv) > 2 else "runs/ecology-v1-precondition")

Z = [json.loads(l) for l in open(Z_ROOT / "grazed" / "grazed.jsonl")]
S = [json.loads(l) for l in open(S_ROOT / "compare" / "compare.jsonl")]
zby = {(r["candidate"], r["seed"], r["age"]): r for r in Z}
sby = {(r["candidate"], r["seed"], r["age"]): r for r in S}
CONFIGS = ["baseline", "fast-leaf"]
SEEDS = sorted({r["seed"] for r in Z})
Z_AGES = sorted({r["age"] for r in Z})
FORMS = ["lantern (10)", "sail (5)", "mossback (4)", "skimmer (5)", "form 4"]
KINDS = [0, 1, 2, 3]

# The three openings, as (label, kind, age). `kind` says which file the row comes from.
ARMS = (
    [("status quo", "z", 0)]
    + [("plant-only 48,000", "s", 48_000)]
    + [(f"coupled-grazed {a:,}", "z", a) for a in Z_AGES if a > 0]
)


def s_common(c, seed, age):
    """S's terminal starved cells on the common §11 reference, her own script's definition."""
    ref = {x["cell"]: x["p_open"]
           for x in sby[(c, seed, 0)]["evaluation"]["movement"]["plant_budget"]["cells"]}
    cells = sby[(c, seed, age)]["evaluation"]["movement"]["plant_budget"]["cells"]
    watched = [(x["cell"], x["p_final"]) for x in cells if ref.get(x["cell"], 0.0) > 1e-9]
    return (
        sum(1 for cid, pf in watched if pf < 0.25 * ref[cid]),
        sum(1 for cid, pf in watched if pf < 0.5 * ref[cid]),
        st.median(pf / ref[cid] for cid, pf in watched),
    )


def arm(kind, c, seed, age):
    """One arm's measures, from whichever recorder produced it, on one common vocabulary."""
    if kind == "z":
        r = zby[(c, seed, age)]
        h = r["horizon"]
        traj = r["trajectory"]
        hour = traj[-1] if traj else {}
        return dict(
            opening=r["opening"]["foliage"],
            opening_cv=r["opening"]["foliage_cv"],
            crossings=h["crossings"]["depletions"],
            crossings_common=h["crossings_common"]["depletions"],
            with_withdrawal=h["depletion_split"]["with_withdrawal"],
            without_withdrawal=h["depletion_split"]["without_withdrawal"],
            ever_visited=h["depletion_split"]["ever_visited"],
            bred=sum(h["founder_broods"]["parents_by_form"]),
            by_kind=h["founder_broods"]["parents_by_form"],
            first_brood=h["founder_broods"]["first_brood_tick_by_form"],
            broods=sum(h["founder_broods"]["broods_by_form"]),
            pop=h["final_population"],
            forms=h["final_forms_present"],
            lineages=h["founder_lineages_alive"],
            evenness=h["mean_form_evenness"],
            guild=h["guild_final"],
            final_foliage=h["final_foliage"],
            late=h["late_foliage_mean"],
            ratio=h["late_foliage_over_opening"],
            below_q_own=h["below_quarter_own"],
            below_q_common=h["below_quarter_common"],
            below_h_common=h["below_half_common"],
            median_ratio=h["median_final_over_seeding"],
            income_hour=hour.get("income", 0.0),
            out_hour=hour.get("foliage_out", 0.0),
            in_hour=hour.get("foliage_in", 0.0),
            withdrawal_hour=hour.get("withdrawal", 0.0),
            trajectory=traj,
        )
    e = sby[(c, seed, age)]["evaluation"]
    m, mv, pb = e["metrics"], e["movement"], e["movement"]["plant_budget"]
    q, hf, med = s_common(c, seed, age)
    traj = e.get("opening") or []
    hour = traj[-1] if traj else {}
    # S's rows store `first_brood_tick_by_form` on the **world clock**, so a 48,000-tick arm
    # reads 51,001 where Z's recorder — which measures every interval from the founding —
    # reads 3,001. Subtracted here so the two columns mean the same thing.
    first = [None if t is None else t - age
             for t in mv["founder_broods"]["first_brood_tick_by_form"]]
    late = (m.get("late") or {}).get("mean_producer", m["final_producer"])
    return dict(
        opening=m["opening_foliage"],
        opening_cv=float("nan"),
        crossings=mv["crossings"]["depletions"],
        crossings_common=float("nan"),  # S's rows carry no second counter; only the terminal
        with_withdrawal=pb["with_withdrawal"],
        without_withdrawal=pb["without_withdrawal"],
        ever_visited=pb["ever_visited"],
        bred=sum(mv["founder_broods"]["parents_by_form"]),
        by_kind=mv["founder_broods"]["parents_by_form"],
        first_brood=first,
        broods=sum(mv["founder_broods"]["broods_by_form"]),
        pop=m["final_population"],
        forms=m["final_forms_present"],
        lineages=m["founder_lineages_alive"],
        evenness=m["mean_form_evenness"],
        guild=(m.get("late") or {}).get("guild_final", [0, 0, 0]),
        final_foliage=m["final_producer"],
        late=late,
        ratio=late / m["opening_foliage"] if m["opening_foliage"] else float("nan"),
        below_q_own=float("nan"),
        below_q_common=q,
        below_h_common=hf,
        median_ratio=med,
        income_hour=hour.get("income", 0.0),
        out_hour=hour.get("foliage_out", 0.0),
        in_hour=hour.get("foliage_in", 0.0),
        withdrawal_hour=hour.get("withdrawal", 0.0),
        trajectory=traj,
    )


def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else float("nan")


def cells(kind, c, age):
    return [arm(kind, c, s, age) for s in SEEDS]


# --- 0. the reproduction, before anything is interpreted ----------------------------------
print("## 0. The reproduction of S's status-quo rows\n")
ok = tot = 0
for c in CONFIGS:
    for s in SEEDS:
        tot += 1
        h = zby[(c, s, 0)]["horizon"]
        m = sby[(c, s, 0)]["evaluation"]["metrics"]
        if (h["final_state_hash"], h["final_ecology_hash"]) == (
            m["final_state_hash"], m["final_ecology_hash"]):
            ok += 1
print(f"`final_state_hash` **and** `final_ecology_hash`: **{ok} of {tot}**\n")

# --- 1. the three openings ----------------------------------------------------------------
print("## 1. The three openings\n")
print("| config | opening | opening ΣP | CV | founders that bred, of 24 | broods | "
      "lineages alive | evenness | final pop | forms | late ΣP (absolute) | late ÷ opening |")
print("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
for c in CONFIGS:
    for label, kind, age in ARMS:
        a = cells(kind, c, age)
        print(f"| {c} | {label} | {mean(x['opening'] for x in a):.1f} | "
              f"{mean(x['opening_cv'] for x in a):.3f} | "
              f"{mean(x['bred'] for x in a):.1f} | {mean(x['broods'] for x in a):.1f} | "
              f"{mean(x['lineages'] for x in a):.1f} | {mean(x['evenness'] for x in a):.3f} | "
              f"{mean(x['pop'] for x in a):.1f} | {mean(x['forms'] for x in a):.1f} | "
              f"**{mean(x['late'] for x in a):.1f}** | {mean(x['ratio'] for x in a):.2f} |")
print()

# --- 2. the founders, by kind -------------------------------------------------------------
print("## 2. The founders, by kind\n")
print("| config | opening | " + " | ".join(FORMS[:4]) + " | first brood, lantern | "
      "first brood, sail |")
print("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |")
for c in CONFIGS:
    for label, kind, age in ARMS:
        a = cells(kind, c, age)
        by = [mean(x["by_kind"][k] for x in a) for k in KINDS]
        fb = []
        for k in (0, 1):
            t = [x["first_brood"][k] for x in a if x["first_brood"][k] is not None]
            fb.append(f"{mean(t):,.0f} ({len(t)}/{len(a)})" if t else "never")
        print(f"| {c} | {label} | " + " | ".join(f"{v:.1f}" for v in by)
              + f" | {fb[0]} | {fb[1]} |")
print()

# --- 3. the depletion counts, on both references ------------------------------------------
print("## 3. Depletion, on the arm's own reference and on the common §11 one\n")
print("| config | opening | crossings (own ref, 6 seeds) | with withdrawal | without | "
      "ever visited | crossings (§11 ref) | terminal below ¼ §11 | below ½ §11 | "
      "median P_final ÷ P_§11 |")
print("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
for c in CONFIGS:
    for label, kind, age in ARMS:
        a = cells(kind, c, age)
        tot_x = sum(x["crossings"] for x in a)
        com = sum(x["crossings_common"] for x in a)
        print(f"| {c} | {label} | {tot_x} | {sum(x['with_withdrawal'] for x in a)} | "
              f"{sum(x['without_withdrawal'] for x in a)} | "
              f"{sum(x['ever_visited'] for x in a)} | "
              f"{'—' if com != com else int(com)} | "
              f"**{mean(x['below_q_common'] for x in a):.1f}** | "
              f"{mean(x['below_h_common'] for x in a):.1f} | "
              f"{mean(x['median_ratio'] for x in a):.2f} |")
print()

# --- 4. the first simulated hour ----------------------------------------------------------
print("## 4. The first simulated hour after founding\n")
MARKS = [0, 6_000, 18_000, 36_000, 72_000]
print("| config | opening | " + " | ".join(f"+{m:,}" for m in MARKS)
      + " | minimum | at tick | income over the hour | foliage out |")
print("| --- | --- | " + " ".join("---: |" for _ in MARKS) + " ---: | ---: | ---: | ---: |")
for c in CONFIGS:
    for label, kind, age in ARMS:
        a = cells(kind, c, age)
        cols = []
        for m in MARKS:
            vals = []
            for x in a:
                pt = min(x["trajectory"], key=lambda p: abs(p["ticks_since_founding"] - m),
                         default=None)
                if pt is not None:
                    vals.append(pt["foliage"])
            cols.append(f"{mean(vals):.1f}" if vals else "—")
        mins, at = [], []
        for x in a:
            if x["trajectory"]:
                pt = min(x["trajectory"], key=lambda p: p["foliage"])
                mins.append(pt["foliage"])
                at.append(pt["ticks_since_founding"])
        print(f"| {c} | {label} | " + " | ".join(cols)
              + f" | {mean(mins):.1f} | {mean(at):,.0f} | "
                f"{mean(x['income_hour'] for x in a):.1f} | "
                f"{mean(x['out_hour'] for x in a):.1f} |")
print()

# --- 5. the pre-registered reading rule ---------------------------------------------------
print("## 5. The reading rule\n")
for c in CONFIGS:
    sq = cells("z", c, 0)
    sq_lineages = mean(x["lineages"] for x in sq)
    sq_even = mean(x["evenness"] for x in sq)
    sq_q = [x["below_q_common"] for x in sq]
    sq_sd = st.stdev(sq_q) if len(sq_q) > 1 else 0.0
    print(f"### {c}\n")
    print(f"Status quo: lineages {sq_lineages:.1f}, evenness {sq_even:.3f}, "
          f"terminal below ¼ §11 {mean(sq_q):.1f} (across-seed sd {sq_sd:.2f})\n")
    print("| age | grazer founders bred (of 10) | lineages ÷ status quo | evenness up | "
          "opening ÷ late | paired Δ starved vs sd | verdict |")
    print("| ---: | ---: | ---: | :-: | ---: | ---: | --- |")
    for age in [a for a in Z_AGES if a > 0]:
        a = cells("z", c, age)
        grazer = mean(x["by_kind"][0] for x in a)
        lin = mean(x["lineages"] for x in a)
        even = mean(x["evenness"] for x in a)
        opening = mean(x["opening"] for x in a)
        late = mean(x["late"] for x in a)
        gap = abs(opening - late) / late if late else float("inf")
        paired = mean(x["below_q_common"] - y["below_q_common"] for x, y in zip(a, sq))
        c1 = grazer >= 9.999 and lin >= 1.5 * sq_lineages and even > sq_even
        c2 = gap <= 0.20
        c3 = abs(paired) <= sq_sd
        verdict = "**better target**" if (c1 and c2 and c3) else "**not**"
        print(f"| {age:,} | {grazer:.1f} | {lin / sq_lineages:.2f}× | "
              f"{'yes' if even > sq_even else 'no'} | {gap * 100:.1f} % | "
              f"{paired:+.1f} vs {sq_sd:.2f} | {verdict} "
              f"(1 {'✓' if c1 else '✗'}, 2 {'✓' if c2 else '✗'}, 3 {'✓' if c3 else '✗'}) |")
    print()

# --- 6. paired, per seed ------------------------------------------------------------------
print("## 6. Paired against the status-quo arm of the same seed\n")
print("| config | age | lineages better/worse | broods | evenness | starved cells fewer/more |")
print("| --- | ---: | :-: | :-: | :-: | :-: |")
for c in CONFIGS:
    sq = cells("z", c, 0)
    for age in [a for a in Z_AGES if a > 0]:
        a = cells("z", c, age)
        def tally(key, better=lambda x, y: x > y):
            b = sum(1 for x, y in zip(a, sq) if better(x[key], y[key]))
            w = sum(1 for x, y in zip(a, sq) if better(y[key], x[key]))
            return f"{b} / {w}"
        print(f"| {c} | {age:,} | {tally('lineages')} | {tally('broods')} | "
              f"{tally('evenness')} | "
              f"{tally('below_q_common', lambda x, y: x < y)} |")
