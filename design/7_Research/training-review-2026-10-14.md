---
date: 2026-10-14
kind: evidence
---

# Training review: how noisy a step is, where the time goes, and what else could train the brains

TRAIN-REVIEW (`design/handoffs/train-review-2026-10-14.md`; background
`design/behaviour-ai-2026-10-11.md` §10.4 and §11.7). Wrysk, 2026-10-14: measure the step's
noise first, then raise the signal per step to fit; and revisit how we train, both the
algorithm and the run speed. This is evidence, not a decision. The topics at the end go to him
one at a time.

## 1. How much of a step is signal

**Runs.** Three stage-1 twin runs (top layer only, σ 0.05, Adam lr 0.01, 1024 pairs × 120 s
twin windows a generation, one start per pair, 16 persistent worlds, train seed 20260924):

| run | job | comparison | generations |
|---|---|---|---|
| lanternjaw all-sign | `1013-024442-live-twins-lj-stage1-cf89` | `carrier_share 1`, `stranger_weight 1` | 45 |
| lanternjaw half-and-half | `1013-055540-twin-mix-lj-stage1-bc92` | `carrier_share 0.5`, `stranger_weight 0`, kin shares 0.5 | 60 |
| seedporter half-and-half | `1013-073334-twin-mix-sp-stage1-169a` | as above | 34 |

**How.** The pair differences are already logged: `generations.jsonl` keeps every pair's
`(d̄/2, −d̄/2)` as `candidate_scores`. `crates/cubarium-search/examples/step_noise.rs`
regenerates each pair's perturbation with the trainer's own `Perturber::eps` (stage 1, the top
slice: 2832 lanternjaw and 2996 seedporter weights) and forms steps with the trainer's own
`optimizer::gradient` (centred ranks). Per generation it reports:

- **split**: the cosine between the steps of two random halves of the generation's pairs (512
  each, each ranked on its own), averaged over 16 random splits;
- **null**: the same after shuffling the d's across pairs (the noise floor);
- **next**: the cosine between consecutive generations' whole steps.

It also replays each run's Adam from its starting centre. All three replays end on the run's
`final-*.json` exactly (max |Δθ| 0) and match every logged `update_rms`, so the perturbations
are the trainer's.

| run | split (mean ± se over gens) | null | next | R² of one pair's d | zero-d pairs |
|---|---|---|---|---|---|
| LJ all-sign | **0.0118 ± 0.0024** | 0.0009 ± 0.0007 | 0.0139 ± 0.0031 | 0.058 ± 0.012 | 0.8 % |
| LJ half-and-half | **0.0035 ± 0.0016** | −0.0007 ± 0.0006 | 0.0059 ± 0.0027 | 0.023 ± 0.009 | 4.8 % |
| SP half-and-half | **0.0066 ± 0.0025** | −0.0002 ± 0.0007 | 0.0167 ± 0.0031 | 0.025 ± 0.013 | 20 % (37 % at gen 33) |

**Reading it.** Every run has a real, persistent gradient: split and next sit well above the
null, and the lanternjaw's split doesn't fade over its run (first half 0.010, second half 0.013).
But a step is almost all luck:

- Two independent n-pair steps have cosine `c(n) = S/(S + V/n)`, with S the true step's
  energy and V one pair's noise. From the halves, `V/S = 512·(1 − c)/c`.
- **R²** is the share of one pair's d that its variant explains: `R² ≈ c·D/(h·(1 − c))` from the
  raw (unranked) split, with D the perturbed weights and h = 512. In a synthetic check
  (D 2832, true R² 0.06) it read 0.07, so it runs ~15 % high. All-sign twins put about 6 %
  of a lanternjaw pair's difference on the variant. Half-and-half put about 2.5 %. The rest is
  what happened after the twins first differed, which is under 2 s into the window in the
  late lanternjaw generations (median `median_first_diff_s` 0.25–2.5).
- **Adam's `update_rms` ~0.0009 is the pure-noise signature,** not a small step:
  `lr·√((1−β1)/(1+β1))·√(D_top/D_all)` = 0.01 · 0.229 · 0.39 = 0.00089. A clean gradient would
  give about 0.0039. The random walk moved each top weight about 0.076 over 45 generations
  (drift 4.0, path 7.5), more than σ.

| run | V/S (pairs) | cos(step, true gradient) at 1024 pairs | pairs for cos 0.5 between two steps | wall per such step today | pairs for cos 0.3 with the truth |
|---|---|---|---|---|---|
| LJ all-sign | 43 k (36–54 k) | 0.15 | 43 k | **2.5 h** (2.1–3.2) | 4.2 k (15 min) |
| LJ half-and-half | 146 k (100–270 k) | 0.08 | 146 k | **6.6 h** (4.5–12) | 14 k (39 min) |
| SP half-and-half | 77 k (56–124 k) | 0.11 | 77 k | **6.1 h** (4.4–9.9) | 7.6 k (36 min) |

Ranges are ±1 se of the split. Wall per pair today: 0.21 s (LJ all-sign), 0.16 s (LJ mix) and
0.29 s (SP), from `twins_wall_s / pairs`.

**Why ES needs so many pairs here.** A pair carries one number (d) for 2832 weights. The pairs
a step needs scale as `D/R²`. Today's 1024 pairs give a step about 0.1–0.15 aligned with the
true gradient. A mostly-signal step needs 40–150 k pairs, 2.5–7 h of desktop per step. A
random low-dimensional subspace does not help, because the signal it keeps shrinks as fast as
the noise. A structured one can, if the gradient lives there (§4, topic 4).

**The lanternjaw-TWIN ICCs agree** (`1013-010026-ljtwin-s0-05-79cd`, `twin_signal.py`).
All-sign ICC over 12 starts is 0.12 [0.02, 0.27] for the first 2 min, 0.12 for 5 min and
0.09 [0.00, 0.23] for 10 min. ICC counts any consistent effect, including nonlinear ones, so it
bounds the linear R² from above; 0.06 sits inside it.

### The zero-difference pairs

A zero pair is a twin whose two copies stayed **bit-identical for the whole 120 s**.
`share_d_zero + share_diverged = 1` in every generation of every run. No carrier's held intent
ever differed between `+ε` and `−ε`, so it is not a window that ran out before an effect
showed. Three causes, in order of evidence:

- **Asleep carriers aren't consulted** (`crates/cubarium-voxel-fauna/src/circadian_tests.rs:235`,
  `an_asleep_animal_is_not_consulted_and_does_not_move`). In the seedporter run the zero share
  rose from 0.05 to 0.37 over generations 18–33, in step with its live movement falling (`mov`
  0.16 → 0.01, `pace` 0.10 → 0.005).
- **The choice is deterministic** (`two_layer/brain.rs:331-470`). Argmax with `hold_s` and
  `switch_margin` flips only when a proposal sits near the margin. LJ-TWIN found 0.8 % of
  identical-input decision samples proposing differently between signs.
- **Fewer carriers.** Half-and-half puts the variant on about 3.6 seedporters or about 4
  lanternjaws a start, against 8 or more all-sign. Lanternjaw zero is 0.8 % all-sign and
  4.8 % mixed.

The seedporter's generations 1–5 (zero 0.33–0.56 while it still moved) fit the second cause.
Its centre then switched 0.51 times a body-minute, against the rules' 7.77.

### What the seedporter did learn

Its run is "flat" in energy (check gain/body −0.0023 → −0.0023), but its behaviour moved:

- switches a body-minute 0.51 → 2.04 (gen 10) → 0.30 → 0.11 (gen 30);
- the roost scene 0.56 → 0.06;
- live movement 0.54 → 0.01.

Its strongest consecutive-step agreement (next 0.03–0.06, gens 6–11) came while movement fell
fastest. That is consistent with the one direction a 120 s energy window can see clearly:
moving and switching cost energy now, and foraging pays later than 2 minutes. It is likely,
not proven, that the seedporter learned to sit still because the window is myopic.

**The checks can't see small gains either.** The lanternjaw brain's check gain/body across
seven checks of one centre lineage ranges 0.081–0.120 (sd ~0.013), on 6 seeds × 600 s against
a fixed heuristic 0.098. A real change under ~0.02 per body is invisible to it.

## 2. Where a generation's time goes

From `generations.jsonl` (all generations; 16 workers, one sim thread per copy):

| run | twins share of wall | copy ticks/s per worker | non-twin s/gen | copies/gen |
|---|---|---|---|---|
| LJ all-sign | 94 % | 1413 | 12.8 | 2050 × 2400 ticks |
| LJ half-and-half | 94 % | 1867 | 10.3 | 2050 × 2400 ticks |
| SP half-and-half | 95 % | 1053 | 15.9 | 2050 × 2400 ticks |

- A generation simulates 4.92 M copy ticks against 192 k live-window ticks. **The twins are 25×
  the live world and 94–95 % of the wall.** Workers are saturated (copy thread-seconds ÷
  twins wall ÷ 16 = 1.00). Copies slow as the persistent worlds grow: seedporter copies ran
  2390 ticks/s/worker at gen 0 and 760 by gen 15, while frondgrazers went 128 → 986 and
  stands 3455 → 46529.
- The live window, update, saves and checks are the other 5 %. Brain checks add ~3 min a run.

**Inside a copy**: `examples/twin_copy_cost.rs` on saved end-of-run worlds, one thread, quick
job `1014-011459-train-review-copy-cost-baa4`. Under `perf` (flat self time, 49 Hz, grouped by
crate and module):

| | seedporter world (82 animals) | lanternjaw world (29 animals) |
|---|---|---|
| water (infiltrate, fall, exchange) | 27 % | 36 % |
| flora (LayerIter, drink_read, light) | 20 % | 19 % |
| fauna senses and perception | 14 % | 15 % |
| fauna other (places refresh, body) | 21 % | 10 % |
| **brain (GRU, two-layer top)** | **1.7 %** | **1.4 %** |
| alloc and sort | 4 % | 3 % |
| opening a copy (decode, install) | ~1 % (0.025–0.04 s of 2–5 s) | ~1 % |
| twin pose hash and metering | < 0.1 % | < 0.1 % |

- **Water is still the biggest phase at `water_every = 20`.** The setting coarsens only the
  lateral and groundwater phases. Infiltration and fall still run every tick
  (`crates/cubarium-voxel/src/world.rs:1505-1514`).
- **Batching inference would buy nothing** (≤ 1.7 %).
- A whole copy costs about 1.15× bare steps.
- Single-thread speeds in the quick lane were 420–1100 ticks/s, under the training workers'
  1050–1870. That job ran at idle priority on a loaded desktop (load ~7), so read the shares,
  not the absolute speed.

**Obvious wins**, none built:

| win | what | expected |
|---|---|---|
| twin fidelity | copies run water's vertical phases coarse too (or freeze water) and step flora growth every k ticks: both copies of a twin share the approximation, so the comparison stays matched | ~45–55 % of copy time → **~1.8–2×** |
| threads | 28 desktop threads (labd's desktop budget) instead of 16; SMT helps less than linearly | ~1.2–1.4× |
| eidolon | copies are independent: ship a generation's 16 starts (~6.5 MB each) and half the pairs to eidolon (21 threads) | ~+1.3–2× |
| lazy fork | run one copy until a carrier's `+ε` and `−ε` top would choose differently (two top heads on one input, ~1 % cost), fork only then; a no-flip twin costs one copy | saves 1–37 % of copies (the zero share) plus prefixes |
| shorter windows | if the ICC holds at shorter windows, cost per unit signal falls with the window | see §3 |
| smaller worlds | twin copies of a smaller world (fewer stands and bodies) | proportional, but changes what is learned |

Stacked (fidelity × threads × eidolon) that is roughly 3–5× in pairs per hour. That alone does
not close a 40–150× gap.

## 3. Window length

Does a shorter twin window buy signal more cheaply? Quick job
`1014-012013-train-review-window-icc-1d89`: `lj_twin` (all-sign, σ 0.05, the lanternjaw's
starting centre, 12 worlds founded as training founds them, 30 min warm-up, `water_every 20`),
64 pairs × 12 starts at each window, on the same starts. Consistency is the ICC of d over
starts (`scripts/brain-pilot/twin_signal.py`). The LJ-TWIN rows are its earlier run (`1013-010026-ljtwin-s0-05-79cd`,
16 pairs × 12 starts, `water_every 4`, read from the first minutes of 10-minute twins).

| window | energy ICC [95 %] | stomach-aware ICC | sd d | twins never diverged | body-windows with a meal (> 0.5 ref) | thread-s per twin |
|---|---|---|---|---|---|---|
| 30 s | 0.011 [−0.018, 0.050] | — | 0.004 | 20 % | 2 % | 0.97 |
| 60 s | 0.010 [−0.019, 0.048] | 0.045 [0.008, 0.095] | 0.008 | 10 % | 10 % | 1.75 |
| 2 min (this probe) | pending: same job, `twin_signal.py` on `.claude/worktrees/train-review/runs/window-icc/w2` | | | | | |
| 2 min (LJ-TWIN) | 0.118 [0.019, 0.273] | 0.134 | 0.019 | — | — | (~2.9, scaled) |
| 5 min (LJ-TWIN) | 0.122 [0.022, 0.278] | 0.129 | 0.089 | — | — | (~7, scaled) |
| 10 min (LJ-TWIN) | 0.092 [0.004, 0.234] | 0.098 | 0.277 | 0 % | 33 % | 14.3 |

- **Under a minute the energy signal is gone.** The variant changes behaviour consistently
  even at 30 s: the ambush and patrol share ICCs are 0.15–0.21. But meals haven't happened
  or haven't turned into energy yet. Counting gut contents (`stomach_aware_reward`) recovers
  some at 60 s (0.045), because it sees the meal before digestion does.
- **Past 2 minutes the ICC doesn't grow** (0.12 → 0.12 → 0.09) while cost grows linearly.
- So 2 minutes, today's window, is near the cheapest signal per thread-second. **The window is
  not the lever**: at best a factor ~1, not the 40–150 the step needs.

## 4. Algorithms for this setup

The setup:

- small nets, with a top of ~2.9 k weights at stage 1;
- a 4 Hz discrete intent choice per body, argmax with hold and margin;
- energy returns over minutes, with rare decisive events (meals carry 61 % of Σd² in LJ-TWIN);
- many bodies per world;
- inference at ~1.5 % of the sim.

| method | what changes | signal | build | risks |
|---|---|---|---|---|
| **A. ES as today, more pairs** | `pairs`, or accumulate steps over k generations (Adam β1, or a step every k gens) | linear in pairs: 4× pairs → cos with the truth 0.15 → 0.29 (LJ all-sign) | config only | wall: 4× pairs is ~15 min/gen at today's speed; the noise random-walk keeps moving the centre (lower lr limits it) |
| **B. ES on a structured subspace** | perturb only where the gradient likely lives: the intent head's last layer (~menu × hidden) or per-intent biases (menu ≈ 6–8 numbers), or guided ES (perturb partly along the last k steps, which agree, next > 0) | pairs needed fall ∝ D if the signal is there: 2.9 k → 0.5 k is ~6×; biases only (8) is ~350× but can learn only "how often each intent" | small (a `Perturber` mask or basis) | can't learn state-dependent choices from biases alone; guided ES assumes the gradient persists (measured next ≈ split, so partly) |
| **C. Variance reduction on d** | rank shaping (in), antithetic (in), common starts (in). Left: shorter windows (§3), all-sign over half-and-half (2.5× R²), control variates on luck the variant didn't cause (hard: after the first divergence everything is caused) | up to ~2.5× (layout) and the window factor | config | half-and-half exists to separate kin and direct effects; all-sign can't |
| **D. Per-decision policy gradient on the intent choice** | during training the top *samples* its proposal at the arbitration temperature (hold and margin stay as environment rules); each decision's advantage = discounted energy return (+ kin shares) over the next minutes minus a learned value baseline V(state) (GAE); update = advantage-weighted log-prob gradient of the top. The bottom is frozen at stage 1, so the top's input (GRU state) is fixed and **no backprop through time** is needed: it is `clone.rs`'s top cross-entropy gradient (`two_layer/clone.rs:924`, finite-difference tested at `tests.rs:718`) with weights | variance scales with return noise and the decisions in the credit horizon, not with D. Data comes from the live windows themselves: no twin copies, so **~25× less sim per generation**, and every decision of every body is a sample. Unmeasured here; ES on D ≈ 3 k against PG on discrete choices with a baseline is typically a large gap in PG's favour | moderate: decision log (hidden state, mask, proposal, time) per body, returns and advantages, a value head, PPO-style clipped update; reuses the clone gradient | sampling adds exploration noise in training worlds only (deploy at argmax); a learned critic can be wrong; credit over minutes with rare meals is still noisy; rewards stay energy and kin (never behaviour); on-policy, so live worlds run the current centre (they already do) |
| **E. Rollouts at decision points** | at a sampled decision, fork the world once per valid intent (the twin machinery), run each a short horizon (30–60 s) with the centre after, and train the top toward the best (policy improvement by Monte-Carlo comparison) | each sample compares whole intents (a large behavioural difference, not a σ-sized nudge), so signal per fork is far above a twin's 6 %; per-fork chaos remains | moderate–large: fork scheduling per decision, menu-wide copies | expensive per sample (menu × horizon); myopic to its horizon; biased toward intents that pay fast |

**Readout for any of them:** the same split-half cosine. Form one generation's update from
two halves of its data (bodies, worlds or pairs) and compare. It is the apples-to-apples
yardstick, and `step_noise` already does it for ES.

## 5. Topics for Wrysk, one at a time

**Topic 1: whether the next ES runs use all-sign or half-and-half twins.**
- Options:
  - (a) keep half-and-half;
  - (b) all-sign while ES is the trainer;
  - (c) all-sign for the lanternjaw, half-and-half where kin effects matter (loftstrider,
    Veilhound).
- What he'd see: the same runs with ~2.5× the signal per pair on the lanternjaw (R² 0.058 vs
  0.023).
- Cost: config only (`config/training/kin-twins-2026-10-13.toml` exists).
- **Recommendation: (b)/(c).** All-sign's summed score already counts what relatives gain.
  The split into direct and kin effects can come back once the signal stands up.

**Topic 2: twin window length.**
- Options:
  - (a) keep 120 s;
  - (b) 60 s on the stomach-aware score (ICC 0.045 at half the cost of 2 min);
  - (c) longer windows (5–10 min).
- What he'd see: (a) nothing changes; (b) generations about 2× faster with about the same
  signal per wall-hour; (c) slower generations with no more signal per pair.
- Cost: config only.
- **Recommendation: (a).** Under a minute the energy hasn't arrived; past two minutes the ICC
  doesn't grow. Revisit (b) only if topic 5 adopts the stomach-aware score.

**Topic 3: speed, which wins to build.**
- Options:
  - (a) twin fidelity (coarse vertical water and flora in copies, ~1.8–2×);
  - (b) 28 desktop threads (~1.2–1.4×);
  - (c) eidolon as a second copy runner (~1.3–2×);
  - (d) lazy fork (1–37 % of copies);
  - (e) none until the algorithm topic is decided.
- What he'd see: generations 2–5× faster, same numbers otherwise.
- Cost: (a) and (b) are small; (c) and (d) are moderate.
- **Recommendation: (a) + (b) now.** They help every method that simulates, D included. Then
  (c) if ES stays.

**Topic 4: the training algorithm.**
- Options:
  - (A) ES with ~4× pairs;
  - (B) ES on a structured subspace (intent head or guided ES);
  - (D) a per-decision policy-gradient pilot on the top's intent choice (stage 1, bottom
    frozen, live windows, no twins);
  - (E) rollout comparisons at decisions.
- What he'd see: the pilot reports the same split-half cosine as today's runs, side by side on
  one lineage, before any long run.
- Cost: A is config only; B is small; D is moderate (reuses the clone gradient); E is
  moderate–large.
- **Recommendation: D as a pilot, with ES (A + topic 1) as the baseline it must beat.** ES's
  D/R² scaling puts a mostly-signal step at hours of desktop. PG doesn't scale with D and
  drops the 25× twin overhead.

**Topic 5: the reward horizon and the checks.**
- The 120 s windows likely taught the seedporter to sit still: switches 2.04 → 0.11, roost
  0.56 → 0.06, movement 0.54 → 0.01, energy flat.
- The checks can't resolve gains under ~0.02 per body.
- Options:
  - (a) leave both;
  - (b) a longer credit horizon: longer twin windows (cost), or a value baseline that
    bootstraps beyond the window (comes with D);
  - (c) count gut contents in the score (the existing `stomach_aware_reward` switch;
    LJ-TWIN's stomach-aware ICC matches the energy one from 2 minutes on, and at 60 s it
    holds 0.045 where energy holds 0.010);
  - (d) more check seeds (6 → 24, ~4× check time, still ~1 % of a run).
- **Recommendation: (d) now, and (b) through topic 4's value baseline.** Rewards stay energy
  and kin. The fix is the horizon the score can see, not a behaviour term.

## Reproduce

```text
# per run, ~15 s each
target/release/examples/step_noise run=runs/lab/<job>/run/run start=<starting centre .json>
# copy cost (one thread), under perf if wanted
target/release/examples/twin_copy_cost world=<run>/worlds/world-0 lineage=seedporter centre=<run>/centres/final-seedporter.json pairs=1
```

Starting centres: lanternjaw
`runs/lab/1012-154001-brest-lj-clone-then-smok-fb68/run/clone/lanternjaw-two-layer.json`,
seedporter `runs/lab/1012-154002-brest-clone-seedporter-72f7/run/run/seedporter-two-layer.json`.
